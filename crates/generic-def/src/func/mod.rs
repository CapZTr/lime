mod and;
mod constant;
mod maj;
mod xor;

use std::fmt::Display;

use delegate::delegate;
use eggmock::GateFunction;
use strum::EnumString;

use crate::{
    BoolHint, display_maybe_inverted,
    func::{and::AndEval, constant::ConstEval, maj::MajEval, xor::XorEval},
};

// Gate type without input/output inverters
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, EnumString)]
#[strum(ascii_case_insensitive)]
pub enum Gate {
    And,
    Maj,
    Xor,
    #[strum(disabled)]
    Constant(bool),
}

impl Gate {
    pub fn evaluate(&self) -> GateEvaluation {
        match self {
            Self::And => GateEvaluation::And(AndEval::new()),
            Self::Maj => GateEvaluation::Maj(MajEval::new()),
            Self::Xor => GateEvaluation::Xor(XorEval::default()),
            Self::Constant(c) => GateEvaluation::Const(ConstEval::new(*c)),
        }
    }

    pub fn gate_function(&self) -> Option<GateFunction> {
        match self {
            Self::And => Some(GateFunction::And),
            Self::Maj => Some(GateFunction::Maj),
            Self::Xor => Some(GateFunction::Xor),
            _ => None,
        }
    }
}

impl Display for Gate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::And => write!(f, "and"),
            Self::Maj => write!(f, "maj"),
            Self::Xor => write!(f, "xor"),
            Self::Constant(c) => write!(f, "{c:?}"),
        }
    }
}

/// Gate with an optional output inverter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Function {
    pub inverted: bool,
    pub gate: Gate,
}

impl Function {
    pub fn evaluate(self, arity: usize) -> FunctionEvaluation {
        FunctionEvaluation {
            inverted: self.inverted,
            gate: self.gate.evaluate(),
            arity,
        }
    }
}

impl Display for Function {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        display_maybe_inverted(f, self.inverted)?;
        write!(f, "{}", self.gate)
    }
}

trait EvaluationMethods {
    fn hint(&self, arity: usize, target: bool) -> Option<BoolHint>;
    fn hint_id(&self, arity: usize, inverted: bool) -> Option<BoolHint>;
    fn id_inverted(&self) -> Option<bool>;
    fn add(&mut self, value: bool);
    fn add_unknown(&mut self);
    fn count(&self) -> usize;
    fn evaluate(&self) -> Option<bool>;
}

#[derive(Debug, Clone, Copy)]
pub struct FunctionEvaluation {
    inverted: bool,
    gate: GateEvaluation,
    arity: usize,
}

impl FunctionEvaluation {
    pub fn hint(&self, target: bool) -> Option<BoolHint> {
        if self.count() == self.arity {
            if self.evaluate() == Some(target) {
                Some(BoolHint::Any)
            } else {
                None
            }
        } else {
            self.gate.hint(self.arity, target ^ self.inverted)
        }
    }
    pub fn hint_id(&self, inverted: bool) -> Option<BoolHint> {
        if self.count() == self.arity {
            if let Some(id_inverted) = self.id_inverted()
                && id_inverted == inverted
            {
                Some(BoolHint::Any)
            } else {
                None
            }
        } else {
            self.gate.hint_id(self.arity, inverted ^ self.inverted)
        }
    }
    pub fn id_inverted(&self) -> Option<bool> {
        self.gate
            .id_inverted()
            .map(|inverted| inverted ^ self.inverted)
    }
    pub fn add(&mut self, value: bool) {
        assert!(self.count() < self.arity);
        self.gate.add(value);
    }
    pub fn add_unknown(&mut self) {
        assert!(self.count() < self.arity);
        self.gate.add_unknown();
    }
    pub fn evaluate(&self) -> Option<bool> {
        self.gate.evaluate().map(|v| v ^ self.inverted)
    }
    pub fn count(&self) -> usize {
        self.gate.count()
    }
}

#[derive(Debug, Clone, Copy)]
pub enum GateEvaluation {
    And(AndEval),
    Maj(MajEval),
    Xor(XorEval),
    Const(ConstEval),
}

impl GateEvaluation {
    delegate! {
        to match self {
            Self::And(and) => and,
            Self::Maj(maj) => maj,
            Self::Xor(xor) => xor,
            Self::Const(c) => c,
        } {
            pub fn hint(&self, arity: usize, target: bool) -> Option<BoolHint>;
            pub fn hint_id(&self, arity: usize, inverted: bool) -> Option<BoolHint>;
            pub fn id_inverted(&self) -> Option<bool>;
            pub fn add(&mut self, value: bool);
            pub fn add_unknown(&mut self);
            pub fn evaluate(&self) -> Option<bool>;
            pub fn count(&self) -> usize;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    pub fn evaluate_maj() {
        for (values, result) in [
            (&[true] as &[bool], true),
            (&[true, false, true], true),
            (&[true, false, false], false),
            (&[true, false, true, false, false], false),
        ] {
            let mut eval = Function {
                gate: Gate::Maj,
                inverted: false,
            }
            .evaluate(values.len());
            values.iter().for_each(|value| eval.add(*value));
            assert_eq!(eval.evaluate(), Some(result), "invalid result")
        }
    }

    #[test]
    pub fn evaluate_and() {
        for (values, result) in [
            (&[true] as &[bool], true),
            (&[false], false),
            (&[true, false], false),
            (&[true, true, true], true),
        ] {
            let mut eval = Function {
                gate: Gate::And,
                inverted: false,
            }
            .evaluate(values.len());
            values.iter().for_each(|value| eval.add(*value));
            assert_eq!(eval.evaluate(), Some(result), "invalid result")
        }
    }

    #[test]
    pub fn evaluate_const() {
        for (values, c) in [
            (&[true] as &[bool], true),
            (&[false], false),
            (&[true, false], false),
            (&[true, true, true], true),
        ] {
            let mut eval = Function {
                gate: Gate::Constant(c),
                inverted: false,
            }
            .evaluate(values.len());
            values.iter().for_each(|value| eval.add(*value));
            assert_eq!(eval.evaluate(), Some(c), "invalid result")
        }
    }
}
