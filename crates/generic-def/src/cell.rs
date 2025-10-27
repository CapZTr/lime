use std::borrow::Cow;
use std::fmt::{self, Debug, Display, Formatter};
use std::hash::Hash;

use derive_more::From;
use itertools::Either;

use crate::{PatBase, display_index};

pub type CellIndex = u32;

pub trait CellType: 'static + Copy + Debug + PartialEq + Eq + Hash + PartialOrd + Ord {
    /// The type of the constant (pseudo-)cell. This cell type has 2 cells where the cell index
    /// is equivalent to the cell value (i.e. `0` is `false` and `1` is true)
    const CONSTANT: Self;

    /// Number of cells of this type or `None` if infinite amount is available.
    fn count(self) -> Option<CellIndex>;
    fn name(self) -> Cow<'static, str>;
    fn cell_iter(self) -> impl Iterator<Item = Cell<Self>> {
        match self.count() {
            Some(count) => Either::Left(0..count),
            None => Either::Right(0..),
        }
        .into_iter()
        .map(move |idx| Cell::new(self, idx))
    }
    fn constant(value: bool) -> Cell<Self> {
        Cell::new(Self::CONSTANT, value as CellIndex)
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Cell<CT>(CT, CellIndex);

impl<CT> Cell<CT> {
    pub fn new(typ: CT, idx: CellIndex) -> Self {
        Self(typ, idx)
    }

    pub fn index(self) -> CellIndex {
        self.1
    }

    pub fn typ(self) -> CT {
        self.0
    }

    /// Returns the value of this cell if it is a constant or `None` if it is not.
    pub fn constant_value(self) -> Option<bool>
    where
        CT: CellType,
    {
        if self.typ() == CT::CONSTANT {
            Some(self.index() != 0)
        } else {
            None
        }
    }

    pub fn map_cell_type<To>(self, map: impl FnOnce(CT) -> To) -> Cell<To> {
        Cell::new(map(self.0), self.1)
    }
}

impl<CT: CellType> Display for Cell<CT> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        if self.0 == CT::CONSTANT {
            write!(f, "{}", self.1 != 0)
        } else {
            write!(f, "{}", self.0.name())?;
            display_index(f, self.1)
        }
    }
}

#[doc(hidden)]
pub fn __display_cell_type<T: CellType>(typ: T, f: &mut Formatter<'_>) -> Result<(), fmt::Error> {
    write!(f, "{}", typ.name())
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash, PartialOrd, Ord, From)]
pub enum CellPat<CT> {
    #[from]
    Cell(Cell<CT>),
    #[from]
    Type(CT),
}

impl<CT: CellType> Display for CellPat<CT> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Cell(cell) => Display::fmt(cell, f),
            Self::Type(typ) => Display::fmt(&typ.name(), f),
        }
    }
}

impl<CT> CellPat<CT> {
    pub fn new_from_type_and_index(typ: CT, index: Option<CellIndex>) -> Self {
        match index {
            Some(idx) => Self::Cell(Cell::new(typ, idx)),
            None => Self::Type(typ),
        }
    }
    pub fn index(self) -> Option<CellIndex> {
        match self {
            CellPat::Type(_) => None,
            CellPat::Cell(cell) => Some(cell.index()),
        }
    }
    pub fn any(self) -> Cell<CT> {
        match self {
            CellPat::Type(typ) => Cell::new(typ, 0),
            CellPat::Cell(cell) => cell,
        }
    }
}

impl<CT: CellType> PatBase for CellPat<CT> {
    type CellType = CT;
    type Instance = Cell<CT>;

    fn cell_type(&self) -> Self::CellType {
        match self {
            CellPat::Type(typ) => *typ,
            CellPat::Cell(cell) => cell.typ(),
        }
    }

    fn cell_index(&self) -> Option<CellIndex> {
        self.index()
    }

    fn matches(&self, cell: &Self::Instance) -> bool {
        match *self {
            CellPat::Type(typ) => cell.typ() == typ,
            CellPat::Cell(self_cell) => self_cell == *cell,
        }
    }
}

impl<CT: CellType> CellPat<CT> {
    pub fn is_superset_of(self, pat: CellPat<CT>) -> bool {
        match (self, pat) {
            (CellPat::Type(super_typ), CellPat::Cell(sub_cell)) => sub_cell.typ() == super_typ,
            (CellPat::Cell(_), CellPat::Type(_)) => false,
            (CellPat::Type(super_typ), CellPat::Type(sub_typ)) => super_typ == sub_typ,
            (CellPat::Cell(super_cell), CellPat::Cell(sub_cell)) => super_cell == sub_cell,
        }
    }
    pub fn is_subset_of(self, pat: CellPat<CT>) -> bool {
        !self.is_superset_of(pat)
    }
    pub fn get_constant(self, value: bool) -> Option<Cell<CT>> {
        match self {
            CellPat::Type(typ) if typ == CT::CONSTANT => Some(CT::constant(value)),
            CellPat::Cell(cell) if cell.constant_value() == Some(value) => Some(cell),
            _ => None,
        }
    }
}
