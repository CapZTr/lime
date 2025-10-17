use std::{fmt::Display, ops::Index, sync::Arc};

use derive_more::{Deref, From};
use itertools::{Either, Itertools};

use crate::{BoolHint, BoolSet, Cell, CellIndex, CellType, Operand, OperandPat};

pub trait PatBase: Copy {
    type CellType;
    type Instance;

    fn cell_type(&self) -> Self::CellType;
    fn cell_index(&self) -> Option<CellIndex>;
    fn matches(&self, instance: &Self::Instance) -> bool;
}

#[derive(Deref, Debug, Clone)]
#[deref(forward)]
pub struct Pats<P>(pub Arc<[P]>);

impl<P> Pats<P> {
    pub fn new(pats: Vec<P>) -> Self {
        Self(pats.into())
    }

    /// Returns an iterator over all cell types that occur in this pattern.
    /// Will most likely contain duplicates.
    pub fn cell_types(&self) -> impl Iterator<Item = P::CellType>
    where
        P: PatBase,
    {
        self.0.iter().map(|typ| typ.cell_type())
    }

    /// Checks whether any of the contained patterns matches the given value.
    pub fn matches(&self, op: &P::Instance) -> bool
    where
        P: PatBase,
    {
        self.0.iter().any(|typ| typ.matches(op))
    }
}

impl<CT: CellType> Pats<OperandPat<CT>> {
    pub fn fit(&self, cell: Cell<CT>) -> BoolSet {
        self.iter().map(|op| op.fit(cell)).collect()
    }
    pub fn try_fit_constant(&self, hint: BoolHint) -> Option<(bool, Operand<CT>)> {
        self.iter()
            .filter_map(|op| op.try_fit_constant(hint))
            .next()
    }
}

impl<P: Display> Display for Pats<P> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0.iter().format(" | "))
    }
}

#[derive(Deref, From, Debug, Clone)]
#[deref(forward)]
pub struct TuplePat<P>(Vec<Pats<P>>);

impl<P> TuplePat<P> {
    pub fn new(operands: Vec<Pats<P>>) -> Self {
        Self(operands)
    }

    /// Adds all defined by this `TuplePat` combinations to the given vector.
    pub fn combinations(&self, combinations: &mut Vec<Vec<P>>)
    where
        P: PatBase,
    {
        fn combinations_helper<P: Clone>(
            tuple: &TuplePat<P>,
            combinations: &mut Vec<Vec<P>>,
            combination: &mut Vec<P>,
            pos: usize,
        ) {
            if pos == tuple.len() {
                combinations.push(combination.clone());
                return;
            }
            for pat in tuple[pos].iter() {
                combination.push(pat.clone());
                combinations_helper(tuple, combinations, combination, pos + 1);
                combination.pop();
            }
        }
        combinations_helper(self, combinations, &mut Vec::new(), 0);
    }

    pub fn as_slice(&self) -> &[Pats<P>] {
        self.0.as_slice()
    }

    /// Returns an iterator over all cell types that occur in this pattern.
    /// Will most likely contain duplicates.
    pub fn cell_types(&self) -> impl Iterator<Item = P::CellType>
    where
        P: PatBase,
    {
        self.0.iter().flat_map(|types| types.cell_types())
    }

    /// Returns whether the given tuple matches this tuple pattern.
    pub fn matches(&self, tuple: &[P::Instance]) -> bool
    where
        P: PatBase,
    {
        tuple.len() == self.0.len()
            && tuple
                .iter()
                .zip(self.0.iter())
                .all(|(instance, pattern)| pattern.matches(instance))
    }
}

impl<P: Display> Display for TuplePat<P> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "({})", self.0.iter().format(", "))
    }
}

#[derive(Debug, Clone, Deref)]
pub struct TuplePats<P> {
    arity: usize,
    #[deref(forward)]
    tuples: Arc<[TuplePat<P>]>,
}

impl<P> TuplePats<P> {
    pub fn new(tuples: Vec<TuplePat<P>>) -> Self {
        let mut iter = tuples.iter();
        let arity = iter
            .next()
            .expect("at least one tuple has to be present")
            .len();
        for tuple in iter {
            assert_eq!(tuple.len(), arity, "tuple lengths do not match");
        }
        Self {
            arity,
            tuples: tuples.into(),
        }
    }

