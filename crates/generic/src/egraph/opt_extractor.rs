use std::fmt::Debug;

use eggmock::{
    EggExt, NetworkLanguage,
    egg::{self, Analysis, EClass, EGraph, Id, Language},
};
use rustc_hash::FxHashMap;

pub trait OptCostFunction<L: Language, A: Analysis<L>>: Sized {
    type Cost: PartialOrd + Debug + Clone + Default;

    fn cost(
        &mut self,
        eclass: &EClass<L, A::Data>,
        enode: &L,
        choices: &Choices<Self, L, A>,
    ) -> Option<Self::Cost>;
}

pub struct Choices<'g, CF: OptCostFunction<L, A>, L: Language, A: Analysis<L>> {
    graph: &'g EGraph<L, A>,
    costs: FxHashMap<Id, (CF::Cost, L)>,
}

impl<'g, CF: OptCostFunction<L, A>, L: Language, A: Analysis<L>> Choices<'g, CF, L, A> {
    pub fn find_best(&self, class: Id) -> Option<&(CF::Cost, L)> {
        self.costs.get(&self.graph.find(class))
    }
}

/// An extractor heavily inspired by egg's [Extractor](eggmock::egg::Extractor), which allows
/// ignoring certain nodes by returning [None] from their cost function.
pub struct OptExtractor<'g, CF: OptCostFunction<L, A>, L: Language, A: Analysis<L>> {
    cost_fn: CF,
    costs: Choices<'g, CF, L, A>,
}

impl<'g, CF: OptCostFunction<L, A>, L: Language, A: Analysis<L>> OptExtractor<'g, CF, L, A> {
    pub fn new(graph: &'g EGraph<L, A>, cost_fn: CF) -> Self {
        let mut extractor = Self {
            cost_fn,
            costs: Choices {
                graph,
                costs: Default::default(),
            },
        };
        extractor.find_costs();
        extractor
    }

    pub fn choices(&self) -> &Choices<'g, CF, L, A> {
        &self.costs
    }

    fn find_costs(&mut self) {
        let mut changed = true;
        //let mut queue = BTreeSet::from_iter(leaf_eclasses(self.costs.graph));
        while changed {
            changed = false;
            // let class = &self.costs.graph[id];
            for class in self.costs.graph.classes() {
                let old_cost = self.costs.costs.remove(&class.id);
                let new_cost = self.determine_class_costs(class);
                match (old_cost, new_cost) {
                    (None, Some(new)) => {
                        self.costs.costs.insert(class.id, new);
                        //queue.extend(class.parents().map(|id| self.costs.graph.find(id)));
                        changed = true;
                    }
                    (Some(old), Some(new)) if new.0 < old.0 => {
                        self.costs.costs.insert(class.id, new);
                        //queue.extend(class.parents().map(|id| self.costs.graph.find(id)));
                        changed = true;
                    }
                    (Some(old_cost), _) => {
                        self.costs.costs.insert(class.id, old_cost);
                    }
                    _ => {
                        self.costs.costs.remove(&class.id);
                    }
                }
            }
        }
    }

    fn determine_class_costs(&mut self, class: &EClass<L, A::Data>) -> Option<(CF::Cost, L)> {
        class
            .iter()
            .map(|node| (self.opt_node_cost(node, class), node))
            .filter_map(|(cost, node)| cost.map(|cost| (cost, node)))
            .min_by(|(a, _), (b, _)| a.partial_cmp(b).unwrap())
            .map(|(cost, node)| (cost, node.clone()))
    }

    fn opt_node_cost(&mut self, node: &L, class: &EClass<L, A::Data>) -> Option<CF::Cost> {
        self.costs
            .costs
            .insert(class.id, (CF::Cost::default(), node.clone()));
        if node.all(|id| self.costs.costs.contains_key(&id)) {
            self.cost_fn.cost(class, node, &self.costs)
        } else {
            None
        }
    }
}

impl<'a, CF: OptCostFunction<L, A>, L: NetworkLanguage, A: Analysis<L>> EggExt
    for Choices<'a, CF, L, A>
{
    type Language = L;

    fn get_node(&self, id: egg::Id) -> &Self::Language {
        &self.find_best(id).expect("class should be extractable").1
    }
}
