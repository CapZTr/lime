use core::slice;
use std::iter;

use derive_more::Display;
use egg::{Analysis, CostFunction, EGraph, Extractor, Id, Language, LpCostFunction, RecExpr};
use eggmock::{Network, NetworkLanguage, Node, Signal};
use either::Either;
use itertools::Itertools;
use lime_generic_def::{
    BoolSet, Cell, CellPat, CellType, Gate, InputIndices, Instruction, InstructionType, Outputs,
    TuplesDef, set::Set,
};
use ordered_float::OrderedFloat;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::{ArchitectureMeta, copy::copy_cost, cost::OperationCost, untyped_ntk::UntypedNetwork};

const INPUT_INSTRUCTION_TYPE: u8 = u8::MAX;
const FALSE_INSTRUCTION_TYPE: u8 = u8::MAX - 1;

#[derive(Debug, Clone, Hash, PartialEq, Eq, PartialOrd, Ord, Display)]
pub enum InstructionEGraphLanguage {
    #[display("I{_0}")]
    Input(u32),
    #[display("false")]
    False,
    #[display("Instr{_0}")]
    Instruction(u8, Vec<Id>),
    #[display("{}", if *inverted { "!" } else { "*" })]
    InstructionValue {
        instruction_type: u8,
        instruction_arity: usize,
        inverted: bool,
        instruction: Id,
    },
    #[display("D{_0}")]
    Dummy(u32),
}

#[derive(Debug, Copy, Clone, Hash, PartialEq, Eq)]
pub enum InstructionEGraphLanguageDiscriminant {
    Input,
    False,
    Instruction(u8),
    InstructionValue(u8, usize, bool),
    Dummy,
}

impl Language for InstructionEGraphLanguage {
    type Discriminant = InstructionEGraphLanguageDiscriminant;

    fn discriminant(&self) -> Self::Discriminant {
        match self {
            Self::Input(_) => InstructionEGraphLanguageDiscriminant::Input,
            Self::False => InstructionEGraphLanguageDiscriminant::False,
            Self::Instruction(id, _) => InstructionEGraphLanguageDiscriminant::Instruction(*id),
            Self::InstructionValue {
                instruction_type,
                instruction_arity,
                inverted,
                ..
            } => InstructionEGraphLanguageDiscriminant::InstructionValue(
                *instruction_type,
                *instruction_arity,
                *inverted,
            ),
            Self::Dummy(_) => InstructionEGraphLanguageDiscriminant::Dummy,
        }
    }

    fn matches(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Input(i1), Self::Input(i2)) => i1 == i2,
            (Self::False, Self::False) => true,
            (Self::Instruction(id1, ins1), Self::Instruction(id2, ins2)) => {
                id1 == id2 && ins1.len() == ins2.len()
            }
            (
                Self::InstructionValue {
                    instruction_type: it1,
                    instruction_arity: ia1,
                    inverted: i1,
                    ..
                },
                Self::InstructionValue {
                    instruction_type: it2,
                    instruction_arity: ia2,
                    inverted: i2,
                    ..
                },
            ) => it1 == it2 && i1 == i2 && ia1 == ia2,
            (Self::Dummy(d1), Self::Dummy(d2)) => d1 == d2,
            _ => false,
        }
    }

    fn children(&self) -> &[Id] {
        match self {
            Self::Input(_) | Self::Dummy(_) | Self::False => &[],
            Self::Instruction(_, inputs) => inputs,
            Self::InstructionValue { instruction, .. } => slice::from_ref(instruction),
        }
    }

    fn children_mut(&mut self) -> &mut [Id] {
        match self {
            Self::Input(_) | Self::Dummy(_) | Self::False => &mut [],
            Self::Instruction(_, inputs) => inputs,
            Self::InstructionValue { instruction, .. } => slice::from_mut(instruction),
        }
    }
}

struct TransformationState<'s, CT: CellType, L: NetworkLanguage, N: Analysis<L>> {
    original: &'s EGraph<L, N>,
    egraph: EGraph<InstructionEGraphLanguage, ()>,
    mappings: FxHashMap<Id, [Id; 2]>,
    arch: &'s ArchitectureMeta<CT>,
    dummy_counter: u32,
}

