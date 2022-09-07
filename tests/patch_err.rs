mod common;

use cargo_test_macro::cargo_test;
use cargo_test_support::{main_file, project};

const MANIFEST: &str = r#"
    [package]
    name = "example"
    version = "0.1.0"
    authors = ["wycats@example.com"]

    [dependencies]
    serde = { git = "https://github.com/serde-rs/serde.git", tag = "v1.0.110" }

    [workspace.metadata.patch.serde]
    patches = ["test.patch"]
"#;

#[cargo_test]
fn patch_context_mismatch() {
    let patch = r#"--- LICENSE-MIT      2020-05-20 18:44:09.709027472 +0200
+++ LICENSE-MIT 2020-05-20 18:58:46.253762666 +0200
@@ -8,9 +8,7 @@
 this line of context doesn't match
 neither does this one
 or this
-The above copyright notice and this permission notice
-shall be included in all copies or substantial portions
-of the Software.
+PATCHED
 
 THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF
 ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED
"#;
    let p = project()
        .file("Cargo.toml", MANIFEST)
        .file("src/main.rs", &main_file(r#""i am foo""#, &[]))
        .file("test.patch", patch)
        .build();

    p.process(&common::cargo_patch_exe())
        .with_stderr("Error: failed to apply patch to LICENSE-MIT on line 8")
        .with_status(1)
        .run();
}

#[cargo_test]
fn patch_deleted_mismatch() {
    let patch = r#"--- LICENSE-MIT      2020-05-20 18:44:09.709027472 +0200
+++ LICENSE-MIT 2020-05-20 18:58:46.253762666 +0200
@@ -8,9 +8,7 @@
 is furnished to do so, subject to the following
 conditions:
 
-The above copyright notice and this permission notice
-this is a line which doesn't match the source file
-therefore this patch should fail.
+PATCHED
 
 THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF
 ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED
"#;
    let p = project()
        .file("Cargo.toml", MANIFEST)
        .file("src/main.rs", &main_file(r#""i am foo""#, &[]))
        .file("test.patch", patch)
        .build();

    p.process(&common::cargo_patch_exe())
        .with_stderr("Error: failed to apply patch to LICENSE-MIT on line 12")
        .with_status(1)
        .run();
}
