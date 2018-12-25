# Cargo-sysroot

[![Crates.io](https://img.shields.io/crates/v/cargo-sysroot.svg)](https://crates.io/crates/cargo-sysroot)
![maintenance-as-is](https://img.shields.io/badge/maintenance-as--is-yellow.svg)

A (dumb) tool to compile libcore and friends for no_std crates.

This is not a wrapper like `cargo xbuild` or `xargo`, this is a standalone tool you call once.
This has the nice benefit of actually working with standard tools like RLS, clippy,
or even the simple `cargo check`.

## New in 0.5.0

Support for the new sysroot build process, and more reliable overall.

Liballoc!

I'm no longer mysterious!

## Prerequisite

* A nightly compiler.
* The `rust-src` component must be installed for the active toolchain.
* Your `Cargo.toml` file must contain `package.metadata.cargo-sysroot.target`, where target is a target specifiction json file.
    * A rust supported target may also work, but this is untested.
* OR Pass `--target` on the commandline, ex `cargo sysroot --target path/to/target.json`

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

This tool will generate a `.cargo/config` for you that looks something like this

```toml
[build]
target = "path/to/your/target/specification/json"
rustflags = [
    "--sysroot",
    "full/path/to/target/sysroot",
]
```

The sysroot will be located at `target/sysroot` and the target directory for building it at `target/sysroot/target`.

Due to how rust sysroots work, you can use multiple different target specifications at a time without rebuilding.
Switching between them will require manually changing `.cargo/config`, however.

Note that this tool is currently quite stupid, so it won't attempt to do anything if that file already exists.
In this case you will have to edit it manually.

This will allow Cargo to properly build your project with the normal commands such as `cargo build`.
You may wish to modify this file to make use of the `target.$triple.runner` key. See the [Cargo Documentation](https://doc.rust-lang.org/cargo/reference/config.html#configuration-keys) for details.
Note that the author experienced problems with the `$triple` variant not working, and you may experience better success with the `cfg` variant.

If you update your Rust nightly version you will need to run `cargo-sysroot` again.
Note that doing this will cause cargo to detect that libcore has changed and rebuild your entire project.

## Recomendations

If you have more complicated needs than can be satisfied by `target.$triple.runner`, which doesn't yet support passing arguments, the author recommends using a tool such as [cargo-make](https://crates.io/crates/cargo-make).

Use my other crate, [`cargo-image`](https://crates.io/crates/cargo-image) to build an image suitable for running in QEMU.

## Details

The sysroot crates are compiled with the `--release` switch.
compilter_builtins is built with the `mem` and `core` features, which provides `memcpy` and related.

The sysroot crates will share any profile information your crate specifies. Eg if you enable debug for `release`, the sysroot crates will have that too. This matches `cargo-xbuild` behaviour and is required for the `bootloader` crate to function.

## TODO

* Allow specifying a custom `rust-src`.
* Allow disabling the `mem` feature.

## FAQ

* Q: Why are all versions before 0.5.0 yanked?
* A: They didn't work correctly due to bugs or changes in the standard distribution.

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
