#pragma once

#include "eggmock.h"
#include "utils.h"

#include <mockturtle/networks/mig.hpp>

#include <cstdint>
#include <cstring>
#include <string>
#include <utility>

extern "C"
{
  struct ambit_compiler_statistics
  {
    uint64_t egraph_classes;
    uint64_t egraph_nodes;
    uint64_t egraph_size;

    uint64_t instruction_count;

    uint64_t t_runner;
    uint64_t t_extractor;
    uint64_t t_compiler;

    const char* program_str;
  };

  struct ambit_compiler_settings
  {
    bool print_program;
    bool verbose;
    bool preoptimize = true;
    bool rewrite = true;
  };

  struct ambit_compiler_settings_ffi
  {
    bool print_program;
    bool verbose;
    bool rewrite = true;

    ambit_compiler_settings_ffi( ambit_compiler_settings s )
        : print_program( s.print_program ), verbose( s.verbose ), rewrite( s.rewrite ) {}
  };

  void ambit_free_program_string(char* ptr);

  eggmock::mig_receiver<ambit_compiler_statistics> ambit_compile_ffi(
      ambit_compiler_settings_ffi settings );
  eggmock::mig_receiver<ambit_compiler_statistics> ambit_rewrite_ffi(
      ambit_compiler_settings_ffi settings,
      eggmock::mig_receiver<void> receiver );
}

class ProgramString {
    char* ptr_ = nullptr;

public:
    ProgramString() = default;
    
    explicit ProgramString(char* ptr) : ptr_(ptr) {}
    
    ~ProgramString() {
        reset();
    }
    
    void reset() {
        if (ptr_) {
            ambit_free_program_string(ptr_);
            ptr_ = nullptr;
        }
    }
    
    std::string str() const {
        return ptr_ ? std::string(ptr_) : "";
    }
    
    explicit operator bool() const {
        return ptr_ != nullptr;
    }
    
    const char* c_str() const {
        return ptr_;
    }
    
    ProgramString(const ProgramString&) = delete;
    ProgramString& operator=(const ProgramString&) = delete;
    
    ProgramString(ProgramString&& other) noexcept : ptr_(other.ptr_) {
        other.ptr_ = nullptr;
    }
    
    ProgramString& operator=(ProgramString&& other) noexcept {
        if (this != &other) {
            reset();
            ptr_ = other.ptr_;
            other.ptr_ = nullptr;
        }
        return *this;
    }
};

inline std::pair<mockturtle::mig_network, ambit_compiler_statistics> ambit_rewrite(
    ambit_compiler_settings settings,
    mockturtle::mig_network& ntk,
    ProgramString& program_out )
{
  if ( settings.preoptimize )
  {
    preoptimize_mig( ntk );
  }
  mockturtle::mig_network out;
  auto stat = eggmock::send_mig(
      ntk, ambit_rewrite_ffi( settings, eggmock::receive_mig( out ) ) );
  program_out = ProgramString(const_cast<char*>(stat.program_str));
  stat.program_str = nullptr;
  return { out, stat };
}

inline ambit_compiler_statistics ambit_compile(
    ambit_compiler_settings settings,
    mockturtle::mig_network& ntk,
    ProgramString& program_out )
{
  if ( settings.preoptimize )
  {
    preoptimize_mig( ntk );
  }
  mockturtle::mig_network out;
  auto stat = eggmock::send_mig( ntk, ambit_compile_ffi( settings ) );
  program_out = ProgramString(const_cast<char*>(stat.program_str));
  stat.program_str = nullptr;
  return stat;
}
