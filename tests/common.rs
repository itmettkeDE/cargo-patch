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
