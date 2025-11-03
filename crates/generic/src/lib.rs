pub mod compilation;
pub mod copy;
pub mod cost;
pub mod definitions;
pub mod egraph;
pub mod program;
mod test;
pub mod untyped_ntk;
mod utils;
pub mod validation;

use std::ffi::CString;
use std::os::raw::c_char;
use std::{ffi::c_double, rc::Rc, time::Instant};

use derive_more::Deref;
pub use eggmock;
use eggmock::{Gate, Network, Node, Receiver, ReceiverFFI};
use itertools::Itertools;
pub use lime_generic_def;
use lime_generic_def::{Architecture, Cell, CellType};
pub use lime_macros::define_generic_architecture;

use crate::{
    compilation::{CandidateSelection, CompilationMode, CompilationParameters, compile},
    copy::CopyGraph,
    cost::OperationCost,
    egraph::{RewritingStatistics, RewritingStrategy, rewriting_receiver},
    untyped_ntk::UntypedNetwork,
    validation::rebuild_network,
};

#[derive(Deref)]
pub struct ArchitectureMeta<CT> {
    #[deref]
    pub arch: Architecture<CT>,
    pub copy_graph: CopyGraph<CT>,
}

#[derive(Debug)]
#[repr(C)]
pub struct CompilerStatistics {
    pub rewrite: RewritingStatistics,
    pub ntk_size: u64,
    pub t_compile: u64,
    pub cost: c_double,
    pub num_cells: u64,
    pub num_instr: u64,
    pub validation_success: bool,
}

#[derive(Debug)]
pub struct CompilerResult {
    pub stats: CompilerStatistics,
    pub program: String,
}

#[repr(C)]
pub struct CompilerSettings {
    pub rewriting: RewritingStrategy,
    pub rewriting_size_factor: u64,
    pub validator: ReceiverFFI<'static, bool>,
    pub mode: CompilationMode,
    pub candidate_selector: CandidateSelection,
}

pub fn generic_compiler_entrypoint<CT: CellType, C: OperationCost<CT>>(
    arch: Architecture<CT>,
    cost: C,
    settings: CompilerSettings,
    disjunct_input_output: bool,
) -> impl Receiver<Gate = UntypedNetwork, Result = CompilerStatistics> {
    let arch = Rc::new(ArchitectureMeta {
        copy_graph: CopyGraph::build(&arch, &cost),
        arch,
    });
    rewriting_receiver(
        arch.clone(),
        settings.rewriting,
        settings.rewriting_size_factor as usize,
        settings.candidate_selector,
        settings.mode,
        cost.clone(),
        disjunct_input_output,
    )
    .map(move |(ntk, rewriting_statistics)| {
        let input_cells = get_input_cells(&arch, &ntk);
        // add false node to match mockturtle network count if unchanged
        let ntk_size = ntk.size() as u64 + (!ntk.contains(&Node::False)) as u64;
        let t_compile = Instant::now();
        let result = compile(CompilationParameters {
            arch,
            cost: cost.clone(),
            input_cells: input_cells.clone(),
            network: ntk,
            mode: settings.mode,
            candidate_selection: settings.candidate_selector,
            disjunct_input_output,
        })
        .expect("compiler should succeed");
        let t_compile = (Instant::now() - t_compile).as_millis() as u64;

        eprintln!("=== final program:");
        eprintln!("{}", result.program);

        eprintln!("=== output cells:");
        eprintln!("{}", result.outputs.iter().join("\n"));

        let validation_success =
            match rebuild_network(&result.program, &input_cells, &result.outputs) {
                Ok(ntk) => ntk.send(settings.validator.with_input()),
                Err(err) => {
                    eprintln!("could not rebuild network: {err:?}");
                    false
                }
            };

        let num_cells = result.program.num_cells() as u64;
        let cost = cost.program_cost(&result.program);
        let num_instr = result.program.instructions().count() as u64;

        CompilerStatistics {
            cost: cost.0,
            ntk_size,
            rewrite: rewriting_statistics,
            t_compile,
            num_cells,
            num_instr,
            validation_success,
        }
    })
}

