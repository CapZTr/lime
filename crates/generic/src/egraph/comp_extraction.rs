use crate::compilation::{CandidateSelection, CompilationMode, CompilationParameters, compile};
use crate::cost::{Cost, OperationCost};
use crate::egraph::opt_extractor::{Choices, OptCostFunction};
use crate::{ArchitectureMeta, get_input_cells};

use eggmock::egg::{Analysis, EClass};
use eggmock::{EggExt, NetworkLanguage, NetworkReceiver, Signal};
use lime_generic_def::CellType;
use std::rc::Rc;

pub struct CompilingCostFunction<CT: CellType, C: OperationCost<CT>> {
    pub arch: Rc<ArchitectureMeta<CT>>,
    pub cost: C,
    pub mode: CompilationMode,
    pub candidate_selection: CandidateSelection,
    pub disjunct_input_output: bool,
    pub memusage: bool,
}

impl<L: NetworkLanguage, A: Analysis<L>, CT: CellType, C: OperationCost<CT>> OptCostFunction<L, A>
    for CompilingCostFunction<CT, C>
{
    type Cost = Cost;

    fn cost(
        &mut self,
        eclass: &EClass<L, A::Data>,
        enode: &L,
        choices: &Choices<Self, L, A>,
    ) -> Option<Self::Cost> {
        if enode.children().contains(&eclass.id) {
            return None;
        }

        let mut ntk = choices.send(NetworkReceiver::default(), enode.children().iter().copied())?;
        let output = match enode.to_node(|_, idx| ntk.outputs()[idx]) {
            Some(node) => Signal::new(ntk.add(node), false),
            None => !ntk.outputs()[0], // NOT
        };
        ntk.set_outputs(vec![output]);

        let result = compile(CompilationParameters {
            arch: self.arch.clone(),
            input_cells: get_input_cells(&self.arch, &ntk),
            network: ntk,
            cost: self.cost.clone(),
            mode: self.mode,
            candidate_selection: self.candidate_selection,
            disjunct_input_output: self.disjunct_input_output,
        })?;
        Some(if self.memusage {
            (result.program.num_cells() as u32).into()
        } else {
            self.cost.program_cost(&result.program)
        })
    }
}