impl<'s, CT: CellType, L: NetworkLanguage, N: Analysis<L>> TransformationState<'s, CT, L, N> {
    pub fn get_mapped_eclasses(&mut self, id: Id) -> [Id; 2] {
        *self.mappings.entry(id).or_insert_with(|| {
            let d1 = self.dummy_counter;
            self.dummy_counter += 1;
            let d2 = self.dummy_counter;
            self.dummy_counter += 1;
            let class1 = self.egraph.add(InstructionEGraphLanguage::Dummy(d1));
            let class2 = self.egraph.add(InstructionEGraphLanguage::Dummy(d2));
            [class1, class2]
        })
    }
    pub fn union(&mut self, id1: Id, id2: Id) -> bool {
        self.egraph.union(id1, id2)
    }
    pub fn build_instruction_eclasses(
        &mut self,
        eclass_id: Id,
        node: &L,
    ) -> impl Iterator<Item = (Id, u8, bool)> {
        if node.is_not() {
            let inv_eclass_id = &self.original.find(node.children()[0]);
            let classes1 = self.get_mapped_eclasses(eclass_id);
            let classes2 = self.get_mapped_eclasses(*inv_eclass_id);
            for inv in [true, false] {
                self.union(classes1[inv as usize], classes2[!inv as usize]);
            }
            return None.into_iter().flatten();
        }
        if node.is_input() || node.is_false() {
            let (id, typ) = match node.input_id() {
                Some(id) => (
                    self.egraph.add(InstructionEGraphLanguage::Input(id)),
                    INPUT_INSTRUCTION_TYPE,
                ),
                None => (
                    self.egraph.add(InstructionEGraphLanguage::False),
                    FALSE_INSTRUCTION_TYPE,
                ),
            };
            return Some(Either::Left(iter::once((id, typ, false))))
                .into_iter()
                .flatten();
        }
        let gate = node.gate_function().unwrap();
        let arity = node.children().len();
        Some(Either::Right(
            self.arch
                .instructions()
                .iter()
                .filter(move |typ| {
                    typ.function.gate.gate_function() == Some(gate)
                        && typ.arity().is_none_or(|instr_arity| arity == instr_arity)
                })
                .map(|instr| {
                    let children = node
                        .children()
                        .iter()
                        .enumerate()
                        .map(|(i, id)| {
                            self.get_mapped_eclasses(*id)
                                [instr.input_inverted.contains(&i) as usize]
                        })
                        .collect_vec();
                    let id = self
                        .egraph
                        .add(InstructionEGraphLanguage::Instruction(instr.id, children));
                    (id, instr.id, instr.function.inverted)
                }),
        ))
        .into_iter()
        .flatten()
    }
}

pub fn transform_egraph<L: NetworkLanguage, N: Analysis<L>, CT: CellType>(
    egraph: &EGraph<L, N>,
    arch: &ArchitectureMeta<CT>,
    outputs: &[Id],
) -> (EGraph<InstructionEGraphLanguage, ()>, Vec<Id>) {
    let mut state = TransformationState {
        original: egraph,
        arch,
        egraph: Default::default(),
        mappings: Default::default(),
        dummy_counter: 0,
    };

    for eclass in egraph.classes() {
        let mapped_eclass_ids = state.get_mapped_eclasses(eclass.id);
        for node in &eclass.nodes {
            for (instruction_id, instruction_type, instruction_inverted_node) in state
                .build_instruction_eclasses(eclass.id, node)
                .collect_vec()
            {
                for access_inverted in [true, false] {
                    let access_node =
                        state
                            .egraph
                            .add(InstructionEGraphLanguage::InstructionValue {
                                instruction_type,
                                instruction_arity: node.len(),
                                inverted: access_inverted,
                                instruction: instruction_id,
                            });
                    state.union(
                        mapped_eclass_ids[(instruction_inverted_node ^ access_inverted) as usize],
                        access_node,
                    );
                }
            }
        }
    }

    for eclass in state.egraph.classes_mut() {
        eclass
            .nodes
            .retain(|node| !matches!(node, InstructionEGraphLanguage::Dummy(_)));
    }
    state.egraph.rebuild();

    let outputs = outputs
        .iter()
        .map(|id| state.egraph.find(state.mappings[id][0]))
        .collect_vec();
    (state.egraph, outputs)
}

pub trait IdToLang {
    fn at(&self, id: Id) -> &InstructionEGraphLanguage;
}

impl IdToLang for RecExpr<InstructionEGraphLanguage> {
    fn at(&self, id: Id) -> &InstructionEGraphLanguage {
        &self[id]
    }
}

