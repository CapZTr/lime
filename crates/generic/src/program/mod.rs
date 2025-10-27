pub mod collection;
pub mod state;

use std::rc::Rc;

use eggmock::{Gate, Id};
use lime_generic_def::{Cell, CellPat, CellType, PatBase, Pats, set::Set};
use rustc_hash::FxHashSet;

use crate::{
    compilation::CompilationParameters,
    copy::spilling::spill_if_necessary,
    cost::OperationCost,
    program::state::{Candidates, CellStates, Operation, Program, StateSavepoint, Uses},
};

pub trait ProgramVersion {
    type CT: CellType;
    type G: Gate;
    type C: OperationCost<Self::CT>;

    fn branch(&mut self) -> impl ProgramVersion<CT = Self::CT, G = Self::G, C = Self::C>;

    fn parameters(&self) -> &Rc<CompilationParameters<Self::CT, Self::G, Self::C>>;
    fn append(&mut self, instr: Operation<Self::CT>);
    fn state_mut(&mut self) -> &mut impl CellStates<Self::CT>;
    fn state(&self) -> &impl CellStates<Self::CT>;
    fn candidates(&self) -> &Candidates;
    fn uses(&self) -> &Uses;
    fn output_ids(&self) -> &FxHashSet<Id>;
    fn program(&self) -> &Program<Self::CT>;

    fn consider(self);

    fn has_free_cell_for_cell_pat(&self, pat: &impl PatBase<CellType = Self::CT>) -> bool {
        let free_cells = self.state().free_cells(pat.cell_type());
        match pat.cell_index() {
            Some(idx) => free_cells.contains(idx),
            None => free_cells.iter().next().is_some(),
        }
    }
    fn has_free_cell_for_cell_pats(&self, pats: &Pats<impl PatBase<CellType = Self::CT>>) -> bool {
        pats.iter().any(|pat| self.has_free_cell_for_cell_pat(pat))
    }

    fn find_preferred_free_cell_for_type(
        &self,
        typ: Self::CT,
        not: &impl Set<Cell<Self::CT>>,
    ) -> Option<Cell<Self::CT>> {
        if let Some(cell) = self
            .state()
            .free_cells(typ)
            .iter()
            .map(|cell_idx| Cell::new(typ, cell_idx))
            .find(|cell| !not.contains(cell))
        {
            return Some(cell);
        }
        let cell = typ.cell_iter().find(|cell| !not.contains(cell))?;
        Some(cell)
    }

    fn find_preferred_free_cell_for_pat(
        &self,
        pat: CellPat<Self::CT>,
        not: &impl Set<Cell<Self::CT>>,
    ) -> Option<Cell<Self::CT>> {
        match pat.index() {
            Some(idx) => {
                let cell = Cell::new(pat.cell_type(), idx);
                if not.contains(&cell) {
                    return None;
                }
                Some(cell)
            }
            None => self.find_preferred_free_cell_for_type(pat.cell_type(), not),
        }
    }

    fn make_overridable_cell_for_pat(
        &mut self,
        pat: CellPat<Self::CT>,
        not: &impl Set<Cell<Self::CT>>,
    ) -> Option<Cell<Self::CT>> {
        let cell = self.find_preferred_free_cell_for_pat(pat, not)?;
        spill_if_necessary(self, cell);
        Some(cell)
    }

    fn is_last_use(&self, id: Id) -> bool {
        let uses = self.uses().get(id);
        let all_uses = self.parameters().network.node_output_ids(id).len()
            + self.output_ids().contains(&id) as usize;
        uses + 1 >= all_uses
    }
}

pub struct DummyProgramVersion<'a, 'b, CT: CellType, G: Gate, C: OperationCost<CT>> {
    savepoint: &'a mut StateSavepoint<'b, CT, G>,
    params: &'a Rc<CompilationParameters<CT, G, C>>,
}
impl<'a, 'b, CT: CellType, G: Gate, C: OperationCost<CT>> DummyProgramVersion<'a, 'b, CT, G, C> {
    pub fn new(
        savepoint: &'a mut StateSavepoint<'b, CT, G>,
        params: &'a Rc<CompilationParameters<CT, G, C>>,
    ) -> Self {
        Self { savepoint, params }
    }
}

impl<'a, 'b, CT: CellType, G: Gate, C: OperationCost<CT>> ProgramVersion
    for DummyProgramVersion<'a, 'b, CT, G, C>
{
    type CT = CT;
    type G = G;
    type C = C;

    fn branch(&mut self) -> impl ProgramVersion<CT = Self::CT, G = Self::G, C = Self::C> {
        if true {
            unimplemented!("cannot branch a dummy program state")
        }
        DummyProgramVersion {
            params: self.params,
            savepoint: self.savepoint,
        }
    }

    fn parameters(&self) -> &Rc<CompilationParameters<Self::CT, Self::G, Self::C>> {
        self.params
    }

    fn append(&mut self, instr: Operation<Self::CT>) {
        self.savepoint.append_instruction(instr);
    }

    fn state_mut(&mut self) -> &mut impl CellStates<Self::CT> {
        self.savepoint
    }

    fn state(&self) -> &impl CellStates<Self::CT> {
        self.savepoint
    }

    fn candidates(&self) -> &Candidates {
        self.savepoint.candidates()
    }

    fn output_ids(&self) -> &FxHashSet<Id> {
        self.savepoint.output_ids()
    }

    fn uses(&self) -> &Uses {
        self.savepoint.uses()
    }

    fn program(&self) -> &Program<Self::CT> {
        self.savepoint.program()
    }

    fn consider(self) {
        unimplemented!("cannot consider a dummy program state")
    }
}
