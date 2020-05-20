# cargo-patch

`Cargo-Patch` is a Cargo Subcommand which allows
patching dependencies using patch files.

## Installation

Simply run:

```sh
cargo install cargo-patch
```

## Usage

To patch a dependecy one has to add the following
to `Cargo.toml`:

```toml
[package.metadata.patch.serde]
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

## Patch format

You can either use [diff](http://man7.org/linux/man-pages/man1/diff.1.html) or
[git](https://linux.die.net/man/1/git) to create patch files. Important is that
file paths are relativ and inside the dependency

## Features

- [x] Patch dependencies from crates.io
- [ ] Patch dependencies from git url
- [ ] Handle Workspaces
- [x] Use error messages which noone understands

## Limitations

Its only possible to patch dependencies of binary crates as it is not possible
for a subcommand to intercept the build process.

