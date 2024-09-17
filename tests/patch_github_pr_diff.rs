use cargo_test_macro::cargo_test;
use cargo_test_support::{main_file, project, Execs, Project};

mod common;

fn gen_execs(patch: &str) -> (Execs, Project) {
    static MANIFEST: &str = r#"
[package]
name = "example"
version = "0.1.0"
authors = ["empty"]

[dependencies]
serde = { git = "https://github.com/serde-rs/serde.git", tag = "v1.0.110" }

[workspace.metadata.patch.serde]
patches = [{ path = "test.patch", source = "GithubPrDiff" }]
"#;

    let p = project()
        .file("Cargo.toml", MANIFEST)
        .file("src/main.rs", &main_file(r#""i am foo""#, &[]))
        .file("test.patch", patch)
        .build();

    (p.process(common::cargo_patch_exe()), p)
}

#[allow(deprecated)]
#[cargo_test]
fn patch_file() {
    let (mut e, p) = gen_execs(
        r#"--- a/LICENSE-MIT	2020-05-20 18:44:09.709027472 +0200
+++ b/LICENSE-MIT	2020-05-20 18:58:46.253762666 +0200
@@ -8,9 +8,7 @@ Patch license
 is furnished to do so, subject to the following
 conditions:

-The above copyright notice and this permission notice
-shall be included in all copies or substantial portions
-of the Software.
+PATCHED

 THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF
 ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED
"#
        .replace("\n\n", "\n \n") // ide is deleting the prefix space in empty line
        .as_str(),
    );

    e.with_stdout("Patched serde: LICENSE-MIT").run();

    let license_mit = p
        .build_dir()
        .join("patch")
        .join("serde")
        .join("LICENSE-MIT");
    let license =
        std::fs::read_to_string(license_mit).expect("Unable to read license file");
    assert!(license.contains("PATCHED"));
}
