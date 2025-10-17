# LIME

## Dependencies
- Up-to-date C++ and Rust toolchains
- COIN-OR CBC solver

## Reproducing Results
```
// Create binary
git submodule update --init --recursive
mkdir build
cd build
cmake .. -DCMAKE_BUILD_TYPE=Release
make lime_gp_benchmark

// Run benchmarks
cd ..
cd bench
cargo run --bin bench-script
// wait for out-TIMESTAMP.json to be created after benchmarks finish
```

## Interesting Files and Folders
- `src/gp_benchmark_main.cpp`: Benchmark entrypoint
- `crates/generic`: Main implementation
  - `crates/generic/mod.rs`: Rust-side optimization & code generation entrypoint
  - `crates/generic/definitions.rs`: Architecture defintions
- `crates/[generic-def | macros]`: ADL
