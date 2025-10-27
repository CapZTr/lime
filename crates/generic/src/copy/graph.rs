use std::{
    collections::hash_map::Entry,
    fmt::Debug,
    iter::{self},
};

use either::Either;
use lime_generic_def::{
    Architecture, Cell, CellIndex, CellPat, CellType, Instruction, Operand, PatBase,
};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::{
    copy::{
        discovery::find_copy_instructions, discovery_constant::find_set_constant,
        placeholder::CellOrVar,
    },
    cost::{Cost, OperationCost},
};

pub const FROM_VAR: CellIndex = 0;
pub const TO_VAR: CellIndex = 1;

#[derive(Debug)]
pub struct Edge<CT> {
    pub inverted: bool,
    pub computes_from_inverted: bool,
    pub template: Vec<Instruction<CellOrVar<CT>, CT>>,
    pub cost: Cost,
}

#[derive(Default, Debug)]
pub struct TypeCellPat<V> {
    pub value: V,
    pub children: FxHashMap<CellIndex, V>,
}

impl<V> TypeCellPat<V> {
    pub fn value_or_default(&mut self, idx: Option<CellIndex>) -> &mut V
    where
        V: Default,
    {
        match idx {
            None => &mut self.value,
            Some(idx) => self.children.entry(idx).or_default(),
        }
    }
}

#[derive(Debug)]
pub struct TypeNodes<CT, V>(pub FxHashMap<CT, TypeCellPat<V>>);

impl<CT: CellType, V> TypeNodes<CT, V> {
    fn iter(&self) -> impl Iterator<Item = (CellPat<CT>, &V)> {
        self.0.iter().flat_map(|(typ, node)| {
            std::iter::once((CellPat::Type(*typ), &node.value)).chain(
                node.children
                    .iter()
                    .map(|(idx, value)| (CellPat::Cell(Cell::new(*typ, *idx)), value)),
            )
        })
    }
}

pub type FromTypeNode<CT> = TypeNodes<CT, [Option<Edge<CT>>; 2]>;

pub struct CopyGraph<CT> {
    pub(super) nodes: TypeNodes<CT, FromTypeNode<CT>>,
}

pub struct FindParams<'a, CT, OC: OperationCost<CT>> {
    pub arch: &'a Architecture<CT>,
    pub cost: &'a OC,
    pub graph: &'a mut CopyGraph<CT>,
}

impl<CT: CellType> CopyGraph<CT> {
    pub fn build(arch: &Architecture<CT>, cost: &impl OperationCost<CT>) -> Self {
        let mut graph = Self {
            nodes: Default::default(),
        };
        let mut params = FindParams {
            arch,
            cost,
            graph: &mut graph,
        };
        find_set_constant(&mut params);
        find_copy_instructions(&mut params);
        graph
    }

    pub fn nodes(&self) -> FxHashSet<CellPat<CT>> {
        let mut result = FxHashSet::default();
        for (src_typ, src_typenode) in &self.nodes.0 {
            if !src_typenode.value.0.is_empty() {
                result.insert(CellPat::Type(*src_typ));
            }

            // from cell type
            for (dst_typ, dst_typenode) in &src_typenode.value.0 {
                if dst_typenode.value.iter().any(|ed| ed.is_some()) {
                    result.insert(CellPat::Type(*dst_typ));
                }
                for dst_idx in dst_typenode.children.keys() {
                    result.insert(CellPat::Cell(Cell::new(*dst_typ, *dst_idx)));
                }
            }

            // from cell of this type
            for (src_idx, dst_typenodes) in &src_typenode.children {
                result.insert(CellPat::Cell(Cell::new(*src_typ, *src_idx)));
                for (dst_typ, dst_typenode) in &dst_typenodes.0 {
                    if dst_typenode.value.iter().any(|ed| ed.is_some()) {
                        result.insert(CellPat::Type(*dst_typ));
                    }
                    for dst_idx in dst_typenode.children.keys() {
                        result.insert(CellPat::Cell(Cell::new(*dst_typ, *dst_idx)));
                    }
                }
            }
        }
        result
    }

