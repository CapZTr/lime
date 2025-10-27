// (taken from egg, includes solution from https://github.com/egraphs-good/egg/issues/207)
//
// Copyright 2019 Max Willsey
//
// Permission is hereby granted, free of charge, to any person obtaining
// a copy of this software and associated documentation files (the
// "Software"), to deal in the Software without restriction, including
// without limitation the rights to use, copy, modify, merge, publish,
// distribute, sublicense, and/or sell copies of the Software, and to
// permit persons to whom the Software is furnished to do so, subject to
// the following conditions:
//
// The above copyright notice and this permission notice shall be
// included in all copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND,
// EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF
// MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND
// NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE
// LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION
// OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION
// WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.

use std::collections::{HashMap, HashSet};

use coin_cbc::{Col, Model, Sense};
use egg::{Analysis, EGraph, Id, Language, LpCostFunction, RecExpr};

/// A structure to perform extraction using integer linear programming.
/// This uses the [`cbc`](https://projects.coin-or.org/Cbc) solver.
/// You must have it installed on your machine to use this feature.
/// You can install it using:
///
/// | OS               | Command                                  |
/// |------------------|------------------------------------------|
/// | Fedora / Red Hat | `sudo dnf install coin-or-Cbc-devel`     |
/// | Ubuntu / Debian  | `sudo apt-get install coinor-libcbc-dev` |
/// | macOS            | `brew install cbc`                       |
///
/// On macOS, you might also need the following in your `.zshrc` file:
/// `export LIBRARY_PATH=$LIBRARY_PATH:$(brew --prefix)/lib`
///
/// # Example
/// ```
/// use egg::*;
/// let mut egraph = EGraph::<SymbolLang, ()>::default();
///
/// let f = egraph.add_expr(&"(f x x x)".parse().unwrap());
/// let g = egraph.add_expr(&"(g (g x))".parse().unwrap());
/// egraph.union(f, g);
/// egraph.rebuild();
///
/// let best = Extractor::new(&egraph, AstSize).find_best(f).1;
/// let lp_best = LpExtractor::new(&egraph, AstSize).solve(f);
///
/// // In regular extraction, cost is measures on the tree.
/// assert_eq!(best.to_string(), "(g (g x))");
///
/// // Using ILP only counts common sub-expressions once,
/// // so it can lead to a smaller DAG expression.
/// assert_eq!(lp_best.to_string(), "(f x x x)");
/// assert_eq!(lp_best.len(), 2);
/// ```
#[cfg_attr(docsrs, doc(cfg(feature = "lp")))]
pub struct LpExtractor<'a, L: Language, N: Analysis<L>> {
    egraph: &'a EGraph<L, N>,
    model: Model,
    vars: HashMap<Id, ClassVars>,
}

struct ClassVars {
    active: Col,
    order: Col,
    nodes: Vec<Col>,
}

