use std::{collections::hash_map::Entry, fmt::Debug};

use blanket::blanket;
use derive_where::derive_where;
use eggmock::{Id, Signal};
use lime_generic_def::{Architecture, Cell, CellType};
use rustc_hash::FxHashMap;

use crate::program::state::free::FreeCells;

/// Keeps track of the state of the cells in a memory array.
///
/// Contains mappings from
/// * cell to signal (1:1)
/// * and signal to cells (1:n)
#[blanket(derive(Mut))]
pub trait CellStates<CT: CellType>: Sized + Debug {
    /// Returns the signal stored in the given cell if known.
    fn cell(&self, cell: Cell<CT>) -> Option<Signal>;
    /// Returns all cells that contain the given signal.
    fn cells_with(&self, signal: Signal) -> impl Iterator<Item = Cell<CT>> + '_;
    fn cells_with_id(&self, id: Id) -> impl Iterator<Item = (Cell<CT>, bool)> + '_ {
        self.cells_with(Signal::new(id, false))
            .map(|cell| (cell, false))
            .chain(
                self.cells_with(Signal::new(id, true))
                    .map(|cell| (cell, true)),
            )
    }
    fn all_cells_with(&self, signal: Signal) -> impl Iterator<Item = (Cell<CT>, bool)> {
        self.cells_with_id(signal.node_id())
            .map(move |(cell, inv)| (cell, inv ^ signal.is_inverted()))
    }
    fn contains_id(&self, id: Id) -> bool {
        self.cells_with(Signal::new(id, false)).next().is_some()
            || self.cells_with(Signal::new(id, true)).next().is_some()
    }
    /// Sets the signal of the given cell.
    ///
    /// Returns the signal that the cell did store before this operation (which may be equal to the
    /// given signal if it did not change).
    fn set<Sig: Into<Option<Signal>>>(&mut self, cell: Cell<CT>, signal: Sig) -> Option<Signal>;
    fn clear_all_by_id(&mut self, id: Id);
    fn free_cells(&self, typ: CT) -> &FreeCells;
}

#[derive_where(Debug; CT: CellType)]
pub struct CellStatesStore<CT> {
    #[derive_where(skip)]
    signal_to_cells: FxHashMap<Signal, Vec<Cell<CT>>>,
    cell_to_signal: FxHashMap<Cell<CT>, Signal>,
    #[derive_where(skip)]
    free_cells: FxHashMap<CT, FreeCells>,
}

impl<CT: CellType> CellStatesStore<CT> {
    pub fn new(arch: &Architecture<CT>) -> Self {
        let mut free_cells = FxHashMap::default();
        free_cells.insert(CT::CONSTANT, FreeCells::new(Some(0)));
        for typ in arch.types() {
            if *typ != CT::CONSTANT {
                free_cells.insert(*typ, FreeCells::new(typ.count()));
            }
        }
        Self {
            signal_to_cells: Default::default(),
            cell_to_signal: Default::default(),
            free_cells,
        }
    }

    pub fn cell(&self, cell: Cell<CT>) -> Option<Signal> {
        self.cell_to_signal.get(&cell).copied()
    }

