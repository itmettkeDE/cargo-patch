//! `Cargo-Patch` is a Cargo Subcommand which allows
//! patching dependencies using patch files.
//!
//! # Installation
//!
//! Simply run:
//!
//! ```sh
//! cargo install cargo-patch
//! ```
//!
//! # Usage
//!
//! To patch a dependency one has to add the following
//! to `Cargo.toml`:
//!
//! ```toml
//! [package.metadata.patch.serde]
//! version = "1.0"
//! patches = [
//!     "test.patch"
//! ]
//! ```
//!
//! It specifies which dependency to patch (in this case
//! serde) and one or more patchfiles to apply. Running:
//!
//! ```sh
//! cargo patch
//! ```
//!
//! will download the serde package specified in the
//! dependency section to the `target/patch` folder
//! and apply the given patches. To use the patched
//! version one has to override the dependency using
//! `replace` like this
//!
//! ```toml
//! [patch.crates-io]
//! serde = { path = './target/patch/serde-1.0.110' }
//! ```
//!
//! # Patch format
//!
//! You can either use [diff](http://man7.org/linux/man-pages/man1/diff.1.html) or
//! [git](https://linux.die.net/man/1/git) to create patch files. Important is that
//! file paths are relativ and inside the dependency
//!
//! # Limitations
//!
//! Its only possible to patch dependencies of binary crates as it is not possible
//! for a subcommand to intercept the build process.
//!

#![warn(
    absolute_paths_not_starting_with_crate,
    anonymous_parameters,
    // box_pointers,
    deprecated_in_future,
    explicit_outlives_requirements,
    indirect_structural_match,
    keyword_idents,
    macro_use_extern_crate,
    meta_variable_misuse,
    missing_copy_implementations,
    missing_debug_implementations,
    missing_docs,
    missing_doc_code_examples,
    non_ascii_idents,
    private_doc_tests,
    single_use_lifetimes,
    trivial_casts,
    trivial_numeric_casts,
    unreachable_pub,
    unsafe_code,
    unstable_features,
    unused_extern_crates,
    unused_import_braces,
    unused_lifetimes,
    unused_qualifications,
    unused_results,
    variant_size_differences
)]
#![warn(
    clippy::correctness,
    clippy::restriction,
    clippy::style,
    clippy::pedantic,
    clippy::complexity,
    clippy::perf,
    clippy::cargo,
    clippy::nursery
)]
#![allow(
    clippy::implicit_return,
    clippy::missing_docs_in_private_items,
    clippy::result_expect_used,
    clippy::shadow_reuse,
    clippy::option_expect_used,
    clippy::similar_names,
    clippy::else_if_without_else,
    clippy::multiple_crate_versions,
    clippy::module_name_repetitions,
    clippy::print_stdout,
    clippy::integer_arithmetic
)]

use anyhow::Result;
use cargo::{
    core::{package::Package, shell::Verbosity, PackageId, Resolve, Workspace},
    ops::resolve_ws,
    util::{config::Config, important_paths::find_root_manifest_for_wd},
};
use failure::err_msg;
use fs_extra::dir::{copy, CopyOptions};
use patch::{Line, Patch};
use semver::VersionReq;
use std::{
    fs,
    io::ErrorKind,
    path::{Path, PathBuf},
};
use toml::value::Value;

#[derive(Debug, Clone)]
struct PatchEntry {
    name: String,
    version: Option<VersionReq>,
    patches: Vec<PathBuf>,
}

#[allow(clippy::wildcard_enum_match_arm)]
fn clear_patch_folder() -> Result<()> {
    match fs::remove_dir_all("target/patch") {
        Ok(_) => Ok(()),
        Err(err) => match err.kind() {
            ErrorKind::NotFound => Ok(()),
            _ => Err(err.into()),
        },
    }
}

fn setup_config() -> Result<Config> {
    let config = Config::default()?;
    config.shell().set_verbosity(Verbosity::Quiet);
    Ok(config)
}

fn find_cargo_toml(path: &Path) -> Result<PathBuf> {
    let path = fs::canonicalize(path)?;
    find_root_manifest_for_wd(&path)
}

fn fetch_workspace<'a>(config: &'a Config, path: &Path) -> Result<Workspace<'a>> {
    Workspace::new(path, config)
}

