//! Utility.
use anyhow::*;
use fs_extra::dir::{copy, CopyOptions};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

/// Get the configured rustc sysroot.
/// This is the HOST sysroot.
fn get_rustc_sysroot() -> Result<PathBuf> {
    let rustc = Command::new("rustc")
        .arg("--print")
        .arg("sysroot")
        .output()?;
    let sysroot = PathBuf::from(
        std::str::from_utf8(&rustc.stdout)
            .context("Failed to convert sysroot path to utf-8")?
            .trim(),
    );
    Ok(sysroot)
}

/// Get the `rust-src` component of the current toolchain.
///
/// See <https://rust-lang.github.io/rustup/faq.html#can-rustup-download-the-rust-source-code>
pub fn get_rust_src() -> Result<PathBuf> {
    Ok(get_rustc_sysroot()?
        .join("lib")
        .join("rustlib")
        .join("src")
        .join("rust")
        .join("library"))
}

/// Host tools such as rust-lld need to be in the sysroot to link correctly.
/// Copies entire host target, so stuff like tests work.
#[allow(clippy::blocks_in_if_conditions)]
pub fn copy_host_tools(local_sysroot: &Path) -> Result<()> {
    let root = get_rustc_sysroot()?;
    let host = root
        .file_stem()
        .context("Couldn't get host sysroot")?
        .to_str()
        .context("Invalid utf-8 in host sysroot path")?
        .split('-')
        .skip(1)
        .collect::<Vec<_>>()
        .join("-");
    let local_sysroot = local_sysroot.join("lib").join("rustlib").join(&host);
    let src = root.join("lib").join("rustlib").join(&host);

    let src_meta = fs::metadata(&src)
        .with_context(|| format!("Couldn't get metadata for {}", src.display()))?;
    let to_meta = fs::metadata(&local_sysroot)
        .with_context(|| format!("Couldn't get metadata for {}", local_sysroot.display()));

    // If our host tools bin dir doesn't exist it always needs updating.
    if let Ok(to_meta) = to_meta {
        // If our sysroot is older than the installed component we need to update
        // A newer rust-src should always have a newer modified time.
        // Whereas we should always have a newer modified time if we're up to date.
        if to_meta.modified().with_context(|| {
            format!(
                "Couldn't get modification time for {}",
                local_sysroot.display()
            )
        })? > src_meta.modified().with_context(|| {
            format!(
                "Couldn't get modification time for {}",
                local_sysroot.display()
            )
        })? {
            return Ok(());
        }
    }
    let mut options = CopyOptions::new();
    options.overwrite = true;
    let local_sysroot = local_sysroot.parent().unwrap();
    copy(&src, &local_sysroot, &options).with_context(|| {
        format!(
            "Couldn't copy from `{}` to `{}`",
            src.display(),
            local_sysroot.display()
        )
    })?;
    Ok(())
}
