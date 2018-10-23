# Cargo-sysroot

A (dumb) tool to automatically compile libcore and friends for no_std crates.

## Getting Started

Run `cargo install cargo-sysroot`.

Cargo-sysroot will forward all commandline arguments to Cargo.
This will respect directory overrides based on the current working directory.

To build a project with this simply run `cargo sysroot build`
Note that even just `cargo sysroot` will build libcore and libcompiler_builtins, as cargo-sysroot
isn't a very smart tool. It makes no attempt to understand Cargo's commandline,
it will simply attempt to build libcore using Cargo.
Since Cargo is used to build them, this means they won't be needlessly rebuilt.

This tool will generate a .cargo/config for you that looks something like this

```toml
[build]
target = "path/to/my/target/specification/json"
rustflags = [
    "--sysroot",
    "full/path/to/target/sysroot",
]
```

Note that this tool is currently quite stupid, so it won't attempt to do anything if that file already exists.

This will allow Cargo to properly build your project with or without cargo-sysroot.
If you update your Rust nightly version you will need to run cargo-sysroot again.
Note that doing this will cause cargo to detect that libcore has changed and rebuild your entire project.

## Limitations

* Liballoc is currently unsupported.

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
