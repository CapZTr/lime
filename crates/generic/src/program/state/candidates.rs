use std::collections::hash_map::Entry;

use derive_more::{Deref, DerefMut};
use eggmock::Id;
use rustc_hash::{FxHashMap, FxHashSet};

#[derive(Default, Debug, Deref, DerefMut)]
pub struct Candidates(FxHashSet<Id>);

impl Candidates {
    pub fn add(&mut self, candidate: Id) -> bool {
        self.0.insert(candidate)
    }
    pub fn remove(&mut self, candidate: Id) -> bool {
        self.0.remove(&candidate)
    }
    pub fn savepoint(&mut self) -> CandidatesSavepoint<'_> {
        CandidatesSavepoint {
            candidates: self,
            changes: Default::default(),
        }
    }
}

#[derive(Debug)]
pub struct CandidatesSavepoint<'a> {
    candidates: &'a mut Candidates,
    changes: ChangeMap,
}

#[derive(Default, Clone, Debug)]
pub struct CandidatesDelta {
    changes: ChangeMap,
}

#[derive(PartialEq, Eq, Clone, Copy, Debug)]
enum Change {
    Added,
    Removed,
}

impl<'a> CandidatesSavepoint<'a> {
    pub fn candidates(&self) -> &Candidates {
        self.candidates
    }
    pub fn add(&mut self, candidate: Id) -> bool {
        if self.candidates.add(candidate) {
            assert!(self.changes.add(candidate));
            true
        } else {
            false
        }
    }
    pub fn remove(&mut self, candidate: Id) -> bool {
        if self.candidates.remove(candidate) {
            assert!(self.changes.remove(candidate));
            true
        } else {
            false
        }
    }
    pub fn savepoint(&mut self) -> CandidatesSavepoint<'_> {
        CandidatesSavepoint {
            candidates: self.candidates,
            changes: Default::default(),
        }
    }
    pub fn retain(mut self) {
        self.changes.0.clear();
    }
    pub fn replay(&mut self, delta: &CandidatesDelta) {
        for (candidate, change) in &delta.changes.0 {
            match change {
                Change::Added => self.add(*candidate),
                Change::Removed => self.remove(*candidate),
            };
        }
    }
    pub fn append_to_delta(&self, delta: &mut CandidatesDelta) {
        for (candidate, change) in &self.changes.0 {
            match change {
                Change::Added => delta.changes.add(*candidate),
                Change::Removed => delta.changes.remove(*candidate),
            };
        }
    }
}

impl<'a> Drop for CandidatesSavepoint<'a> {
    fn drop(&mut self) {
        for (candidate, change) in &self.changes.0 {
            match change {
                Change::Added => self.candidates.remove(*candidate),
                Change::Removed => self.candidates.add(*candidate),
            };
        }
    }
}

#[derive(Default, Clone, Debug)]
struct ChangeMap(FxHashMap<Id, Change>);

impl ChangeMap {
    fn add(&mut self, id: Id) -> bool {
        match self.0.entry(id) {
            Entry::Occupied(entry) => match entry.get() {
                Change::Removed => {
                    entry.remove();
                    true
                }
                Change::Added => false,
            },
            Entry::Vacant(entry) => {
                entry.insert_entry(Change::Added);
                true
            }
        }
    }
    fn remove(&mut self, id: Id) -> bool {
        match self.0.entry(id) {
            Entry::Occupied(entry) => match entry.get() {
                Change::Added => {
                    entry.remove();
                    true
                }
                Change::Removed => false,
            },
            Entry::Vacant(entry) => {
                entry.insert_entry(Change::Removed);
                true
            }
        }
    }
}
