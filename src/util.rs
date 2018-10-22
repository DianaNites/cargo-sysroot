//! Utility.
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::str;

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

/// Use rustup which to find the correct executable.
/// Unless the undocumented? enviroment variable "RUSTUP_TOOLCHAIN" is removed
/// Rustup directory overrides won't work, for any tool.
pub fn get_rust_cmd(name: &str) -> Command {
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

/// Get the rust-src component of the host sysroot.
pub fn get_rust_src_dir() -> PathBuf {
    let mut sysroot = get_rustc_sysroot();
    sysroot.push("lib");
    sysroot.push("rustlib");
    sysroot.push("src");
    sysroot.push("rust");
    sysroot.push("src");
    sysroot
}

/// The location the new sysroot will be at.
/// This relies on the current working directory.
/// This returns the canonical path.
pub fn get_local_sysroot_dir() -> PathBuf {
    let mut x = PathBuf::new();
    x.push("target");
    x.push("sysroot");
    fs::create_dir_all(&x).unwrap();
    x.canonicalize().unwrap()
}

pub fn get_target_dir(mut base: PathBuf) -> PathBuf {
    base.push("target");
    base
}

/// The location IN the local sysroot for libcore and friends.
pub fn get_output_dir<T: AsRef<Path>>(mut base: PathBuf, target: T) -> PathBuf {
    base.push("lib");
    base.push("rustlib");
    base.push(target.as_ref().file_stem().unwrap());
    base.push("lib");
    fs::create_dir_all(&base).unwrap();
    base
}
