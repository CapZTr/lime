#![allow(dead_code)]

use std::{
    collections::HashMap,
    env,
    fmt::{Display, Formatter, Result},
    sync::Arc,
};

use crate::{
    benchmark::{
        ARCHITECTURES, BENCHMARKS, Benchmark, BenchmarkCmdLineResult, BenchmarkResult, FailReason,
    },
    db::read_from_file,
    utils::{arch_name, benchmark_name},
};

mod benchmark;
mod db;
mod utils;

fn main() -> anyhow::Result<()> {
    let all_results = read_from_file(env::args().nth(1).unwrap())?;
    generate_table_codegen(&all_results);
    generate_table_rewriting(&all_results);
    Ok(())
}

fn generate_table_rewriting(all_results: &HashMap<Benchmark, BenchmarkResult>) {
    let comparison = BenchmarkGroup {
        candidate_selection: "all",
        mode: "greedy",
        rewrite_size_factor: 0,
        rewrite_strategy: "none",
        title: "".to_string(),
    };
    let metrics = [
        Metric::cost(),
        Metric::cost().improvement(&comparison),
        // Metric::transformed_cost(),
        Metric::time(),
    ];
    let mut groups = Vec::new();
    for (rw_strat, title) in [
        ("compiling", "Code gen. guided"),
        ("lp", "Local CF (#abbr.a[ILP])"),
        ("greedy", "Local CF (Greedy)"),
    ] {
        groups.push(BenchmarkGroup {
            title: format!("[{title}]"),
            candidate_selection: "all",
            mode: "greedy",
            rewrite_size_factor: 100,
            rewrite_strategy: rw_strat,
        });
    }
    generate_table(all_results, &groups, &metrics);
}

fn generate_table_codegen(all_results: &HashMap<Benchmark, BenchmarkResult>) {
    let metrics = [
        Metric::cost(),
        Metric::utilization(),
        // Metric::instructions(),
        Metric::time(),
    ];
    let mut groups = Vec::new();
    for (mode, mode_disp) in [("greedy", "Greedy"), ("exhaustive", "Exhaustive")] {
        for (candsel, candsel_disp) in [("all", "All"), ("plim_compiler", "@PLiMCompiler")] {
            groups.push(BenchmarkGroup {
                title: format!("[{mode_disp} / {candsel_disp}]"),
                candidate_selection: candsel,
                mode,
                rewrite_size_factor: 0,
                rewrite_strategy: "none",
            });
        }
    }
    generate_table(all_results, &groups, &metrics);
}

fn generate_table(
    all_results: &HashMap<Benchmark, BenchmarkResult>,
    groups: &[BenchmarkGroup],
    metrics: &[Metric],
) {
    let benchmarks = BENCHMARKS;
    let num_metrics = metrics.len();
    let num_groups = groups.len();
    let columns = 2 + num_metrics * groups.len();
    println!(
        r#"
#import "/common.typ": *
#set text(size: 7pt)
"#
    );

    let gaps = format!("(auto, {})", "1fr,".repeat(num_metrics - 1));

    for architecture in ARCHITECTURES {
        println!(
            r#"
        #table(
            columns: {columns},
            column-gutter: (1fr,) + {gaps} * {num_groups},
            align: (x, y) => if y == 0 {{
              center
            }} else if x > 0 and y > 0 {{
              right
            }} else {{
              left
            }},
        "#
        );

        let mut first_line = format!(
            "table.cell(colspan: 2, strong[{}]), ",
            arch_name(architecture)
        );
        let mut second_line = String::from("[Benchmark], $|N|$, ");
        for BenchmarkGroup { title, .. } in groups {
            first_line.push_str(&format!("table.cell(colspan: {num_metrics}, {title}), "));
            for metric in metrics {
                second_line.push_str(metric.title);
                second_line.push_str(", ");
            }
        }
        for i in 0..groups.len() {
            println!("table.vline(x: {}), ", 2 + i * num_metrics)
        }
        println!("table-header({first_line}{second_line}),");

        for benchmark in benchmarks {
            let get_results = |group: &BenchmarkGroup| {
                let benchmark = Benchmark {
                    benchmark: benchmark.to_string(),
                    arch: architecture.to_string(),
                    mode: group.mode.to_string(),
                    candidate_selection: group.candidate_selection.to_string(),
                    rewriting_mode: group.rewrite_strategy.to_string(),
                    rewriting_size_factor: group.rewrite_size_factor,
                };
                (
                    benchmark.clone(),
                    all_results
                        .get(&benchmark)
                        .expect(&format!("result should be present {benchmark:?}")),
                )
            };

            // collect and print network data
            let (n_inputs, n_outputs, n_nodes) = groups
                .iter()
                .map(get_results)
                .filter_map(|(_, result)| result.result.as_ref().ok())
                .map(|result| (result.n_inputs, result.n_outputs, result.n_nodes))
                .next()
                .unwrap();
            print!(
                "[{} (${n_inputs}$/${n_outputs}$)], ${n_nodes}$,",
                benchmark_name(benchmark)
            );

            // collect minimum metric values
            let min_metrics: Vec<_> = metrics
                .iter()
                .map(|metric| {
                    groups
                        .iter()
                        .map(get_results)
                        .filter_map(|(benchmark, result)| {
                            Some((benchmark, result, result.result.as_ref().ok()?))
                        })
                        .map(|result| (metric.get)(&result.0, result.1, result.2, all_results))
                        .min_by(|a, b| a.partial_cmp(b).unwrap())
                })
                .collect();
            for group in groups {
                let (benchmark, result) = get_results(group);
                match &result.result {
                    Ok(data) => {
                        if data.validation_success != 1 {
                            panic!("validation failed for {benchmark:?}")
                        }
                        for (i, metric) in metrics.iter().enumerate() {
                            let min = min_metrics[i].as_ref().unwrap();
                            let value = (metric.get)(&benchmark, result, data, all_results);
                            if metric.highlight && value <= *min {
                                print!("strong({value}), ")
                            } else {
                                print!("{value}, ")
                            }
                        }
                    }
                    Err(FailReason::Timeout) => {
                        print!("table.cell(colspan: {num_metrics}, align(center, $hourglass$)),")
                    }
                    Err(FailReason::Infeasible) => {
                        print!("table.cell(colspan: {num_metrics}, align(center, $crossmark$)),")
                    }
                    Err(_) => {
                        if result.stderr.contains("generic/src/egraph/mod.rs:108:22")
                            || result.stderr.contains("generic/src/egraph/mod.rs:106:22")
                        {
                            print!(
                                "table.cell(colspan: {num_metrics}, align(center, $arrow.cw$)),"
                            );
                        } else if result.stderr.contains("lp_extract.rs:194:80")
                            || result.stderr.contains("lp_extract.rs:193:13")
                            || result
                                .stderr
                                .contains("generic/src/egraph/opt_extractor.rs:111:29")
                        {
                            print!(
                                "table.cell(colspan: {num_metrics}, align(center, $crossmark$)),"
                            );
                        } else {
                            panic!("{benchmark:?}: {result:?}")
                        }
                    }
                }
            }
            println!()
        }
        println!(")");
    }
}

