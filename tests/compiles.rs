use anyhow::{Context, Result};
use cargo_sysroot::{build_sysroot_with, get_rust_src, Sysroot};
use std::path::Path;

/// Test that all targets compile as expected.
#[test]
fn all_compile() -> Result<()> {
    for sys in &[
        Sysroot::Core,
        Sysroot::CompilerBuiltins,
        Sysroot::Alloc,
        // Sysroot::Std,
    ] {
        eprintln!("Sysroot {:?}, path {}", sys, sysroot.display());
        let build_dir = tempfile::tempdir()?;
        let sysroot = build_sysroot_with(
            None,
            build_dir.path(),
            // Path::new("x86_64-unknown-uefi"),
            Path::new("x86_64-unknown-linux-gnu"),
            // Path::new("spirv-unknown-unknown"),
            &get_rust_src()?,
            *sys,
            false,
        )
        .with_context(|| format!("Error compiling Sysroot: {:?}", sys))?;
    }
    Ok(())
}
