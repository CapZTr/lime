use std::fmt::{Debug, Display};

use crate::{BoolHint, Cell, CellIndex, CellPat, CellType, PatBase, display_maybe_inverted};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Operand<CT> {
    pub cell: Cell<CT>,
    pub inverted: bool,
}

impl<CT: CellType> Operand<CT> {
    pub fn map_cell_type<NewCT: CellType>(self, map: impl FnOnce(CT) -> NewCT) -> Operand<NewCT> {
        Operand {
            cell: Cell::new(map(self.cell.typ()), self.cell.index()),
            inverted: self.inverted,
        }
    }
}

impl<CT> Display for Operand<CT>
where
    CT: CellType,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        display_maybe_inverted(f, self.inverted)?;
        write!(f, "{}", self.cell)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct OperandPat<CT> {
    pub cell: CellPat<CT>,
    pub inverted: bool,
}

impl<CT: CellType> OperandPat<CT> {
    pub fn index(&self) -> Option<CellIndex> {
        self.cell.index()
    }

    pub fn typ(&self) -> CT {
        self.cell.cell_type()
    }

    /// Returns an operand that fits this type.
    pub fn any(&self) -> Operand<CT> {
        Operand {
            cell: self.cell.any(),
            inverted: self.inverted,
        }
    }

    /// Checks whether the given `cell` can be used for an operand of this type. If it can, returns
    /// whether the operand must reference the inverted cell value.
    pub fn fit(&self, cell: Cell<CT>) -> Option<bool> {
        if self.cell.matches(&cell) {
            Some(self.inverted)
        } else {
            None
        }
    }

    /// Attempts to find a constant value operand that fits this operand type and so that
    ///
    /// * if `required` is given, the operand has its value
    /// * else if `preferred` is given and this operand matches both constants, the operand has the
    ///   preferred value
    ///
    /// Returns the matching operand together with its value.
    pub fn try_fit_constant(&self, mut hint: BoolHint) -> Option<(bool, Operand<CT>)> {
        if self.cell.cell_type() != CT::CONSTANT {
            return None;
        }
        hint = hint.map(|v| v ^ self.inverted);
        match (self.cell.index(), hint) {
            (None, BoolHint::Require(v)) | (None, BoolHint::Prefer(v)) => Some(v),
            (None, BoolHint::Any) => Some(true),
            (Some(i), BoolHint::Require(required)) => {
                if required == (i != 0) {
                    Some(required)
                } else {
                    None
                }
            }
            (Some(i), _) => Some(i != 0),
        }
        .map(|value| {
            (
                value ^ self.inverted,
                Operand {
                    cell: CT::constant(value),
                    inverted: self.inverted,
                },
            )
        })
    }
}

impl<CT: CellType> PatBase for OperandPat<CT> {
    type CellType = CT;
    type Instance = Operand<CT>;

    fn cell_type(&self) -> Self::CellType {
        self.cell.cell_type()
    }

    fn cell_index(&self) -> Option<CellIndex> {
        self.cell.cell_index()
    }

    fn matches(&self, op: &Self::Instance) -> bool {
        op.inverted == self.inverted && self.cell.matches(&op.cell)
    }
}

impl<CT> Display for OperandPat<CT>
where
    CT: CellType,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        display_maybe_inverted(f, self.inverted)?;
        Display::fmt(&self.cell, f)
    }
}

#[cfg(test)]
mod tests {
    use itertools::Itertools;

    use crate::{CellIndex, tests::DummyCellType};

    use super::*;

    #[test]
    fn fit() {
        assert_eq!(
            OperandPat {
                cell: CellPat::Cell(Cell::new(DummyCellType::B, 10)),
                inverted: true,
            }
            .fit(Cell::new(DummyCellType::B, 10)),
            Some(true)
        );
        assert_eq!(
            OperandPat {
                cell: CellPat::Cell(Cell::new(DummyCellType::A, 1)),
                inverted: false,
            }
            .fit(Cell::new(DummyCellType::A, 1)),
            Some(false)
        );
        assert_eq!(
            OperandPat {
                cell: CellPat::Cell(Cell::new(DummyCellType::B, 0)),
                inverted: true,
            }
            .fit(Cell::new(DummyCellType::B, 1)),
            None
        );
        assert_eq!(
            OperandPat {
                cell: CellPat::Cell(Cell::new(DummyCellType::A, 0)),
                inverted: true,
            }
            .fit(Cell::new(DummyCellType::B, 0)),
            None
        );
    }

    #[test]
    pub fn try_fit_constant() {
        for inverted in [true, false] {
            for (cell_value, required, preferred) in [None, Some(true), Some(false)]
                .into_iter()
                .tuple_combinations()
            {
                let typ = OperandPat {
                    cell: match cell_value {
                        None => CellPat::Type(DummyCellType::Constant),
                        Some(value) => {
                            CellPat::Cell(Cell::new(DummyCellType::Constant, value as CellIndex))
                        }
                    },
                    inverted,
                };
                let hint = match (required, preferred) {
                    (Some(required), _) => BoolHint::Require(required),
                    (None, Some(preferred)) => BoolHint::Prefer(preferred),
                    (None, None) => BoolHint::Any,
                };
                let result = typ.try_fit_constant(hint);
                let possible = match (required, cell_value) {
                    (Some(required), Some(cell_value)) => required == cell_value ^ inverted,
                    _ => true,
                };
                if let Some((value, operand)) = result {
                    assert!(possible, "should not fit");
                    let operand_value =
                        operand.cell.constant_value().expect("should be a constant")
                            ^ operand.inverted;
                    assert_eq!(value, operand_value, "value and operand_value do not match");
                    assert!(
                        required.is_none_or(|required| value == required),
                        "value is not the required value"
                    );
                    assert!(
                        typ.fit(operand.cell)
                            .is_none_or(|inverted| inverted == operand.inverted),
                        "returned operand does not fit operand type"
                    );
                    if let (None, Some(preferred)) = (required, preferred) {
                        let can_be_preferred =
                            cell_value.is_none_or(|cell_value| preferred == cell_value ^ inverted);
                        assert_eq!(
                            can_be_preferred,
                            preferred == value,
                            "should be preferred value"
                        );
                    }
                } else {
                    assert!(!possible, "should fit");
                }
            }
        }
    }
}
