use std::{
    error::Error,
    time::{Duration, SystemTime},
};

use anyhow::Result;
use serde::{Deserialize, Serialize};
use tokio::process::Command;

pub const ARCHITECTURES: &[&str] = &["imply", "plim", "felix", "ambit"];
pub const BENCHMARKS: &[&str] = &[
    "fa",
    "add2",
    "add4",
    "mux1",
    "mux3",
    "mul2",
    "mul4",
    /*"gt",
    "kogge_stone",*/
    "ntk/ctrl.aig",
    "ntk/int2float.aig",
    "ntk/dec.aig",
    "ntk/router.aig",
    "ntk/cavlc.aig",
    "ntk/priority.aig",
];

#[derive(Clone, Debug, Serialize, Deserialize, Hash, PartialEq, Eq)]
pub struct Benchmark {
    pub benchmark: String,
    pub arch: String,
    pub mode: String,
    pub candidate_selection: String,
    pub rewriting_mode: String,
    pub rewriting_size_factor: usize,
}

#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct BenchmarkCmdLineResult {
    pub t_preopt: u64,
    pub n_nodes: u64,
    pub n_inputs: u64,
    pub n_outputs: u64,
    pub t_runner: u64,
    pub n_nodes_pre_trim: u64,
    pub t_trim: u64,
    pub n_nodes_post_trim: u64,
    pub t_extractor: u64,
    #[serde(default = "zero")]
    pub rebuilt_ntk_cost: f64,
    pub ntk_size: u64,
    pub t_compile: u64,
    pub t_cost: f64,
    pub num_cells: u64,
    pub num_instr: u64,
    pub validation_success: u8,
}

fn zero() -> f64 {
    0.0
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BenchmarkResult {
    pub result: Result<BenchmarkCmdLineResult, FailReason>,
    pub t_total: u64,
    pub stdout: String,
    pub stderr: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum FailReason {
    Infeasible,
    Timeout,
    Error(String),
    Other,
}

impl<E: Error> From<E> for FailReason {
    fn from(value: E) -> Self {
        Self::Error(format!("{value}"))
    }
}

pub async fn run_benchmark(
    exec: &'static str,
    benchmark: &Benchmark,
    timeout: Duration,
) -> BenchmarkResult {
    println!("running benchmark {benchmark:?}");
    let mut command = Command::new(exec);
    let start = SystemTime::now();
    let total = || start.elapsed().unwrap().as_millis() as u64;
    command
        .arg(&benchmark.benchmark)
        .arg(&benchmark.arch)
        .arg(&benchmark.mode)
        .arg(&benchmark.candidate_selection)
        .arg(&benchmark.rewriting_mode)
        .arg(benchmark.rewriting_size_factor.to_string())
        .kill_on_drop(true);
    let output = match tokio::time::timeout(timeout, command.output()).await {
        Ok(ok) => ok,
        Err(_timeout) => {
            return BenchmarkResult {
                result: Err(FailReason::Timeout),
                t_total: total(),
                stdout: Default::default(),
                stderr: Default::default(),
            };
        }
    };
    let output = match output {
        Err(error) => {
            return BenchmarkResult {
                result: Err(error.into()),
                t_total: total(),
                stdout: Default::default(),
                stderr: Default::default(),
            };
        }
        Ok(output) => output,
    };
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    BenchmarkResult {
        result: parse_output(&stdout, &stderr),
        t_total: total(),
        stdout,
        stderr,
    }
}

fn parse_output(stdout: &str, stderr: &str) -> Result<BenchmarkCmdLineResult, FailReason> {
    match stdout.lines().find(|line| line.starts_with("RESULTS")) {
        Some(results) => {
            let results = &results["RESULTS\t".len()..];
            let record: BenchmarkCmdLineResult = csv::ReaderBuilder::new()
                .delimiter(b'\t')
                .has_headers(false)
                .from_reader(results.as_bytes())
                .deserialize()
                .next()
                .unwrap()?;
            Ok(record)
        }
        None => {
            if stderr.contains("Infeasible") {
                Err(FailReason::Infeasible)
            } else {
                Err(FailReason::Other)
            }
        }
    }
}
