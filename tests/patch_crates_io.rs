mod common;

use cargo_test_macro::cargo_test;
use cargo_test_support::{main_file, project};

#[allow(deprecated)]
#[cargo_test]
fn patch_crates_io_invalid_dependency() {
    let manifest = r#"
        [package]
        name = "example"
        version = "0.1.0"
        authors = ["wycats@example.com"]

        [dependencies]
        asdf = "1.0"

        [package.metadata.patch.asdf]
        patches = [
            "test.patch"
        ]
    "#;
    let p = project()
        .file("Cargo.toml", manifest)
        .file("src/main.rs", &main_file(r#""i am foo""#, &[]))
        .file("test.patch", r#""#)
        .build();

    p.process(common::cargo_patch_exe())
        .with_stderr_contains(
            "Error: failed to select a version for the requirement [..]",
        )
        .with_stderr_contains("[..]asdf[..]")
        .with_status(1)
        .run();
}

#[allow(deprecated)]
#[cargo_test]
fn patch_crates_io_missing_patch() {
    let manifest = r#"
        [package]
        name = "example"
        version = "0.1.0"
        authors = ["wycats@example.com"]

        [dependencies]
        serde = "=1.0.110"

        [package.metadata.patch.serde]
        patches = [
            "test.patch"
        ]
    "#;
    let p = project()
        .file("Cargo.toml", manifest)
        .file("src/main.rs", &main_file(r#""i am foo""#, &[]))
        .build();

    p.process(common::cargo_patch_exe())
        .with_stderr("Error: Unable to find patch file with path: \"test.patch\"\n")
        .with_status(1)
        .run();
}

#[allow(deprecated)]
#[cargo_test]
fn patch_crates_io_invalid_patch() {
    let manifest = r#"
        [package]
        name = "example"
        version = "0.1.0"
        authors = ["wycats@example.com"]

        [dependencies]
        serde = "=1.0.110"

        [package.metadata.patch.serde]
        patches = [
            "test.patch"
        ]
    "#;
    let p = project()
        .file("Cargo.toml", manifest)
        .file("src/main.rs", &main_file(r#""i am foo""#, &[]))
        .file("test.patch", r#""#)
        .build();

    p.process(common::cargo_patch_exe())
        .with_stderr("Error: Unable to parse patch file\n")
        .with_status(1)
        .run();
}

#[allow(deprecated)]
#[cargo_test]
fn patch_crates_io_simple() {
    let manifest = r#"
        [package]
        name = "example"
        version = "0.1.0"
        authors = ["wycats@example.com"]

        [dependencies]
        serde = "=1.0.110"

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
        .file("Cargo.toml", manifest)
        .file("src/main.rs", &main_file(r#""i am foo""#, &[]))
        .file("test.patch", patch)
        .build();

    p.process(common::cargo_patch_exe())
        .cwd(p.root())
        .with_stdout("Patched serde: LICENSE-MIT\n")
        .run();

    let license_mit = p
        .build_dir()
        .join("patch")
        .join("serde-1.0.110")
        .join("LICENSE-MIT");
    let licenses =
        std::fs::read_to_string(license_mit).expect("Unable to read license file");
    assert!(licenses.contains("PATCHED"));
}

#[allow(deprecated)]
#[cargo_test]
fn patch_crates_io_detailed() {
    let manifest = r#"
        [package]
        name = "example"
        version = "0.1.0"
        authors = ["wycats@example.com"]

        [dependencies]
        serde = { version = "=1.0.110", features = ["derive"] }

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
        .file("Cargo.toml", manifest)
        .file("src/main.rs", &main_file(r#""i am foo""#, &[]))
        .file("test.patch", patch)
        .build();

    p.process(common::cargo_patch_exe())
        .cwd(p.root())
        .with_stdout("Patched serde: LICENSE-MIT\n")
        .run();

    let license_mit = p
        .build_dir()
        .join("patch")
        .join("serde-1.0.110")
        .join("LICENSE-MIT");
    let licenses =
        std::fs::read_to_string(license_mit).expect("Unable to read license file");
    assert!(licenses.contains("PATCHED"));
}

#[allow(deprecated)]
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
        serde = { version = "=1.0.110", features = ["derive"] }
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
        .file("Cargo.toml", manifest)
        .file("src/main.rs", &main_file(r#""i am foo""#, &[]))
        .file("test.patch", patch)
        .file("test/Cargo.toml", test_manifest)
        .file("test/src/main.rs", &main_file(r#""i am foo""#, &[]))
        .build();

    p.process(common::cargo_patch_exe())
        .with_stdout("Patched serde: LICENSE-MIT\n")
        .run();

    let license_mit = p
        .build_dir()
        .join("patch")
        .join("serde-1.0.110")
        .join("LICENSE-MIT");
    let licenses =
        std::fs::read_to_string(license_mit).expect("Unable to read license file");
    assert!(licenses.contains("PATCHED"));
}

#[allow(deprecated)]
#[cargo_test]
fn patch_git_workspace_metadata() {
    let manifest = r#"
        [workspace]
        members = ["test"]

        [workspace.metadata.patch.serde]
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
        serde = { version = "=1.0.110", features = ["derive"] }
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
        .file("Cargo.toml", manifest)
        .file("test.patch", patch)
        .file("test/Cargo.toml", test_manifest)
        .file("test/src/main.rs", &main_file(r#""i am foo""#, &[]))
        .build();

    p.process(common::cargo_patch_exe())
        .with_stdout("Patched serde: LICENSE-MIT\n")
        .run();

    let license_mit = p
        .build_dir()
        .join("patch")
        .join("serde-1.0.110")
        .join("LICENSE-MIT");
    let licenses =
        std::fs::read_to_string(license_mit).expect("Unable to read license file");
    assert!(licenses.contains("PATCHED"));
}
