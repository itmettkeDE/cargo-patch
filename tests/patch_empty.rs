mod common;

use cargo_test_macro::cargo_test;
use cargo_test_support::{main_file, project};

#[allow(deprecated)]
#[cargo_test]
fn patch_empty_no_config() {
    let p = project().build();

    p.process(common::cargo_patch_exe())
        .with_stderr_contains("Error: failed to parse manifest at [..]")
        .with_status(1)
        .run();
}

#[allow(deprecated)]
#[cargo_test]
fn patch_empty_no_src() {
    let manifest = r#"
        [package]
        name = "example"
        version = "0.1.0"
        authors = ["wycats@example.com"]
    "#;
    let p = project().file("Cargo.toml", manifest).build();

    p.process(common::cargo_patch_exe())
        .with_stderr_contains("Error: failed to parse manifest at [..]")
        .with_status(1)
        .run();
}

#[allow(deprecated)]
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

    p.process(common::cargo_patch_exe())
        .with_stdout("No patches found\n")
        .run();
}

#[allow(deprecated)]
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

    p.process(common::cargo_patch_exe())
        .with_stderr("Unable to find package serde in dependencies\n")
        .run();
}