    pub fn all_optimal_edges_matching(
        &self,
        from: CellPat<CT>,
        to: CellPat<CT>,
        inverted: bool,
    ) -> impl Iterator<Item = (CellPat<CT>, CellPat<CT>, &Edge<CT>)> + '_ {
        // determine all relevant values for the typeset based on the "matching node"
        // the value of CellPat::Type(_) is the last element, which is important
        fn relevant_nodes<CT: CellType, V>(
            typenodes: &TypeNodes<CT, V>,
            node: CellPat<CT>,
        ) -> impl Iterator<Item = (CellPat<CT>, &V)> + '_ {
            typenodes
                .0
                .get(&node.cell_type())
                .into_iter()
                .flat_map(move |typenode| {
                    match node {
                        CellPat::Type(typ) => Either::Left(
                            typenode
                                .children
                                .iter()
                                .map(move |(idx, edges)| (Cell::new(typ, *idx), edges)),
                        ),
                        CellPat::Cell(cell) => {
                            let edge = typenode.children.get(&cell.index());
                            Either::Right(edge.map(|edge| (cell, edge)).into_iter())
                        }
                    }
                    .map(|(cell, edges)| (CellPat::Cell(cell), edges))
                    .chain(iter::once((
                        CellPat::Type(node.cell_type()),
                        &typenode.value,
                    )))
                })
        }

        let mut all_covered = false;
        relevant_nodes(&self.nodes, from)
            .map_while(move |(from_node, edges)| {
                if all_covered {
                    // we already have a full cover, we are done
                    return None;
                }
                let mut all_to_covered = false;
                let edges = relevant_nodes(edges, to)
                    .map_while(move |(to_node, edges)| {
                        if all_to_covered {
                            return None;
                        }
                        Some(edges[inverted as usize].as_ref().map(|edge| {
                            all_to_covered = matches!(to_node, CellPat::Type(_))
                                || matches!(to, CellPat::Cell(_));
                            (from_node, to_node, edge)
                        }))
                    })
                    .flatten();
                all_covered = all_to_covered
                    && (matches!(from_node, CellPat::Type(_)) || matches!(from, CellPat::Cell(_)));
                Some(edges)
            })
            .flatten()
    }

    pub fn consider_edge(&mut self, from: CellPat<CT>, to: CellPat<CT>, edge: Edge<CT>)
    where
        Cost: PartialOrd + Clone,
    {
        let inverted = edge.inverted as usize;
        let from_typenode = self.nodes.0.entry(from.cell_type()).or_default();
        if let CellPat::Cell(_) = from
            && let Some(to_typenode) = from_typenode.value.0.get(&to.cell_type())
        {
            // if from's parent already points to the target's parent with a smaller cost we do
            // not need this
            if let Some(existing_edge) = &to_typenode.value[inverted]
                && edge.cost >= existing_edge.cost
            {
                return;
            }
            // if from's parent already points to the target with a smaller cost we do not need
            // this
            if let CellPat::Cell(to_cell) = to
                && let Some(existing_edge) = to_typenode
                    .children
                    .get(&to_cell.index())
                    .and_then(|edges| edges[inverted].as_ref())
                && edge.cost >= existing_edge.cost
            {
                return;
            }
        }
        let from_edges = from_typenode.value_or_default(from.index());
        let to_typenode = from_edges.0.entry(to.cell_type()).or_default();

        // if from already points to the target's parent with a smaller cost we do not need this
        if let CellPat::Cell(_) = to
            && let Some(existing_edge) = &to_typenode.value[inverted]
            && edge.cost >= existing_edge.cost
        {
            return;
        }

        // insert the new edge
        let edge_cost = edge.cost;
        let current = &mut to_typenode.value_or_default(to.index())[inverted];
        match current {
            None => *current = Some(edge),
            Some(current) if current.cost > edge_cost => *current = edge,
            _ => {
                // better solution was in place, no-op
                return;
            }
        };

        // now we might have added an edge that is a better solution than others, let's see...

        // We will use this closure later to decide which edge entries to retain. It removes the
        // edge if it is more expensive than the newly added edge and returns true if the array
        // still contains something afterward.
        let check_retain = move |edges: &mut [Option<Edge<CT>>; 2]| -> bool {
            // does it have an edge and if yes, is it cheaper? if not, remove it
            let opt_edge = &mut edges[inverted];
            if let Some(edge) = opt_edge
                && edge.cost >= edge_cost
            {
                *opt_edge = None;
            }
            // remove from map if candidate has no more associated edges
            edges.iter().any(|opt| opt.is_some())
        };

        // if we have an edge from the from node, but to a child of the to node with equal or worse
        // cost, we can remove that edge:
        if let CellPat::Type(_) = to {
            to_typenode.children.retain(|_, edges| check_retain(edges));
        }

        // if we have an edge from any of from's child nodes to to or any of its children with equal
        // or more cost, we can remove that edge as well:
        if let CellPat::Type(_) = from {
            from_typenode.children.retain(|_, from_edges| {
                let Entry::Occupied(mut to_typenode_entry) = from_edges.0.entry(to.cell_type())
                else {
                    return true;
                };
                let to_typenode = to_typenode_entry.get_mut();
                match to.index() {
                    // to is a type node, we may delete edges to the type value and children
                    None => {
                        to_typenode.children.retain(|_, edges| check_retain(edges));
                        check_retain(&mut to_typenode.value);
                    }
                    // to is a cell node, hence we may only delete edges to the respective child
                    Some(idx) => {
                        let Entry::Occupied(mut entry) = to_typenode.children.entry(idx) else {
                            return true;
                        };
                        if !check_retain(entry.get_mut()) {
                            entry.remove();
                        }
                    }
                };
                // did we delete all edges to the type of to? if yes, remove the entry
                if to_typenode.value.iter().all(|opt| opt.is_none())
                    && to_typenode.children.is_empty()
                {
                    to_typenode_entry.remove();
                }
                // did we remove the last entry for this cell of the from-type?
                !from_edges.0.is_empty()
            });
        }
    }
}

