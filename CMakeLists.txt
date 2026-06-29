cmake_minimum_required(VERSION 3.20)
project(curvepress VERSION 0.2.0 LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

# ── Header-only library target ────────────────────────────────────────────────
add_library(curvepress INTERFACE)
add_library(curvepress::curvepress ALIAS curvepress)

target_include_directories(curvepress INTERFACE
    $<BUILD_INTERFACE:${CMAKE_CURRENT_SOURCE_DIR}/include>
    $<INSTALL_INTERFACE:include>
)

# ── Install rules ─────────────────────────────────────────────────────────────
include(GNUInstallDirs)
install(TARGETS curvepress EXPORT curvepressTargets)
install(DIRECTORY include/ DESTINATION ${CMAKE_INSTALL_INCLUDEDIR})
install(EXPORT curvepressTargets
    FILE curvepressTargets.cmake
    NAMESPACE curvepress::
    DESTINATION ${CMAKE_INSTALL_LIBDIR}/cmake/curvepress
)

include(CMakePackageConfigHelpers)
write_basic_package_version_file(
    "${CMAKE_CURRENT_BINARY_DIR}/curvepressConfigVersion.cmake"
    VERSION ${PROJECT_VERSION}
    COMPATIBILITY AnyNewerVersion
)
configure_package_config_file(
    "${CMAKE_CURRENT_SOURCE_DIR}/cmake/curvepressConfig.cmake.in"
    "${CMAKE_CURRENT_BINARY_DIR}/curvepressConfig.cmake"
    INSTALL_DESTINATION ${CMAKE_INSTALL_LIBDIR}/cmake/curvepress
)
install(FILES
    "${CMAKE_CURRENT_BINARY_DIR}/curvepressConfig.cmake"
    "${CMAKE_CURRENT_BINARY_DIR}/curvepressConfigVersion.cmake"
    DESTINATION ${CMAKE_INSTALL_LIBDIR}/cmake/curvepress
)

# ── Optional Python extension (pybind11, driven by scikit-build-core) ─────────
option(CURVEPRESS_BUILD_PYTHON "Build pybind11 Python extension" OFF)

if(CURVEPRESS_BUILD_PYTHON)
    find_package(pybind11 REQUIRED)
    pybind11_add_module(_curvepress python/bindings.cpp)
    target_link_libraries(_curvepress PRIVATE curvepress::curvepress)
    install(TARGETS _curvepress DESTINATION curvepress)
endif()

# ── Optional GTest tests ──────────────────────────────────────────────────────
option(CURVEPRESS_BUILD_TESTS "Build GTest test suite" OFF)

if(CURVEPRESS_BUILD_TESTS)
    find_package(GTest REQUIRED)
    enable_testing()

    add_executable(curvepress_tests tests/cpp/test_cpp.cpp)
    target_link_libraries(curvepress_tests PRIVATE curvepress::curvepress GTest::gtest_main)
    target_compile_definitions(curvepress_tests PRIVATE
        CURVEPRESS_TEST_DATA_DIR="${CMAKE_CURRENT_SOURCE_DIR}/tests/test_data"
    )

    include(GoogleTest)
    gtest_discover_tests(curvepress_tests)
endif()
