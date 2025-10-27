#![allow(dead_code)]

use std::{
    collections::HashMap,
    env::args,
    sync::{Arc, Mutex},
    time::Duration,
};

use crate::{
    benchmark::{ARCHITECTURES, BENCHMARKS, Benchmark, FailReason, run_benchmark},
    db::{Entry, read_from_file, write_to_file},
};

mod benchmark;
mod db;

#[tokio::main]
async fn main() {
    // keep all benchmarks in the argument
    let previous = if let Some(path) = args().nth(1) {
        read_from_file(path).unwrap()
    } else {
        HashMap::default()
    };

    let timeout = Duration::new(60 * 60, 0);
    let mut benchmarks = compiler_benchmarks();
    benchmarks.extend(rewrite_benchmarks());
    benchmarks.extend(simdram_benchmarks());
    let benchmarks = Arc::new(Mutex::new(benchmarks));
    let entries = Arc::new(Mutex::new(previous));
    let mut handles = Vec::new();
    for _ in 0..7 {
        let benchmarks = benchmarks.clone();
        let entries = entries.clone();
        handles.push(tokio::task::spawn(async move {
            while let Some(benchmark) = { benchmarks.lock().unwrap().pop() } {
                let result = if let Some(result) = entries.lock().unwrap().get(&benchmark)
                    && matches!(result.result, Err(FailReason::Timeout))
                    && false
                {
                    result.clone()
                } else {
                    run_benchmark("../build/lime_gp_benchmark", &benchmark, timeout).await
                };
                entries.lock().unwrap().insert(benchmark, result);
            }
        }))
    }
    for handle in handles {
        handle.await.unwrap();
    }
    let entries = entries.lock().unwrap();
    write_to_file(
        &entries
            .iter()
            .map(|(benchmark, result)| Entry {
                benchmark: benchmark.clone(),
                result: result.clone(),
            })
            .collect::<Vec<_>>(),
    )
    .unwrap();
}

fn simdram_benchmarks() -> Vec<Benchmark> {
    let mut benchmarks = Vec::new();
    for benchmark in ["fa", "fs", "mux"] {
        benchmarks.push(Benchmark {
            arch: "simdram".to_string(),
            benchmark: benchmark.to_string(),
            candidate_selection: "all".to_string(),
            mode: "exhaustive".to_string(),
            rewriting_mode: "none".to_string(),
            rewriting_size_factor: 100,
        });
        for rewriting_mode in ["lp", "compiling", "greedy"] {
            benchmarks.push(Benchmark {
                arch: "simdram".to_string(),
                benchmark: benchmark.to_string(),
                candidate_selection: "all".to_string(),
                mode: "greedy".to_string(),
                rewriting_mode: rewriting_mode.to_string(),
                rewriting_size_factor: 100,
            });
        }
    }
    benchmarks
}

fn rewrite_benchmarks() -> Vec<Benchmark> {
    let mut benchmarks = Vec::new();
    for benchmark in BENCHMARKS {
        for arch in ARCHITECTURES {
            for mode in ["compiling", "lp", "greedy"] {
                benchmarks.push(Benchmark {
                    benchmark: benchmark.to_string(),
                    arch: arch.to_string(),
                    mode: "greedy".to_string(),
                    candidate_selection: "all".to_string(),
                    rewriting_mode: mode.to_string(),
                    rewriting_size_factor: 100,
                });
            }
        }
    }
    benchmarks
}

fn compiler_benchmarks() -> Vec<Benchmark> {
    let mut benchmarks = Vec::new();
    for benchmark in BENCHMARKS {
        for arch in ARCHITECTURES {
            for mode in ["greedy", "exhaustive"] {
                for cand in ["all", "plim_compiler"] {
                    benchmarks.push(Benchmark {
                        benchmark: benchmark.to_string(),
                        arch: arch.to_string(),
                        mode: mode.to_string(),
                        candidate_selection: cand.to_string(),
                        rewriting_mode: "none".to_string(),
                        rewriting_size_factor: 0,
                    });
                }
            }
        }
    }
    benchmarks
}
