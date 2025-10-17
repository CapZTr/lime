use std::iter::once;

use eggmock::{
    FFIGate, GateFunction, ReceiveFrom, Receiver, Signal, define_network,
    egg::{Analysis, ENodeOrVar, Id, Pattern, RecExpr, Rewrite, Var, rewrite},
};
use either::Either;
use itertools::Itertools;
use lime_generic_def::{Architecture, CellType};
use rustc_hash::FxHashSet;
use tracing::warn;

define_network! {
    pub enum UntypedNetwork {
        "and" = And(*),
        "xor" = Xor(*),
        "maj" = Maj(*)
    }
}

impl UntypedNetworkLanguage {
    pub fn new_for_fn(f: GateFunction, inputs: Vec<Id>) -> Self {
        match f {
            GateFunction::And => Self::And(inputs),
            GateFunction::Maj => Self::Maj(inputs),
            GateFunction::Xor => Self::Xor(inputs),
        }
    }
}

impl ReceiveFrom<FFIGate> for UntypedNetwork {
    fn receive_from(from: FFIGate, receiver: &mut impl Receiver<Gate = Self>) -> Signal {
        match from {
            FFIGate::And(inputs) => receiver.create_gate(Self::And(Vec::from(inputs))),
            FFIGate::Xor(inputs) => receiver.create_gate(Self::Xor(Vec::from(inputs))),
            FFIGate::Xor3(inputs) => receiver.create_gate(Self::Xor(Vec::from(inputs))),
            FFIGate::Maj(inputs) => receiver.create_gate(Self::Maj(Vec::from(inputs))),
        }
    }
}

