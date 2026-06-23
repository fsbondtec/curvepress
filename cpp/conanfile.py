from conan import ConanFile
from conan.tools.cmake import CMake, cmake_layout


class CurvepressConan(ConanFile):
    name = "curvepress"
    version = "0.1.0"
    license = "MIT"
    description = "Lossy time series compression: RDP/VW point reduction + quantization"
    homepage = "https://github.com/fsbondtec/curvepress"
    topics = ("compression", "time-series", "rdp", "visvalingam")
    settings = "os", "compiler", "build_type", "arch"
    generators = "CMakeToolchain", "CMakeDeps"
    # No external Conan requires — Rust static lib has zero runtime deps.

    def layout(self):
        cmake_layout(self)

    def build(self):
        cmake = CMake(self)
        cmake.configure(build_script_folder=self.source_folder)
        cmake.build()

    def package(self):
        cmake = CMake(self)
        cmake.install()
        self.copy("*.hpp", dst="include/curvepress", src="include/curvepress")

    def package_info(self):
        self.cpp_info.set_property("cmake_target_name", "curvepress::curvepress")
        self.cpp_info.libs = ["curvepress"]
