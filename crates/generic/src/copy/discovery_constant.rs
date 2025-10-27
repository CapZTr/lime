use lime_generic_def::{
    BoolHint, Cell, CellPat, CellType, InputIndices, Instruction, InstructionType, Operand,
    set::Set,
};
use tracing::warn;

use crate::{
    copy::{
        constant_mapping::{ConstantMappingHint, map_constants},
        graph::{Edge, FindParams, TO_VAR},
        placeholder::CellOrVar,
    },
    cost::OperationCost,
};

/// Finds all side effect free copy instructions from a constant cell that can be found by using the
/// value of the cell.
pub fn find_set_constant<CT: CellType, CF: OperationCost<CT>>(params: &mut FindParams<'_, CT, CF>) {
    let arch = params.arch.clone();
    for instruction in arch.instructions().iter() {
        for value in [true, false] {
            find_for_output(params, instruction, value);
            find_for_input_result(params, instruction, value);
        }
    }
}

fn find_for_input_result<CT: CellType, CF: OperationCost<CT>>(
    params: &mut FindParams<'_, CT, CF>,
    typ: &InstructionType<CT>,
    value: bool,
) {
    if !typ.outputs.contains_none() {
        return;
    }
    let InputIndices::Index(target_idx) = typ.input_override else {
        return;
    };
    for combination in typ.input.combinations() {
        let mappings = map_constants(
            typ.function,
            ConstantMappingHint::Value(value),
            typ.input_inverted,
            &combination,
            typ.input_range,
            Some(target_idx),
            None,
        );
        for (mut mapping, mut eval) in mappings {
            match eval.hint(value) {
                None | Some(BoolHint::Require(_)) => continue,
                _ => {}
            }
            eval.add_unknown();
            let result_value = eval.evaluate();
            if result_value != Some(value) {
                warn!(
                    ?typ,
                    ?combination,
                    ?value,
                    ?result_value,
                    ?mapping,
                    "did not get expected value for constant mapping"
                );
                continue;
            }
            let to = combination[target_idx];
            mapping.insert(target_idx, Cell::new(CellOrVar::Var, TO_VAR));
            add_edges(
                params,
                Instruction {
                    typ: typ.clone(),
                    inputs: mapping,
                    outputs: vec![],
                },
                value ^ typ.input_inverted.contains(&target_idx),
                to,
                value,
            );
        }
    }
}

fn find_for_output<CT: CellType, CF: OperationCost<CT>>(
    params: &mut FindParams<'_, CT, CF>,
    typ: &InstructionType<CT>,
    value: bool,
) {
    if typ.input_override != InputIndices::None {
        return;
    }
    for combination in typ.input.combinations() {
        let mappings = map_constants(
            typ.function,
            ConstantMappingHint::Value(value),
            typ.input_inverted,
            &combination,
            typ.input_range,
            None,
            None,
        );
        for (mapping, eval) in mappings {
            let result_value = eval.evaluate();
            if result_value != Some(value) {
                warn!(
                    ?typ,
                    ?combination,
                    ?value,
                    ?result_value,
                    ?mapping,
                    "did not get expected value for constant mapping"
                );
                continue;
            }
            for to in typ.outputs.length_one_patterns() {
                add_edges(
                    params,
                    Instruction {
                        typ: typ.clone(),
                        inputs: mapping.clone(),
                        outputs: vec![Operand {
                            cell: Cell::new(CellOrVar::Var, TO_VAR),
                            inverted: to.inverted,
                        }],
                    },
                    value ^ to.inverted,
                    to.cell,
                    value,
                );
            }
        }
    }
}

/// Assuming that an instruction with the given type and inputs produces the given value and that
/// value is written into to, add the relevant edges to this the graph.
fn add_edges<CT: CellType, CF: OperationCost<CT>>(
    params: &mut FindParams<'_, CT, CF>,
    instruction: Instruction<CellOrVar<CT>, CT>,
    value: bool,
    to: CellPat<CT>,
    eval_value: bool,
) {
    let cost = params.cost.cost(&instruction);
    for inverted in [true, false] {
        let graph = &mut params.graph;
        let from_node = CellPat::Cell(CT::constant(value ^ inverted));

        let edge = Edge {
            computes_from_inverted: value ^ eval_value ^ inverted,
            inverted,
            template: vec![instruction.clone()],
            cost,
        };
        graph.consider_edge(from_node, to, edge);
    }
}