#[derive(Clone, Debug)]
struct BenchmarkGroup {
    title: String,
    mode: &'static str,
    candidate_selection: &'static str,
    rewrite_strategy: &'static str,
    rewrite_size_factor: usize,
}

#[derive(Clone)]
struct Metric {
    get: Arc<
        dyn Fn(
            &Benchmark,
            &BenchmarkResult,
            &BenchmarkCmdLineResult,
            &HashMap<Benchmark, BenchmarkResult>,
        ) -> MetricValue,
    >,
    title: &'static str,
    highlight: bool,
}

impl Metric {
    fn cost() -> Metric {
        Metric {
            get: Arc::new(|_, _, cmd, _| MetricValue::Float(cmd.t_cost)),
            title: "[cost]",
            highlight: true,
        }
    }
    fn utilization() -> Metric {
        Metric {
            get: Arc::new(|_, _, cmd, _| MetricValue::Int(cmd.num_cells)),
            title: "$\"#\"C$",
            highlight: true,
        }
    }
    fn instructions() -> Metric {
        Metric {
            get: Arc::new(|_, _, cmd, _| MetricValue::Int(cmd.num_instr)),
            title: "$\"#\"I$",
            highlight: true,
        }
    }
    fn time() -> Metric {
        Metric {
            get: Arc::new(|_, result, _, _| MetricValue::TimeMs(result.t_total)),
            title: "$t$",
            highlight: false,
        }
    }
    fn transformed_cost() -> Metric {
        Metric {
            get: Arc::new(|_, _, cmd, _| MetricValue::Float(cmd.rebuilt_ntk_cost)),
            highlight: true,
            title: "[ntkcost]",
        }
    }
    fn improvement(&self, comparison: &BenchmarkGroup) -> Metric {
        let comparison = comparison.clone();
        let s = self.clone();
        Metric {
            get: Arc::new(move |benchmark, res, res_cmd, results| {
                let other = Benchmark {
                    arch: benchmark.arch.clone(),
                    benchmark: benchmark.benchmark.clone(),
                    candidate_selection: comparison.candidate_selection.to_string(),
                    mode: comparison.mode.to_string(),
                    rewriting_mode: comparison.rewrite_strategy.to_string(),
                    rewriting_size_factor: comparison.rewrite_size_factor,
                };
                let other_res = results.get(&other).unwrap();
                let other_metric = (s.get)(
                    &other,
                    &other_res,
                    other_res.result.as_ref().unwrap(),
                    results,
                );
                let self_metric = (s.get)(benchmark, res, res_cmd, results);
                self_metric.improvement(&other_metric)
            }),
            title: "[impr.]",
            highlight: false,
        }
    }
}

#[derive(PartialOrd, PartialEq)]
enum MetricValue {
    TimeMs(u64),
    Float(f64),
    Int(u64),
    Percentage(f64),
}

impl MetricValue {
    pub fn improvement(&self, other: &MetricValue) -> MetricValue {
        MetricValue::Percentage((other.value_f64() - self.value_f64()) / other.value_f64() * 100.0)
    }

    fn value_f64(&self) -> f64 {
        match self {
            Self::Float(f) => *f,
            Self::Int(i) => *i as f64,
            Self::Percentage(p) => *p,
            Self::TimeMs(t) => *t as f64,
        }
    }
}

impl Display for MetricValue {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        match self {
            Self::Int(v) => write!(f, "${v}$"),
            Self::TimeMs(t) => write!(f, "${:.1}s$", *t as f64 / 1000.0),
            Self::Float(v) => write!(f, "${:}$", (v * 10.0).round() / 10.0),
            Self::Percentage(v) => write!(f, "${:.1}%$", v),
        }
    }
}
