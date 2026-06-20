fn main() {
    // Only run cbindgen when the `capi` feature is active.
    if std::env::var("CARGO_FEATURE_CAPI").is_ok() {
        let crate_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
        let out_file = std::path::PathBuf::from(&crate_dir)
            .join("include")
            .join("curvepress.h");

        std::fs::create_dir_all(out_file.parent().unwrap()).unwrap();

        cbindgen::Builder::new()
            .with_crate(&crate_dir)
            .with_config(
                cbindgen::Config::from_file(
                    std::path::PathBuf::from(&crate_dir).join("cbindgen.toml"),
                )
                .unwrap_or_default(),
            )
            .generate()
            .expect("cbindgen failed")
            .write_to_file(&out_file);

        println!("cargo:rerun-if-changed=src/capi.rs");
        println!("cargo:rerun-if-changed=cbindgen.toml");
    }
}
