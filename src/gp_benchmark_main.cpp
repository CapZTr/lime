#include "gp.h"
#include "utils.h"
#include <chrono>
#include <iostream>
#include <mockturtle/networks/aig.hpp>
#include <mockturtle/networks/mig.hpp>
#include <mockturtle/networks/xag.hpp>
#include <string>

template<class ntk_t>
int run_benchmark(
    std::string const& benchmark, compiler_settings settings,
    eggmock::receiver_ffi<compiler_statistics> ( *compile )( compiler_settings ) )
{
  auto opt_ntk = get_ntk<ntk_t>( benchmark );
  if ( !opt_ntk )
  {
    std::cerr << "invalid benchmark" << std::endl;
    return 1;
  }
  auto ntk = *opt_ntk;
  auto t_start = std::chrono::high_resolution_clock::now();
  preoptimize<ntk_t>( ntk );
  auto t_preoptimize = std::chrono::high_resolution_clock::now() - t_start;
  std::cerr << "preoptimize done" << std::endl;
  settings.validator = new_validator<ntk_t>( ntk );
  compiler_statistics stat = eggmock::send_ntk( ntk, eggmock::receiver( compile( settings ) ) );
  std::cerr << "done" << std::endl;

  std::cout << "RESULTS\t"
            << std::chrono::duration_cast<std::chrono::milliseconds>( t_preoptimize ).count() << "\t"
            << ntk.size() << "\t"
            << ntk.num_pis() << "\t"
            << ntk.num_pos() << "\t"
            << stat.rewrite.t_runner << "\t"
            << stat.rewrite.n_nodes_pre_trim << "\t"
            << stat.rewrite.t_trim << "\t"
            << stat.rewrite.n_nodes_post_trim << "\t"
            << stat.rewrite.t_extractor << "\t"
            << stat.rewrite.rebuilt_ntk_cost << "\t"
            << stat.ntk_size << "\t"
            << stat.t_compile << "\t"
            << stat.cost << "\t"
            << stat.num_cells << "\t"
            << stat.num_instr << "\t"
            << stat.validation_success << std::endl;

  return 0;
}

int main( int argc, char** argv )
{
  // exec
  //  <benchmark>
  //  <arch:                imply / plim / felix / ambit>
  //  <mode:                greedy / exhaustive>
  //  <candidate selection: all / plim_compiler>
  //  <rewriting mode:      none / compiling / lp / greedy>
  //  <rewriting size factor>

  if ( argc != 7 )
  {
    std::cerr << "usage: " << argv[0]
              << "  <benchmark>\n"
              << "  <arch:                imply / plim / felix / ambit>\n"
              << "  <mode:                greedy / exhaustive>\n"
              << "  <candidate selection: all / plim_compiler>\n"
              << "  <rewriting mode:      none / compiling / lp / greedy>\n"
              << "  <rewriting size factor>" << std::endl;
    return 1;
  }

  std::string benchmark = argv[1];
  std::string arch = argv[2];
  std::string mode = argv[3];
  std::string candsel = argv[4];
  std::string rewriting = argv[5];
  std::string rewriting_size_factor_str = argv[6];

  compiler_settings settings;
  if ( mode == "greedy" )
  {
    settings.mode = compilation_mode::greedy;
  }
  else if ( mode == "exhaustive" )
  {
    settings.mode = compilation_mode::exhaustive;
  }
  else
  {
    std::cerr << "invalid mode" << std::endl;
    return 1;
  }

  if ( candsel == "all" )
  {
    settings.candidate_selection = candidate_selection_mode::all;
  }
  else if ( candsel == "plim_compiler" )
  {
    settings.candidate_selection = candidate_selection_mode::mig_based_compiler;
  }
  else
  {
    std::cerr << "invalid candidate selection strategy" << std::endl;
    return 1;
  }

  if ( rewriting == "none" )
  {
    settings.rewriting = rewriting_strategy::none;
  }
  else if ( rewriting == "compiling" )
  {
    settings.rewriting = rewriting_strategy::compiling;
  }
  else if ( rewriting == "compiling_memusage" )
  {
    settings.rewriting = rewriting_strategy::compiling_memusage;
  }
  else if ( rewriting == "lp" )
  {
    settings.rewriting = rewriting_strategy::lp;
  }
  else if ( rewriting == "greedy" )
  {
    settings.rewriting = rewriting_strategy::greedy_estimate;
  }
  else
  {
    std::cerr << "invalid rewriting strategy" << std::endl;
    return 1;
  }

  settings.rewriting_size_factor = std::stoull( rewriting_size_factor_str );

  compiler_statistics stat;
  if ( arch == "imply" )
  {
    return run_benchmark<mockturtle::aig_network>( benchmark, settings, gp_compile_imply );
  }
  else if ( arch == "plim" )
  {
    return run_benchmark<mockturtle::mig_network>( benchmark, settings, gp_compile_plim );
  }
  else if ( arch == "felix" )
  {
    return run_benchmark<mockturtle::xag_network>( benchmark, settings, gp_compile_felix );
  }
  else if ( arch == "ambit" )
  {
    return run_benchmark<mockturtle::mig_network>( benchmark, settings, gp_compile_ambit );
  }
  else if ( arch == "simdram" )
  {
    return run_benchmark<mockturtle::mig_network>( benchmark, settings, gp_compile_simdram );
  }
}
