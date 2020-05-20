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
//! - [x] Patch dependencies from crates.io
//! - [ ] Patch dependencies from git url
//! - [ ] Handle Workspaces
//! - [x] Use error messages which noone understands
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
        source::{Source, SourceMap},
        summary::Summary,
        SourceId,
    },
    sources::registry::RegistrySource,
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
    path::PathBuf,
};

#[derive(Debug, Clone, Deserialize)]
struct PatchSection {
    patch: HashMap<String, PatchEntry>,
}

#[derive(Debug, Clone, Deserialize)]
struct PatchEntry {
    patches: Vec<PathBuf>,
}

fn clear_patch_folder() -> Result<()> {
    fs::remove_dir_all("target/patch").map_err(Into::into)
}

fn fetch_manifest() -> Result<Manifest<PatchSection>> {
    Manifest::from_path_with_metadata("./Cargo.toml").map_err(Into::into)
}

fn setup_config() -> Result<Config> {
    Config::default()
}

fn get_source(config: &Config) -> Result<SourceId> {
    SourceId::crates_io(config)
}

fn get_patches(
    manifest: &Manifest<PatchSection>,
) -> Option<&HashMap<String, PatchEntry>> {
    manifest
        .package
        .as_ref()
        .and_then(|p| p.metadata.as_ref())
        .map(|p| &p.patch)
}

fn handle_patch(
    name: &str,
    entry: &PatchEntry,
    deps: &[&DepsSet],
    source: SourceId,
    config: &Config,
) -> Result<()> {
    let dep = match deps.iter().find_map(|dep| dep.get(name)) {
        None => {
            eprintln!("Unable to find package {} in dependencies", name);
            return Ok(());
        }
        Some(dep) => dep,
    };
    let (name, ver) = get_name_and_version(name, dep);
    let dep = get_dependency(name, ver, source)?;
    let mut registry = setup_registry(source, config)?;
    let summary = get_summary(name, &dep, &mut registry)?;
    let mut sources = SourceMap::new();
    sources.insert(Box::new(registry));
    let pkg_set = PackageSet::new(&[summary.package_id()], sources, config)?;
    let pkg = download_package(&summary, &pkg_set)?;
    let path = copy_package(pkg)?;
    apply_patches(entry, &path)?;
    Ok(())
}

fn get_name_and_version<'a>(
    name: &'a str,
    dep: &'a Dependency,
) -> (&'a str, Option<&'a str>) {
    match dep {
        Dependency::Simple(version) => (name, Some(version)),
        Dependency::Detailed(detail) => (
            detail.package.as_deref().unwrap_or(name),
            detail.version.as_deref(),
        ),
    }
}

fn get_dependency(
    name: &str,
    version: Option<&str>,
    source: SourceId,
) -> Result<CDep> {
    CDep::parse_no_deprecated(name, version, source)
}

fn setup_registry(source: SourceId, config: &Config) -> Result<RegistrySource<'_>> {
    Ok(RegistrySource::remote(source, &HashSet::new(), config))
}

fn get_summary(
    name: &str,
    dep: &CDep,
    registry: &mut RegistrySource<'_>,
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
    pkg_set.get_one(summary.package_id()).map_err(Into::into)
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

fn apply_patches(entry: &PatchEntry, path: &PathBuf) -> Result<()> {
    for patch in &entry.patches {
        let data = fs::read_to_string(patch)?;
        let patches = Patch::from_multiple(&data)
            .map_err(|_| err_msg("Unable to parse patch file").compat())?;
        for patch in patches {
            let file_path = path.clone();
            let file_path = file_path.join(patch.old.path.as_ref());
            let file_path = file_path.canonicalize()?;
            if file_path.starts_with(&path) {
                let data = fs::read_to_string(&file_path)?;
                let data = apply_patch(patch, &data);
                fs::write(file_path, data)?;
            } else {
                println!("{:?}", path);
                println!("{:?}", file_path);
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

fn main() -> Result<()> {
    clear_patch_folder()?;
    let manifest = fetch_manifest()?;
    let config = setup_config()?;
    let source_id = get_source(&config)?;
    let _lock = config.acquire_package_cache_lock()?;
    let patches = match get_patches(&manifest) {
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
        handle_patch(name, entry, &deps, source_id, &config)?;
    }
    Ok(())
}
