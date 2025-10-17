#![allow(dead_code)]

use std::env::args;

use rusqlite::{Connection, params};

use crate::db::read_from_file;

mod benchmark;
mod db;
mod utils;

fn main() {
    let conn = Connection::open("db.sqlite").unwrap();
    let data = read_from_file(args().nth(1).unwrap()).unwrap();

    conn.execute_batch(
        r#"
        drop table if exists data;
        create table if not exists data (
            benchmark text,
            arch text,
            mode text,
            candidate_selection text,
            rewriting_mode text,
            rewriting_size_factor integer,
            t_preopt integer,
            n_nodes integer,
            n_inputs integer,
            n_outputs integer,
            t_runner integer,
            n_nodes_pre_trim integer,
            t_trim integer,
            n_nodes_post_trim integer,
            t_extractor integer,
            rebuilt_ntk_cost real,
            ntk_size integer,
            t_compile integer,
            t_cost real,
            num_cells integer,
            num_instr integer,
            ok integer,
            t_total integer
        );
        "#,
    )
    .unwrap();
    for (benchmark, origre) in data {
        let (ok, result) = match origre.result {
            Err(_err) => (false, Default::default()),
            Ok(result) => (result.validation_success == 1, result),
        };
        conn.execute(
            "insert into data values (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23)",
            params![
                benchmark.benchmark,
                benchmark.arch,
                benchmark.mode,
                benchmark.candidate_selection,
                benchmark.rewriting_mode,
                benchmark.rewriting_size_factor,
                result.t_preopt,
                result.n_nodes,
                result.n_inputs,
                result.n_outputs,
                result.t_runner,
                result.n_nodes_pre_trim,
                result.t_trim,
                result.n_nodes_post_trim,
                result.t_extractor,
                result.rebuilt_ntk_cost,
                result.ntk_size,
                result.t_compile,
                result.t_cost,
                result.num_cells,
                result.num_instr,
                ok,
                origre.t_total,
            ]
        ).unwrap();
    }
    conn.close().unwrap();
}
