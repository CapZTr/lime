use crate::{BoolHint, func::EvaluationMethods};

#[derive(Debug, Copy, Clone)]
pub struct XorEval {
    num: u8,
    val: Option<bool>,
}

impl Default for XorEval {
    fn default() -> Self {
        Self {
            num: 0,
            val: Some(false),
        }
    }
}

impl EvaluationMethods for XorEval {
    fn hint(&self, arity: usize, target: bool) -> Option<BoolHint> {
        let val = self.val?;
        if (self.num + 1) as usize == arity {
            Some(BoolHint::Require(val ^ target))
        } else {
            Some(BoolHint::Any)
        }
    }

    fn hint_id(&self, arity: usize, inverted: bool) -> Option<BoolHint> {
        let val = self.val?;
        if arity as u8 == self.num + 1 {
            if val == inverted {
                Some(BoolHint::Any)
            } else {
                None
            }
        } else if arity as u8 == self.num + 2 {
            Some(BoolHint::Require(val ^ inverted))
        } else {
            Some(BoolHint::Any)
        }
    }

    fn id_inverted(&self) -> Option<bool> {
        self.val
    }

    fn add(&mut self, value: bool) {
        match &mut self.val {
            None => {}
            Some(val) => *val ^= value,
        }
        self.num += 1
    }

    fn add_unknown(&mut self) {
        self.val = None;
        self.num += 1
    }

    fn count(&self) -> usize {
        self.num as usize
    }

    fn evaluate(&self) -> Option<bool> {
        self.val
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_xor_eval() {
        let mut eval = XorEval::default();

        assert_eq!(eval.hint(1, true), Some(BoolHint::Require(true)));
        assert_eq!(eval.hint(1, false), Some(BoolHint::Require(false)));
        assert_eq!(eval.hint(2, true), Some(BoolHint::Any));
        assert_eq!(eval.hint(2, false), Some(BoolHint::Any));

        assert_eq!(eval.hint_id(1, true), None);
        assert_eq!(eval.hint_id(1, false), Some(BoolHint::Any));
        assert_eq!(eval.hint_id(2, true), Some(BoolHint::Require(true)));
        assert_eq!(eval.hint_id(2, false), Some(BoolHint::Require(false)));

        eval.add(true);

        assert_eq!(eval.hint(2, true), Some(BoolHint::Require(false)));
        assert_eq!(eval.hint(2, false), Some(BoolHint::Require(true)));
        assert_eq!(eval.hint(3, true), Some(BoolHint::Any));
        assert_eq!(eval.hint(3, false), Some(BoolHint::Any));

        assert_eq!(eval.hint_id(2, true), Some(BoolHint::Any));
        assert_eq!(eval.hint_id(2, false), None);
        assert_eq!(eval.hint_id(3, true), Some(BoolHint::Require(false)));
        assert_eq!(eval.hint_id(3, false), Some(BoolHint::Require(true)));

        let mut eval = XorEval::default();
        eval.add(false);

        assert_eq!(eval.hint(2, true), Some(BoolHint::Require(true)));
        assert_eq!(eval.hint(2, false), Some(BoolHint::Require(false)));
        assert_eq!(eval.hint(3, true), Some(BoolHint::Any));
        assert_eq!(eval.hint(3, false), Some(BoolHint::Any));

        assert_eq!(eval.hint_id(2, true), None);
        assert_eq!(eval.hint_id(2, false), Some(BoolHint::Any));
        assert_eq!(eval.hint_id(3, true), Some(BoolHint::Require(true)));
        assert_eq!(eval.hint_id(3, false), Some(BoolHint::Require(false)));
    }
}
