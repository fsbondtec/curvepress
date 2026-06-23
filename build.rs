fn main() {
    #[cfg(feature = "capi")]
    generate_capi_header();
}

#[cfg(feature = "capi")]
fn generate_capi_header() {
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