pub fn create_rewrites<N: Analysis<UntypedNetworkLanguage>, CT: CellType>(
    architecture: &Architecture<CT>,
) -> Vec<Rewrite<UntypedNetworkLanguage, N>> {
    use lime_generic_def::Gate::*;

    let gates = FxHashSet::from_iter(architecture.instructions().gates());
    let mut rewrites = Vec::new();

    rewrites.extend(rewrite!("not-not"; "(! (! ?a))" <=> "?a"));

    if gates.contains(&And) {
        rewrites.extend([
            rewrite!("and-ident"; "(and ?x (! f))" => "?x"),
            rewrite!("and-annulment"; "(and ?x f)" => "f"),
            rewrite!("and-idempotency"; "(and ?x ?x)" => "?x"),
            rewrite!("and-complement"; "(and ?x (! ?x))" => "f"),
            rewrite!("and-comm"; "(and ?x ?y)" => "(and ?y ?x)"),
            rewrite!("and-assoc"; "(and ?x (and ?y ?z))" => "(and (and ?x ?y) ?z)"),
            rewrite!("and-consensus"; "
                (and
                    (and
                        (! (and (! ?x) ?y))
                        (! (and ?x ?z))
                    )
                    (! (and ?y ?z))
                )
            " => "
                (and
                    (! (and (! ?x) ?y))
                    (! (and ?x ?z))
                )
            "),
        ]);
        rewrites.extend(
            rewrite!("and-dist"; "(and ?x (! (and (! ?y) (! ?z)) ))" <=> "(! (and (! (and ?x ?y)) (! (and ?x ?z)) ))")
        );
    }

    if gates.contains(&Maj) {
        rewrites.extend([
            rewrite!("maj-commute-1"; "(maj ?x ?y ?z)" => "(maj ?y ?x ?z)"),
            rewrite!("maj-commute-2"; "(maj ?x ?y ?z)" => "(maj ?z ?y ?x)"),
            rewrite!("maj-majority-1"; "(maj ?x ?x ?y)" => "?x"),
            rewrite!("maj-majority-2"; "(maj ?x (! ?x) ?y)" => "?y"),
            rewrite!("maj-assoc"; "(maj ?x ?u (maj ?y ?u ?z))" => "(maj ?z ?u (maj ?y ?u ?x))"),
        ]);
        rewrites.extend(
            rewrite!("maj-dist"; "(maj ?x ?y (maj ?u ?v ?z))" <=> "(maj (maj ?x ?y ?u) (maj ?x ?y ?v) ?z)")
        );
        rewrites.extend(
            rewrite!("maj-inv-prop"; "(! (maj ?x ?y ?z))" <=> "(maj (! ?x) (! ?y) (! ?z))"),
        );
    }

    if gates.contains(&Xor) {
        rewrites.extend([
            rewrite!("xor-identity"; "(xor ?x f)" => "?x"),
            rewrite!("xor-identity-inv"; "(xor ?x (! f))" => "(! ?x)"),
            rewrite!("xor-annulment"; "(xor ?x ?x)" => "f"),
            rewrite!("xor-annulment-inv"; "(xor ?x (! ?x))" => "(! f)"),
            rewrite!("xor-comm"; "(xor ?x ?y)" => "(xor ?y ?x)"),
            rewrite!("xor-assoc"; "(xor ?x (xor ?y ?z))" => "(xor (xor ?x ?y) ?z)"),
        ]);
        rewrites.extend(rewrite!("xor-inv-prop"; "(! (xor ?x ?y))" <=> "(xor (! ?x) ?y)"));
    }

    if gates.contains(&Maj) && gates.contains(&Xor) {
        rewrites.extend(
            rewrite!("maj-xor-dist"; "(maj (xor ?x ?u) (xor ?y ?u) (xor ?z ?u))" <=> "(xor (maj ?x ?y ?z) ?u)")
        );
        rewrites.extend(
            rewrite!("maj-xor-comp-assoc"; "(maj ?x ?y (xor (! ?y) ?z))" <=> "(maj ?x ?y (xor ?x ?z))")
        );
    }

    if gates.contains(&And) && gates.contains(&Xor) {
        rewrites.extend(
            rewrite!("and-xor-dist"; "(and ?x (xor ?y ?z))" <=> "(xor (and ?x ?y) (and ?x ?z))"),
        );
        rewrites.extend(rewrite!("and-xor-conv"; "
                (and
                    (! (and ?x (! ?y)))
                    (! (and (! ?x) ?y))
                )" <=> "(! (xor ?x ?y))"));
    }

    if gates.contains(&And) && gates.contains(&Maj) {
        rewrites.extend(rewrite!("maj-and-conv1"; "(maj ?a ?b f)" <=> "(and ?a ?b)"));
        rewrites.extend(rewrite!("maj-and-conv2"; "(! (maj ?x ?y ?z))" <=> r#"
            (and
                (and
                    (! (and ?x ?y))
                    (! (and ?y ?z))
                )
                (! (and ?x ?z))
            )
        "#));
    }

    // folding of "base" gates
    add_associative_folds(architecture, &mut rewrites, GateFunction::And, 2);
    add_associative_folds(architecture, &mut rewrites, GateFunction::Xor, 2);
    add_maj_folds(architecture, &mut rewrites);

    rewrites
}

fn add_maj_folds<N: Analysis<UntypedNetworkLanguage>, CT>(
    arch: &Architecture<CT>,
    rewrites: &mut Vec<Rewrite<UntypedNetworkLanguage, N>>,
) {
    /*fn helper(
        expr: &mut RecExpr<ENodeOrVar<UntypedNetworkLanguage>>,
        arity: usize,
        x: usize,
        y: usize,
        t: Id,
        f: Id,
    ) -> Id {
        let y_num_nodes = arity - y;
        let left = y - min(y, arity / 2);
        if x < left {
            f
        } else if x >= left + y_num_nodes {
            t
        } else if y == arity - 1 {
            Id::from(y)
        } else {
            let bottom = helper(expr, arity, x, y + 1, t, f);
            let right = helper(expr, arity, x + 1, y + 1, t, f);
            expr.add(ENodeOrVar::ENode(UntypedNetworkLanguage::Maj(vec![
                Id::from(y),
                bottom,
                right,
            ])))
        }
    }*/

    for arity in arch
        .instructions()
        .iter()
        .filter(|op| {
            op.function.gate.gate_function() == Some(GateFunction::Maj)
                && op.arity().is_some_and(|arity| arity > 3)
        })
        .flat_map(|op| op.arity())
        .unique()
    {
        if arity == 5 {
            rewrites.push(rewrite!("maj-fold-5-hardcoded"; "(maj (maj ?x ?y ?z) ?t (maj (maj ?x ?y ?u) ?u ?z))" => "(maj ?x ?y ?z ?t ?u)"));
        }
        warn!("unused majority instruction with arity {arity}")
        /*let mut expr = RecExpr::default();
        let mut ids = Vec::new();
        for i in 0..arity {
            ids.push(expr.add(ENodeOrVar::Var(Var::from_u32(i as u32))));
        }
        let mut out = expr.clone();
        out.add(ENodeOrVar::ENode(UntypedNetworkLanguage::Maj(ids)));

        let f = expr.add(ENodeOrVar::ENode(UntypedNetworkLanguage::False));
        let t = expr.add(ENodeOrVar::ENode(UntypedNetworkLanguage::Not(f)));
        helper(&mut expr, arity, 0, 0, t, f);
        rewrites.push(
            Rewrite::new(
                format!("maj-fold-{arity}"),
                Pattern::new(expr),
                Pattern::new(out),
            )
            .expect("should be a valid rewrite"),
        );*/
    }
}

fn add_associative_folds<N: Analysis<UntypedNetworkLanguage>, CT>(
    arch: &Architecture<CT>,
    rewrites: &mut Vec<Rewrite<UntypedNetworkLanguage, N>>,
    gate_fn: GateFunction,
    base_n: usize,
) {
    // TODO: Fold for n-ary operations
    for arity in arch
        .instructions()
        .iter()
        .filter(|op| op.function.gate.gate_function() == Some(gate_fn) && op.arity() != Some(1))
        .flat_map(|op| match op.arity() {
            None => Either::Left(3..10),
            Some(arity) => Either::Right(once(arity)),
        })
        .unique()
    {
        if arity != base_n {
            let input_pattern = build_associative_fold_pattern(arity, base_n, gate_fn);

            // TODO: write a custom Applier that constructs the node without a Pattern
            let mut output_pattern = RecExpr::default();
            let mut inputs = Vec::new();
            for i in 0..arity {
                let id = output_pattern.add(ENodeOrVar::Var(Var::from_u32(i as u32)));
                inputs.push(id);
            }
            output_pattern.add(ENodeOrVar::ENode(UntypedNetworkLanguage::new_for_fn(
                gate_fn, inputs,
            )));

            rewrites.push(
                Rewrite::new(
                    format!("{gate_fn:?}-fold-{arity}"),
                    input_pattern,
                    Pattern::new(output_pattern),
                )
                .expect("rewrite should be valid"),
            );
        }
    }
}

fn build_associative_fold_pattern(
    num: usize,
    base_n: usize,
    gate_fn: GateFunction,
) -> Pattern<UntypedNetworkLanguage> {
    let mut expr: Vec<ENodeOrVar<UntypedNetworkLanguage>> = Vec::new();
    let mut values: Vec<Id> = Vec::new();
    for i in 0..num {
        expr.push(ENodeOrVar::Var(Var::from_u32(i as u32)));
        values.push(Id::from(i));
    }

    loop {
        if values.len() == 1 {
            break;
        }
        if values.len() < base_n {
            panic!("cannot build fold pattern for BASE_N {base_n}, num {num}");
        }
        // iterate over the current values, always taking base_n sized chunks and folding them together
        // replace the ith element of values with the id of the folded node
        // then we can resize values to the smaller size at the end
        let mut i = 0;
        loop {
            let fold_start = i * base_n;
            let ids = &values[fold_start..];
            if ids.len() < base_n {
                break;
            }
            let ids = &ids[..base_n];
            let node = UntypedNetworkLanguage::new_for_fn(gate_fn, Vec::from(ids));
            let node_id = Id::from(expr.len());
            expr.push(ENodeOrVar::ENode(node));
            values[i] = node_id;
            i += 1;
        }
        // append the trailing non-folded values
        for k in (i * base_n)..values.len() {
            values[i] = values[k];
            i += 1
        }
        values.truncate(i);
    }
    Pattern::new(RecExpr::from(expr))
}
