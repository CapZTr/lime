#![allow(clippy::result_unit_err)]

mod boolhint;
mod boolset;
mod cell;
mod func;
mod instruction;
mod operand;
mod outputs;
mod patterns;
mod range;
pub mod set;

use std::fmt::{Display, Formatter};

use itertools::Itertools;

pub use self::{
    boolhint::BoolHint, boolset::BoolSet, cell::*, func::*, instruction::*, operand::*, outputs::*,
    patterns::*, range::*,
};

/// Abstractly describes a Logic-in-Memory architecture.
#[derive(Clone)]
pub struct Architecture<CT> {
    instructions: InstructionTypes<CT>,
    types: Vec<CT>,
}

impl<CT: CellType> Architecture<CT> {
    pub fn new(instructions: InstructionTypes<CT>) -> Self {
        let mut types = instructions.cell_types().collect::<Vec<_>>();
        types.sort();
        types.dedup();
        Self {
            instructions,
            types,
        }
    }
}

impl<CT> Architecture<CT> {
    pub fn instructions(&self) -> &InstructionTypes<CT> {
        &self.instructions
    }
    pub fn types(&self) -> &[CT] {
        &self.types
    }
}

fn display_maybe_inverted(f: &mut Formatter<'_>, inverted: bool) -> std::fmt::Result {
    if inverted { write!(f, "!") } else { Ok(()) }
}

fn display_index<D: Display>(f: &mut Formatter<'_>, idx: D) -> std::fmt::Result {
    write!(f, "[{idx}]")
}

fn check_no_duplicate_cells<'a, CT: CellType>(
    ops: impl IntoIterator<Item = &'a Cell<CT>>,
) -> Result<(), ()> {
    if ops.into_iter().duplicates().next().is_some() {
        Err(())
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::borrow::Cow;

    use super::*;

    #[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
    pub enum DummyCellType {
        Constant,
        A,
        B,
    }

    impl CellType for DummyCellType {
        const CONSTANT: Self = Self::Constant;

        fn count(self) -> Option<CellIndex> {
            match self {
                Self::Constant => Some(2),
                Self::A => Some(4),
                Self::B => None,
            }
        }

        fn name(self) -> Cow<'static, str> {
            match self {
                Self::Constant => "bool",
                Self::A => "A",
                Self::B => "B",
            }
            .into()
        }
    }
}
