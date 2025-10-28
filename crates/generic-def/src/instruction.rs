use std::{
    borrow::Cow,
    fmt::{self, Display},
    hash::{Hash, Hasher},
    iter::once,
    sync::Arc,
};

use derive_more::Deref;
use itertools::{Either, Itertools};
use rustc_hash::FxHashMap;

use crate::{
    Cell, CellPat, CellType, Function, Gate, Operand, Outputs, TuplesDef, check_no_duplicate_cells,
    range::Range, set::Set,
};

#[derive(Debug, Clone, Deref)]
#[deref(forward)]
pub struct InstructionTypes<CT>(Arc<[InstructionType<CT>]>);

impl<CT> InstructionTypes<CT> {
    pub fn new(mut types: Vec<InstructionType<CT>>) -> Self {
        types.sort_by_key(|typ| typ.id);
        types
            .iter()
            .enumerate()
            .for_each(|(i, instr)| assert_eq!(instr.id, i as u8));
        Self(types.into())
    }
    pub fn cell_types(&self) -> impl Iterator<Item = CT>
    where
        CT: CellType,
    {
        self.0.iter().flat_map(|typ| typ.cell_types())
    }
    pub fn gates(&self) -> impl Iterator<Item = Gate> {
        self.0
            .iter()
            .filter(|typ| typ.arity() != Some(1))
            .map(|typ| typ.function.gate)
    }
    pub fn by_id(&self, id: u8) -> &InstructionType<CT> {
        &self.0[id as usize]
    }
}

#[derive(Debug, Clone)]
pub struct InstructionType<CT> {
    pub id: u8,
    pub name: Cow<'static, str>,

    pub input: TuplesDef<CellPat<CT>>,
    pub input_override: InputIndices,
    pub input_inverted: InputIndices,
    pub input_range: Range,

    pub function: Function,
    pub outputs: Outputs<CT>,
}

impl<CT> InstructionType<CT> {
    pub fn cell_types(&self) -> impl Iterator<Item = CT>
    where
        CT: CellType,
    {
        self.input.cell_types().chain(self.outputs.cell_types())
    }

    pub fn arity(&self) -> Option<usize> {
        self.input
            .arity()
            .map(|arity| self.input_range.num_elements_in(arity))
    }
}

impl<CT> PartialEq for InstructionType<CT> {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
    }
}

impl<CT> Eq for InstructionType<CT> {}

impl<CT> PartialOrd for InstructionType<CT> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl<CT> Ord for InstructionType<CT> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.name.cmp(&other.name)
    }
}

impl<CT> Hash for InstructionType<CT> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.name.hash(state);
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Instruction<CT, TypCT = CT> {
    pub typ: InstructionType<TypCT>,
    pub inputs: Vec<Cell<CT>>,
    pub outputs: Vec<Operand<CT>>,
}

impl<CT: CellType> Instruction<CT, CT> {
    pub fn validate(&self) -> Result<(), ()> {
        check_no_duplicate_cells(&self.inputs)?;
        check_no_duplicate_cells(self.outputs.iter().map(|op| &op.cell))?;
        if !self.typ.input.matches(&self.inputs) {
            return Err(());
        }
        if !self
            .typ
            .outputs
            .iter()
            .any(|ops| ops.matches(&self.outputs))
        {
            return Err(());
        }
        Ok(())
    }

    // returns (cell, inverted) pairs for all output cells, normalized to cell `false` for constants
    pub fn write_cell_inverted_map(&self) -> FxHashMap<Cell<CT>, bool> {
        // write_operands returns input operands first, so output operands override input operands
        // as expected
        FxHashMap::from_iter(self.write_operands().map(|op| {
            if op.cell == CT::constant(true) {
                (CT::constant(false), !op.inverted)
            } else {
                (op.cell, op.inverted)
            }
        }))
    }

    pub fn overridden_input_operands(&self) -> impl Iterator<Item = Operand<CT>> {
        match self.typ.input_override {
            InputIndices::All => Either::Left(self.inputs.iter().enumerate()),
            InputIndices::None => Either::Left([].iter().enumerate()),
            InputIndices::Index(idx) => Either::Right(once((idx, &self.inputs[idx]))),
        }
        .map(|(i, &cell)| Operand {
            cell,
            inverted: self.typ.input_inverted.contains(&i),
        })
    }

    pub fn write_operands(&self) -> impl Iterator<Item = Operand<CT>> {
        self.overridden_input_operands()
            .chain(self.outputs.iter().copied())
    }

    pub fn write_cells(&self) -> impl Iterator<Item = Cell<CT>> {
        self.write_operands().map(|op| op.cell)
    }

    pub fn read_cells(&self) -> impl Iterator<Item = Cell<CT>> {
        self.typ.input_range.slice(&self.inputs).1.iter().copied()
    }
}

impl<CT, TypCT> Display for Instruction<CT, TypCT>
where
    CT: CellType,
    TypCT: CellType,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}({})", self.typ.name, self.inputs.iter().format(", "),)?;
        if !self.outputs.is_empty() {
            write!(f, " -> ({})", self.outputs.iter().format(", "))?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum InputIndices {
    All,
    None,
    Index(usize),
}

impl Set<usize> for InputIndices {
    fn contains(&self, e: &usize) -> bool {
        match *self {
            Self::None => false,
            Self::All => true,
            Self::Index(i) => *e == i,
        }
    }
}
