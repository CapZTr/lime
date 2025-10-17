mod constant_mapping;
mod discovery;
mod discovery_constant;
mod graph;
pub mod placeholder;
pub mod spilling;

use std::{
    cmp::{Ordering, Reverse},
    collections::{BinaryHeap, hash_map::Entry},
    fmt::Debug,
    iter, panic,
};

use derive_more::Deref;
use either::Either;
use lime_generic_def::{Cell, CellIndex, CellPat, CellType, PatBase, set::Set};
use rustc_hash::{FxHashMap, FxHashSet};

pub use self::graph::CopyGraph;
use crate::{
    copy::graph::{Edge, TypeNodes},
    cost::Cost,
    program::{
        ProgramVersion,
        state::{CellStates, Operation},
    },
};

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq, Deref)]
struct INode<CT> {
    #[deref]
    node: CellPat<CT>,
    invert: bool,
    jumped_from: Option<CellPat<CT>>,
}

impl<CT: CellType> INode<CT> {
    pub fn is_allowed(&self, forbidden: &FxHashSet<Cell<CT>>) -> bool {
        if let CellPat::Cell(cell) = &self.node {
            !forbidden.contains(cell)
        } else {
            true
        }
    }
}

pub struct PathMemo<'g, CT: CellType>(FindPathResult<CT, PathTracker<'g, CT>>);

pub fn copy_cost<CT: CellType, F: Into<CellPat<CT>>>(
    graph: &CopyGraph<CT>,
    from: F,
    to: CellPat<CT>,
    invert: bool,
    forbidden: &FxHashSet<Cell<CT>>,
) -> Option<Cost> {
    find_path(
        (),
        graph,
        from,
        forbidden,
        matches_node(to, invert),
        |_, _, _| {},
    )
    .map(|result| result.cost)
}

pub fn copy_cost_with_path<'g, CT: CellType, F: Into<CellPat<CT>>>(
    graph: &'g CopyGraph<CT>,
    from: F,
    to: CellPat<CT>,
    invert: bool,
    forbidden: &FxHashSet<Cell<CT>>,
) -> Option<(Cost, PathMemo<'g, CT>)> {
    let result = find_path(
        PathTracker(FxHashMap::default()),
        graph,
        from,
        forbidden,
        matches_node(to, invert),
        |tracker, node, via| {
            tracker.0.insert(node, via);
        },
    );
    result.map(|result| (result.cost, PathMemo(result)))
}

#[must_use]
pub fn perform_copy<V: ProgramVersion>(
    PathMemo(result): PathMemo<'_, V::CT>,
    target: &mut V,
    mut from: Cell<V::CT>,
    to: CellPat<V::CT>,
    forbidden: &FxHashSet<Cell<V::CT>>,
) -> Option<Cell<V::CT>> {
    if to.matches(&from) && !result.to.invert && !forbidden.contains(&from) {
        return Some(from);
    }
    let mut path = result.state.reconstruct(result.from, result.to);
    path.1.last_mut().unwrap().1 = to;

    let mut signal = target
        .state()
        .cell(from)
        .expect("from cell should have an associated signal");
    for (edge, target_pat) in path.1 {
        let target_cell =
            target.make_overridable_cell_for_pat(target_pat, &forbidden.and(&from))?;
        let instructions = edge.instantiate(from, target_cell).collect();
        let operation = Operation::Copy {
            from,
            to: target_cell,
            inverted: edge.inverted,
            instructions,
            spill: false,
            computes_from_inverted: edge.computes_from_inverted,
        };
        from = target_cell;
        signal = signal ^ edge.inverted;
        target.state_mut().set(target_cell, signal);
        target.append(operation);
    }
    Some(from)
}

struct FindPathResult<CT: CellType, S> {
    state: S,
    cost: Cost,
    from: INode<CT>,
    to: INode<CT>,
}

fn matches_node<CT: CellType>(to: CellPat<CT>, invert: bool) -> impl Fn(INode<CT>) -> bool {
    move |node| {
        node.invert == invert
            && (*node == to || matches!(*node, CellPat::Type(typ) if typ == to.cell_type()))
    }
}