fn get_patches(package: &Package) -> Vec<PatchEntry> {
    let manifest = package.manifest();
    manifest
        .custom_metadata()
        .and_then(|v| v.get("patch"))
        .and_then(Value::as_table)
        .map_or_else(
            || vec![],
            |v| {
                v.iter()
                    .filter_map(|(k, v)| parse_patch_entry(k, v))
                    .collect()
            },
        )
}

fn parse_patch_entry(name: &str, entry: &Value) -> Option<PatchEntry> {
    let entry = match entry.as_table() {
        None => {
            eprintln!("Entry {} must contain a table.", name);
            return None;
        }
        Some(e) => e,
    };
    let version = entry.get("version").and_then(|e| {
        let value = e.as_str().and_then(|s| VersionReq::parse(s).ok());

        if value.is_none() {
            eprintln!("Version must be a value semver string: {}", e);
        }
        value
    });
    let patches = entry
        .get("patches")
        .and_then(Value::as_array)
        .map_or_else(|| vec![], |entries| parse_patches(entries));
    Some(PatchEntry {
        name: name.to_string(),
        version,
        patches,
    })
}

fn parse_patches(entries: &[Value]) -> Vec<PathBuf> {
    entries
        .iter()
        .filter_map(|e| {
            let value = e.as_str().map(PathBuf::from);
            if value.is_none() {
                eprintln!("Patch Entry must be a string: {}", e);
            }
            value
        })
        .collect()
}

fn get_ids(
    patches: Vec<PatchEntry>,
    resolve: &Resolve,
) -> Vec<(PatchEntry, PackageId)> {
    patches.into_iter().filter_map(|patch_entry| {
        let mut matched_dep = None;
        for dep in resolve.iter() {
            if dep.name().as_str() == patch_entry.name
                && patch_entry.version.as_ref().map_or(true, |ver| ver.matches(dep.version()))
            {
                if matched_dep.is_none() {
                    matched_dep = Some(dep);
                } else {
                    eprintln!("There are multiple versions of {} available. Try specifying a version.", patch_entry.name);
                }
            }
        }
        if matched_dep.is_none() {
            eprintln!("Unable to find package {} in dependencies", patch_entry.name);
        }
        matched_dep.map(|v| (patch_entry, v))
    }).collect()
}

fn copy_package(pkg: &Package) -> Result<PathBuf> {
    fs::create_dir_all("target/patch/")?;
    let options = CopyOptions::new();
    let _ = copy(pkg.root(), "target/patch/", &options)?;
    if let Some(name) = pkg.root().file_name() {
        let buf = PathBuf::from("target/patch/");
        let buf = buf.join(name).canonicalize()?;
        Ok(buf)
    } else {
        Err(err_msg("Dependency Folder does not have a name")
            .compat()
            .into())
    }
}

fn apply_patches(name: &str, patches: &[PathBuf], path: &PathBuf) -> Result<()> {
    for patch in patches {
        let data = read_to_string(patch)?;
        let patches = Patch::from_multiple(&data)
            .map_err(|_| err_msg("Unable to parse patch file").compat())?;
        for patch in patches {
            let file_path = path.clone();
            let file_path = file_path.join(patch.old.path.as_ref());
            let file_path = file_path.canonicalize()?;
            if file_path.starts_with(&path) {
                let data = read_to_string(&file_path)?;
                let data = apply_patch(patch, &data);
                fs::write(file_path, data)?;
                println!("Patched {}", name);
            } else {
                return Err(err_msg("Patch file tried to escape dependency folder")
                    .compat()
                    .into());
            }
        }
    }
    Ok(())
}

#[allow(
    clippy::as_conversions,
    clippy::indexing_slicing,
    clippy::cast_possible_truncation
)]
fn apply_patch(diff: Patch, old: &str) -> String {
    let old_lines = old.lines().collect::<Vec<&str>>();
    let mut out: Vec<&str> = vec![];
    let mut old_line = 0;
    for hunk in diff.hunks {
        while old_line < hunk.old_range.start - 1 {
            out.push(old_lines[old_line as usize]);
            old_line += 1;
        }
        old_line += hunk.old_range.count;
        for line in hunk.lines {
            match line {
                Line::Add(s) | Line::Context(s) => out.push(s),
                Line::Remove(_) => {}
            }
        }
    }
    for line in &old_lines[(old_line as usize)..] {
        out.push(line);
    }
    if old.ends_with('\n') {
        out.push("");
    }
    out.join("\n")
}

