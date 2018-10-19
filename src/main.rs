extern crate toml;

use std::env;
use std::fs;
use std::io::prelude::*;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::str;
use toml::Value;

/// Read the target specification to use.
/// This is located in Cargo.toml.
/// target can be a relative or absolute path.
/// Relative paths will be relative to the directory containing Cargo.toml.
/// ```toml
/// [package.metadata.cargo-sysroot]
/// target = "path"
/// ```
fn get_target() -> PathBuf {
    let cargo = Path::new("Cargo.toml");
    let toml = {
        let mut s = String::new();
        fs::File::open(cargo)
            .unwrap()
            .read_to_string(&mut s)
            .unwrap();
        s
    };
    let target = toml.parse::<Value>().unwrap();
    let target = target["package"]["metadata"]["cargo-sysroot"]["target"]
        .as_str()
        .unwrap();
    PathBuf::from(target)
}

/// The location the new sysroot will be at.
fn get_local_sysroot_dir() -> PathBuf {
    let mut x = PathBuf::new();
    x.push("target");
    x.push("_my_sysroot");
    fs::create_dir_all(&x).unwrap();
    x
}

fn get_target_dir() -> PathBuf {
    let mut x = get_local_sysroot_dir();
    x.push("target");
    x
}

/// The location IN the local sysroot for libcore and friends.
fn get_output_dir() -> PathBuf {
    let mut x = get_local_sysroot_dir();
    x.push("lib");
    x.push("rustlib");
    x.push(get_target().file_stem().unwrap());
    x.push("lib");
    fs::create_dir_all(&x).unwrap();
    x
}

/// Use rustup which to find the correct executable.
/// Unless the undocumented? enviroment variable "RUSTUP_TOOLCHAIN" is removed
/// Rustup directory overrides won't work, for any tool.
fn get_rust_cmd(name: &str) -> Command {
    let rw = Command::new("rustup")
        .arg("which")
        .arg(name)
        .env_remove("RUSTUP_TOOLCHAIN")
        .output()
        .unwrap();
    let mut cmd = Command::new(PathBuf::from(str::from_utf8(&rw.stdout).unwrap().trim()));
    cmd.env_remove("RUSTUP_TOOLCHAIN");
    cmd
}

/// Get the configured rustc sysroot.
/// This is the HOST sysroot.
fn get_rustc_sysroot() -> PathBuf {
    let rustc = get_rust_cmd("rustc")
        .arg("--print")
        .arg("sysroot")
        .output()
        .unwrap();
    PathBuf::from(str::from_utf8(&rustc.stdout).unwrap().trim())
}

/// Get the rust-src component of the host sysroot.
fn get_rust_src_dir() -> PathBuf {
    let mut sysroot = get_rustc_sysroot();
    sysroot.push("lib");
    sysroot.push("rustlib");
    sysroot.push("src");
    sysroot.push("rust");
    sysroot.push("src");
    sysroot
}

fn compile_core() {
    let mut libcore = get_rust_src_dir();
    libcore.push("libcore");
    libcore.push("Cargo.toml");

    let _ = get_rust_cmd("cargo")
        .arg("build")
        .arg("--out-dir")
        .arg(get_output_dir())
        .arg("--target-dir")
        .arg(get_target_dir())
        .arg("--target")
        .arg(get_target())
        .arg("--release")
        .arg("-Z")
        .arg("unstable-options")
        .env("RUSTFLAGS", "-Z no-landing-pads")
        //
        .arg("--manifest-path")
        .arg(libcore)
        .status();
}

fn compile_compiler_builtins() {
    use std::ffi::OsString;
    let mut builtins = get_rust_src_dir();
    builtins.push("libcompiler_builtins");
    builtins.push("Cargo.toml");

    let mut flags = OsString::from("-Z no-landing-pads --sysroot ");
    flags.push(get_local_sysroot_dir().canonicalize().unwrap());

    let _ = get_rust_cmd("cargo")
        .arg("build")
        .arg("--out-dir")
        .arg(get_output_dir())
        .arg("--target-dir")
        .arg(get_target_dir())
        .arg("--target")
        .arg(get_target())
        .arg("--release")
        .arg("-Z")
        .arg("unstable-options")
        .env("RUSTFLAGS", flags)
        //
        .arg("--features")
        .arg("mem")
        .arg("--manifest-path")
        .arg(builtins)
        .status();
}

fn main() {
    // HACK
    let _ = env::set_current_dir(r#"C:\_Diana\Projects\diaos"#).unwrap();
    let target = get_target();

    compile_core();
    compile_compiler_builtins();

    println!("Target: {:#?}", target);
}
