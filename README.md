# Cargo-sysroot

[![Crates.io](https://img.shields.io/crates/v/cargo-sysroot.svg)](https://crates.io/crates/cargo-sysroot)
![maintenance-as-is](https://img.shields.io/badge/maintenance-as--is-yellow.svg)

A simple tool to compile the sysroot crates for your no_std application, while using the standard cargo tools.

This is not a wrapper like `cargo xbuild` or `xargo`, this is a standalone tool you call once beforehand.
This has the nice benefit of actually working with standard tools like RLS, clippy,
or even the simple `cargo check`. It accomplishes this by generating a `.cargo/config` for you.

## Prerequisite

* A nightly compiler.
* The `rust-src` component must be installed for the active toolchain.
* Your `Cargo.toml` file ***MUST*** contain `package.metadata.cargo-sysroot.target`, where `target` is a target specification json file.
  * A built-in target may also work, but this is untested.
* OR Pass `--target` on the command line, ex `cargo sysroot --target path/to/target.json`

### Example `Cargo.toml`

```toml
[package]
name = "My Project"
version = "0.1.0"
authors = ["Me <Me@Me.com>"]

[package.metadata.cargo-sysroot]
target = "my_custom_target.json" # This is relative to Cargo.toml
```

## Getting Started

* Run `cargo install cargo-sysroot`.
* Run `cargo sysroot` in the working directory of your project.

This tool will generate a `.cargo/config.toml` for you that looks something like this.
This can be disabled via the `--no-config` command-line option,
but note that you will then have to tell cargo about your target and sysroot location manually.

```toml
[build]
target = <package.metadata.cargo-sysroot.target>
rustflags = [
    "--sysroot",
    "<project root>/target/sysroot",
]
```

The sysroot will be located at `target/sysroot` and the target directory for building it at `target/sysroot/target`.

Due to how the rust sysroot works, you can use multiple different target specifications at a time without rebuilding, by simply passing a different `--target` to cargo.

Note that this tool is currently quite simple, so it won't attempt to do anything if that file already exists.
In this case you will have to edit it manually.

This will allow Cargo to properly build your project with the normal commands, such as `cargo build`.

You may wish to modify this file to make use of the `target.$triple.runner` key. See the [Cargo Documentation](https://doc.rust-lang.org/cargo/reference/config.html#configuration-keys) for details.
Note that the author experienced problems with the `$triple` variant not working, and you may experience better success with the `cfg` variant.

If you update your Rust nightly version you will need to run `cargo-sysroot` again,
causing cargo to detect the update and rebuild the sysroot and your project.

## Recommendations

If you have more complicated needs than can be satisfied by `target.$triple.runner`,
which doesn't support complex-ish modifications of the command line.

The author recommends their own, [`cargo-runner`](https://crates.io/crates/cargo-runner)
to solve this, it allows specifying the command-line in `Cargo.toml` and applying a suffix
to the path from Cargo.

Alternatives include [cargo-make](https://crates.io/crates/cargo-make),
which you can setup to run whatever you like, instead of using `cargo run`.

Use my other crate, [`cargo-image`](https://crates.io/crates/cargo-image) to build an image suitable for running in QEMU.

## Details

The sysroot crates are compiled with the `--release` switch.
compiler_builtins is built with the `mem` and `core` features, which provides `memcpy` and related.

The sysroot crates will share any profile information your crate specifies. Eg if you enable debug for `release`, the sysroot crates will have that too. This matches `cargo-xbuild` behavior and is required for the `bootloader` crate to function.

You can pass custom rust sources through the `--rust-src-dir` flag.

## TODO

* Allow disabling the `mem` feature.

## FAQ

* Q: Why did you write this over just using `cargo-xbuild`
* A: It was easier and simpler than getting `cargo-xbuild` to work reliably or with any other standard tools.

## License

Licensed under either of

* Apache License, Version 2.0
   ([LICENSE-APACHE](LICENSE-APACHE) or <http://www.apache.org/licenses/LICENSE-2.0)>
* MIT license
   ([LICENSE-MIT](LICENSE-MIT) or <http://opensource.org/licenses/MIT)>

at your option.

## Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.
