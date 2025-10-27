use std::{
    fmt::{Debug, Display, Formatter},
    slice,
};

use eggmock::Id;
use lime_generic_def::{Cell, CellType, Instruction};
use rustc_hash::FxHashSet;

#[derive(Debug, Clone)]
pub enum Operation<CT> {
    Candidate(Instruction<CT>, Id),
    Other {
        instructions: Vec<Instruction<CT>>,
        comment: Option<String>,
    },
    Copy {
        from: Cell<CT>,
        to: Cell<CT>,
        inverted: bool,
        instructions: Vec<Instruction<CT>>,
        spill: bool,
        computes_from_inverted: bool,
    },
}

impl<CT> Operation<CT> {
    pub fn instructions(&self) -> &[Instruction<CT>] {
        match self {
            Self::Candidate(instr, _) => slice::from_ref(instr),
            Self::Copy { instructions, .. } => instructions,
            Self::Other { instructions, .. } => instructions,
        }
    }
    pub fn instructions_mut(&mut self) -> &mut [Instruction<CT>] {
        match self {
            Self::Candidate(instr, _) => slice::from_mut(instr),
            Self::Copy { instructions, .. } => instructions,
            Self::Other { instructions, .. } => instructions,
        }
    }
    pub fn comment(&self) -> Option<String>
    where
        CT: CellType,
    {
        match self {
            Self::Candidate(_, id) => Some(format!("compute candidate {id:?}")),
            Self::Copy {
                from,
                to,
                inverted,
                spill,
                ..
            } => {
                let inv = if *inverted { "!" } else { "" };
                let copy = if *spill { "spill" } else { "copy" };
                Some(format!("{copy} {from} {inv}-> {to}"))
            }
            Self::Other { comment, .. } => comment.clone(),
        }
    }
}

impl<CT: CellType> Display for Operation<CT> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        if let Some(comment) = self.comment() {
            writeln!(f, "// {comment}")?;
        }
        for instr in self.instructions() {
            writeln!(f, "{instr}")?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct Program<CT>(pub Vec<Operation<CT>>);

impl<CT> Default for Program<CT> {
    fn default() -> Self {
        Self(Default::default())
    }
}

impl<CT> Program<CT> {
    pub fn savepoint(&mut self) -> ProgramSavepoint<'_, CT> {
        ProgramSavepoint {
            previous_len: self.0.len(),
            program: self,
        }
    }
    pub fn instructions(&self) -> impl Iterator<Item = &Instruction<CT>> {
        self.0.iter().flat_map(Operation::instructions)
    }

    pub fn num_cells(&self) -> usize
    where
        CT: CellType,
    {
        self.instructions()
            .flat_map(|instr| {
                let input_cells = instr.inputs.iter().copied();
                let output_cells = instr.outputs.iter().map(|op| op.cell);
                input_cells.chain(output_cells)
            })
            .collect::<FxHashSet<_>>()
            .len()
    }
}

impl<CT: CellType> Display for Program<CT> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        for op in &self.0 {
            writeln!(f, "{op}")?;
        }
        Ok(())
    }
}

#[derive(Debug)]
pub struct ProgramSavepoint<'a, CT> {
    program: &'a mut Program<CT>,
    previous_len: usize,
}

impl<'a, CT> ProgramSavepoint<'a, CT> {
    pub fn savepoint(&mut self) -> ProgramSavepoint<'_, CT> {
        ProgramSavepoint {
            previous_len: self.program.0.len(),
            program: self.program,
        }
    }
    pub fn program(&self) -> &Program<CT> {
        self.program
    }
    pub fn append(&mut self, instr: Operation<CT>)
    where
        CT: CellType,
    {
        self.program.0.push(instr);
    }
    pub fn append_to_delta(&self, delta: &mut ProgramDelta<CT>)
    where
        CT: Clone,
    {
        delta
            .0
            .0
            .extend(self.program.0[self.previous_len..].iter().cloned());
    }
    pub fn replay(&mut self, delta: ProgramDelta<CT>) {
        self.program.0.extend(delta.0.0);
    }
    pub fn retain(mut self) {
        self.previous_len = self.program.0.len()
    }
}

impl<'a, CT> Drop for ProgramSavepoint<'a, CT> {
    fn drop(&mut self) {
        self.program.0.truncate(self.previous_len);
    }
}

#[derive(Clone)]
pub struct ProgramDelta<CT>(Program<CT>);

impl<CT: CellType> Debug for ProgramDelta<CT> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("ProgramDelta")
            .field(&format!("{}", self.0))
            .finish()
    }
}

impl<CT> ProgramDelta<CT> {
    pub fn as_program(&self) -> &Program<CT> {
        &self.0
    }
}

impl<CT> Default for ProgramDelta<CT> {
    fn default() -> Self {
        Self(Default::default())
    }
}
