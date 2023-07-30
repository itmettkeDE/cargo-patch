#[allow(dead_code)]
pub fn cargo_patch_exe() -> std::path::PathBuf {
    std::env::var_os("CARGO_BIN_PATH")
        .map(std::path::PathBuf::from)
        .or_else(|| {
            std::env::current_exe().ok().map(|mut path| {
                path.pop();
                if path.ends_with("deps") {
                    path.pop();
                }
                path
            })
        })
        .unwrap_or_else(|| {
            panic!("CARGO_BIN_PATH wasn't set. Cannot continue running test")
        })
        .join(format!("cargo-patch{}", std::env::consts::EXE_SUFFIX))
}

#[allow(dead_code)]
pub fn cargo_patch_lib() -> String {
    std::env::var("CARGO_MANIFEST_DIR").expect("Missing CARGO_MANIFEST_DIR")
}

#[allow(dead_code)]
pub fn build_rs() -> &'static str {
    r#"
        fn main() {
            println!("cargo:rerun-if-changed=Cargo.toml");
            println!("cargo:rerun-if-changed=patches/");
            cargo_patch::patch().expect("Failed while patching");
        }
    "#
}
