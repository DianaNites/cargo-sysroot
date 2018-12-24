//! Utility.
use fs_extra::dir::{copy, CopyOptions};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    str,
};

/// Get the configured rustc sysroot.
/// This is the HOST sysroot.
fn get_rustc_sysroot() -> PathBuf {
    let rustc = Command::new("rustc")
        .arg("--print")
        .arg("sysroot")
        .output()
        .unwrap();
    PathBuf::from(str::from_utf8(&rustc.stdout).unwrap().trim())
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

/// Host tools such as rust-lld need to be in the sysroot to link correctly.
/// Copies entire host target, so stuff like tests work.
#[allow(dead_code)]
pub fn copy_host_tools(mut local_sysroot: PathBuf) {
    let mut root = get_rustc_sysroot();
    let host = root
        .file_stem()
        .unwrap()
        .to_str()
        .unwrap()
        .split('-')
        .skip(1)
        .collect::<Vec<_>>()
        .join("-");
    local_sysroot.push("lib");
    local_sysroot.push("rustlib");
    local_sysroot.push(&host);
    let src = {
        root.push("lib");
        root.push("rustlib");
        root.push(&host);
        root
    };
    let srcm = fs::metadata(&src).unwrap();
    let tom = fs::metadata(&local_sysroot);
    // If our host tools bin dir doesn't exist it always needs updating.
    if let Ok(tom) = tom {
        // If our sysroot is older than the installed component we need to update
        // A newer rust-src should always have a newer modifed time.
        // Whereas we should always have a newer modifed time if we're up to date.
        if tom.modified().unwrap() > srcm.modified().unwrap() {
            return;
        }
    }
    fs::create_dir_all(&local_sysroot).unwrap();
    let mut options = CopyOptions::new();
    options.overwrite = true;
    copy(src, local_sysroot.parent().unwrap(), &options).unwrap();
}
