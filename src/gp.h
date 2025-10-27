#pragma once

#include <eggmock.hpp>

#include <mockturtle/networks/mig.hpp>

#include <cstdint>

extern "C"
{
  struct rewriting_statistics
  {
    uint64_t t_runner;
    uint64_t n_nodes_pre_trim;
    uint64_t t_trim;
    uint64_t n_nodes_post_trim;
    uint64_t t_extractor;
    double rebuilt_ntk_cost;
  };
  struct compiler_statistics
  {
    rewriting_statistics rewrite;
    uint64_t ntk_size;
    uint64_t t_compile;
    double cost;
    uint64_t num_cells;
    uint64_t num_instr;
    bool validation_success;
  };
  enum class rewriting_strategy
  {
    none,
    lp,
    compiling,
    compiling_memusage,
    greedy_estimate,
  };
  enum class compilation_mode
  {
    greedy,
    exhaustive,
  };
  enum class candidate_selection_mode
  {
    all,
    mig_based_compiler,
  };
  struct compiler_settings
  {
    rewriting_strategy rewriting;
    uint64_t rewriting_size_factor;
    eggmock::receiver_ffi<bool> validator;
    compilation_mode mode;
    candidate_selection_mode candidate_selection;
  };
}

extern "C"
{
  eggmock::receiver_ffi<compiler_statistics> gp_compile_ambit( compiler_settings settings );
  eggmock::receiver_ffi<compiler_statistics> gp_compile_simdram( compiler_settings settings );
  eggmock::receiver_ffi<compiler_statistics> gp_compile_imply( compiler_settings settings );
  eggmock::receiver_ffi<compiler_statistics> gp_compile_felix( compiler_settings settings );
  eggmock::receiver_ffi<compiler_statistics> gp_compile_plim( compiler_settings settings );
}