impl<CT: CellType> Edge<CT> {
    pub fn instantiate<TargetCT>(
        &self,
        from: Cell<TargetCT>,
        to: Cell<TargetCT>,
    ) -> impl Iterator<Item = Instruction<TargetCT, CT>>
    where
        TargetCT: CellType,
        CT: Into<TargetCT>,
    {
        let map_operand = move |operand: &Operand<CellOrVar<CT>>| -> Operand<TargetCT> {
            let idx = operand.cell.index();
            let cell = match operand.cell.typ() {
                CellOrVar::Var if idx == FROM_VAR => from,
                CellOrVar::Var if idx == TO_VAR => to,
                CellOrVar::Var => panic!("invalid variable index"),
                CellOrVar::Cell(typ) => Cell::new(typ.into(), idx),
            };
            Operand {
                cell,
                inverted: operand.inverted,
            }
        };
        let map_cell = move |cell: &Cell<CellOrVar<CT>>| -> Cell<TargetCT> {
            let idx = cell.index();
            match cell.typ() {
                CellOrVar::Var if idx == FROM_VAR => from,
                CellOrVar::Var if idx == TO_VAR => to,
                CellOrVar::Var => panic!("invalid variable index"),
                CellOrVar::Cell(typ) => Cell::new(typ.into(), idx),
            }
        };
        self.template.iter().map(move |instruction| Instruction {
            typ: instruction.typ.clone(),
            inputs: Vec::from_iter(instruction.inputs.iter().map(map_cell)),
            outputs: Vec::from_iter(instruction.outputs.iter().map(map_operand)),
        })
    }
}

impl<CT: CellType> Debug for CopyGraph<CT> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "CopyGraph (")?;
        for (from, to_edges) in self.nodes.iter() {
            for (to, edges) in to_edges.iter() {
                for edge in [true, false]
                    .into_iter()
                    .filter_map(|inverted| edges[inverted as usize].as_ref())
                {
                    write!(f, "  {from} -> ")?;
                    if edge.inverted {
                        write!(f, "!")?;
                    }
                    writeln!(
                        f,
                        "{to} with cost {:?} (computes_from_inverted: {})",
                        edge.cost, edge.computes_from_inverted
                    )?;
                    for instruction in &edge.template {
                        writeln!(f, "    {instruction}")?;
                    }
                }
            }
        }
        write!(f, ")")?;
        Ok(())
    }
}

impl<CT, V: Default> Default for TypeNodes<CT, V> {
    fn default() -> Self {
        Self(Default::default())
    }
}