#[allow(clippy::wildcard_enum_match_arm)]
fn read_to_string(path: &PathBuf) -> Result<String> {
    match fs::read_to_string(path) {
        Ok(data) => Ok(data),
        Err(err) => match err.kind() {
            ErrorKind::NotFound => Err(err_msg(format!(
                "Unable to find patch file with path: {:?}",
                path
            ))
            .compat()
            .into()),
            _ => Err(err.into()),
        },
    }
}

fn main() -> Result<()> {
    clear_patch_folder()?;
    let config = setup_config()?;
    let _lock = config.acquire_package_cache_lock()?;
    let workspace_path = find_cargo_toml(&PathBuf::from("."))?;
    let workspace = fetch_workspace(&config, &workspace_path)?;
    let (pkg_set, resolve) = resolve_ws(&workspace)?;

    let mut patched = false;
    for member in workspace.members() {
        let patches = get_patches(member);
        let ids = get_ids(patches, &resolve);
        let packages = ids
            .into_iter()
            .map(|(p, id)| pkg_set.get_one(id).map(|v| (p, v)))
            .collect::<Result<Vec<(PatchEntry, &Package)>>>()?;
        for (patch, package) in packages {
            let path = copy_package(package)?;
            patched = true;
            apply_patches(&patch.name, &patch.patches, &path)?;
        }
    }
    if !patched {
        println!("No patches found")
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::apply_patch;
    use patch::Patch;

    #[test]
    fn apply_patch_simply() {
        let patch = r#"--- test	2020-05-21 08:50:06.629765310 +0200
+++ test	2020-05-21 08:50:19.689878523 +0200
@@ -1,6 +1,6 @@
 This is the first line
 
-This is the second line
+This is the patched line
 
 This is the third line
 
"#;
        let content = r#"This is the first line

This is the second line

This is the third line
"#;
        let patched = r#"This is the first line

This is the patched line

This is the third line
"#;
        let patch = Patch::from_single(patch).expect("Unable to parse patch");
        let test_patched = apply_patch(patch, content);
        assert_eq!(patched, test_patched, "Patched content does not match");
    }

    #[test]
    fn apply_patch_toml() {
        let patch = r#"--- test1	2020-05-22 17:30:38.119170176 +0200
+++ test2	2020-05-22 17:30:48.905935473 +0200
@@ -2,8 +2,7 @@
 adipiscing elit, sed do eiusmod tempor 
 incididunt ut labore et dolore magna 
 aliqua. Ut enim ad minim veniam, quis 
-nostrud exercitation ullamco laboris 
-nisi ut aliquip ex ea commodo consequat. 
+PATCHED
 Duis aute irure dolor in reprehenderit 
 in voluptate velit esse cillum dolore 
 eu fugiat nulla pariatur. Excepteur sint 
"#;
        let content = r#"Lorem ipsum dolor sit amet, consectetur 
adipiscing elit, sed do eiusmod tempor 
incididunt ut labore et dolore magna 
aliqua. Ut enim ad minim veniam, quis 
nostrud exercitation ullamco laboris 
nisi ut aliquip ex ea commodo consequat. 
Duis aute irure dolor in reprehenderit 
in voluptate velit esse cillum dolore 
eu fugiat nulla pariatur. Excepteur sint 
occaecat cupidatat non proident, sunt in 
culpa qui officia deserunt mollit anim 
id est laborum.
"#;
        let patched = r#"Lorem ipsum dolor sit amet, consectetur 
adipiscing elit, sed do eiusmod tempor 
incididunt ut labore et dolore magna 
aliqua. Ut enim ad minim veniam, quis 
PATCHED
Duis aute irure dolor in reprehenderit 
in voluptate velit esse cillum dolore 
eu fugiat nulla pariatur. Excepteur sint 
occaecat cupidatat non proident, sunt in 
culpa qui officia deserunt mollit anim 
id est laborum.
"#;
        let patch = Patch::from_single(patch).expect("Unable to parse patch");
        let test_patched = apply_patch(patch, content);
        assert_eq!(patched, test_patched, "Patched content does not match");
    }
}
