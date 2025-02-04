#include "eggmock.h"

#include <mockturtle/io/write_dot.hpp>
#include <mockturtle/networks/mig.hpp>

using namespace mockturtle;
using namespace eggmock;

extern "C"
{
  mig_receiver<mig_rewrite> ambit_rewriter();
}

int main()
{
  mig_network in;
  const auto b_i = in.create_pi();
  const auto b_i_next = in.create_pi();
  const auto m = in.create_pi();

  const auto O1 = in.create_and( m, b_i_next );
  const auto O2 = in.create_and( in.create_not( m ), b_i );
  const auto bi = in.create_or( O1, O2 );
  in.create_po( bi );

  write_dot( in, "in.dot" );
  mig_network rewritten = rewrite_mig( in, ambit_rewriter() );
  write_dot( rewritten, "out.dot" );
}