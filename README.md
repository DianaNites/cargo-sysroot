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

This tool will (TODO, not yet implemented) generate a .cargo/config for you that looks something like this

```toml
[build]
target = "path/to/my/target/specification/json"
rustflags = [
    "--sysroot",
    "full/path/to/target/sysroot",
]
```

This will allow Cargo to properly build your project with or without cargo-sysroot.
If you update your Rust nightly version you will need to run cargo-sysroot again.
Note that doing this will cause cargo to detect that libcore has changed and rebuild your entire project.
