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
//! To patch a dependecy one has to add the following
//! to `Cargo.toml`:
//!
//! ```toml
//! [package.metadata.patch.serde]
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
//! # Features
//!
//! - [x] Patch dependencies from crates.io
//! - [ ] Patch dependencies from git url
//! - [ ] Handle Workspaces
//! - [x] Use error messages which noone understands
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
    core::{
        dependency::Dependency as CDep,
        package::{Package, PackageSet},
        shell::Verbosity,
        source::{Source, SourceMap},
        summary::Summary,
        GitReference, SourceId,
    },
    sources::{registry::RegistrySource, GitSource},
    util::config::Config,
};
use cargo_toml::{Dependency, DepsSet, Manifest};
use failure::err_msg;
use fs_extra::dir::{copy, CopyOptions};
use patch::{Line, Patch};
use serde::Deserialize;
use std::{
    collections::{HashMap, HashSet},
    fs,
    io::ErrorKind,
    path::PathBuf,
};

#[derive(Debug, Clone, Deserialize)]
struct PatchSection {
    patch: Option<HashMap<String, PatchEntry>>,
}

#[derive(Debug, Clone, Deserialize)]
struct PatchEntry {
    patches: Option<Vec<PathBuf>>,
}

enum DepType<'a> {
    CratesIO {
        name: &'a str,
        version: Option<&'a str>,
    },
    Git {
        name: &'a str,
        version: Option<&'a str>,
        url: &'a str,
        gref: GitReference,
    },
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

fn fetch_manifest() -> Result<Manifest<PatchSection>> {
    Manifest::from_path_with_metadata("./Cargo.toml")
        .map_err(|err| {
            err_msg(format!(
                "Cargo.toml not found or unable to parse. Error: {}",
                err
            ))
            .compat()
        })
        .map_err(Into::into)
}

fn setup_config() -> Result<Config> {
    let config = Config::default()?;
    config.shell().set_verbosity(Verbosity::Quiet);
    Ok(config)
}

fn get_ci_source(config: &Config) -> Result<SourceId> {
    SourceId::crates_io(config)
}

fn get_git_source(url: &str, gref: GitReference) -> Result<SourceId> {
    let url = url.parse()?;
    SourceId::for_git(&url, gref)
}

fn get_patches(
    manifest: &Manifest<PatchSection>,
) -> Option<&HashMap<String, PatchEntry>> {
    manifest
        .package
        .as_ref()
        .and_then(|p| p.metadata.as_ref())
        .and_then(|p| p.patch.as_ref())
}

fn handle_patch(
    name: &str,
    patches: &[PathBuf],
    deps: &[&DepsSet],
    ci_source: SourceId,
    config: &Config,
) -> Result<()> {
    let dep = match deps.iter().find_map(|dep| dep.get(name)) {
        None => {
            eprintln!("Unable to find package {} in dependencies", name);
            return Ok(());
        }
        Some(dep) => dep,
    };
    let dep_type = get_name_and_version(name, dep);
    let (dep, mut registry) =
        get_dependency_and_registry(dep_type, ci_source, config)?;
    let summary = get_summary(name, &dep, &mut registry)?;
    let mut sources = SourceMap::new();
    sources.insert(registry);
    let pkg_set = PackageSet::new(&[summary.package_id()], sources, config)?;
    let pkg = download_package(&summary, &pkg_set)?;
    let path = copy_package(pkg)?;
    apply_patches(name, patches, &path)?;
    Ok(())
}

fn get_name_and_version<'a>(name: &'a str, dep: &'a Dependency) -> DepType<'a> {
    match dep {
        Dependency::Simple(version) => DepType::CratesIO {
            name,
            version: Some(version),
        },
        Dependency::Detailed(detail) => {
            if let Some(ref git) = detail.git {
                let gref = if let Some(ref branch) = detail.branch {
                    GitReference::Branch(branch.into())
                } else if let Some(ref tag) = detail.tag {
                    GitReference::Tag(tag.into())
                } else if let Some(ref rev) = detail.rev {
                    GitReference::Rev(rev.into())
                } else {
                    GitReference::Branch("master".into())
                };
                DepType::Git {
                    name,
                    version: detail.version.as_deref(),
                    url: git,
                    gref,
                }
            } else {
                DepType::CratesIO {
                    name,
                    version: detail.version.as_deref(),
                }
            }
        }
    }
}

fn get_dependency_and_registry<'a>(
    dep_type: DepType<'_>,
    ci_source: SourceId,
    config: &'a Config,
) -> Result<(CDep, Box<dyn Source + 'a>)> {
    let (name, version, source, registry): (
        &str,
        Option<&str>,
        SourceId,
        Box<dyn Source>,
    ) = match dep_type {
        DepType::CratesIO { name, version } => {
            let mut registry =
                RegistrySource::remote(ci_source, &HashSet::new(), config);
            registry.update()?;
            (name, version, ci_source, Box::new(registry))
        }
        DepType::Git {
            name,
            version,
            url,
            gref,
        } => {
            let git_source_id = get_git_source(url, gref)?;
            let mut registry = GitSource::new(git_source_id, config)?;
            registry.update()?;
            (name, version, git_source_id, Box::new(registry))
        }
    };
    let dep = CDep::parse_no_deprecated(name, version, source)?;
    Ok((dep, registry))
}

fn get_summary(
    name: &str,
    dep: &CDep,
    registry: &mut dyn Source,
) -> Result<Summary> {
    let mut summaries = vec![];
    registry.query(dep, &mut |summary| summaries.push(summary))?;
    summaries
        .iter()
        .max_by_key(|s| s.version())
        .cloned()
        .ok_or_else(|| {
            err_msg(format!("Unable to find package: {}", name))
                .compat()
                .into()
        })
}

fn download_package<'a>(
    summary: &Summary,
    pkg_set: &'a PackageSet<'_>,
) -> Result<&'a Package> {
    pkg_set.get_one(summary.package_id())
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
    let manifest = fetch_manifest()?;
    let config = setup_config()?;
    let ci_source = get_ci_source(&config)?;
    let _lock = config.acquire_package_cache_lock()?;
    let patches = match get_patches(&manifest).filter(|p| !p.is_empty()) {
        None => {
            println!("No patches found");
            return Ok(());
        }
        Some(p) => p,
    };
    let deps: Vec<&DepsSet> = vec![
        &manifest.dependencies,
        &manifest.dev_dependencies,
        &manifest.build_dependencies,
    ];
    for (name, entry) in patches {
        if let Some(ref patches) = entry.patches {
            handle_patch(name, patches, &deps, ci_source, &config)?;
        } else {
            println!("No patches found for {}", name);
        }
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
}
