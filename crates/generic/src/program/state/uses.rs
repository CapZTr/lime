use eggmock::Id;
use rustc_hash::{FxHashMap, FxHashSet};

#[derive(Debug)]
pub struct Uses(FxHashMap<Id, usize>, FxHashSet<Id>);

impl Uses {
    pub fn new(never: impl IntoIterator<Item = Id>) -> Self {
        Self(Default::default(), FxHashSet::from_iter(never))
    }
    pub fn get(&self, id: Id) -> usize {
        if self.1.contains(&id) {
            return 0;
        }
        self.0.get(&id).copied().unwrap_or(0)
    }
}

#[derive(Debug)]
pub struct UsesSavepoint<'a> {
    uses: &'a mut Uses,
    increments: Vec<Id>,
}

#[derive(Default, Clone, Debug)]
pub struct UsesDelta(Vec<Id>);

impl<'a> UsesSavepoint<'a> {
    pub fn new(uses: &'a mut Uses) -> Self {
        Self {
            uses,
            increments: Vec::with_capacity(8),
        }
    }
    pub fn uses(&self) -> &Uses {
        self.uses
    }
    pub fn increment(&mut self, id: Id) -> usize {
        let entry = self.uses.0.entry(id).or_default();
        self.increments.push(id);
        *entry += 1;
        if self.uses.1.contains(&id) { 0 } else { *entry }
    }
    pub fn replay(&mut self, mut delta: UsesDelta) {
        for &id in &delta.0 {
            *self.uses.0.entry(id).or_default() += 1
        }
        if self.increments.is_empty() {
            // this is most likely true and we can save us the copying overhead
            self.increments = delta.0;
        } else {
            self.increments.append(&mut delta.0);
        }
    }
    pub fn append_to_delta(&self, delta: &mut UsesDelta) {
        delta.0.extend(&self.increments);
    }
    pub fn savepoint(&mut self) -> UsesSavepoint<'_> {
        UsesSavepoint::new(self.uses)
    }
    pub fn retain(mut self) {
        self.increments.clear();
    }
}

impl Drop for UsesSavepoint<'_> {
    fn drop(&mut self) {
        for id in &self.increments {
            *self.uses.0.get_mut(id).unwrap() -= 1
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    pub fn test_uses() {
        let mut uses = Uses::new([Id::from_usize(0); 0]);
        let id0 = Id::from(0);
        let id1 = Id::from(1);

        // test resetting
        for _ in 0..2 {
            let mut sp = UsesSavepoint::new(&mut uses);
            assert_eq!(sp.increment(id0), 1);
            assert_eq!(sp.increment(id1), 1);
            assert_eq!(sp.increment(id0), 2);
            assert_eq!(sp.increment(id0), 3);
            assert_eq!(sp.increment(id1), 2);
        }

        // test delta
        let mut delta = Default::default();
        {
            let mut sp = UsesSavepoint::new(&mut uses);
            assert_eq!(sp.increment(id0), 1);
            assert_eq!(sp.increment(id0), 2);
            sp.append_to_delta(&mut delta);
            let mut sp = sp.savepoint();
            assert_eq!(sp.increment(id0), 3);
            sp.append_to_delta(&mut delta);
        }

        // test replay and retain
        {
            let mut sp = UsesSavepoint::new(&mut uses);
            sp.replay(delta);
            sp.retain();
        }

        {
            let mut sp = UsesSavepoint::new(&mut uses);
            assert_eq!(sp.increment(id0), 4);
        }
    }
}
