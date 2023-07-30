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

#![deny(clippy::all, clippy::nursery)]
#![deny(nonstandard_style, rust_2018_idioms)]

use anyhow::{anyhow, Result};
use cargo::{
    core::{
        package::{Package, PackageSet},
        registry::PackageRegistry,
        resolver::{features::CliFeatures, HasDevUnits},
        shell::Verbosity,
        PackageId, Resolve, Workspace,
    },
    ops::{get_resolved_packages, load_pkg_lockfile, resolve_with_previous},
    util::{config::Config, important_paths::find_root_manifest_for_wd},
};
use fs_extra::dir::{copy, CopyOptions};
use patch::{Line, Patch};
use semver::VersionReq;
use std::fmt::{Display, Formatter};
use std::{
    fs,
    io::ErrorKind,
    path::{Path, PathBuf},
};
use toml::Value;

#[derive(Debug, Clone, Default)]
enum PatchSource {
    #[default]
    Default,
    GithubPrDiff,
}

#[derive(Debug, Clone)]
struct PatchItem<'a> {
    path: &'a Path,
    source: PatchSource,
}

#[derive(Debug, Clone)]
struct PatchEntry<'a> {
    name: &'a str,
    version: Option<VersionReq>,
    patches: Vec<PatchItem<'a>>,
}

#[derive(Debug)]
struct PatchFailed {
    line: u64,
    file: PathBuf,
}

#[derive(Debug, Eq, PartialEq)]
enum PatchType {
    Modify,
    Create,
    Delete,
}

impl PatchSource {
    fn from_str(s: &str) -> Self {
        match s {
            "Default" => Self::Default,
            "GithubPrDiff" => Self::GithubPrDiff,
            &_ => {
                eprintln!("Unknown patch source: {s}");
                Self::Default
            }
        }
    }
}

impl std::error::Error for PatchFailed {}

impl Display for PatchFailed {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "failed to apply patch to {} on line {}",
            self.file.display(),
            self.line + 1
        )
    }
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

fn resolve_ws<'a>(ws: &Workspace<'a>) -> Result<(PackageSet<'a>, Resolve)> {
    let mut registry = PackageRegistry::new(ws.config())?;
    registry.lock_patches();
    let resolve = {
        let prev = load_pkg_lockfile(ws)?;
        let resolve: Resolve = resolve_with_previous(
            &mut registry,
            ws,
            &CliFeatures::new_all(true),
            HasDevUnits::No,
            prev.as_ref(),
            None,
            &[],
            false,
        )?;
        resolve
    };
    let packages = get_resolved_packages(&resolve, registry)?;
    Ok((packages, resolve))
}

fn get_patches(
    custom_metadata: &Value,
) -> impl Iterator<Item = PatchEntry<'_>> + '_ {
    custom_metadata
        .as_table()
        .and_then(|table| table.get("patch"))
        .into_iter()
        .flat_map(|patch| patch.as_table().into_iter())
        .flat_map(|table| {
            table
                .into_iter()
                .filter_map(|(k, v)| parse_patch_entry(k, v))
        })
}

fn parse_patch_entry<'a>(name: &'a str, entry: &'a Value) -> Option<PatchEntry<'a>> {
    let entry = entry.as_table().or_else(|| {
        eprintln!("Entry {name} must contain a table.");
        None
    })?;

    let version = entry.get("version").and_then(|version| {
        let value = version.as_str().and_then(|s| VersionReq::parse(s).ok());
        if value.is_none() {
            eprintln!("Version must be a value semver string: {version}");
        }
        value
    });

    let patches = entry
        .get("patches")
        .and_then(Value::as_array)
        .into_iter()
        .flat_map(|patches| {
            patches.iter().flat_map(|patch| {
                let item = if patch.is_str() {
                    Some((patch.as_str(), Default::default()))
                } else {
                    patch.as_table().map(
                        |it| (
                            it.get("path").and_then(Value::as_str),
                            it.get("source").and_then(Value::as_str)
                              .map_or_else(Default::default, PatchSource::from_str)
                        ))
                };

                let (path, source) = if let Some(item) = item {item } else {
                    eprintln!("Patch Entry must be a string or a table with path and source: {patch}");
                    return None;
                };

                let path = path.map(Path::new);
                let path = if let Some(path) = path {
                    path
                } else {
                    eprintln!("Patch Entry must be a string or a table with path and source: {patch}");
                    return None;
                };

                Some(PatchItem {
                    path,
                    source,
                })
            })
        })
        .collect();

    Some(PatchEntry {
        name,
        version,
        patches,
    })
}

