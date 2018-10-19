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
    x.push("lib");
    x.push("rustlib");
    x.push(get_target().file_stem().unwrap());
    x.push("lib");
    fs::create_dir_all(&x).unwrap();
    x
}

/// Rustc needs some configuration to work correctly.
/// Unless the undocumented enviroment variable "RUSTUP_TOOLCHAIN" is removed
/// Rustup directory overrides won't work.
fn get_rustc() -> Command {
    let mut cmd = Command::new("rustc");
    cmd.env_remove("RUSTUP_TOOLCHAIN")
        .current_dir(env::current_dir().unwrap());
    cmd
}

/// Get the configured rustc sysroot.
/// This is the HOST sysroot.
fn get_rustc_sysroot() -> PathBuf {
    let rustc = get_rustc().arg("--print").arg("sysroot").output().unwrap();
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
    libcore.push("lib.rs");

    let _ = get_rustc()
        .arg("--out-dir")
        .arg(get_local_sysroot_dir())
        .arg("--target")
        .arg(get_target())
        .arg("-O")
        .arg("-g")
        .arg("--crate-type")
        .arg("rlib")
        .arg("--crate-name")
        .arg("core")
        .arg("-Z")
        .arg("no-landing-pads")
        //
        .arg(libcore)
        .status();
}

fn main() {
    // HACK
    let _ = env::set_current_dir(r#"C:\_Diana\Projects\diaos"#).unwrap();
    let target = get_target();

    compile_core();

    println!("Target: {:#?}", target);
}
