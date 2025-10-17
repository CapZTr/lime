use std::{
    collections::HashSet,
    hash::{BuildHasher, Hash},
};

pub trait Set<E> {
    fn contains(&self, e: &E) -> bool;
    fn and<'a>(&'a self, e: &'a E) -> AndSet<'a, Self, E>
    where
        E: Eq,
    {
        AndSet(self, e)
    }
}

pub enum AllOrNone {
    All,
    None,
}

impl AllOrNone {
    pub fn all(all: bool) -> Self {
        if all { Self::All } else { Self::None }
    }
}

impl<E> Set<E> for AllOrNone {
    fn contains(&self, _: &E) -> bool {
        match self {
            Self::All => true,
            Self::None => false,
        }
    }
}

impl<E: Hash + Eq, H: BuildHasher> Set<E> for HashSet<E, H> {
    fn contains(&self, e: &E) -> bool {
        HashSet::contains(self, e)
    }
}

pub struct AndSet<'a, S: ?Sized, E>(&'a S, &'a E);

impl<'a, S, E> Set<E> for AndSet<'a, S, E>
where
    S: Set<E>,
    E: Eq,
{
    fn contains(&self, e: &E) -> bool {
        *self.1 == *e || self.0.contains(e)
    }
}
