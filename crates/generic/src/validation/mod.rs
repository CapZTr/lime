use eggmock::{Network, Node, Signal};
use lime_generic_def::{Cell, CellType, Gate, set::Set};
use rustc_hash::FxHashMap;

use crate::{program::state::Program, untyped_ntk::UntypedNetwork};

pub fn rebuild_network<CT: CellType>(
    program: &Program<CT>,
    inputs: &[Cell<CT>],
    outputs: &[Cell<CT>],
) -> Result<Network<UntypedNetwork>, String> {
    let mut cells = FxHashMap::default();
    let mut ntk = Network::default();
    let f = Signal::new(ntk.add(Node::False), false);
    cells.insert(CT::constant(false), f);
    cells.insert(CT::constant(true), !f);
    for (i, cell) in inputs.iter().enumerate() {
        let id = ntk.add(Node::Input(i as u32));
        cells.insert(*cell, Signal::new(id, false));
    }
    for instruction in program.instructions() {
        instruction
            .validate()
            .map_err(|()| format!("invalid instruction {instruction}"))?;
        let inputs = instruction.typ.input_range.slice(&instruction.inputs).1;
        let mut evaluation = instruction.typ.function.evaluate(inputs.len());
        let mut inputs = Vec::new();
        let (in_offset, input_cells, _) = instruction.typ.input_range.slice(&instruction.inputs);
        for (in_idx, input) in input_cells.iter().enumerate() {
            let sig = cells.get(input);
            let Some(sig) = sig else {
                evaluation.add_unknown();
                continue;
            };
            let sig = *sig
                ^ instruction
                    .typ
                    .input_inverted
                    .contains(&(in_offset + in_idx));
            inputs.push(sig);
            let node = ntk.node(sig.node_id());
            if matches!(node, Node::False) {
                evaluation.add(sig.is_inverted());
            } else {
                evaluation.add_unknown();
            }
        }
        let signal = if let Some(value) = evaluation.evaluate() {
            f ^ value
        } else {
            let node = match instruction.typ.function.gate {
                Gate::And => UntypedNetwork::And(inputs),
                Gate::Maj => UntypedNetwork::Maj(inputs),
                Gate::Xor => UntypedNetwork::Xor(inputs),
                // evaluation would have a result
                Gate::Constant(_) => unreachable!(),
            };
            Signal::new(ntk.add(Node::Gate(node)), instruction.typ.function.inverted)
        };
        for op in instruction.write_operands() {
            cells.insert(op.cell, signal ^ op.inverted);
        }
    }
    let mut output_signals = Vec::new();
    for output in outputs {
        output_signals.push(*cells.get(output).expect("output cell should be set"));
    }
    ntk.set_outputs(output_signals);
    Ok(ntk)
}
