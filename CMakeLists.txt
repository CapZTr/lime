cmake_minimum_required(VERSION 3.19)

project(lime)
set(CMAKE_EXPORT_COMPILE_COMMANDS ON)
set(CMAKE_CXX_STANDARD 20)

add_subdirectory(mockturtle)

include(FetchContent)
FetchContent_Declare(
        Corrosion
        GIT_REPOSITORY https://github.com/corrosion-rs/corrosion.git
        GIT_TAG v0.5
)
FetchContent_MakeAvailable(Corrosion)

corrosion_import_crate(MANIFEST_PATH rs/Cargo.toml)

add_library(lime_base src/utils.cpp)
target_link_libraries(lime_base PUBLIC mockturtle lime_rs)

add_executable(lime src/main.cpp)
target_link_libraries(lime PRIVATE lime_base)

# add_executable(lime_ambit_benchmark src/ambit_benchmark_main.cpp)
# target_link_libraries(lime_ambit_benchmark PRIVATE lime_base)