#[must_use = "you should check whether the copy operation was successful!"]
fn find_path<'g, CT: CellType, S, F: Into<CellPat<CT>>>(
    mut state: S,
    graph: &'g CopyGraph<CT>,
    from: F,
    forbidden: &FxHashSet<Cell<CT>>,
    matches: impl Fn(INode<CT>) -> bool,
    mut visit: impl FnMut(&mut S, INode<CT>, Via<'g, CT>),
) -> Option<FindPathResult<CT, S>> {
    let from = from.into();
    let mut costs = FxHashMap::default();
    let mut new_cheaper = move |node: INode<CT>, cost: Cost| {
        let cost_entry = costs.entry(node);
        if matches!(&cost_entry, Entry::Vacant(_))
            || matches!(&cost_entry, Entry::Occupied(entry) if *entry.get() > cost)
        {
            cost_entry.insert_entry(cost);
            true
        } else {
            false
        }
    };

    let mut visited = FxHashSet::default();
    let mut visit_next = BinaryHeap::new();

    let from = INode {
        node: from,
        invert: false,
        // prevent going to from_cell from the parent node of from_cell
        jumped_from: Some(from),
    };

    // we have to start with an operation! we cannot allow Via::FromParent or Via::FromChild
    for (edge, next) in start_operations(graph, from, forbidden) {
        if new_cheaper(next, edge.cost) {
            visit_next.push(Reverse(OrdFirst(edge.cost, next)));
            visit(&mut state, next, Via::Operation { from, edge });
        }
    }

    let mut result = None;
    while let Some(Reverse(OrdFirst(cost, node))) = visit_next.pop() {
        if !visited.insert(node) {
            continue;
        }
        if matches(node)
            && result
                .as_ref()
                .is_none_or(|(_, prev_cost)| *prev_cost > cost)
        {
            result = Some((node, cost))
        }
        for (via, next) in neighbours_of_node(graph, node, forbidden) {
            let next_cost = via.add_cost_to(cost);
            if let Some((_, prev_cost)) = &result
                && *prev_cost < cost
            {
                continue;
            }
            if new_cheaper(next, next_cost) {
                visit_next.push(Reverse(OrdFirst(next_cost, next)));
                visit(&mut state, next, via);
            }
        }
    }
    result.map(|(node, cost)| FindPathResult {
        state,
        cost,
        from,
        to: node,
    })
}

fn start_operations<'g, CT: CellType>(
    graph: &'g CopyGraph<CT>,
    from: INode<CT>,
    forbidden: &FxHashSet<Cell<CT>>,
) -> impl Iterator<Item = (&'g Edge<CT>, INode<CT>)> {
    graph
        .nodes
        .0
        .get(&from.node.cell_type())
        .into_iter()
        .flat_map(move |typenode| {
            neighbours_for_typenodes(from, &typenode.value).chain(match from.node {
                CellPat::Type(_) => Either::Left(iter::empty()),
                CellPat::Cell(cell) => Either::Right(
                    typenode
                        .children
                        .get(&cell.index())
                        .into_iter()
                        .flat_map(move |typenodes| neighbours_for_typenodes(from, typenodes)),
                ),
            })
        })
        .filter(|(_, node)| node.is_allowed(forbidden))
}

fn neighbours_of_node<'g, CT: CellType>(
    graph: &'g CopyGraph<CT>,
    node: INode<CT>,
    forbidden: &FxHashSet<Cell<CT>>,
) -> impl Iterator<Item = (Via<'g, CT>, INode<CT>)> {
    graph
        .nodes
        .0
        .get(&node.cell_type())
        .into_iter()
        .flat_map(move |typenode| match *node {
            CellPat::Cell(cell) => Either::Left(
                // jumping
                if node.jumped_from.is_some() {
                    Either::Left(iter::empty())
                } else {
                    Either::Right(iter::once((
                        Via::FromChild(cell.index()),
                        INode {
                            node: CellPat::Type(cell.typ()),
                            invert: node.invert,
                            jumped_from: Some(*node),
                        },
                    )))
                }
                // edges from the copy graph
                .chain(
                    typenode
                        .children
                        .get(&cell.index())
                        .into_iter()
                        .flat_map(move |typenodes| neighbours_for_typenodes(node, typenodes))
                        .map(move |(edge, to)| (Via::Operation { from: node, edge }, to)),
                ),
            ),
            CellPat::Type(typ) => Either::Right(
                // jumping
                if node.jumped_from.is_some() {
                    Either::Left(iter::empty())
                } else {
                    Either::Right(typenode.children.keys().map(move |idx| {
                        (
                            Via::FromParent,
                            INode {
                                node: CellPat::Cell(Cell::new(typ, *idx)),
                                invert: node.invert,
                                jumped_from: Some(*node),
                            },
                        )
                    }))
                }
                // edges from the copy graph
                .chain(
                    neighbours_for_typenodes(node, &typenode.value)
                        .map(move |(edge, to)| (Via::Operation { from: node, edge }, to)),
                ),
            ),
        })
        .filter(|(_, node)| node.is_allowed(forbidden))
}

