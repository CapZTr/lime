use std::ops::Index;

use eggmock::{Gate, Id, Node, Signal};
use itertools::Itertools;
use lime_generic_def::{
    Cell, CellPat, CellType, InputIndices, Instruction, InstructionType, Operand, PatBase, Pats,
    TuplesDef, set::Set,
};
use ordered_float::OrderedFloat;
use pathfinding::{matrix::Matrix, prelude::kuhn_munkres_min};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::{
    compilation::{StepFn, candidate_selection::CandidateSelector},
    copy::{
        copy_cost, copy_cost_with_path, perform_copy,
        spilling::{estimate_spill_cost_operand_pats, force_spill},
    },
    cost::{Cost, OperationCost},
    program::{
        ProgramVersion,
        state::{CellStates, Operation},
    },
};

use super::CompilationParameters;

pub struct DefaultStepFn<C: CandidateSelector>(pub C);

impl<CT: CellType, G: Gate, C: OperationCost<CT>, CS: CandidateSelector> StepFn<CT, G, C>
    for DefaultStepFn<CS>
{
    fn step(
        &self,
        params: &CompilationParameters<CT, G, C>,
        mut version: impl ProgramVersion<CT = CT, G = G, C = C>,
    ) {
        for candidate_id in self.0.select_candidates(&version).collect_vec() {
            let candidate_node = params.network.node(candidate_id);
            let candidate_gate = match candidate_node {
                Node::Gate(gate) => gate,
                _ => continue,
            };

            for instruction in params.arch.instructions().iter() {
                if instruction.function.gate.gate_function() != Some(candidate_gate.function()) {
                    continue;
                }
                if let Some(arity) = instruction.arity()
                    && arity != candidate_gate.inputs().len()
                {
                    continue;
                }
                match &instruction.input {
                    TuplesDef::Tuples(tuples) => {
                        for tuple in tuples.iter() {
                            let Some(signals) = position_signals(
                                instruction,
                                tuple.as_slice(),
                                candidate_gate,
                                params,
                                &version,
                            ) else {
                                continue;
                            };
                            let mut version = version.branch();
                            if let Some(version) = perform_operation(
                                candidate_id,
                                &mut version,
                                instruction,
                                tuple.as_slice(),
                                &signals,
                                params,
                            ) {
                                version.consider();
                            }
                        }
                    }
                    TuplesDef::Nary(operands) => {
                        let Some(signals) = position_signals(
                            instruction,
                            operands,
                            candidate_gate,
                            params,
                            &version,
                        ) else {
                            continue;
                        };
                        let mut version = version.branch();
                        if let Some(version) = perform_operation(
                            candidate_id,
                            &mut version,
                            instruction,
                            operands,
                            &signals,
                            params,
                        ) {
                            version.consider();
                        }
                    }
                };
            }
        }
    }
}

#[must_use]
fn perform_operation<'v, V: ProgramVersion>(
    candidate_id: Id,
    version: &'v mut V,
    instruction: &InstructionType<V::CT>,
    input: &(impl Index<usize, Output = Pats<CellPat<V::CT>>> + ?Sized),
    signals: &[Signal],
    params: &CompilationParameters<V::CT, V::G, V::C>,
) -> Option<impl ProgramVersion<CT = V::CT, G = V::G, C = V::C> + 'v> {
    let mut used_cells = FxHashSet::default();

    // == place inputs
    let inputs = place_signals(
        input,
        instruction.input_inverted,
        signals,
        params,
        version,
        &mut used_cells,
    )?;
    let mut result = Instruction {
        typ: instruction.clone(),
        inputs,
        outputs: Vec::new(),
    };

    if !params.disjunct_input_output {
        used_cells.clear();
    }

    // == place minimum amount of outputs
    let min_outputs = if instruction.input_override != InputIndices::None {
        0
    } else {
        1
    };
    let output = instruction
        .outputs
        .iter()
        .filter(|output| output.arity().unwrap_or(min_outputs) >= min_outputs)
        .min_by_key(|output| output.arity().unwrap_or(min_outputs))?;
    let mut outputs = Vec::new();
    match output {
        TuplesDef::Nary(nary) => {
            for _ in 0..min_outputs {
                let output = nary
                    .0
                    .iter()
                    .filter_map(|pat| {
                        let cell =
                            version.find_preferred_free_cell_for_pat(pat.cell, &used_cells)?;
                        Some(Operand {
                            cell,
                            inverted: pat.inverted,
                        })
                    })
                    .next()?;
                used_cells.insert(output.cell);
                outputs.push(output);
            }
        }
        TuplesDef::Tuples(tuples) => {
            let (selected_tuple, _) = tuples
                .iter()
                .map(|tuple| {
                    (
                        tuple,
                        tuple
                            .iter()
                            .filter(|pats| version.has_free_cell_for_cell_pats(pats))
                            .count(),
                    )
                })
                .min_by_key(|(_, count)| *count)?;
            for pats in selected_tuple.iter() {
                let Some(output) = pats
                    .iter()
                    .filter_map(|pat| {
                        let cell =
                            version.find_preferred_free_cell_for_pat(pat.cell, &used_cells)?;
                        Some(Operand {
                            cell,
                            inverted: pat.inverted,
                        })
                    })
                    .next()
                else {
                    return None; // should not happen
                };
                used_cells.insert(output.cell);
                outputs.push(output);
            }
        }
    };
    result.outputs = outputs;

    spill_necessary(version, &result);
    apply_state(candidate_id, version, &result);
    version.append(Operation::Candidate(result, candidate_id));
    Some(version.branch())
}

