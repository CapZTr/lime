use std::sync::Arc;

use derive_more::Deref;

use crate::{BoolSet, Cell, CellType, OperandPat, TuplePat, TuplePats, TuplesDef};

#[derive(Debug, Deref, Clone)]
#[deref(forward)]
pub struct Outputs<CT>(Arc<[TuplesDef<OperandPat<CT>>]>);

impl<CT> Outputs<CT> {
    pub fn cell_types(&self) -> impl Iterator<Item = CT>
    where
        CT: CellType,
    {
        self.0.iter().flat_map(|operands| operands.cell_types())
    }
    pub fn contains_none(&self) -> bool {
        self.0.is_empty() || self.iter().any(|operands| operands.arity() == Some(0))
    }
    pub fn new(vec: Vec<TuplesDef<OperandPat<CT>>>) -> Self {
        if vec.is_empty() {
            Self(
                vec![TuplesDef::Tuples(TuplePats::new(vec![TuplePat::new(
                    vec![],
                )]))]
                .into(),
            )
        } else {
            Self(vec.into())
        }
    }
}

impl<CT: CellType> Outputs<CT> {
    /// See: [Operands::fit_cell]
    pub fn fit_cell(&self, cell: Cell<CT>) -> BoolSet {
        self.iter().map(|ops| ops.fit_cell(cell)).collect()
    }
    /// See: [Operands::single_operands]
    pub fn length_one_patterns(&self) -> impl Iterator<Item = OperandPat<CT>> {
        self.iter().flat_map(|ops| ops.length_one_patterns())
    }
}
