#include "utils.h"

#include <cstdint>
#include <mockturtle/algorithms/aig_resub.hpp>
#include <mockturtle/algorithms/cleanup.hpp>
#include <mockturtle/algorithms/cut_rewriting.hpp>
#include <mockturtle/algorithms/functional_reduction.hpp>
#include <mockturtle/algorithms/mig_algebraic_rewriting.hpp>
#include <mockturtle/algorithms/mig_inv_optimization.hpp>
#include <mockturtle/algorithms/mig_inv_propagation.hpp>
#include <mockturtle/algorithms/mig_resub.hpp>
#include <mockturtle/algorithms/node_resynthesis/exact.hpp>
#include <mockturtle/algorithms/node_resynthesis/mig_npn.hpp>
#include <mockturtle/algorithms/node_resynthesis/xag_npn.hpp>
#include <mockturtle/algorithms/resubstitution.hpp>
#include <mockturtle/algorithms/xag_resub.hpp>
#include <mockturtle/networks/aig.hpp>

using namespace mockturtle;

const uint64_t MAX_ITER = 100000;

void preoptimize_mig( mockturtle::mig_network& ntk )
{
  auto last_size = ntk.size() + 1;
  for ( auto i = 0; i < MAX_ITER && last_size > ntk.size(); i++ )
  {
    last_size = ntk.size();
    {
      depth_view depth_mig{ ntk };
      fanout_view fanout_mig{ depth_mig };
      resubstitution_params ps;
      resubstitution_stats st;

      mig_resubstitution2( fanout_mig, ps, &st );
      mig_resubstitution( fanout_mig, ps, &st );
      mig_inv_optimization( fanout_mig );
      functional_reduction( fanout_mig );

      mig_algebraic_depth_rewriting( depth_mig );
      ntk = cleanup_dangling( ntk );
    }
    {
      mig_npn_resynthesis resyn;
      cut_rewriting_params ps;
      ps.cut_enumeration_ps.cut_size = 4;
      ntk = cut_rewriting( ntk, resyn, ps );
      ntk = cleanup_dangling( ntk );
    }
  }
}

void preoptimize_aig( mockturtle::aig_network& ntk )
{
  auto last_size = ntk.size() + 1;
  for ( auto i = 0; i < MAX_ITER && last_size > ntk.size(); i++ )
  {
    last_size = ntk.size();
    {
      depth_view depth_mig{ ntk };
      fanout_view fanout_mig{ depth_mig };
      resubstitution_params ps;
      resubstitution_stats st;

      aig_resubstitution2( fanout_mig, ps, &st );
      aig_resubstitution( fanout_mig, ps, &st );
      functional_reduction( fanout_mig );

      ntk = cleanup_dangling( ntk );
    }
    {
      xag_npn_resynthesis<aig_network> resyn;
      cut_rewriting_params ps;
      ps.cut_enumeration_ps.cut_size = 4;
      ntk = cut_rewriting( ntk, resyn, ps );
      ntk = cleanup_dangling( ntk );
    }
  }
}

void preoptimize_xag( mockturtle::xag_network& ntk )
{
  auto last_size = ntk.size() + 1;
  for ( auto i = 0; i < MAX_ITER && last_size > ntk.size(); i++ )
  {
    last_size = ntk.size();
    {
      depth_view depth_mig{ ntk };
      fanout_view fanout_mig{ depth_mig };
      resubstitution_params ps;
      resubstitution_stats st;

      xag_resubstitution( fanout_mig, ps, &st );
      functional_reduction( fanout_mig );

      ntk = cleanup_dangling( ntk );
    }
    {
      xag_npn_resynthesis<xag_network> resyn;
      cut_rewriting_params ps;
      ps.cut_enumeration_ps.cut_size = 4;
      ntk = cut_rewriting( ntk, resyn, ps );
      ntk = cleanup_dangling( ntk );
    }
  }
}
