use cargo_test_macro::cargo_test;
use cargo_test_support::{cargo_dir, main_file, project};
use std::env;

#[cargo_test]
fn patch_empty_no_config() {
    let p = project().build();

    let patch_bin =
        cargo_dir().join(format!("cargo-patch{}", env::consts::EXE_SUFFIX));
    p.process(&patch_bin)
        .with_stderr_contains("Error: failed to parse manifest at [..]")
        .with_status(1)
        .run();
}

#[cargo_test]
fn patch_empty_no_src() {
    let manifest = r#"
        [package]
        name = "example"
        version = "0.1.0"
        authors = ["wycats@example.com"]
    "#;
    let p = project().file("Cargo.toml", manifest).build();

    let patch_bin =
        cargo_dir().join(format!("cargo-patch{}", env::consts::EXE_SUFFIX));
    p.process(&patch_bin)
        .with_stderr_contains("Error: failed to parse manifest at [..]")
        .with_status(1)
        .run();
}

#[cargo_test]
fn patch_empty_simple() {
    let manifest = r#"
        [package]
        name = "example"
        version = "0.1.0"
        authors = ["wycats@example.com"]
    "#;
    let p = project()
        .file("Cargo.toml", manifest)
        .file("src/main.rs", &main_file(r#""i am foo""#, &[]))
        .build();

    let patch_bin =
        cargo_dir().join(format!("cargo-patch{}", env::consts::EXE_SUFFIX));
    p.process(&patch_bin)
        .with_stdout("No patches found\n")
        .run();
}

#[cargo_test]
fn patch_empty_missing_dependency() {
    let manifest = r#"
        [package]
        name = "example"
        version = "0.1.0"
        authors = ["wycats@example.com"]

        [package.metadata.patch.serde]
        patches = []
    "#;
    let p = project()
        .file("Cargo.toml", manifest)
        .file("src/main.rs", &main_file(r#""i am foo""#, &[]))
        .build();

    let patch_bin =
        cargo_dir().join(format!("cargo-patch{}", env::consts::EXE_SUFFIX));
    p.process(&patch_bin)
        .with_stderr("Unable to find package serde in dependencies\n")
        .run();
}