impl<'e, CF: CostFunction<InstructionEGraphLanguage>, N: Analysis<InstructionEGraphLanguage>>
    IdToLang for Extractor<'e, CF, InstructionEGraphLanguage, N>
{
    fn at(&self, id: Id) -> &InstructionEGraphLanguage {
        self.find_best_node(id)
    }
}

pub fn rebuild_network<CT: CellType, C: OperationCost<CT>>(
    expr: &impl IdToLang,
    outputs: &[Id],
    arch: &ArchitectureMeta<CT>,
    cost: &mut LpInversionCostFunction<CT, C>,
) -> (f64, Network<UntypedNetwork>) {
    let mut ntk = Network::default();
    let mut id_to_signal = FxHashMap::<Id, Signal>::default();
    let mut total_cost = 0.0;

    fn helper<CT: CellType, C: OperationCost<CT>>(
        ntk: &mut Network<UntypedNetwork>,
        expr: &impl IdToLang,
        id_to_signal: &mut FxHashMap<Id, Signal>,
        id: Id,
        arch: &ArchitectureMeta<CT>,
        cost: &mut LpInversionCostFunction<CT, C>,
        total_cost: &mut f64,
    ) -> Signal {
        if let Some(signal) = id_to_signal.get(&id) {
            return *signal;
        }
        let node = expr.at(id);
        *total_cost += cost.get_cost(node);
        let signal = match expr.at(id) {
            InstructionEGraphLanguage::Dummy(_) => unimplemented!(),
            InstructionEGraphLanguage::False => Signal::new(ntk.add(Node::False), false),
            InstructionEGraphLanguage::Input(input) => {
                Signal::new(ntk.add(Node::Input(*input)), false)
            }
            InstructionEGraphLanguage::InstructionValue {
                inverted,
                instruction,
                ..
            } => {
                helper(
                    ntk,
                    expr,
                    id_to_signal,
                    *instruction,
                    arch,
                    cost,
                    total_cost,
                ) ^ *inverted
            }
            InstructionEGraphLanguage::Instruction(instruction_type, children) => {
                let instr = arch.instructions().by_id(*instruction_type);
                let inputs = children
                    .iter()
                    .enumerate()
                    .map(|(i, child_id)| {
                        helper(ntk, expr, id_to_signal, *child_id, arch, cost, total_cost)
                            ^ instr.input_inverted.contains(&i)
                    })
                    .collect_vec();
                let gate = match instr.function.gate {
                    Gate::And => UntypedNetwork::And(inputs),
                    Gate::Xor => UntypedNetwork::Xor(inputs),
                    Gate::Maj => UntypedNetwork::Maj(inputs),
                    _ => unimplemented!(),
                };
                Signal::new(ntk.add(Node::Gate(gate)), instr.function.inverted)
            }
        };
        id_to_signal.insert(id, signal);
        signal
    }

    let outputs = outputs
        .iter()
        .map(|id| {
            helper(
                &mut ntk,
                expr,
                &mut id_to_signal,
                *id,
                arch,
                cost,
                &mut total_cost,
            )
        })
        .collect_vec();
    ntk.set_outputs(outputs);
    eprintln!(
        "rebuilt network with total cost {total_cost}, size {}",
        ntk.size()
    );
    (total_cost, ntk)
}

#[derive(Clone)]
pub struct LpInversionCostFunction<'a, CT: CellType, C: OperationCost<CT>> {
    meta: &'a ArchitectureMeta<CT>,
    inv_cost: f64,
    instr_costs: FxHashMap<(u8, usize), (f64, BoolSet)>,
    cost: C,
}

