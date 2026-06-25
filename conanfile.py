import os

from conan import ConanFile
from conan.tools.files import copy


class CurvepressConan(ConanFile):
    name = "curvepress"
    version = "0.1.0"
    license = "MIT"
    description = "Lossy time series compression: RDP/VW point reduction + quantization"
    homepage = "https://github.com/fsbondtec/curvepress"
    topics = ("compression", "time-series", "rdp", "visvalingam", "quantization")
    settings = "os", "compiler", "build_type", "arch"
    package_type = "static-library"

    # Builds from source: the recipe compiles the Rust core via cargo, which also
    # generates the C header (build.rs -> cbindgen, capi feature). A Rust
    # toolchain (cargo) must be available on the build machine. Everything cargo
    # needs is exported into the recipe.
    exports_sources = (
        "Cargo.toml",
        "Cargo.lock",
        "build.rs",
        "cbindgen.toml",
        "src/*",
        "benches/*",  # Cargo.toml declares [[bench]]; cargo needs the file to parse
        "cpp/include/*",
    )

    def build(self):
        # capi feature: compiles src/capi.rs (the C ABI) and runs build.rs, which
        # writes include/curvepress.h via cbindgen.
        self.run("cargo build --release --features capi", cwd=self.source_folder)

    def package(self):
        # C++ wrapper header: cpp/include/curvepress/curvepress.hpp
        copy(self, "*.hpp",
             src=os.path.join(self.source_folder, "cpp", "include"),
             dst=os.path.join(self.package_folder, "include"))
        # Generated C header: include/curvepress.h
        copy(self, "curvepress.h",
             src=os.path.join(self.source_folder, "include"),
             dst=os.path.join(self.package_folder, "include"))
        # Compiled static library (name differs per platform).
        rust_out = os.path.join(self.source_folder, "target", "release")
        copy(self, "libcurvepress.a", src=rust_out,
             dst=os.path.join(self.package_folder, "lib"), keep_path=False)
        copy(self, "curvepress.lib", src=rust_out,
             dst=os.path.join(self.package_folder, "lib"), keep_path=False)

    def package_info(self):
        self.cpp_info.set_property("cmake_target_name", "curvepress::curvepress")
        self.cpp_info.libs = ["curvepress"]
        if self.settings.os == "Linux":
            self.cpp_info.system_libs = ["pthread", "dl", "m"]
        elif self.settings.os == "Macos":
            self.cpp_info.frameworks = ["Security", "CoreFoundation"]
        elif self.settings.os == "Windows":
            self.cpp_info.system_libs = ["ws2_32", "userenv", "ntdll", "bcrypt"]