fn spill_necessary<V: ProgramVersion>(version: &mut V, instruction: &Instruction<V::CT>) {
    let cells = instruction.write_cells().collect::<FxHashSet<_>>();
    for &cell in &cells {
        let Some(signal) = version.state().cell(cell) else {
            continue;
        };
        if version.is_last_use(signal.node_id()) {
            continue;
        }
        if !version
            .state()
            .cells_with_id(signal.node_id())
            .any(|(other_cell, _)| !cells.contains(&other_cell))
        {
            force_spill(version, cell, &cells);
        }
    }
}

fn apply_state<V: ProgramVersion>(computed: Id, version: &mut V, instruction: &Instruction<V::CT>) {
    for operand in instruction.overridden_input_operands() {
        version.state_mut().set(
            operand.cell,
            Signal::new(
                computed,
                operand.inverted ^ instruction.typ.function.inverted,
            ),
        );
    }
    for output in &instruction.outputs {
        version.state_mut().set(
            output.cell,
            Signal::new(
                computed,
                output.inverted ^ instruction.typ.function.inverted,
            ),
        );
    }
}

pub(super) fn place_signals<V: ProgramVersion>(
    input: &(impl Index<usize, Output = Pats<CellPat<V::CT>>> + ?Sized),
    input_invert: InputIndices,
    signals: &[Signal],
    params: &CompilationParameters<V::CT, V::G, V::C>,
    version: &mut V,
    used_cells: &mut FxHashSet<Cell<V::CT>>,
) -> Option<Vec<Cell<V::CT>>> {
    let mut placed_signals = vec![false; signals.len()];
    let mut cells = FxHashMap::default();
    for _ in 0..signals.len() {
        // place the next cheapest signal
        let sig = signals
            .iter()
            .enumerate()
            .filter(|(input_idx, _)| !placed_signals[*input_idx])
            .flat_map(|(input_idx, signal)| {
                let target_inverted = input_invert.contains(&input_idx);
                input[input_idx]
                    .iter()
                    // convince the borrow checker that this is fine
                    .map(move |cell_pat| (cell_pat, target_inverted))
                    .flat_map(|(target_cell_pat, target_inverted)| {
                        version
                            .state()
                            .all_cells_with(*signal)
                            .filter_map(|(source_cell, source_cell_inverted)| {
                                let requires_inversion = source_cell_inverted ^ target_inverted;
                                if !requires_inversion
                                    && target_cell_pat.matches(&source_cell)
                                    && !used_cells.contains(&source_cell)
                                {
                                    Some((OrderedFloat(0.0), source_cell, None))
                                } else {
                                    copy_cost_with_path(
                                        &params.arch.copy_graph,
                                        source_cell,
                                        *target_cell_pat,
                                        requires_inversion,
                                        used_cells,
                                    )
                                    .map(|(cost, path)| (cost, source_cell, Some(path)))
                                }
                            })
                            .map(move |(cost, from, path)| (cost, target_cell_pat, from, path))
                            .min_by(|a, b| a.0.cmp(&b.0))
                    })
                    .map(move |(cost, target_cell_pat, from, path)| {
                        (cost, input_idx, target_cell_pat, from, path)
                    })
            })
            .min_by(|a, b| a.0.cmp(&b.0));
        let (_, signal_idx, target_cell_pat, from, path) = sig?;
        placed_signals[signal_idx] = true;
        let target_cell = if let Some(path) = path {
            perform_copy(path, version, from, *target_cell_pat, used_cells)?
        } else {
            from
        };

        used_cells.insert(target_cell);
        cells.insert(signal_idx, target_cell);
    }
    let mut result = Vec::with_capacity(signals.len());
    for i in 0..signals.len() {
        result.push(cells.remove(&i).expect("all operands should be present"));
    }
    Some(result)
}

