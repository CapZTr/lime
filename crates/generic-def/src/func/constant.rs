use super::EvaluationMethods;
use crate::BoolHint;

#[derive(Debug, Clone, Copy)]
pub struct ConstEval {
    count: u8,
    const_value: bool,
}

impl ConstEval {
    pub fn new(const_value: bool) -> Self {
        Self {
            const_value,
            count: 0,
        }
    }
}

impl EvaluationMethods for ConstEval {
    fn hint(&self, _arity: usize, target: bool) -> Option<BoolHint> {
        if self.const_value == target {
            Some(BoolHint::Any)
        } else {
            None
        }
    }

    fn hint_id(&self, _arity: usize, _inverted: bool) -> Option<BoolHint> {
        None
    }

    fn id_inverted(&self) -> Option<bool> {
        None
    }

    fn add(&mut self, _value: bool) {
        self.count += 1
    }

    fn add_unknown(&mut self) {
        self.count += 1
    }

    fn evaluate(&self) -> Option<bool> {
        Some(self.const_value)
    }

    fn count(&self) -> usize {
        0
    }
}
