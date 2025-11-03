mod candidates;
mod cells;
mod free;
mod program;
mod uses;

use derive_where::derive_where;
use eggmock::{Gate, Id, Network, Node, Signal};
use itertools::Itertools;
use lime_generic_def::{Cell, CellType};
use rustc_hash::FxHashSet;

use crate::{
    compilation::CompilationParameters, cost::OperationCost, program::state::free::FreeCells,
};

pub use self::{candidates::*, cells::*, program::*, uses::*};

#[derive_where::derive_where(Debug)]
pub struct State<'a, CT: CellType, G> {
    program: Program<CT>,
    cells: CellStatesStore<CT>,
    candidates: Candidates,
    uses: Uses,
    #[derive_where(skip)]
    network: &'a Network<G>,
    #[derive_where(skip)]
    output_ids: FxHashSet<Id>,
}

impl<'a, CT: CellType, G: Gate> State<'a, CT, G> {
    pub fn initialize<C: OperationCost<CT>>(params: &'a CompilationParameters<CT, G, C>) -> Self {
        let mut cells = CellStatesStore::new(&params.arch);
        let mut candidates = Candidates::default();
        let network = &params.network;
        let output_ids = network.outputs().iter().map(|sig| sig.node_id()).collect();
        for &leaf_id in network.leaves() {
            let leaf = network.node(leaf_id);
            match leaf {
                Node::False => {
                    for const_value in [true, false] {
                        cells.set(CT::constant(const_value), Signal::new(leaf_id, const_value));
                    }
                }
                Node::Input(i) => {
                    cells.set(params.input_cells[*i as usize], Signal::new(leaf_id, false));
                }
                Node::Gate(_) => unreachable!("gate cannot be a leaf"),
            }
            for fanout in params.network.node_outputs(leaf_id) {
                if params
                    .network
                    .node(fanout.node_id())
                    .inputs()
                    .iter()
                    .all(|signal| params.network.node(signal.node_id()).is_leaf())
                {
                    candidates.insert(fanout.node_id());
                }
            }
        }
        Self {
            program: Default::default(),
            cells,
            candidates,
            uses: Uses::new(params.network.leaves().iter().copied()),
            network,
            output_ids,
        }
    }
    pub fn savepoint(&mut self) -> StateSavepoint<'_, CT, G> {
        StateSavepoint {
            program: self.program.savepoint(),
            cells: self.cells.savepoint(),
            candidates: self.candidates.savepoint(),
            uses: UsesSavepoint::new(&mut self.uses),
            network: self.network,
            output_ids: &self.output_ids,
        }
    }
    pub fn candidates(&self) -> &Candidates {
        &self.candidates
    }
    pub fn program(&self) -> &Program<CT> {
        &self.program
    }
}

#[derive_where::derive_where(Debug)]
pub struct StateSavepoint<'a, CT: CellType, G> {
    program: ProgramSavepoint<'a, CT>,
    cells: CellStatesSavepoint<'a, CT>,
    candidates: CandidatesSavepoint<'a>,
    uses: UsesSavepoint<'a>,
    #[derive_where(skip)]
    network: &'a Network<G>,
    #[derive_where(skip)]
    output_ids: &'a FxHashSet<Id>,
}

#[derive(Clone)]
#[derive_where(Debug; CT: CellType)]
pub struct StateDelta<CT> {
    program: ProgramDelta<CT>,
    cells: CellStatesDelta<CT>,
    candidates: CandidatesDelta,
    uses: UsesDelta,
}

impl<CT> StateDelta<CT> {
    pub fn program_delta(&self) -> &Program<CT> {
        self.program.as_program()
    }
}

impl<CT> Default for StateDelta<CT> {
    fn default() -> Self {
        Self {
            program: Default::default(),
            cells: Default::default(),
            candidates: Default::default(),
            uses: Default::default(),
        }
    }
}

impl<'a, CT: CellType, G: Gate> StateSavepoint<'a, CT, G> {
    pub fn savepoint(&mut self) -> StateSavepoint<'_, CT, G> {
        StateSavepoint {
            program: self.program.savepoint(),
            cells: self.cells.savepoint(),
            uses: self.uses.savepoint(),
            candidates: self.candidates.savepoint(),
            network: self.network,
            output_ids: self.output_ids,
        }
    }

    pub fn output_ids(&self) -> &FxHashSet<Id> {
        self.output_ids
    }

    pub fn append_instruction(&mut self, instr: Operation<CT>) {
        self.program.append(instr);
    }

    pub fn candidates(&self) -> &Candidates {
        self.candidates.candidates()
    }

    pub fn uses(&self) -> &Uses {
        self.uses.uses()
    }

    pub fn program(&self) -> &Program<CT> {
        self.program.program()
    }

    pub fn append_to_delta(&self, delta: &mut StateDelta<CT>) {
        self.program.append_to_delta(&mut delta.program);
        self.cells.append_to_delta(&mut delta.cells);
        self.candidates.append_to_delta(&mut delta.candidates);
        self.uses.append_to_delta(&mut delta.uses);
    }

    pub fn replay(&mut self, delta: StateDelta<CT>) {
        self.program.replay(delta.program);
        self.cells.replay(&delta.cells);
        self.candidates.replay(&delta.candidates);
        self.uses.replay(delta.uses);
    }

    pub fn retain(self) {
        self.program.retain();
        self.cells.retain();
        self.candidates.retain();
        self.uses.retain();
    }
}

impl<'a, CT: CellType, G: Gate> CellStates<CT> for StateSavepoint<'a, CT, G> {
    fn cell(&self, cell: Cell<CT>) -> Option<Signal> {
        self.cells.cell(cell)
    }

    fn cells_with(&self, signal: Signal) -> impl Iterator<Item = Cell<CT>> + '_ {
        self.cells.cells_with(signal)
    }

    fn set<Sig: Into<Option<Signal>>>(&mut self, cell: Cell<CT>, signal: Sig) -> Option<Signal> {
        let signal = signal.into();
        let previous = self.cells.set(cell, signal);
        if let Some(signal) = signal
            && self.candidates.remove(signal.node_id())
        {
            // we did just compute a new candidate, check if that gave us new candidates
            for fanout_id in self
                .network
                .node_outputs(signal.node_id())
                .iter()
                .map(Signal::node_id)
            {
                let fanout = self.network.node(fanout_id);
                if fanout
                    .inputs()
                    .iter()
                    .all(|fanin| self.cells.contains_id(fanin.node_id()))
                {
                    self.candidates.add(fanout_id);
                }
            }
            for id in self
                .network
                .node(signal.node_id())
                .inputs()
                .iter()
                .map(|signal| signal.node_id())
                .unique()
            {
                if self.uses.increment(id) >= self.network.node_output_ids(id).len()
                    && !self.output_ids.contains(&id)
                {
                    self.cells.clear_all_by_id(id);
                }
            }
        }
        previous
    }

    fn clear_all_by_id(&mut self, id: Id) {
        self.cells.clear_all_by_id(id);
    }

    fn free_cells(&self, typ: CT) -> &FreeCells {
        self.cells.free_cells(typ)
    }
}