pub fn generic_compiler_with_program<CT: CellType, C: OperationCost<CT>>(
    arch: Architecture<CT>,
    cost: C,
    settings: CompilerSettings,
    disjunct_input_output: bool,
) -> impl Receiver<Gate = UntypedNetwork, Result = CompilerResult> {
    let arch = Rc::new(ArchitectureMeta {
        copy_graph: CopyGraph::build(&arch, &cost),
        arch,
    });

    rewriting_receiver(
        arch.clone(),
        settings.rewriting,
        settings.rewriting_size_factor as usize,
        settings.candidate_selector,
        settings.mode,
        cost.clone(),
        disjunct_input_output,
    )
    .map(move |(ntk, rewriting_statistics)| {
        let input_cells = get_input_cells(&arch, &ntk);
        let ntk_size = ntk.size() as u64 + (!ntk.contains(&Node::False)) as u64;

        let t_compile = Instant::now();
        let result = compile(CompilationParameters {
            arch: arch.clone(),
            cost: cost.clone(),
            input_cells: input_cells.clone(),
            network: ntk,
            mode: settings.mode,
            candidate_selection: settings.candidate_selector,
            disjunct_input_output,
        })
        .expect("compiler should succeed");
        let t_compile = (Instant::now() - t_compile).as_millis() as u64;

        let program_string = result.program.to_string();

        eprintln!("=== final program:");
        eprintln!("{}", program_string);

        let validation_success =
            match rebuild_network(&result.program, &input_cells, &result.outputs) {
                Ok(ntk) => ntk.send(settings.validator.with_input()),
                Err(err) => {
                    println!("could not rebuild network: {err:?}");
                    false
                }
            };

        let num_cells = result.program.num_cells() as u64;
        let cost_val = cost.program_cost(&result.program);
        let num_instr = result.program.instructions().count() as u64;

        CompilerResult {
            stats: CompilerStatistics {
                cost: cost_val.0,
                ntk_size,
                rewrite: rewriting_statistics,
                t_compile,
                num_cells,
                num_instr,
                validation_success,
            },
            program: program_string,
        }
    })
}

fn get_input_cells<CT: CellType, G: Gate>(
    arch: &Architecture<CT>,
    ntk: &Network<G>,
) -> Vec<Cell<CT>> {
    let input_ct = arch
        .types()
        .iter()
        .find(|typ| typ.count().is_none())
        .expect("cell type with inifinite cells should be available");
    ntk.inputs()
        .iter()
        .enumerate()
        .map(|(i, _)| Cell::new(*input_ct, i as u32))
        .collect_vec()
}

#[repr(C)]
pub struct CompilerStatisticsFfi {
    pub rewrite: RewritingStatistics,
    pub ntk_size: u64,
    pub t_compile: u64,
    pub cost: c_double,
    pub num_cells: u64,
    pub num_instr: u64,
    pub validation_success: bool,
    pub program_str: *const c_char,
}

#[unsafe(no_mangle)]
pub extern "C" fn gp_free_program_string(ptr: *mut c_char) {
    if !ptr.is_null() {
        unsafe { let _ = CString::from_raw(ptr); }
    }
}

pub fn map_result_to_ffi(
    r: impl Receiver<Gate = UntypedNetwork, Result = CompilerResult> + 'static,
) -> impl Receiver<Gate = UntypedNetwork, Result = CompilerStatisticsFfi> {
    r.map(|res| {
        let cstr = CString::new(res.program).expect("CString conversion failed");
        let ptr = cstr.into_raw();
        CompilerStatisticsFfi {
            rewrite: res.stats.rewrite,
            ntk_size: res.stats.ntk_size,
            t_compile: res.stats.t_compile,
            cost: res.stats.cost,
            num_cells: res.stats.num_cells,
            num_instr: res.stats.num_instr,
            validation_success: res.stats.validation_success,
            program_str: ptr,
        }
    })
}
