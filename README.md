# cargo-patch

`Cargo-Patch` is a Cargo Subcommand which allows
patching dependencies using patch files.

## Installation

Simply run:

```sh
cargo install cargo-patch
```

This is not necessary when patching via `build.rs` file

## Usage

To patch a dependency one has to add the following
to `Cargo.toml`:

```toml
[package.metadata.patch.serde]
version = "1.0"
patches = [
    "test.patch"
]
```

It specifies which dependency to patch (in this case
serde) and one or more patchfiles to apply. Running:

```sh
cargo patch
```

will download the serde package specified in the
dependency section to the `target/patch` folder
and apply the given patches. To use the patched
version one has to override the dependency using
`replace` like this

```toml
[patch.crates-io]
serde = { path = './target/patch/serde-1.0.110' }
```

Instead of running `cargo patch` its also possible to add a `build.rs` file like this:

```rust
fn main() {
    println!("cargo:rerun-if-changed=Cargo.toml");
    println!("cargo:rerun-if-changed=patches/");
    cargo_patch::patch().expect("Failed while patching");
}
```

To make it work, add the cargo-patch library to the `build-dependencies`

```tomlusing the
[build-dependencies]
cargo-patch = "0.3"
```

Note, however, that all your patches should be in a single folder called `patches` or something similar. This is to make sure that the build script is executed again when something changes.

## Patch format

You can either use [diff](http://man7.org/linux/man-pages/man1/diff.1.html) or
[git](https://linux.die.net/man/1/git) to create patch files. Important is that
file paths are relative and inside the dependency.

#### Using diff file generated by GitHub pull request

```toml
[package.metadata.patch.serde]
version = "1.0"
patches = [
    { path = "generatedByGithub.patch", source = "GithubPrDiff" },
    { path = "generatedByGithub2.patch", source = "GithubPrDiff" },
    "test.patch",
    "test2.patch"
]
```

## Limitations

It's only possible to patch dependencies of binary crates as it is not possible
for a subcommand to intercept the build process.