impl<'a, L, N> LpExtractor<'a, L, N>
where
    L: Language,
    N: Analysis<L>,
{
    /// Create an [`LpExtractor`] using costs from the given [`LpCostFunction`].
    /// See those docs for details.
    pub fn new<CF>(egraph: &'a EGraph<L, N>, mut cost_function: CF) -> Self
    where
        CF: LpCostFunction<L, N>,
    {
        let max_order = egraph.total_number_of_nodes() as f64 * 10.0;

        let mut model = Model::default();

        let vars: HashMap<Id, ClassVars> = egraph
            .classes()
            .map(|class| {
                let cvars = ClassVars {
                    active: model.add_binary(),
                    order: model.add_col(),
                    nodes: class.nodes.iter().map(|_| model.add_binary()).collect(),
                };
                model.set_col_upper(cvars.order, max_order);
                (class.id, cvars)
            })
            .collect();

        let mut cycles: HashSet<(Id, usize)> = Default::default();
        find_cycles(egraph, |id, i| {
            cycles.insert((id, i));
        });

        for (&id, class) in &vars {
            // class active == some node active
            // sum(for node_active in class) == class_active
            let row = model.add_row();
            model.set_row_equal(row, 0.0);
            model.set_weight(row, class.active, -1.0);
            for &node_active in &class.nodes {
                model.set_weight(row, node_active, 1.0);
            }

            for (i, (node, &node_active)) in egraph[id].iter().zip(&class.nodes).enumerate() {
                if cycles.contains(&(id, i)) {
                    model.set_col_upper(node_active, 0.0);
                    model.set_col_lower(node_active, 0.0);
                    continue;
                }

                for child in node.children() {
                    let child_active = vars[child].active;
                    // node active implies child active, encoded as:
                    //   node_active <= child_active
                    //   node_active - child_active <= 0
                    let row = model.add_row();
                    model.set_row_upper(row, 0.0);
                    model.set_weight(row, node_active, 1.0);
                    model.set_weight(row, child_active, -1.0);
                }
            }
        }

        model.set_obj_sense(Sense::Minimize);
        for class in egraph.classes() {
            for (node, &node_active) in class.iter().zip(&vars[&class.id].nodes) {
                model.set_obj_coeff(node_active, cost_function.node_cost(egraph, class.id, node));
            }
        }

        dbg!(max_order);

        Self {
            egraph,
            model,
            vars,
        }
    }

    /// Set the cbc timeout in seconds.
    pub fn timeout(&mut self, seconds: f64) -> &mut Self {
        self.model.set_parameter("seconds", &seconds.to_string());
        self
    }

    /// Extract a single rooted term.
    ///
    /// This is just a shortcut for [`LpExtractor::solve_multiple`].
    pub fn solve(&mut self, root: Id) -> RecExpr<L> {
        self.solve_multiple(&[root]).0
    }

    /// Extract (potentially multiple) roots
    pub fn solve_multiple(&mut self, roots: &[Id]) -> (RecExpr<L>, Vec<Id>) {
        let egraph = self.egraph;

        for class in self.vars.values() {
            self.model.set_binary(class.active);
        }

        for root in roots {
            let root = &egraph.find(*root);
            self.model.set_col_lower(self.vars[root].active, 1.0);
        }

        let solution = self.model.solve();
        // log::info!(
        //     "CBC status {:?}, {:?}",
        //     solution.raw().status(),
        //     solution.raw().secondary_status()
        // );

        let mut todo: Vec<Id> = roots.iter().map(|id| self.egraph.find(*id)).collect();
        let mut expr = RecExpr::default();
        // converts e-class ids to e-node ids
        let mut ids: HashMap<Id, Id> = HashMap::default();

        while let Some(&id) = todo.last() {
            if ids.contains_key(&id) {
                todo.pop();
                continue;
            }
            let v = &self.vars[&id];
            assert!(solution.col(v.active) > 0.0);
            let node_idx = v.nodes.iter().position(|&n| solution.col(n) > 0.0).unwrap();
            let node = &self.egraph[id].nodes[node_idx];
            if node.all(|child| ids.contains_key(&child)) {
                let new_id = expr.add(node.clone().map_children(|i| ids[&self.egraph.find(i)]));
                ids.insert(id, new_id);
                todo.pop();
            } else {
                todo.extend_from_slice(node.children())
            }
        }

        let root_idxs = roots.iter().map(|root| ids[root]).collect();

        assert!(expr.is_dag(), "LpExtract found a cyclic term!: {:?}", expr);
        (expr, root_idxs)
    }
}
fn find_cycles<L, N>(egraph: &EGraph<L, N>, mut f: impl FnMut(Id, usize))
where
    L: Language,
    N: Analysis<L>,
{
    let mut pending: HashMap<Id, Vec<(Id, usize)>> = HashMap::default();

    let mut order: HashMap<Id, usize> = HashMap::default();

    let mut memo: HashMap<(Id, usize), bool> = HashMap::default();

    let mut stack: Vec<(Id, usize)> = vec![];

    for class in egraph.classes() {
        let id = class.id;
        for (i, node) in egraph[id].iter().enumerate() {
            for &child in node.children() {
                pending.entry(child).or_insert_with(Vec::new).push((id, i));
            }

            if node.is_leaf() {
                stack.push((id, i));
            }
        }
    }

    let mut count = 0;

    while let Some((id, i)) = stack.pop() {
        if memo.get(&(id, i)).is_some() {
            continue;
        }

        let node = &egraph[id].nodes[i];
        let mut update = false;

        if node.is_leaf() {
            update = true;
        } else if node.children().iter().all(|&x| order.get(&x).is_some()) {
            if let Some(ord) = order.get(&id) {
                update = node
                    .children()
                    .iter()
                    .all(|&x| order.get(&x).unwrap() < ord);
                if !update {
                    memo.insert((id, i), false);
                    continue;
                }
            } else {
                update = true;
            }
        }

        if update {
            if order.get(&id).is_none() {
                order.insert(id, count);
                count = count + 1;
            }
            memo.insert((id, i), true);
            if let Some(mut v) = pending.remove(&id) {
                stack.append(&mut v);
                stack.sort();
                stack.dedup();
            };
        }
    }

    for class in egraph.classes() {
        let id = class.id;
        for (i, _) in egraph[id].iter().enumerate() {
            if let Some(true) = memo.get(&(id, i)) {
                continue;
            }
            f(id, i);
        }
    }
}
