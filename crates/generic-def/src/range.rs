use std::{cmp::min, ops::Index};

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct Range {
    pub start: usize,
}

impl Range {
    pub fn start_offset(self) -> usize {
        self.start
    }
    pub fn iter(self) -> impl Iterator<Item = usize> {
        self.start..
    }
    pub fn contains(self, idx: usize) -> bool {
        self.start <= idx
    }
    pub fn slice<T>(self, slice: &[T]) -> (usize, &[T], usize) {
        let start = min(self.start, slice.len());
        (start, &slice[start..], 0)
    }
    pub fn num_elements_in(self, len: usize) -> usize {
        len.saturating_sub(self.start)
    }
    pub fn map_index(self, idx: usize) -> Option<usize> {
        if idx < self.start {
            None
        } else {
            Some(idx - self.start)
        }
    }
    pub fn index_view<I: Index<usize> + ?Sized>(
        self,
        index: &I,
    ) -> impl Index<usize, Output = I::Output> {
        RangeIndexed(self, index)
    }
}

struct RangeIndexed<'a, I: ?Sized>(Range, &'a I);

impl<'a, I: Index<usize> + ?Sized> Index<usize> for RangeIndexed<'a, I> {
    type Output = I::Output;

    fn index(&self, index: usize) -> &Self::Output {
        &self.1[self
            .0
            .map_index(index)
            .expect("index should be within range")]
    }
}
