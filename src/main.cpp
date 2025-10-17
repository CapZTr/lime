#include "gp.h"
#include "utils.h"

#include <cmath>
#include <mockturtle/algorithms/equivalence_checking.hpp>
#include <mockturtle/algorithms/miter.hpp>
#include <mockturtle/generators/arithmetic.hpp>
#include <mockturtle/io/write_dot.hpp>
#include <mockturtle/networks/aig.hpp>
#include <mockturtle/networks/mig.hpp>
#include <vector>

using namespace mockturtle;
using namespace eggmock;

using ntk_t = mig_network;

int main()
{
  ntk_t in;
  in.create_po( in.create_xor3( in.create_pi(), in.create_pi(), in.create_pi() ) );
  preoptimize( in );

  const auto result = send_ntk( in, receiver( gp_compile_ambit( compiler_settings{
                                        .rewriting = rewriting_strategy::none,
                                        .validator = new_validator( in ),
                                        .mode = compilation_mode::exhaustive,
                                        .candidate_selection = candidate_selection_mode::all,
                                    } ) ) );

  std::cout << "total cost: " << result.num_instr << std::endl;
  std::cout << "total cells: " << result.num_cells << std::endl;
}
