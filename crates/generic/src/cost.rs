use lime_generic_def::Instruction;
use ordered_float::OrderedFloat;

use crate::{copy::placeholder::CellOrVar, program::state::Program};

pub type Cost = OrderedFloat<f64>;

pub trait OperationCost<CT>: Clone {
    fn cost<I: Into<CellOrVar<CT>>>(&self, instruction: &Instruction<I, CT>) -> Cost;
    fn program_cost<'a>(&self, program: &Program<CT>) -> Cost
    where
        CT: 'a,
    {
        program
            .instructions()
            .map(|op| self.cost(op))
            .fold(Default::default(), |a, b| a + b)
    }
}

#[derive(Clone)]
pub struct EqualCosts;

impl<CT> OperationCost<CT> for EqualCosts {
    fn cost<I: Into<CellOrVar<CT>>>(&self, _instruction: &Instruction<I, CT>) -> Cost {
        OrderedFloat(1.0)
    }
}
