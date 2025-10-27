use std::f64;

use lime_generic_def::{
    Cell, CellPat, CellType, PatBase, Pats,
    set::{AllOrNone, Set},
};
use ordered_float::OrderedFloat;
use rustc_hash::FxHashSet;

use crate::{
    copy::{INode, start_operations},
    cost::Cost,
    program::{
        ProgramVersion,
        state::{CellStates, Operation},
    },
    utils::Mean,
};

pub fn spill_if_necessary<V: ProgramVersion + ?Sized>(version: &mut V, from_cell: Cell<V::CT>) {
    let Some(signal) = version.state().cell(from_cell) else {
        return;
    };
    if version
        .state()
        .cells_with_id(signal.node_id())
        .nth(1)
        .is_some()
    {
        version.state_mut().set(from_cell, None);
        return;
    }
    force_spill(version, from_cell, &AllOrNone::None);
}

pub fn force_spill<V: ProgramVersion + ?Sized>(
    version: &mut V,
    from_cell: Cell<V::CT>,
    not: &impl Set<Cell<V::CT>>,
) {
    let Some(signal) = version.state().cell(from_cell) else {
        return;
    };
    let params = version.parameters().clone();
    let (edge, to_cell) = start_operations(
        &params.arch.copy_graph,
        INode {
            node: CellPat::Cell(from_cell),
            invert: false,
            jumped_from: Some(CellPat::Cell(from_cell)),
        },
        &FxHashSet::default(),
    )
    .filter_map(|(edge, to_node)| {
        // attempt to find a free cell for the target node not in _not_
        let cell = match to_node.node {
            CellPat::Type(typ) => Cell::new(
                typ,
                version
                    .state()
                    .free_cells(typ)
                    .iter()
                    .find(|idx| !not.contains(&Cell::new(typ, *idx)))?,
            ),
            CellPat::Cell(cell) => {
                if !not.contains(&cell)
                    && version
                        .state()
                        .free_cells(cell.typ())
                        .contains(cell.index())
                {
                    cell
                } else {
                    return None;
                }
            }
        };
        Some((edge, cell))
    })
    .min_by_key(|(edge, _)| &edge.cost)
    .expect("a spill target should be available");

    version.state_mut().set(to_cell, signal ^ edge.inverted);
    let operation = Operation::Copy {
        from: from_cell,
        to: to_cell,
        inverted: edge.inverted,
        instructions: edge.instantiate(from_cell, to_cell).collect(),
        spill: true,
        computes_from_inverted: edge.computes_from_inverted,
    };
    version.state_mut().set(from_cell, None);
    version.append(operation);
}

fn estimate_spill_cost_cell_pat<V: ProgramVersion>(
    version: &V,
    pat: CellPat<V::CT>,
) -> Option<Cost> {
    if pat.cell_type() == <V::CT as CellType>::CONSTANT {
        return None;
    }
    start_operations(
        &version.parameters().arch.copy_graph,
        INode {
            node: pat,
            invert: false,
            jumped_from: Some(pat),
        },
        &FxHashSet::default(),
    )
    .map(|op| op.0.cost)
    .min()
}

pub fn estimate_spill_cost_operand_pats<V: ProgramVersion>(
    version: &V,
    pats: &Pats<CellPat<V::CT>>,
) -> Cost {
    assert!(!pats.is_empty());
    pats.iter()
        .flat_map(|pat| estimate_spill_cost_cell_pat(version, *pat))
        .mean()
        .unwrap_or(OrderedFloat(f64::INFINITY))
}
