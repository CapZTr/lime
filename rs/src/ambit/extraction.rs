use super::Architecture;
use crate::ambit::compilation::compile;
use crate::opt_extractor::{Choices, OptCostFunction};
use eggmock::egg::{Analysis, EClass, Language};
use eggmock::{EggExt, MigLanguage, NetworkLanguage, NetworkReceiver, Signal};
use std::cmp::Ordering;

pub struct CompilingCostFunction<'a> {
    pub architecture: &'a Architecture,
}

#[derive(Debug, Clone, Default, Eq, PartialEq)]
pub enum NotNesting {
    #[default]
    NotANot,
    FirstNot,
    NestedNots,
}

#[derive(Debug, Clone)]
pub struct CompilingCost {
    not_nesting: NotNesting,
    program_cost: usize,
}

impl<A: Analysis<MigLanguage>> OptCostFunction<MigLanguage, A> for CompilingCostFunction<'_> {
    type Cost = CompilingCost;

    fn cost(
        &mut self,
        eclass: &EClass<MigLanguage, A::Data>,
        enode: &MigLanguage,
        choices: &Choices<Self, MigLanguage, A>,
    ) -> Option<Self::Cost> {
        if enode.children().contains(&eclass.id) {
            return None;
        }

        let not_nesting = if let MigLanguage::Not(id) = enode {
            let prev_cost = &choices.find_best(*id).expect("should be present").0;
            if prev_cost.not_nesting == NotNesting::NotANot {
                NotNesting::FirstNot
            } else {
                NotNesting::NestedNots
            }
        } else {
            NotNesting::NotANot
        };

        let mut ntk = choices.send(NetworkReceiver::default(), enode.children().iter().copied())?;
        let output = match enode.to_node(|_, idx| ntk.outputs()[idx]) {
            Some(node) => Signal::new(ntk.add(node), false),
            None => !ntk.outputs()[0], // NOT
        };
        ntk.set_outputs(vec![output]);

        let program = compile(self.architecture, &ntk).ok()?;
        Some(CompilingCost {
            not_nesting,
            program_cost: program.instructions.len(),
        })
    }
}

impl PartialEq for CompilingCost {
    fn eq(&self, other: &Self) -> bool {
        if other.not_nesting == NotNesting::NestedNots && self.not_nesting == NotNesting::NestedNots
        {
            true
        } else {
            self.program_cost.eq(&other.program_cost)
        }
    }
}

impl PartialOrd for CompilingCost {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        #[allow(clippy::collapsible_else_if)]
        if self.not_nesting == NotNesting::NestedNots {
            if other.not_nesting == NotNesting::NestedNots {
                Some(Ordering::Equal)
            } else {
                Some(Ordering::Greater)
            }
        } else {
            if other.not_nesting == NotNesting::NestedNots {
                Some(Ordering::Less)
            } else {
                self.program_cost.partial_cmp(&other.program_cost)
            }
        }
    }
}
