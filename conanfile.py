import os

from conan import ConanFile
from conan.tools.cmake import CMake, CMakeToolchain, cmake_layout
from conan.tools.files import copy


class CurvepressConan(ConanFile):
    name = "curvepress"
    version = "0.2.0"
    license = "MIT"
    description = (
        "Lossy time-series compression: RDP / Visvalingam-Whyatt point reduction "
        "+ quantization + LEB-128 varint encoding. Header-only C++20 library."
    )
    homepage = "https://github.com/fsbondtec/curvepress"
    url = "https://github.com/conan-io/conan-center-index"
    topics = ("compression", "time-series", "rdp", "visvalingam", "quantization")

    # Header-only: no compiler/OS/arch settings needed for the library itself.
    package_type = "header-library"
    no_copy_source = True

    # Only the headers and the CMake wiring are distributed.
    exports_sources = (
        "include/*",
        "cmake/*",
        "CMakeLists.txt",
        "LICENSE",
    )

    def layout(self):
        cmake_layout(self, src_folder=".")

    def package_id(self):
        self.info.clear()

    def generate(self):
        tc = CMakeToolchain(self)
        tc.generate()

    def build(self):
        # Nothing to compile for a header-only library.
        pass

    def package(self):
        copy(self, "LICENSE",
             src=self.source_folder,
             dst=os.path.join(self.package_folder, "licenses"))
        copy(self, "*.hpp",
             src=os.path.join(self.source_folder, "include"),
             dst=os.path.join(self.package_folder, "include"))

    def package_info(self):
        # Header-only: no compiled library to link.
        self.cpp_info.bindirs = []
        self.cpp_info.libdirs = []
        self.cpp_info.set_property("cmake_target_name", "curvepress::curvepress")
        self.cpp_info.set_property("cmake_file_name", "curvepress")