fn position_signals<V: ProgramVersion>(
    instruction: &InstructionType<V::CT>,
    input: &(impl Index<usize, Output = Pats<CellPat<V::CT>>> + ?Sized),
    gate: &V::G,
    params: &CompilationParameters<V::CT, V::G, V::C>,
    version: &V,
) -> Option<Vec<Signal>> {
    let input = instruction.input_range.index_view(input);
    let input_offset = instruction.input_range.start_offset();
    let arity = gate.inputs().len();

    fn cost_to_f64<I: Into<Option<Cost>>>(cost: I) -> OrderedFloat<f64> {
        match cost.into() {
            None => OrderedFloat(f64::INFINITY),
            Some(value) => value,
        }
    }

    let spilling_costs = (0..arity)
        .map(|i| estimate_spill_cost_operand_pats(version, &input[i]))
        .map(cost_to_f64)
        .collect_vec();

    let mut matrix = Matrix::new_square(arity, Default::default());
    for operand_idx in 0..arity {
        for signal_idx in 0..arity {
            let signal = gate.inputs()[signal_idx];
            let mut has_match = false;
            let mut min_cost = input[operand_idx]
                .iter()
                .flat_map(|target_cell_pat| {
                    let target_cell_inverted = instruction
                        .input_inverted
                        .contains(&(input_offset + operand_idx));
                    version
                        .state()
                        .all_cells_with(signal)
                        .filter_map(|(source_cell, source_cell_inverted)| {
                            let requires_inversion = source_cell_inverted ^ target_cell_inverted;
                            if !requires_inversion && target_cell_pat.matches(&source_cell) {
                                has_match = true;
                                Some(OrderedFloat(0.0))
                            } else {
                                copy_cost(
                                    &params.arch.copy_graph,
                                    source_cell,
                                    *target_cell_pat,
                                    requires_inversion,
                                    &FxHashSet::default(),
                                )
                            }
                        })
                        .min()
                })
                .min();
            // add estimated spilling cost for replacing current value
            if !has_match && !version.has_free_cell_for_cell_pats(&input[operand_idx]) {
                min_cost = min_cost.map(|cost| cost + spilling_costs[operand_idx]);
            }
            matrix[(operand_idx, signal_idx)] = cost_to_f64(min_cost);
        }
    }

    // add estimated spilling cost for replacing overridden value
    for operand_idx in 0..arity {
        if !instruction.input_override.contains(&operand_idx) {
            continue;
        }
        for signal_idx in 0..arity {
            let signal = gate.inputs()[signal_idx];
            // if
            // - we do not copy the value somewhere else,
            // - the value does not exist somewhere and
            // - we will need it again later and
            // then we will most likely need to spill it
            if matrix[(operand_idx, signal_idx)] == 0.0
                && version
                    .state()
                    .cells_with_id(signal.node_id())
                    .nth(1)
                    .is_none()
                && !version.is_last_use(signal.node_id())
            {
                matrix[(operand_idx, signal_idx)] += spilling_costs[operand_idx];
            }
        }
    }

    // check that matrix has an optimal selection
    for i in 0..arity {
        let mut row_has_sol = false;
        let mut col_has_sol = false;
        for j in 0..arity {
            row_has_sol |= matrix[(i, j)] != f64::INFINITY;
            col_has_sol |= matrix[(j, i)] != f64::INFINITY;
        }
        if !row_has_sol || !col_has_sol {
            panic!("impossible {}\n{gate:?}", version.program());
        }
    }

    // rows: operands
    // cols: signals
    // kuhn_munkres_min returns row -> column, i.e. operand -> signal
    let (_, operand_to_signal) = kuhn_munkres_min(&matrix);

    let mut signals = Vec::new();
    for signal_idx in operand_to_signal {
        signals.push(gate.inputs()[signal_idx]);
    }

    Some(signals)
}
