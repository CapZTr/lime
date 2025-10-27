use std::{
    collections::HashMap,
    fs::File,
    io::{BufReader, BufWriter},
    path::Path,
    time::UNIX_EPOCH,
};

use serde::{Deserialize, Serialize};

use crate::benchmark::{Benchmark, BenchmarkResult};

#[derive(Debug, Serialize, Deserialize)]
pub struct Entry {
    pub benchmark: Benchmark,
    pub result: BenchmarkResult,
}

pub fn write_to_file(entries: &[Entry]) -> anyhow::Result<()> {
    let t = UNIX_EPOCH.elapsed().unwrap().as_secs();
    let file = File::create_new("out-".to_string() + &t.to_string() + ".json")?;
    let writer = BufWriter::new(file);
    serde_json::to_writer(writer, entries)?;
    Ok(())
}

pub fn read_from_file(
    path: impl AsRef<Path>,
) -> anyhow::Result<HashMap<Benchmark, BenchmarkResult>> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let entries: Vec<Entry> = serde_json::from_reader(reader)?;
    Ok(entries
        .into_iter()
        .map(|entry| (entry.benchmark, entry.result))
        .collect())
}
