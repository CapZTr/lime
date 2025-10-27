use std::borrow::Cow;

use derive_more::From;
use lime_generic_def::{CellIndex, CellType};

#[derive(Debug, Copy, Clone, Hash, PartialEq, Eq, PartialOrd, Ord, From)]
pub enum CellOrVar<CT> {
    Var,
    Cell(#[from] CT),
}

impl<CT: CellType> CellType for CellOrVar<CT> {
    const CONSTANT: Self = Self::Cell(CT::CONSTANT);

    fn count(self) -> Option<CellIndex> {
        match self {
            Self::Var => None,
            Self::Cell(typ) => typ.count(),
        }
    }

    fn name(self) -> Cow<'static, str> {
        match self {
            Self::Var => "Var".into(),
            Self::Cell(typ) => typ.name(),
        }
    }
}
