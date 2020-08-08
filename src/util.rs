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

/// Host tools such as rust-lld need to be in the sysroot to link correctly.
/// Copies entire host target, so stuff like tests work.
pub fn copy_host_tools(local_sysroot: &Path) {
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
    let local_sysroot = local_sysroot.join("lib").join("rustlib").join(&host);
    let src = {
        root.push("lib");
        root.push("rustlib");
        root.push(&host);
        root
    };
    let src_meta = fs::metadata(&src).unwrap();
    let to_meta = fs::metadata(&local_sysroot);
    // If our host tools bin dir doesn't exist it always needs updating.
    if let Ok(to_meta) = to_meta {
        // If our sysroot is older than the installed component we need to update
        // A newer rust-src should always have a newer modified time.
        // Whereas we should always have a newer modified time if we're up to date.
        if to_meta.modified().unwrap() > src_meta.modified().unwrap() {
            return;
        }
    }
    fs::create_dir_all(&local_sysroot).unwrap();
    let mut options = CopyOptions::new();
    options.overwrite = true;
    copy(src, local_sysroot.parent().unwrap(), &options).unwrap();
}
