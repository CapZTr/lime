use std::iter;

use eggmock::Id;
use either::Either;
use itertools::{Itertools, MinMaxResult};
use tracing::warn;

use crate::program::ProgramVersion;

pub trait CandidateSelector {
    fn select_candidates<V: ProgramVersion>(&self, version: &V) -> impl Iterator<Item = Id>;
}

pub struct AllCandidates;

impl CandidateSelector for AllCandidates {
    fn select_candidates<V: ProgramVersion>(&self, version: &V) -> impl Iterator<Item = Id> {
        version.candidates().iter().cloned()
    }
}

// Soeken, Mathias, et al. "An MIG-based compiler for programmable logic-in-memory architectures."
// Proceedings of the 53rd Annual Design Automation Conference. 2016.
pub struct MIGBasedCompilerCandidateSelection;

impl CandidateSelector for MIGBasedCompilerCandidateSelection {
    fn select_candidates<V: ProgramVersion>(&self, version: &V) -> impl Iterator<Item = Id> {
        let mut iter = version.candidates().iter();
        let Some(mut v) = iter
            .next()
            .map(|id| MBCSelectionCandidate::new(version, *id))
        else {
            return None.into_iter();
        };
        for u in iter {
            let u = MBCSelectionCandidate::new(version, *u);
            if u.releasing_children > v.releasing_children
                || u.largest_level_parent < v.smallest_level_parent
            {
                v = u;
            }
        }
        Some(v.node).into_iter()
    }
}

struct MBCSelectionCandidate {
    node: Id,
    releasing_children: usize,
    largest_level_parent: usize,
    smallest_level_parent: usize,
}

impl MBCSelectionCandidate {
    fn new<V: ProgramVersion>(version: &V, node: Id) -> Self {
        let releasing_children = get_releasing_children(version, node);
        let (smallest_level_parent, largest_level_parent) =
            match parent_levels(version, node).minmax() {
                MinMaxResult::NoElements => {
                    warn!("dangling node");
                    (0, 0)
                }
                MinMaxResult::OneElement(el) => (el, el),
                MinMaxResult::MinMax(min, max) => (min, max),
            };
        MBCSelectionCandidate {
            node,
            releasing_children,
            largest_level_parent,
            smallest_level_parent,
        }
    }
}

fn parent_levels<V: ProgramVersion>(version: &V, node: Id) -> impl Iterator<Item = usize> {
    let ntk = &version.parameters().network;
    ntk.node_output_ids(node)
        .iter()
        .map(|fanout| ntk.level(*fanout))
        .chain(if version.output_ids().contains(&node) {
            Either::Left(iter::once(ntk.max_level() + 1))
        } else {
            Either::Right(iter::empty())
        })
}

fn get_releasing_children<V: ProgramVersion>(version: &V, node: Id) -> usize {
    let ntk = &version.parameters().network;
    ntk.node(node)
        .inputs()
        .iter()
        .filter(|fanin| ntk.node_output_ids(fanin.node_id()).len() == 1)
        .count()
}
