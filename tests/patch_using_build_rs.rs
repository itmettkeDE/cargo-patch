mod common;

use cargo_test_macro::cargo_test;
use cargo_test_support::{main_file, project};

#[cargo_test]
fn patch_using_build_rs() {
    let manifest = format!(
        r#"
        [package]
        name = "example"
        version = "0.1.0"
        authors = ["wycats@example.com"]

        [dependencies]
        memchr = "=2.5.0"

        [build-dependencies]
        cargo-patch = {{ path = "{cargo_patch_path}" }}

        [package.metadata.patch.memchr]
        patches = [
            "patches/test.patch"
        ]
    "#,
        cargo_patch_path = common::cargo_patch_lib()
    );
    let patch = r#"--- LICENSE-MIT	2023-07-30 15:38:00.598467733 +0200
+++ LICENSE-MIT	2023-07-30 15:39:01.284727222 +0200
@@ -9,8 +9,7 @@
 copies of the Software, and to permit persons to whom the Software is
 furnished to do so, subject to the following conditions:
 
-The above copyright notice and this permission notice shall be included in
-all copies or substantial portions of the Software.
+PATCHED
 
 THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
 IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
"#;
    let p = project()
        .file("Cargo.toml", &manifest)
        .file("src/main.rs", &main_file(r#""i am foo""#, &[]))
        .file("patches/test.patch", patch)
        .file("build.rs", common::build_rs())
        .build();

    p.process("cargo").arg("build").cwd(p.root()).run();

    let license_mit = p
        .build_dir()
        .join("patch")
        .join("memchr-2.5.0")
        .join("LICENSE-MIT");
    let licenes =
        std::fs::read_to_string(license_mit).expect("Unable to read license file");
    assert!(licenes.contains("PATCHED"));
}