impl<'a, CT: CellType, C: OperationCost<CT>> LpInversionCostFunction<'a, CT, C> {
    pub fn new(meta: &'a ArchitectureMeta<CT>, cost: C) -> Self {
        Self {
            cost,
            inv_cost: estimate_inversion_cost(meta),
            meta,
            instr_costs: Default::default(),
        }
    }
    fn get_instr_cost(&mut self, id: u8, arity: usize) -> (f64, BoolSet) {
        if id == INPUT_INSTRUCTION_TYPE {
            return (0.0, BoolSet::Single(false));
        }
        if id == FALSE_INSTRUCTION_TYPE {
            return (0.0, BoolSet::All);
        }
        *self.instr_costs.entry((id, arity)).or_insert_with(|| {
            let instr = self.meta.arch.instructions().by_id(id);
            let set = get_output_inverted(instr, arity);
            let cost = self
                .cost
                .cost(&Instruction::<CT> {
                    typ: instr.clone(),
                    inputs: Vec::new(),
                    outputs: Vec::new(),
                })
                .0;
            (cost, set)
        })
    }
    fn get_cost(&mut self, enode: &InstructionEGraphLanguage) -> f64 {
        let cost = match enode {
            InstructionEGraphLanguage::Dummy(_) => 10_000_000.0, // large, please don't select me :(
            InstructionEGraphLanguage::False => 0.0,
            InstructionEGraphLanguage::Input(_) => 0.0,
            InstructionEGraphLanguage::Instruction(id, ins) => {
                self.get_instr_cost(*id, ins.len()).0
            }
            InstructionEGraphLanguage::InstructionValue {
                instruction_type,
                instruction_arity,
                inverted,
                ..
            } => {
                let inv = self.get_instr_cost(*instruction_type, *instruction_arity).1;
                if !inv.contains(*inverted) {
                    self.inv_cost
                } else {
                    0.0
                }
            }
        };
        cost // + 0.01 + 0.01 * enode.children().len() as f64
    }
}

impl<'a, N: Analysis<InstructionEGraphLanguage>, CT: CellType, C: OperationCost<CT>>
    LpCostFunction<InstructionEGraphLanguage, N> for LpInversionCostFunction<'a, CT, C>
{
    fn node_cost(
        &mut self,
        _egraph: &EGraph<InstructionEGraphLanguage, N>,
        _eclass: Id,
        enode: &InstructionEGraphLanguage,
    ) -> f64 {
        self.get_cost(enode)
    }
}

impl<'s, CT: CellType, C: OperationCost<CT>> CostFunction<InstructionEGraphLanguage>
    for LpInversionCostFunction<'s, CT, C>
{
    type Cost = OrderedFloat<f64>;

    fn cost<Cs>(&mut self, enode: &InstructionEGraphLanguage, mut costs: Cs) -> Self::Cost
    where
        Cs: FnMut(Id) -> Self::Cost,
    {
        OrderedFloat(
            self.get_cost(enode) + enode.children().iter().map(|id| costs(*id).0).sum::<f64>(),
        )
    }
}

pub fn estimate_inversion_cost<CT: CellType>(meta: &ArchitectureMeta<CT>) -> f64 {
    let mut nodes = meta.copy_graph.nodes();
    nodes.remove(&CellPat::Type(CT::CONSTANT));
    nodes.remove(&CellPat::Cell(Cell::new(CT::CONSTANT, 0)));
    nodes.remove(&CellPat::Cell(Cell::new(CT::CONSTANT, 1)));
    let mut cost_sum = 0.0;
    let mut cost_n = 0;
    for src in &nodes {
        for dst in &nodes {
            if let Some(cost) = copy_cost(&meta.copy_graph, *src, *dst, true, &FxHashSet::default())
            {
                cost_sum += cost.0;
                cost_n += 1;
            }
        }
    }
    if cost_n == 0 {
        panic!("no inversion operation found");
    }
    cost_sum / cost_n as f64
}

fn get_output_inverted<CT>(instruction: &InstructionType<CT>, arity: usize) -> BoolSet {
    let mut set = BoolSet::None;
    set = set.insert_all(get_output_invert_boolset(&instruction.outputs));
    if set == BoolSet::All {
        return set;
    }
    set = set.insert_all(get_input_override_invert_boolset(
        instruction.input_override,
        instruction.input_inverted,
        arity,
    ));
    set
}

pub fn get_output_invert_boolset<CT>(outputs: &Outputs<CT>) -> BoolSet {
    let mut set = BoolSet::None;
    for output in outputs.iter() {
        match output {
            TuplesDef::Nary(nary) => {
                set = set.insert_all(nary.0.iter().map(|op| op.inverted).collect())
            }
            TuplesDef::Tuples(tuples) => {
                for tuple in tuples.iter() {
                    set = set.insert_all(
                        tuple
                            .iter()
                            .flat_map(|operand_pats| operand_pats.iter().map(|op| op.inverted))
                            .collect(),
                    );
                    if set == BoolSet::All {
                        return set;
                    }
                }
            }
        };
        if set == BoolSet::All {
            return set;
        }
    }
    set
}

pub fn get_input_override_invert_boolset(
    indices: InputIndices,
    invert: InputIndices,
    arity: usize,
) -> BoolSet {
    if indices == InputIndices::None {
        return BoolSet::None;
    }
    // TODO: handle forwarded range correctly
    (0..arity).map(|i| invert.contains(&i)).collect()
}
