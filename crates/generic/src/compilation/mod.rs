mod candidate_selection;
pub mod optimization;
pub mod step;

use std::rc::Rc;

use eggmock::{Gate, Network};
use itertools::Itertools;
use lime_generic_def::{Cell, CellPat, CellType, InputIndices, NaryPat, Pats};
use rustc_hash::FxHashSet;

use crate::{
    ArchitectureMeta,
    compilation::{
        candidate_selection::{AllCandidates, MIGBasedCompilerCandidateSelection},
        optimization::optimize_outputs,
        step::{DefaultStepFn, place_signals},
    },
    cost::{Cost, OperationCost},
    program::{
        DummyProgramVersion, ProgramVersion,
        collection::DeltaCollectionProgramVersion,
        state::{Program, State, StateDelta, StateSavepoint},
    },
};

#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub enum CompilationMode {
    Greedy,
    Exhaustive,
}

#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub enum CandidateSelection {
    All,
    MIGBasedCompiler,
}

pub struct CompilationParameters<CT: CellType, G, C: OperationCost<CT>> {
    pub arch: Rc<ArchitectureMeta<CT>>,
    pub network: Network<G>,
    pub input_cells: Vec<Cell<CT>>,
    pub cost: C,
    pub mode: CompilationMode,
    pub candidate_selection: CandidateSelection,
    pub disjunct_input_output: bool,
}

pub trait StepFn<CT: CellType, G: Gate, C: OperationCost<CT>> {
    fn step(
        &self,
        params: &CompilationParameters<CT, G, C>,
        version: impl ProgramVersion<CT = CT, G = G, C = C>,
    );
}

pub struct CompilationResult<CT> {
    pub program: Program<CT>,
    pub outputs: Vec<Cell<CT>>,
}

pub fn compile<
    CT: CellType,
    G: Gate,
    C: OperationCost<CT>,
    P: Into<Rc<CompilationParameters<CT, G, C>>>,
>(
    params: P,
) -> Option<CompilationResult<CT>> {
    let params = params.into();
    let result = match &(params.mode, &params.candidate_selection) {
        (CompilationMode::Exhaustive, CandidateSelection::All) => {
            exhaustive_search(&params, DefaultStepFn(AllCandidates))
        }
        (CompilationMode::Exhaustive, CandidateSelection::MIGBasedCompiler) => {
            exhaustive_search(&params, DefaultStepFn(MIGBasedCompilerCandidateSelection))
        }
        (CompilationMode::Greedy, CandidateSelection::All) => {
            greedy_search(&params, &DefaultStepFn(AllCandidates))
        }
        (CompilationMode::Greedy, CandidateSelection::MIGBasedCompiler) => {
            greedy_search(&params, &DefaultStepFn(MIGBasedCompilerCandidateSelection))
        }
    }?;
    if result.outputs.len() != params.network.outputs().len() {
        None
    } else {
        Some(result)
    }
}

fn greedy_search<CT: CellType, G: Gate, C: OperationCost<CT>>(
    params: &Rc<CompilationParameters<CT, G, C>>,
    step: &impl StepFn<CT, G, C>,
) -> Option<CompilationResult<CT>> {
    let mut state = State::initialize(params);
    loop {
        if state.candidates().is_empty() {
            let mut state = state.savepoint();
            return finalize(&mut state, params);
        } else {
            let mut deltas = Vec::new();
            step.step(
                params,
                DeltaCollectionProgramVersion::new(state.savepoint(), params, &mut deltas),
            );
            let delta = deltas
                .into_iter()
                .min_by_key(|delta| params.cost.program_cost(delta.program_delta()))?;
            let mut state_sp = state.savepoint();
            state_sp.replay(delta);
            state_sp.retain();
        }
    }
}

fn exhaustive_search<CT: CellType, G: Gate, C: OperationCost<CT>>(
    params: &Rc<CompilationParameters<CT, G, C>>,
    strategy: impl StepFn<CT, G, C>,
) -> Option<CompilationResult<CT>> {
    let mut state = State::initialize(params);
    let mut result = None;
    exhaustive_search_recurse(
        params,
        &mut result,
        state.savepoint(),
        vec![Default::default()],
        &strategy,
    );
    result.map(|(_, result)| result)
}

fn exhaustive_search_recurse<CT: CellType, G: Gate, C: OperationCost<CT>>(
    params: &Rc<CompilationParameters<CT, G, C>>,
    best: &mut Option<(Cost, CompilationResult<CT>)>,
    mut state: StateSavepoint<CT, G>,
    deltas: Vec<StateDelta<CT>>,
    step: &impl StepFn<CT, G, C>,
) {
    if state.candidates().is_empty() {
        let result = finalize(&mut state, params).expect("output placement should be possible");
        let cost = params.cost.program_cost(&result.program);
        if best
            .as_ref()
            .map(|(prev_cost, best)| {
                cost < *prev_cost
                    || (cost == *prev_cost && best.program.num_cells() > result.program.num_cells())
            })
            .unwrap_or(true)
        {
            *best = Some((cost, result));
        }
    } else {
        for delta in deltas {
            let mut deltas = Vec::new();
            let mut state = state.savepoint();
            state.replay(delta);

            step.step(
                params,
                DeltaCollectionProgramVersion::new(state.savepoint(), params, &mut deltas),
            );

            exhaustive_search_recurse(params, best, state, deltas, step);
        }
    }
}

fn finalize<CT: CellType, G: Gate, C: OperationCost<CT>>(
    state: &mut StateSavepoint<CT, G>,
    params: &Rc<CompilationParameters<CT, G, C>>,
) -> Option<CompilationResult<CT>> {
    let mut version = DummyProgramVersion::new(state, params);
    let ops = NaryPat(Pats(
        params
            .arch
            .types()
            .iter()
            .filter(|typ| typ.count().is_none())
            .map(|typ| CellPat::Type(*typ))
            .collect_vec()
            .into(),
    ));
    let outputs = place_signals(
        &ops,
        InputIndices::None,
        params.network.outputs(),
        params,
        &mut version,
        &mut FxHashSet::default(),
    )?;
    let mut program = state.program().clone();
    optimize_outputs(&mut program);
    Some(CompilationResult { program, outputs })
}