fn get_id(
    name: &str,
    version: &Option<VersionReq>,
    resolve: &Resolve,
) -> Option<PackageId> {
    let mut matched_dep = None;
    for dep in resolve.iter() {
        if dep.name().as_str() == name
            && version
                .as_ref()
                .map_or(true, |ver| ver.matches(dep.version()))
        {
            if matched_dep.is_none() {
                matched_dep = Some(dep);
            } else {
                eprintln!("There are multiple versions of {name} available. Try specifying a version.");
            }
        }
    }
    if matched_dep.is_none() {
        eprintln!("Unable to find package {name} in dependencies");
    }
    matched_dep
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
        Err(anyhow!("Dependency Folder does not have a name"))
    }
}

fn do_patch(
    diff: Patch<'_>,
    old_path: Option<PathBuf>,
    new_path: Option<PathBuf>,
) -> Result<PatchType> {
    // delete
    if new_path.is_none() {
        if let Some(old) = old_path {
            fs::remove_file(old)?;
            return Ok(PatchType::Delete);
        }
        return Err(anyhow!("Both old and new file are all empty."));
    }
    let new_path = new_path.unwrap();

    let (old_data, patch_type) = if let Some(old) = old_path {
        // modify
        (fs::read_to_string(old)?, PatchType::Modify)
    } else {
        // create
        ("".to_string(), PatchType::Create)
    };

    let data =
        apply_patch(diff, &old_data).map_err(|line| PatchFailed {
            file: PathBuf::from(new_path.to_owned().file_name().map_or_else(
                || "".to_string(),
                |it| it.to_string_lossy().to_string(),
            )),
            line,
        })?;

    if patch_type == PatchType::Create {
        if let Some(parent) = new_path.parent() {
            fs::create_dir_all(parent)?;
        }
    }
    fs::write(&new_path, data)?;

    Ok(patch_type)
}

fn apply_patches<'a>(
    name: &str,
    patches: impl Iterator<Item = PatchItem<'a>> + 'a,
    path: &Path,
) -> Result<()> {
    for PatchItem {
        path: patch,
        source,
    } in patches
    {
        let data = read_to_string(patch)?;
        let patches = Patch::from_multiple(&data)
            .map_err(|_| anyhow!("Unable to parse patch file"))?;
        for patch in patches {
            fn check_path<P: AsRef<Path>>(
                base: &Path,
                path: P,
                loc: &str,
            ) -> Result<PathBuf> {
                let path = base.join(path);
                let canonicalize_result = path.canonicalize();

                if canonicalize_result.is_err()
                    && path.to_string_lossy().contains("..")
                {
                    return Err(anyhow!(
                        "Failed to canonicalize path and the path has .. in it. ({loc})",
                    ));
                } else if canonicalize_result.is_err() {
                    return Ok(path);
                }

                if canonicalize_result?.strip_prefix(base).is_err() {
                    return Err(anyhow!(
                        "Patch file tried to escape dependency folder ({loc})",
                    ));
                }

                Ok(path)
            }

            let (old_path, new_path) = match source {
                PatchSource::Default => {
                    (patch.old.path.as_ref(), patch.new.path.as_ref())
                }
                PatchSource::GithubPrDiff => (
                    patch
                        .old
                        .path
                        .strip_prefix("a/")
                        .unwrap_or_else(|| patch.old.path.as_ref()),
                    patch
                        .new
                        .path
                        .strip_prefix("b/")
                        .unwrap_or_else(|| patch.new.path.as_ref()),
                ),
            };

            let loc = format!("{name}: {old_path} -> {new_path}");
            let loc_simple = format!("{name}: {old_path}");

            let new_file_path = check_path(path, new_path, &loc);
            let old_file_path = check_path(path, old_path, &loc);

            let new_file_path = if patch.new.path == "/dev/null" {
                None
            } else {
                Some(new_file_path?)
            };
            let old_file_path = if patch.old.path == "/dev/null" {
                None
            } else {
                Some(old_file_path?)
            };

            let patch_type = do_patch(patch, old_file_path, new_file_path)?;

            let loc = match patch_type {
                PatchType::Modify => loc_simple,
                PatchType::Create | PatchType::Delete => loc,
            };
            println!("Patched {loc}");
        }
    }
    Ok(())
}

