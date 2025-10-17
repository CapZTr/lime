#![cfg(test)]
#![allow(dead_code)]

use std::{borrow::Cow, rc::Rc};

use eggmock::{Id, Network, Node, Signal};
use lime_generic_def::{Cell, Instruction, InstructionType, Operand};
use rustc_hash::FxHashMap;

use crate::{
    ArchitectureMeta,
    compilation::{
        CandidateSelection, CompilationMode, CompilationParameters, compile,
        optimization::optimize_outputs,
    },
    copy::CopyGraph,
    cost::EqualCosts,
    definitions::{Ambit, AmbitCellType, FELIX, IMPLY, PLiM},
    program::state::{Operation, Program},
    untyped_ntk::UntypedNetwork,
};

#[test]
fn print_copy_graphs() {
    println!("Ambit: {:?}", CopyGraph::build(&Ambit::new(), &EqualCosts));
    println!("PLiM: {:?}", CopyGraph::build(&PLiM::new(), &EqualCosts));
    println!("FELIX: {:?}", CopyGraph::build(&FELIX::new(), &EqualCosts));
    println!("IMPLY: {:?}", CopyGraph::build(&IMPLY::new(), &EqualCosts));
}

#[test]
fn test_opt() {
    let ambit = Ambit::new();
    let types: FxHashMap<Cow<'static, str>, &InstructionType<AmbitCellType>> = ambit
        .instructions()
        .iter()
        .map(|instr| (instr.name.clone(), instr))
        .collect();
    let mut program = Program(vec![
        Operation::Candidate(
            Instruction {
                inputs: vec![
                    Cell::new(AmbitCellType::T, 0),
                    Cell::new(AmbitCellType::T, 1),
                    Cell::new(AmbitCellType::T, 2),
                ],
                outputs: vec![],
                typ: types["TRA"].clone(),
            },
            Id::from_usize(0),
        ),
        Operation::Copy {
            computes_from_inverted: false,
            from: Cell::new(AmbitCellType::D, 1),
            to: Cell::new(AmbitCellType::T, 3),
            inverted: false,
            instructions: vec![Instruction {
                inputs: vec![Cell::new(AmbitCellType::D, 1)],
                outputs: vec![Operand {
                    cell: Cell::new(AmbitCellType::T, 3),
                    inverted: false,
                }],
                typ: types["RC"].clone(),
            }],
            spill: false,
        },
        Operation::Copy {
            from: Cell::new(AmbitCellType::D, 1),
            to: Cell::new(AmbitCellType::T, 1),
            inverted: false,
            instructions: vec![],
            spill: false,
            computes_from_inverted: false,
        },
        Operation::Copy {
            from: Cell::new(AmbitCellType::T, 3),
            to: Cell::new(AmbitCellType::T, 2),
            inverted: false,
            instructions: vec![],
            spill: false,
            computes_from_inverted: false,
        },
        Operation::Copy {
            from: Cell::new(AmbitCellType::T, 0),
            to: Cell::new(AmbitCellType::DCC, 1),
            inverted: true,
            instructions: vec![],
            spill: false,
            computes_from_inverted: false,
        },
    ]);
    println!("{program}");
    optimize_outputs(&mut program);
    println!("===================");
    println!("{program}");
}

fn mux1() -> Network<UntypedNetwork> {
    let mut ntk = Network::default();
    let i0 = Signal::new(ntk.add(Node::Input(0)), false);
    let i1 = Signal::new(ntk.add(Node::Input(1)), false);
    let i2 = Signal::new(ntk.add(Node::Input(2)), false);
    let f = Signal::new(ntk.add(Node::False), false);

    let n1 = Signal::new(
        ntk.add(Node::Gate(UntypedNetwork::Maj(vec![i0, i1, f]))),
        false,
    );
    let n2 = Signal::new(
        ntk.add(Node::Gate(UntypedNetwork::Maj(vec![!i1, f, i2]))),
        false,
    );
    let n3 = Signal::new(
        ntk.add(Node::Gate(UntypedNetwork::Maj(vec![!f, n1, n2]))),
        false,
    );

    ntk.set_outputs(vec![n3]);

    ntk
}

fn mux2() -> Network<UntypedNetwork> {
    let mut ntk = Network::default();
    let i0 = Signal::new(ntk.add(Node::Input(0)), false);
    let i1 = Signal::new(ntk.add(Node::Input(1)), false);
    let i2 = Signal::new(ntk.add(Node::Input(2)), false);
    let f = Signal::new(ntk.add(Node::False), false);

    let n1 = Signal::new(
        ntk.add(Node::Gate(UntypedNetwork::Maj(vec![i0, i1, f]))),
        false,
    );
    let n2 = Signal::new(
        ntk.add(Node::Gate(UntypedNetwork::Maj(vec![n1, !i1, i2]))),
        false,
    );
    let n3 = Signal::new(
        ntk.add(Node::Gate(UntypedNetwork::Maj(vec![!f, n1, n2]))),
        false,
    );

    ntk.set_outputs(vec![n3]);

    ntk
}

#[test]
fn test_compile() {
    let ntk = mux2();
    let arch = Ambit::new();
    let cost = EqualCosts;
    let arch = ArchitectureMeta {
        copy_graph: CopyGraph::build(&arch, &cost),
        arch,
    };
    let program = compile(CompilationParameters {
        arch: Rc::new(arch),
        candidate_selection: CandidateSelection::All,
        cost: EqualCosts,
        disjunct_input_output: false,
        input_cells: vec![
            Cell::new(AmbitCellType::D, 0),
            Cell::new(AmbitCellType::D, 1),
            Cell::new(AmbitCellType::D, 2),
        ],
        mode: CompilationMode::Exhaustive,
        network: ntk,
    });
    println!("{}", program.unwrap().program)
}
