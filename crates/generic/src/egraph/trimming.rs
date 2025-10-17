use egg::{Analysis, EGraph, Id, Language};
use eggmock::NetworkLanguage;
use itertools::Itertools;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::egraph::analysis::LimeAnalysis;

pub fn trim_egraph<L: NetworkLanguage>(egraph: &mut EGraph<L, LimeAnalysis>, _outputs: &[Id]) {
    trim_eclasses_commutative(egraph);
}

fn trim_eclasses_commutative<L: Language, N: Analysis<L>>(egraph: &mut EGraph<L, N>) {
    let mut removed = 0;
    let mut id_map = FxHashMap::default();
    for class in egraph.classes() {
        for node in &class.nodes {
            for &id in node.children() {
                id_map.entry(id).or_insert_with(|| egraph.find(id));
            }
        }
    }
    for class in egraph.classes_mut() {
        let mut found_nodes: FxHashMap<L::Discriminant, FxHashSet<Vec<(egg::Id, usize)>>> =
            FxHashMap::default();
        class.nodes.retain_mut(|node| {
            if node.is_leaf() {
                return true;
            }
            let mut map: FxHashMap<egg::Id, usize> = FxHashMap::default();
            node.children()
                .iter()
                .for_each(|id| *map.entry(id_map[id]).or_default() += 1);
            let mut vec = map.into_iter().collect_vec();
            vec.sort_by_key(|(id, _)| *id);
            if !found_nodes
                .entry(node.discriminant())
                .or_default()
                .insert(vec)
            {
                removed += 1;
                false
            } else {
                true
            }
        });
    }
    eprintln!("removed {removed} nodes");
}
