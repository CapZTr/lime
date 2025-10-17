use either::Either;
use lime_generic_def::CellIndex;
use rustc_hash::FxHashSet;

#[derive(Debug)]
pub struct FreeCells(FreeCellsInner);

#[derive(Debug)]
enum FreeCellsInner {
    RightOpen {
        before: FxHashSet<CellIndex>,
        first: CellIndex,
    },
    Set(FxHashSet<CellIndex>),
}

impl FreeCells {
    pub fn new(num_cells: Option<CellIndex>) -> Self {
        Self(match num_cells {
            Some(num) => FreeCellsInner::Set((0..num).collect()),
            None => FreeCellsInner::RightOpen {
                before: Default::default(),
                first: 0,
            },
        })
    }

    /// Returns whether the index was previously not free
    pub fn add(&mut self, mut idx: CellIndex) -> bool {
        match &mut self.0 {
            FreeCellsInner::RightOpen { before, first } => {
                if idx >= *first {
                    false
                } else if idx == *first - 1 {
                    assert!(!before.remove(&idx));
                    let idx = loop {
                        if idx == 0 {
                            break idx;
                        }
                        idx -= 1;
                        if !before.remove(&idx) {
                            break idx + 1;
                        }
                    };
                    *first = idx;
                    true
                } else {
                    before.insert(idx)
                }
            }
            FreeCellsInner::Set(set) => set.insert(idx),
        }
    }

    /// Returns whether the index was previously free
    pub fn remove(&mut self, idx: CellIndex) -> bool {
        match &mut self.0 {
            FreeCellsInner::RightOpen { before, first } => {
                if idx >= *first {
                    for i in *first..idx {
                        debug_assert!(before.insert(i), "{self:?} / {idx} / {i}");
                    }
                    *first = idx + 1;
                    false
                } else if idx == *first - 1 {
                    assert!(!before.remove(&idx));
                    false
                } else {
                    before.remove(&idx)
                }
            }
            FreeCellsInner::Set(set) => set.remove(&idx),
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = CellIndex> {
        match &self.0 {
            FreeCellsInner::Set(set) => Either::Left(set.iter().copied()),
            FreeCellsInner::RightOpen { before, first } => {
                Either::Right(before.iter().copied().chain(*first..))
            }
        }
    }

    pub fn contains(&self, idx: CellIndex) -> bool {
        match &self.0 {
            FreeCellsInner::RightOpen { before, first } => *first <= idx || before.contains(&idx),
            FreeCellsInner::Set(set) => set.contains(&idx),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_free_cells_open() {
        let mut cells = FreeCells::new(None);
        assert!(
            matches!(&cells.0, FreeCellsInner::RightOpen { before, first } if *first == 0 && before.is_empty())
        );
        cells.remove(2);
        assert!(
            matches!(&cells.0, FreeCellsInner::RightOpen { before, first } if before.iter().eq(&[0, 1]) && *first == 3)
        )
    }
}
