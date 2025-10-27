use std::{
    rc::Rc,
    time::{Duration, Instant},
};

use egg::{AstDepth, EGraph, Extractor, LpExtractor, Runner};
use eggmock::{EggExt, Network, NetworkReceiver, Receiver};
use lime_generic_def::CellType;

use crate::{
    ArchitectureMeta,
    compilation::{CandidateSelection, CompilationMode},
    cost::OperationCost,
    egraph::{
        analysis::LimeAnalysis,
        comp_extraction::CompilingCostFunction,
        opt_extractor::OptExtractor,
        transform::{LpInversionCostFunction, rebuild_network, transform_egraph},
        trimming::trim_egraph,
    },
    untyped_ntk::{UntypedNetwork, UntypedNetworkLanguage, create_rewrites},
};

mod analysis;
mod comp_extraction;
mod opt_extractor;
mod transform;
mod trimming;

#[repr(C)]
pub enum RewritingStrategy {
    None,
    LP,
    Compiling,
    CompilingMemusage,
    GreedyEstimate,
}

#[derive(Debug)]
#[repr(C)]
pub struct RewritingStatistics {
    pub t_runner: u64,
    pub n_nodes_pre_trim: u64,
    pub t_trim: u64,
    pub n_nodes_post_trim: u64,
    pub t_extractor: u64,
    pub rebuilt_ntk_cost: std::ffi::c_double,
}

pub fn rewriting_receiver<CT: CellType, C: OperationCost<CT>>(
    arch: Rc<ArchitectureMeta<CT>>,
    strategy: RewritingStrategy,
    size_factor: usize,
    candidate_selection: CandidateSelection,
    compilation_mode: CompilationMode,
    cost: C,
    disjunct_input_output: bool,
) -> impl Receiver<Gate = UntypedNetwork, Result = (Network<UntypedNetwork>, RewritingStatistics)> {
    EGraph::<UntypedNetworkLanguage, LimeAnalysis>::default().map(move |(egraph, mut outputs)| {
        eprintln!("rewriting to size {size_factor}");
        let rules = create_rewrites(&arch);

        let t_runner = Instant::now();
        let mut egraph = if !matches!(strategy, RewritingStrategy::None) {
            let runner = Runner::default()
                .with_node_limit(size_factor * egraph.total_size())
                .with_egraph(egraph)
                .with_iter_limit(usize::MAX)
                .with_time_limit(Duration::new(60 * 5, 0))
                .run(&rules);
            eprintln!("Rewriting done! Report: {}", runner.report());
            runner.egraph
        } else {
            egraph
        };
        let t_runner = (Instant::now() - t_runner).as_millis() as u64;
        let mut rebuilt_ntk_cost = 0.0;

        // canonicalize IDs
        outputs.iter_mut().for_each(|id| *id = egraph.find(*id));

        let n_nodes_pre_trim = egraph.total_number_of_nodes() as u64;
        let t_trim = Instant::now();
        if matches!(strategy, RewritingStrategy::Compiling) {
            trim_egraph(&mut egraph, &outputs);
        }
        let t_trim = (Instant::now() - t_trim).as_millis() as u64;
        let n_nodes_post_trim = egraph.total_number_of_nodes() as u64;
        eprintln!("Trimmed to size {}", egraph.total_number_of_nodes());

        let t_extractor = Instant::now();
        let ntk = match strategy {
            RewritingStrategy::Compiling | RewritingStrategy::CompilingMemusage => {
                let extractor = OptExtractor::new(
                    &egraph,
                    CompilingCostFunction {
                        arch,
                        candidate_selection,
                        mode: compilation_mode,
                        cost,
                        disjunct_input_output,
                        memusage: matches!(strategy, RewritingStrategy::CompilingMemusage),
                    },
                );
                extractor
                    .choices()
                    .send(NetworkReceiver::default(), outputs.iter().cloned())
                    .unwrap()
            }
            RewritingStrategy::GreedyEstimate => {
                eprintln!("transforming");
                let (transformed, outputs) = transform_egraph(&egraph, &arch, &outputs);
                eprintln!("extracting");
                let mut cost = LpInversionCostFunction::new(&arch, cost.clone());
                let extractor = Extractor::new(&transformed, cost.clone());
                let (cost, ntk) = rebuild_network(&extractor, &outputs, &arch, &mut cost);
                rebuilt_ntk_cost = cost;
                ntk
            }
            RewritingStrategy::LP => {
                eprintln!("transforming");
                let (transformed, outputs) = transform_egraph(&egraph, &arch, &outputs);
                eprintln!("extracting");
                let mut cost = LpInversionCostFunction::new(&arch, cost.clone());
                let mut extractor = LpExtractor::new(&transformed, cost.clone());
                let (expr, outputs) = extractor.solve_multiple(&outputs);
                let (cost, ntk) = rebuild_network(&expr, &outputs, &arch, &mut cost);
                rebuilt_ntk_cost = cost;
                ntk
            }
            RewritingStrategy::None => Extractor::new(&egraph, AstDepth)
                .send(NetworkReceiver::default(), outputs.iter().cloned())
                .unwrap(),
        };
        let t_extractor = (Instant::now() - t_extractor).as_millis() as u64;
        eprintln!("t-extractor: {t_extractor}");

        (
            ntk,
            RewritingStatistics {
                n_nodes_post_trim,
                n_nodes_pre_trim,
                t_extractor,
                t_runner,
                t_trim,
                rebuilt_ntk_cost,
            },
        )
    })
}
