use std::{cmp::Reverse, mem::take};

use itertools::Itertools;
use lime_generic_def::{CellPat, CellType, Operand, PatBase, TuplesDef};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::program::state::{Operation, Program};

pub fn optimize_outputs<CT: CellType>(program: &mut Program<CT>) {
    let mut source_op_i = 0;
    loop {
        if source_op_i == program.0.len() {
            break;
        }
        for source_instr_i in 0..program.0[source_op_i].instructions().len() {
            let source_op = &program.0[source_op_i];
            let source_op_instr = source_op.instructions();
            let instr = &source_op_instr[source_instr_i];
            // cell -> inverted
            let mut output_cells = instr.write_cell_inverted_map();

            // allow RC(<src>) -> (<dst1>); RC(<src>) -> (<dst2>) to be merged to
            // RC(<src>) -> (<dst1>, <dst2>)
            if source_instr_i == source_op_instr.len() - 1
                && let Operation::Copy {
                    from,
                    computes_from_inverted,
                    ..
                } = source_op
            {
                if *from == CT::constant(true) {
                    // normalize to false
                    output_cells
                        .entry(CT::constant(false))
                        .or_insert(!*computes_from_inverted);
                } else {
                    output_cells.entry(*from).or_insert(*computes_from_inverted);
                }
            }

            // handle rest of the current operation
            let mut rw_between = FxHashSet::default();
            for source_op_remaining_i in source_instr_i + 1..source_op_instr.len() {
                let instr = &source_op.instructions()[source_op_remaining_i];
                rw_between.extend(instr.read_cells());
                for cell in instr.write_cells() {
                    output_cells.remove(&cell);
                }
            }

            // determine which operations we can possibly elide
            // elements: (operation_idx, target_operand)
            let mut elided_copy_operations = Vec::new();
            for elided_copy_op_i in source_op_i + 1..program.0.len() {
                let op = &program.0[elided_copy_op_i];
                if output_cells.is_empty() {
                    break;
                }
                if let Operation::Copy {
                    from, to, inverted, ..
                } = op
                {
                    let (from, inverted) = if *from == CT::constant(true) {
                        (CT::constant(false), !*inverted)
                    } else {
                        (*from, *inverted)
                    };
                    let (from, inverted) = (&from, &inverted);
                    if let Some(&inverted_out) = output_cells.get(from) {
                        if !rw_between.contains(to) {
                            let operand = Operand {
                                cell: *to,
                                inverted: inverted ^ inverted_out,
                            };
                            elided_copy_operations.push((elided_copy_op_i, operand));
                        }
                        rw_between.extend([*from, *to]);
                        // TODO: transitive copy?
                        // output_cells.remove(to);
                        output_cells.insert(*to, inverted ^ inverted_out);
                    } else {
                        rw_between.extend([*from, *to]);
                        output_cells.remove(to);
                    }
                } else {
                    for instr in op.instructions() {
                        rw_between.extend(instr.read_cells().chain(instr.write_cells()));
                        for cell in instr.write_cells() {
                            output_cells.remove(&cell);
                        }
                    }
                }
            }

            if elided_copy_operations.is_empty() {
                continue;
            }

            // change output operands of the instruction
            let Some((output, mut elided_ops)) = instr
                .typ
                .outputs
                .iter()
                .flat_map(|operands| {
                    let combinations = match operands {
                        TuplesDef::Nary(_) => unimplemented!(),
                        TuplesDef::Tuples(_) => operands.combinations(),
                    };
                    combinations.into_iter().filter_map(|tuple| {
                        let tuple_len = tuple.len();
                        // assign cells first, then types to allow greater flexibility
                        let mut tuple = tuple.into_iter().enumerate().collect_vec();
                        tuple.sort_by_key(|(_, op)| match op.cell {
                            CellPat::Type(_) => 1,
                            CellPat::Cell(_) => 0,
                        });

                        let mut operands = FxHashMap::default();

                        // (1) map previous outputs
                        for previous in &instr.outputs {
                            let (tuple_i, &(i, _)) = tuple
                                .iter()
                                .enumerate()
                                .find(|(_, (_, op_pat))| op_pat.matches(previous))?;
                            operands.insert(i, previous);
                            tuple.swap_remove(tuple_i);
                        }

                        // (2) map remaining outputs
                        let mut elided_operation_idxs = Vec::new();
                        for (elided_op_idx, operand) in &elided_copy_operations {
                            let Some((tuple_i, &(i, _))) = tuple
                                .iter()
                                .enumerate()
                                .find(|(_, (_, op_pat))| op_pat.matches(operand))
                            else {
                                continue;
                            };
                            operands.insert(i, operand);
                            elided_operation_idxs.push(*elided_op_idx);
                            tuple.swap_remove(tuple_i);
                        }

                        let mut output = Vec::with_capacity(tuple.len());
                        for i in 0..tuple_len {
                            let &&operand = operands.get(&i)?;
                            output.push(operand);
                        }
                        Some((output, elided_operation_idxs))
                    })
                })
                .max_by_key(|(output, _)| output.len())
            else {
                continue;
            };

            program.0[source_op_i].instructions_mut()[source_instr_i].outputs = output;
            elided_ops.sort_by_key(|i| Reverse(*i));
            for elided in elided_ops {
                program.0.remove(elided);
            }

            let op = &mut program.0[source_op_i];
            if let Operation::Copy {
                instructions,
                from,
                to,
                inverted,
                ..
            } = op
            {
                *op = Operation::Other {
                    instructions: take(instructions),
                    comment: Some(format!(
                        "optimized copy from {from} to {to} (inverted: {inverted})"
                    )),
                }
            }
        }
        source_op_i += 1;
    }
}
