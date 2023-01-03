use cargo_test_macro::cargo_test;
use cargo_test_support::{main_file, project, Execs, Project};

mod common;

static TEST_CONTENT: &str = r#"first

second

third"#;

fn gen_execs(patch: &str) -> (Execs, Project) {
    static MANIFEST: &str = r#"
[package]
name = "example"
version = "0.1.0"
authors = ["empty"]

[dependencies]
serde = { git = "https://github.com/serde-rs/serde.git", tag = "v1.0.110" }

[workspace.metadata.patch.serde]
patches = ["test.patch"]
"#;

    let p = project()
        .file("Cargo.toml", MANIFEST)
        .file("src/main.rs", &main_file(r#""i am foo""#, &[]))
        .file("test.patch", patch)
        .build();

    (p.process(common::cargo_patch_exe()), p)
}

#[cargo_test]
fn patch_create_file() {
    let (mut e, p) = gen_execs(
        r#"--- /dev/null
+++ test.txt
@@ -0,0 +1,5 @@
+first
+
+second
+
+third
"#,
    );

    e.with_stdout("Patched serde: /dev/null -> test.txt").run();

    let file = p.build_dir().join("patch").join("serde").join("test.txt");

    let content = std::fs::read_to_string(file).expect("Unable to read test file");
    assert_eq!(content.as_str(), TEST_CONTENT);
}

#[cargo_test]
fn patch_delete_file() {
    let (mut e, p) = gen_execs(
        r#"--- /dev/null
+++ test.txt
@@ -0,0 +1,5 @@
+first
+
+second
+
+third
--- test.txt
+++ /dev/null
@@ -1,5 +0,0 @@
-first
-
-second
-
-third
"#,
    );

    e.with_stdout(
        "Patched serde: /dev/null -> test.txt\nPatched serde: test.txt -> /dev/null",
    )
    .run();

    let file = p.build_dir().join("patch").join("serde").join("test.txt");
    assert!(!file.exists())
}

#[cargo_test]
fn patch_invalid_both_empty() {
    let (mut e, _) = gen_execs(
        r#"--- /dev/null
+++ /dev/null
@@ -0,0 +1,5 @@
+first
+
+second
+
+third
"#,
    );

    e.with_stderr("Error: Both old and new file are all empty.")
        .with_status(1)
        .run();
}