/// Apply a patch to the given text.
/// If the apply fails (i.e. due to mismatch in context lines), returns an Err with the line number
/// it failed on (0-based).
#[allow(
    clippy::as_conversions,
    clippy::indexing_slicing,
    clippy::cast_possible_truncation
)]
fn apply_patch(diff: Patch<'_>, old: &str) -> Result<String, u64> {
    let old_lines = old.lines().collect::<Vec<&str>>();
    let mut out: Vec<&str> = vec![];
    let mut old_line = 0;
    for hunk in diff.hunks {
        while hunk.old_range.start != 0 && old_line < hunk.old_range.start - 1 {
            out.push(old_lines[old_line as usize]);
            old_line += 1;
        }
        for line in hunk.lines {
            match line {
                Line::Context(line) => {
                    let old = old_lines.get(old_line as usize);
                    if old != Some(&line) {
                        return Err(old_line);
                    }
                    if (old_line as usize) < old_lines.len() {
                        out.push(line);
                    }
                    old_line += 1;
                }
                Line::Add(s) => out.push(s),
                Line::Remove(line) => {
                    if old_lines[old_line as usize] != line {
                        return Err(old_line);
                    }
                    old_line += 1;
                }
            }
        }
    }
    for line in old_lines.get((old_line as usize)..).unwrap_or(&[]) {
        out.push(line);
    }
    if old.ends_with('\n') {
        out.push("");
    }
    Ok(out.join("\n"))
}

#[allow(clippy::wildcard_enum_match_arm)]
fn read_to_string(path: &Path) -> Result<String> {
    match fs::read_to_string(path) {
        Ok(data) => Ok(data),
        Err(err) => match err.kind() {
            ErrorKind::NotFound => {
                Err(anyhow!("Unable to find patch file with path: {:?}", path))
            }
            _ => Err(err.into()),
        },
    }
}

pub fn patch() -> Result<()> {
    clear_patch_folder()?;
    let config = setup_config()?;
    let _lock = config.acquire_package_cache_lock()?;
    let workspace_path = find_cargo_toml(&PathBuf::from("."))?;
    let workspace = fetch_workspace(&config, &workspace_path)?;
    let (pkg_set, resolve) = resolve_ws(&workspace)?;

    let custom_metadata = workspace.custom_metadata().into_iter().chain(
        workspace
            .members()
            .flat_map(|member| member.manifest().custom_metadata()),
    );

    let patches = custom_metadata.flat_map(get_patches);
    let ids = patches.flat_map(|patch| {
        get_id(patch.name, &patch.version, &resolve).map(|id| (patch, id))
    });

    let mut patched = false;

    for (patch, id) in ids {
        let package = pkg_set.get_one(id)?;
        let path = copy_package(package)?;
        patched = true;
        apply_patches(patch.name, patch.patches.into_iter(), &path)?;
    }

    if !patched {
        println!("No patches found");
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
        let test_patched =
            apply_patch(patch, content).expect("Failed to apply patch");
        assert_eq!(patched, test_patched, "Patched content does not match");
    }

    #[test]
    fn apply_patch_middle() {
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
        let test_patched =
            apply_patch(patch, content).expect("Failed to apply patch");
        assert_eq!(patched, test_patched, "Patched content does not match");
    }

    #[test]
    fn apply_patch_no_context_override() {
        let patch = r#"--- test        2020-06-06 10:06:44.375560000 +0200
+++ test2       2020-06-06 10:06:49.245635957 +0200
@@ -1,3 +1,3 @@
 test5
-test2
+test4
 test3
"#;
        let content = r#"test1
test2
test3
"#;
        let patch = Patch::from_single(patch).expect("Unable to parse patch");
        assert_eq!(apply_patch(patch, content), Err(0)); // first line context doesn't match
    }
}
