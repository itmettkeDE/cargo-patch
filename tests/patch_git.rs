use cargo_test_macro::cargo_test;
use cargo_test_support::{cargo_dir, main_file, project};
use std::env;

#[cargo_test]
fn patch_git_invalid_dependency() {
    let manifest = r#"
        [package]
        name = "example"
        version = "0.1.0"
        authors = ["wycats@example.com"]

        [dependencies]
        asdf = { git = "https://github.com/mettke/asdf.git" }

        [package.metadata.patch.asdf]
        patches = [
            "test.patch"
        ]
    "#;
    let p = project()
        .file("Cargo.toml", &manifest)
        .file("src/main.rs", &main_file(r#""i am foo""#, &[]))
        .file("test.patch", r#""#)
        .build();

    let patch_bin =
        cargo_dir().join(format!("cargo-patch{}", env::consts::EXE_SUFFIX));
    p.process(&patch_bin)
    .with_stderr_contains("Error: failed to get `asdf` as a dependency of package [..]")
        .with_status(1)
        .run();
}

#[cargo_test]
fn patch_git_missing_patch() {
    let manifest = r#"
        [package]
        name = "example"
        version = "0.1.0"
        authors = ["wycats@example.com"]

        [dependencies]
        serde = { git = "https://github.com/serde-rs/serde.git", tag = "v1.0.110" }

        [package.metadata.patch.serde]
        patches = [
            "test.patch"
        ]
    "#;
    let p = project()
        .file("Cargo.toml", &manifest)
        .file("src/main.rs", &main_file(r#""i am foo""#, &[]))
        .build();

    let patch_bin =
        cargo_dir().join(format!("cargo-patch{}", env::consts::EXE_SUFFIX));
    p.process(&patch_bin)
        .with_stderr("Error: Unable to find patch file with path: \"test.patch\"\n")
        .with_status(1)
        .run();
}

#[cargo_test]
fn patch_git_invalid_patch() {
    let manifest = r#"
        [package]
        name = "example"
        version = "0.1.0"
        authors = ["wycats@example.com"]

        [dependencies]
        serde = { git = "https://github.com/serde-rs/serde.git", tag = "v1.0.110" }

        [package.metadata.patch.serde]
        patches = [
            "test.patch"
        ]
    "#;
    let p = project()
        .file("Cargo.toml", &manifest)
        .file("src/main.rs", &main_file(r#""i am foo""#, &[]))
        .file("test.patch", r#""#)
        .build();

    let patch_bin =
        cargo_dir().join(format!("cargo-patch{}", env::consts::EXE_SUFFIX));
    p.process(&patch_bin)
        .with_stderr("Error: Unable to parse patch file\n")
        .with_status(1)
        .run();
}

#[cargo_test]
fn patch_git_detailed() {
    let manifest = r#"
        [package]
        name = "example"
        version = "0.1.0"
        authors = ["wycats@example.com"]

        [dependencies]
        serde = { git = "https://github.com/serde-rs/serde.git", tag = "v1.0.110" }

        [package.metadata.patch.serde]
        patches = [
            "test.patch"
        ]
    "#;
    let patch = r#"--- LICENSE-MIT	2020-05-20 18:44:09.709027472 +0200
+++ LICENSE-MIT	2020-05-20 18:58:46.253762666 +0200
@@ -8,9 +8,7 @@
 is furnished to do so, subject to the following
 conditions:
 
-The above copyright notice and this permission notice
-shall be included in all copies or substantial portions
-of the Software.
+PATCHED
 
 THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF
 ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED
"#;
    let p = project()
        .file("Cargo.toml", &manifest)
        .file("src/main.rs", &main_file(r#""i am foo""#, &[]))
        .file("test.patch", &patch)
        .build();

    let patch_bin =
        cargo_dir().join(format!("cargo-patch{}", env::consts::EXE_SUFFIX));
    p.process(&patch_bin).with_stdout("Patched serde\n").run();

    let license_mit = p
        .build_dir()
        .join("patch")
        .join("serde")
        .join("LICENSE-MIT");
    let licenes =
        std::fs::read_to_string(license_mit).expect("Unable to read license file");
    assert!(licenes.contains("PATCHED"));
}

#[cargo_test]
fn patch_git_workspace_root() {
    let manifest = r#"
        [package]
        name = "example"
        version = "0.1.0"
        authors = ["wycats@example.com"]

        [workspace]
        members = ["test"]

        [package.metadata.patch.serde]
        patches = [
            "test.patch"
        ]
    "#;
    let test_manifest = r#"
        [package]
        name = "example_test"
        version = "0.1.0"
        authors = ["wycats@example.com"]

        [dependencies]
        serde = { git = "https://github.com/serde-rs/serde.git", tag = "v1.0.110" }
    "#;
    let patch = r#"--- LICENSE-MIT	2020-05-20 18:44:09.709027472 +0200
+++ LICENSE-MIT	2020-05-20 18:58:46.253762666 +0200
@@ -8,9 +8,7 @@
 is furnished to do so, subject to the following
 conditions:
 
-The above copyright notice and this permission notice
-shall be included in all copies or substantial portions
-of the Software.
+PATCHED
 
 THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF
 ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED
"#;
    let p = project()
        .file("Cargo.toml", &manifest)
        .file("src/main.rs", &main_file(r#""i am foo""#, &[]))
        .file("test.patch", &patch)
        .file("test/Cargo.toml", &test_manifest)
        .file("test/src/main.rs", &main_file(r#""i am foo""#, &[]))
        .build();

    let patch_bin =
        cargo_dir().join(format!("cargo-patch{}", env::consts::EXE_SUFFIX));
    p.process(&patch_bin).with_stdout("Patched serde\n").run();

    let license_mit = p
        .build_dir()
        .join("patch")
        .join("serde")
        .join("LICENSE-MIT");
    let licenes =
        std::fs::read_to_string(license_mit).expect("Unable to read license file");
    assert!(licenes.contains("PATCHED"));
}