    pub fn arity(&self) -> usize {
        self.arity
    }

    /// Returns an iterator over all cell types that occur in this pattern.
    /// Will most likely contain duplicates.
    pub fn cell_types(&self) -> impl Iterator<Item = P::CellType>
    where
        P: PatBase,
    {
        self.tuples.iter().flat_map(|tuple| tuple.cell_types())
    }

    /// Returns true if any of these tuple patterns matches the tuple.
    pub fn matches(&self, tuple: &[P::Instance]) -> bool
    where
        P: PatBase,
    {
        self.tuples.iter().any(|tuple_pat| tuple_pat.matches(tuple))
    }

    pub fn combinations(&self) -> Vec<Vec<P>>
    where
        P: PatBase,
    {
        let mut combinations = Vec::new();
        self.tuples
            .iter()
            .for_each(|tuple| tuple.combinations(&mut combinations));
        combinations
    }
}

impl<CT: CellType> TuplePats<OperandPat<CT>> {
    pub fn fit(&self, cell: Cell<CT>) -> BoolSet {
        self.tuples
            .iter()
            .filter(|set| set.len() == 1)
            .map(|set| set[0].fit(cell))
            .collect()
    }
}

#[derive(Debug, Clone)]
pub struct NaryPat<P>(pub Pats<P>);

impl<P> NaryPat<P> {
    /// Returns true iff the given tuple matches this `NaryPat`.
    pub fn matches(&self, tuple: &[P::Instance]) -> bool
    where
        P: PatBase,
    {
        tuple.iter().all(|instance| self.0.matches(instance))
    }
}

impl<P> Index<usize> for NaryPat<P> {
    type Output = Pats<P>;

    fn index(&self, _: usize) -> &Self::Output {
        &self.0
    }
}

#[derive(Debug, Clone)]
pub enum TuplesDef<P> {
    Nary(NaryPat<P>),
    Tuples(TuplePats<P>),
}

impl<P> TuplesDef<P> {
    /// Returns the number of decribed operands or `None` if the number is variable.
    pub fn arity(&self) -> Option<usize> {
        match self {
            Self::Nary(_) => None,
            Self::Tuples(tuples) => Some(tuples.arity()),
        }
    }

    /// Returns an iterator over all cell types that occur in this pattern.
    /// Will most likely contain duplicates.
    pub fn cell_types(&self) -> impl Iterator<Item = P::CellType>
    where
        P: PatBase,
    {
        match self {
            Self::Tuples(tuples) => Either::Left(tuples.cell_types()),
            Self::Nary(nary) => Either::Right(nary.0.cell_types()),
        }
    }

    pub fn matches(&self, tuple: &[P::Instance]) -> bool
    where
        P: PatBase,
    {
        match self {
            Self::Nary(nary) => nary.matches(tuple),
            Self::Tuples(tuples) => tuples.matches(tuple),
        }
    }

    /// Returns all combinations of operands that fit this description. For descriptions of n-ary
    /// operands returns only a minimal set of combinations (i.e. slices of length 1).
    pub fn combinations(&self) -> Vec<Vec<P>>
    where
        P: PatBase,
    {
        match self {
            Self::Tuples(tuples) => tuples.combinations(),
            Self::Nary(nary) => nary.0.iter().map(|typ| vec![*typ]).collect(),
        }
    }

    pub fn length_one_patterns(&self) -> impl Iterator<Item = P>
    where
        P: PatBase,
    {
        match self {
            Self::Tuples(tuples) => Either::Left(
                tuples
                    .iter()
                    .filter(|tuple| (tuple.as_slice().len() == 1))
                    .flat_map(|tuple| tuple[0].iter().copied()),
            ),
            Self::Nary(nary) => Either::Right(nary.0.iter().copied()),
        }
    }
}

impl<CT: CellType> TuplesDef<OperandPat<CT>> {
    /// Returns the inverted-values for which using **only** the given cell for the described
    /// operands described is valid
    pub fn fit_cell(&self, cell: Cell<CT>) -> BoolSet {
        match self {
            Self::Tuples(tuples) => tuples.fit(cell),
            Self::Nary(typ) => typ.0.fit(cell),
        }
    }
}
