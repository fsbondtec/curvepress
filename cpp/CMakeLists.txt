cmake_minimum_required(VERSION 3.20)
project(curvepress_cpp CXX)

set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

# ─── Locate Cargo ──────────────────────────────────────────────────────────────
find_program(CARGO cargo REQUIRED)

# ─── Build the Rust static library with the capi feature ──────────────────────
set(RUST_LIB_DIR "${CMAKE_SOURCE_DIR}/../target/release")

# Detect platform-specific static lib name.
if(WIN32)
    set(RUST_LIB "${RUST_LIB_DIR}/curvepress.lib")
else()
    set(RUST_LIB "${RUST_LIB_DIR}/libcurvepress.a")
endif()

add_custom_command(
    OUTPUT "${RUST_LIB}"
    COMMAND ${CARGO} build --release --manifest-path "${CMAKE_SOURCE_DIR}/../Cargo.toml"
            --features capi
    WORKING_DIRECTORY "${CMAKE_SOURCE_DIR}/.."
    COMMENT "Building Rust core with capi feature…"
    VERBATIM
)
add_custom_target(rust_core ALL DEPENDS "${RUST_LIB}")

# ─── Import the Rust static lib as an imported target ─────────────────────────
add_library(curvepress_rust STATIC IMPORTED GLOBAL)
set_target_properties(curvepress_rust PROPERTIES
    IMPORTED_LOCATION "${RUST_LIB}"
)
add_dependencies(curvepress_rust rust_core)

# ─── curvepress::curvepress interface target ───────────────────────────────────
add_library(curvepress_hpp INTERFACE)
add_library(curvepress::curvepress ALIAS curvepress_hpp)

target_include_directories(curvepress_hpp INTERFACE
    "${CMAKE_SOURCE_DIR}/include"               # cpp/include (curvepress.hpp)
    "${CMAKE_SOURCE_DIR}/../include"            # generated curvepress.h
)

target_link_libraries(curvepress_hpp INTERFACE curvepress_rust)

# Platform link requirements for the Rust static lib.
if(UNIX AND NOT APPLE)
    target_link_libraries(curvepress_hpp INTERFACE pthread dl)
elseif(APPLE)
    target_link_libraries(curvepress_hpp INTERFACE "-framework Security")
endif()

# ─── Catch2 tests (optional, skip if not found) ──────────────────────────────
find_package(Catch2 3 QUIET)
if(Catch2_FOUND)
    add_executable(test_cpp "${CMAKE_SOURCE_DIR}/../tests/cpp/test_cpp.cpp")
    target_link_libraries(test_cpp PRIVATE curvepress::curvepress Catch2::Catch2WithMain)
    include(CTest)
    include(Catch)
    catch_discover_tests(test_cpp)
endif()
