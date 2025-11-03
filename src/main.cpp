#include "ambit.h"
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
  // in.create_po( in.create_xor3( in.create_pi(), in.create_pi(), in.create_pi() ) );
  const auto lhs1 = in.create_pi();
  const auto rhs1 = in.create_pi();
  const auto rhs2 = in.create_pi();
  const auto rhs3 = in.create_pi();
  const auto cin1 = in.create_pi();
  const auto [s1, cout1] = mockturtle::full_adder(in, lhs1, rhs1, cin1);
  in.create_po(cout1);
  const auto cin2 = in.create_pi();
  const auto [s2, cout2] = mockturtle::full_adder(in, s1, rhs2, cin2);
  in.create_po(cout2);
  const auto cin3 = in.create_pi();
  const auto [s3, cout3] = mockturtle::full_adder(in, s2, rhs3, cin3);
  in.create_po(cout3);
  in.create_po(s3);
  preoptimize( in );

  ProgramStringGP program_str;
  auto result = send_ntk( in, receiver( gp_compile_ambit_with_program( compiler_settings{
  // auto result = send_ntk( in, receiver( gp_compile_ambit( compiler_settings{
                                        .rewriting = rewriting_strategy::none,
                                        .validator = new_validator( in ),
                                        .mode = compilation_mode::exhaustive,
                                        .candidate_selection = candidate_selection_mode::all,
                                    } ) ) );
  // if (result.program_str) {
  //   program_str = ProgramStringGP(const_cast<char*>(result.program_str));
  //   result.program_str = nullptr;
  // }
  // std::cout << "Generated program:\n" << program_str.str() << "\n";

  std::cout << "total cost: " << result.num_instr << std::endl;
  std::cout << "total cells: " << result.num_cells << std::endl;
}
