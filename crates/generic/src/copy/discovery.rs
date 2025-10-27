use std::cmp::min;

use itertools::Itertools;
use lime_generic_def::{
    BoolHint, Cell, CellPat, CellType, InputIndices, Instruction, InstructionType, Operand,
    set::Set,
};
use tracing::warn;

use crate::{
    copy::{
        constant_mapping::{ConstantMappingHint, map_constants},
        graph::{Edge, FROM_VAR, FindParams, TO_VAR},
        placeholder::CellOrVar,
    },
    cost::OperationCost,
};

pub fn find_copy_instructions<CT: CellType, CF: OperationCost<CT>>(
    params: &mut FindParams<'_, CT, CF>,
) {
    let arch = params.arch.clone();
    for instruction in arch.instructions().iter() {
        for inverted in [true, false] {
            find_using_input_override(params, instruction, inverted);
            find_using_output(params, instruction, inverted);
        }
    }
}

fn find_using_input_override<CT: CellType, CF: OperationCost<CT>>(
    params: &mut FindParams<CT, CF>,
    typ: &InstructionType<CT>,
    inverted: bool,
) {
    // determine to which operand the result will be written
    let InputIndices::Index(to_idx) = typ.input_override else {
        return;
    };
    for combination in typ.input.combinations() {
        let to = combination[to_idx];
        for (from_idx, from) in combination.iter().enumerate() {
            if from_idx == to_idx {
                continue;
            }
            for (mut mapping, mut eval) in map_constants(
                typ.function,
                ConstantMappingHint::Identity { inverted },
                typ.input_inverted,
                &combination,
                typ.input_range,
                Some(from_idx),
                Some(to_idx),
            ) {
                let Some(to_hint) = eval.hint_id(inverted) else {
                    continue;
                };
                // now we have two scenarios:
                // 1. the function is already known to be a identity no matter what value to currently
                //    holds in which case we are effectively done here
                // 2. the function may become an identity if to has the correct value for it, which we
                //    try to enforce using a previously discovered copy instruction (i.e. setting it to a
                //    constant value)
                let mut templates = Vec::new();
                if let BoolHint::Require(to_value) = to_hint {
                    eval.add(to_value);

                    let const_cell = CT::constant(to_value);
                    for (_, _, edge) in params.graph.all_optimal_edges_matching(
                        CellPat::Cell(const_cell),
                        to,
                        typ.input_inverted.contains(&to_idx),
                    ) {
                        templates.push(
                            edge.instantiate(
                                const_cell.map_cell_type(CellOrVar::from),
                                Cell::new(CellOrVar::Var, TO_VAR),
                            )
                            .collect_vec(),
                        )
                    }
                } else {
                    eval.add_unknown();
                    templates.push(vec![]);
                };
                if eval.id_inverted() != Some(inverted) {
                    warn!(
                        ?typ,
                        ?mapping,
                        ?eval,
                        "mapping did not result in the expected identity"
                    );
                    continue;
                }
                mapping.insert(
                    min(mapping.len(), from_idx),
                    Cell::new(CellOrVar::<CT>::Var, FROM_VAR),
                );
                mapping.insert(to_idx, Cell::new(CellOrVar::<CT>::Var, TO_VAR));
                for mut template in templates {
                    let instruction = Instruction {
                        typ: typ.clone(),
                        inputs: mapping.clone(),
                        outputs: vec![],
                    };
                    let cost = params.cost.cost(&instruction);
                    let cost = template
                        .iter()
                        .fold(cost, |cost, op| cost + params.cost.cost(op));
                    template.push(instruction);
                    params.graph.consider_edge(
                        *from,
                        to,
                        Edge {
                            inverted: typ.input_inverted.contains(&to_idx)
                                ^ typ.input_inverted.contains(&from_idx)
                                ^ inverted,
                            cost,
                            template,
                            computes_from_inverted: inverted
                                ^ typ.input_inverted.contains(&from_idx),
                        },
                    );
                }
            }
        }
    }
}

fn find_using_output<CT: CellType, CF: OperationCost<CT>>(
    params: &mut FindParams<CT, CF>,
    typ: &InstructionType<CT>,
    inverted: bool,
) {
    if typ.input_override != InputIndices::None {
        return;
    }
    for combination in typ.input.combinations() {
        for (from_idx, from) in combination.iter().enumerate() {
            let mappings = map_constants(
                typ.function,
                ConstantMappingHint::Identity { inverted },
                typ.input_inverted,
                &combination,
                typ.input_range,
                Some(from_idx),
                None,
            );
            for (mut mapping, eval) in mappings {
                if eval.id_inverted() != Some(inverted) {
                    if combination.len() != 1 {
                        warn!(
                            ?typ,
                            ?mapping,
                            ?eval,
                            "mapping did not result in the expected identity"
                        );
                    }
                    continue;
                }
                mapping.insert(from_idx, Cell::new(CellOrVar::Var, FROM_VAR));
                for output in typ.outputs.length_one_patterns() {
                    let instruction = Instruction {
                        typ: typ.clone(),
                        inputs: mapping.clone(),
                        outputs: vec![Operand {
                            cell: Cell::new(CellOrVar::Var, TO_VAR),
                            inverted: output.inverted,
                        }],
                    };
                    let cost = params.cost.cost(&instruction);
                    params.graph.consider_edge(
                        *from,
                        output.cell,
                        Edge {
                            inverted: inverted
                                ^ output.inverted
                                ^ typ.input_inverted.contains(&from_idx),
                            template: vec![instruction],
                            cost,
                            computes_from_inverted: inverted
                                ^ typ.input_inverted.contains(&from_idx),
                        },
                    );
                }
            }
        }
    }
}