    pub fn cells_with(&self, signal: Signal) -> impl Iterator<Item = Cell<CT>> + '_ {
        self.signal_to_cells
            .get(&signal)
            .into_iter()
            .flatten()
            .copied()
    }

    pub fn set<S: Into<Option<Signal>>>(&mut self, cell: Cell<CT>, signal: S) -> Option<Signal> {
        let signal = signal.into();

        let previous = {
            match self.cell_to_signal.entry(cell) {
                Entry::Occupied(mut entry) => {
                    if Some(*entry.get()) == signal {
                        return signal;
                    } else if let Some(signal) = signal {
                        Some(entry.insert(signal))
                    } else {
                        Some(entry.remove())
                    }
                }
                Entry::Vacant(entry) => match signal {
                    None => return None,
                    Some(signal) => {
                        entry.insert(signal);
                        None
                    }
                },
            }
        };

        // if a signal was already stored in this cell, we need to remove the reverse mapping
        // (signal -> cell) as well
        if let Some(previous) = previous {
            let cells = self.signal_to_cells.get_mut(&previous).unwrap();
            let idx = cells
                .iter()
                .position(|cell_for_previous| *cell_for_previous == cell)
                .unwrap();
            cells.swap_remove(idx);
            debug_assert!(
                !cells.contains(&cell),
                "cell should only be in signal -> cell mapping once"
            )
        }

        // add signal -> cell mapping
        if let Some(signal) = signal {
            self.signal_to_cells.entry(signal).or_default().push(cell);
        };

        let free_cells = self
            .free_cells
            .get_mut(&cell.typ())
            .expect("unknown cell type");
        if signal.is_none() {
            free_cells.add(cell.index())
        } else {
            free_cells.remove(cell.index())
        };
        previous
    }

    pub fn clear_all_by_id(&mut self, id: Id, mut callback: impl FnMut(Cell<CT>, Signal)) {
        for cell in [true, false].iter().flat_map(|inv| {
            self.signal_to_cells
                .remove(&Signal::new(id, *inv))
                .into_iter()
                .flatten()
        }) {
            self.free_cells
                .get_mut(&cell.typ())
                .expect("unknown cell type")
                .add(cell.index());
            let signal = self.cell_to_signal.remove(&cell).unwrap();
            callback(cell, signal);
        }
    }

    pub fn free_cells(&self, typ: CT) -> &FreeCells {
        self.free_cells.get(&typ).expect("unknown cell type")
    }

    pub fn savepoint(&mut self) -> CellStatesSavepoint<'_, CT> {
        CellStatesSavepoint::new(self)
    }
}

#[derive(Debug)]
pub struct CellStatesSavepoint<'a, CT: CellType> {
    store: &'a mut CellStatesStore<CT>,
    previous: FxHashMap<Cell<CT>, Option<Signal>>,
}

#[derive(Clone, Debug)]
pub struct CellStatesDelta<CT>(FxHashMap<Cell<CT>, Option<Signal>>);

impl<CT> Default for CellStatesDelta<CT> {
    fn default() -> Self {
        Self(Default::default())
    }
}

impl<'a, CT: CellType> CellStatesSavepoint<'a, CT> {
    pub fn new(store: &'a mut CellStatesStore<CT>) -> Self {
        Self {
            store,
            previous: Default::default(),
        }
    }
    pub fn savepoint(&mut self) -> CellStatesSavepoint<'_, CT> {
        CellStatesSavepoint::new(self.store)
    }
    pub fn replay(&mut self, delta: &CellStatesDelta<CT>) {
        for (&cell, &sig) in &delta.0 {
            self.set(cell, sig);
        }
    }
    pub fn append_to_delta(&self, delta: &mut CellStatesDelta<CT>) {
        for &cell in self.previous.keys() {
            let change = self.store.cell(cell);
            delta.0.insert(cell, change);
        }
    }
    pub fn retain(mut self) {
        self.previous.clear();
    }
}

impl<'a, CT: CellType> CellStates<CT> for CellStatesSavepoint<'a, CT> {
    fn cell(&self, cell: Cell<CT>) -> Option<Signal> {
        self.store.cell(cell)
    }

    fn cells_with(&self, signal: Signal) -> impl Iterator<Item = Cell<CT>> + '_ {
        self.store.cells_with(signal)
    }

    fn set<Sig: Into<Option<Signal>>>(&mut self, cell: Cell<CT>, signal: Sig) -> Option<Signal> {
        let previous = self.store.set(cell, signal);
        self.previous.entry(cell).or_insert(previous);
        previous
    }

    fn clear_all_by_id(&mut self, id: Id) {
        self.store.clear_all_by_id(id, |cell, signal| {
            self.previous.entry(cell).or_insert(Some(signal));
        });
    }

    fn free_cells(&self, typ: CT) -> &FreeCells {
        self.store.free_cells(typ)
    }
}

impl<'a, CT: CellType> Drop for CellStatesSavepoint<'a, CT> {
    fn drop(&mut self) {
        for (&cell, &previous) in &self.previous {
            self.store.set(cell, previous);
        }
    }
}
