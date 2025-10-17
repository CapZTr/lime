use std::rc::Rc;

use eggmock::{Gate, Id};
use lime_generic_def::CellType;
use rustc_hash::FxHashSet;

use crate::{
    compilation::CompilationParameters,
    cost::OperationCost,
    program::{
        self, ProgramVersion,
        state::{Candidates, CellStates, Operation, StateDelta, StateSavepoint},
    },
};

pub struct DeltaCollectionProgramVersion<'a, CT: CellType, G: Gate, C: OperationCost<CT>> {
    state: StateSavepoint<'a, CT, G>,
    original_delta: StateDelta<CT>,
    params: &'a Rc<CompilationParameters<CT, G, C>>,
    collection: &'a mut Vec<StateDelta<CT>>,
}

impl<'a, CT: CellType, G: Gate, C: OperationCost<CT>> DeltaCollectionProgramVersion<'a, CT, G, C> {
    pub fn new(
        state: StateSavepoint<'a, CT, G>,
        params: &'a Rc<CompilationParameters<CT, G, C>>,
        collections: &'a mut Vec<StateDelta<CT>>,
    ) -> Self {
        Self {
            state,
            original_delta: Default::default(),
            params,
            collection: collections,
        }
    }
    pub fn delta(&self) -> StateDelta<CT> {
        let mut original_delta = self.original_delta.clone();
        self.state.append_to_delta(&mut original_delta);
        original_delta
    }
}

impl<'a, CT: CellType, G: Gate, C: OperationCost<CT>> ProgramVersion
    for DeltaCollectionProgramVersion<'a, CT, G, C>
{
    type CT = CT;
    type G = G;
    type C = C;

    fn branch(&mut self) -> impl ProgramVersion<CT = Self::CT, G = Self::G, C = Self::C> {
        DeltaCollectionProgramVersion {
            original_delta: self.delta(),
            state: self.state.savepoint(),
            collection: self.collection,
            params: self.params,
        }
    }
    fn parameters(&self) -> &Rc<CompilationParameters<CT, G, C>> {
        self.params
    }
    fn append(&mut self, instr: Operation<CT>) {
        self.state.append_instruction(instr);
    }
    fn state_mut(&mut self) -> &mut impl CellStates<CT> {
        &mut self.state
    }
    fn state(&self) -> &impl CellStates<CT> {
        &self.state
    }
    fn candidates(&self) -> &Candidates {
        self.state.candidates()
    }
    fn uses(&self) -> &super::state::Uses {
        self.state.uses()
    }
    fn output_ids(&self) -> &FxHashSet<Id> {
        self.state.output_ids()
    }
    fn consider(self) {
        self.collection.push(self.delta());
    }
    fn program(&self) -> &program::state::Program<Self::CT> {
        self.state.program()
    }
}