fn neighbours_for_typenodes<CT: CellType>(
    from: INode<CT>,
    typenodes: &TypeNodes<CT, [Option<Edge<CT>>; 2]>,
) -> impl Iterator<Item = (&Edge<CT>, INode<CT>)> {
    typenodes.0.iter().flat_map(move |(typ, typenode)| {
        neighbours_for_edges(from, CellPat::Type(*typ), &typenode.value).chain(
            typenode.children.iter().flat_map(move |(idx, edges)| {
                neighbours_for_edges(from, CellPat::Cell(Cell::new(*typ, *idx)), edges)
            }),
        )
    })
}

fn neighbours_for_edges<CT: CellType>(
    from: INode<CT>,
    to: CellPat<CT>,
    edges: &[Option<Edge<CT>>; 2],
) -> impl Iterator<Item = (&Edge<CT>, INode<CT>)> {
    edges.iter().filter_map(Option::as_ref).map(move |edge| {
        (
            edge,
            INode {
                node: to,
                invert: from.invert ^ edge.inverted,
                jumped_from: None,
            },
        )
    })
}

struct PathTracker<'a, CT>(FxHashMap<INode<CT>, Via<'a, CT>>);

type Path<'a, CT> = (CellPat<CT>, Vec<(&'a Edge<CT>, CellPat<CT>)>);

impl<'a, CT: CellType> PathTracker<'a, CT> {
    pub fn reconstruct(&self, from: INode<CT>, to: INode<CT>) -> Path<'a, CT> {
        let mut path = Vec::new();
        let mut curr = to;
        loop {
            if curr == from && !path.is_empty() {
                break;
            }
            // where did we come from?
            let Some(via) = self.0.get(&curr) else {
                break;
            };
            // if we did come from child / parent and not via an operation, we have to take an extra
            // step
            // also, the node in the path should be the cell node of the two, because that node
            // restricts the allowed cells to that specific cell
            let (path_node, via) = match via {
                Via::FromChild(idx) => match *curr {
                    CellPat::Type(typ) => {
                        let child = CellPat::Cell(Cell::new(typ, *idx));
                        (
                            child,
                            self.0
                                .get(&INode {
                                    node: child,
                                    invert: curr.invert,
                                    jumped_from: None,
                                })
                                .expect("if we came from a child, there should be a predecessor"),
                        )
                    }
                    CellPat::Cell(_) => {
                        panic!("if we came from a child, we should be at the parent")
                    }
                },
                Via::FromParent => match *curr {
                    CellPat::Cell(cell) => (
                        CellPat::Cell(cell),
                        self.0
                            .get(&INode {
                                node: CellPat::Type(cell.typ()),
                                invert: curr.invert,
                                jumped_from: None,
                            })
                            .expect("if we came from the parent, there should be a predecessor"),
                    ),
                    CellPat::Type(_) => {
                        panic!("if we came from the parent, we should be at the child")
                    }
                },
                Via::Operation { .. } => (*curr, via),
            };
            let Via::Operation { from, edge } = via else {
                panic!("unexpected parent / child loop in path \nvia: {via:?}");
            };
            path.push((*edge, path_node));
            curr = *from;
        }
        assert!(!path.is_empty());
        assert!(curr == from);
        path.reverse();
        (*curr, path)
    }
}

#[derive(Debug)]
enum Via<'a, CT> {
    FromChild(CellIndex),
    FromParent,
    Operation { from: INode<CT>, edge: &'a Edge<CT> },
}

impl<'a, CT> Via<'a, CT> {
    pub fn add_cost_to(&self, cost: Cost) -> Cost {
        match self {
            Self::FromParent | Self::FromChild(_) => cost,
            Self::Operation { edge, .. } => cost + edge.cost,
        }
    }
}

struct OrdFirst<O, V>(pub O, pub V);

impl<O: Ord, V> PartialEq for OrdFirst<O, V> {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl<O: Ord, V> Eq for OrdFirst<O, V> {}

impl<O: Ord, V> PartialOrd for OrdFirst<O, V> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<O: Ord, V> Ord for OrdFirst<O, V> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0.cmp(&other.0)
    }
}
