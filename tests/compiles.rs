use anyhow::{Context, Result};
use cargo_sysroot::{Sysroot, SysrootBuilder};

/// Test that all targets compile as expected.
#[test]
fn all_compile() -> Result<()> {
    for sys in &[
        Sysroot::Core,
        Sysroot::CompilerBuiltins,
        Sysroot::Alloc,
        // Sysroot::Std,
    ] {
        let build_dir = tempfile::tempdir()?;
        let sysroot = SysrootBuilder::new(*sys)
            .output(build_dir.path().into())
            // .target("x86_64-unknown-uefi".into())
            .target("x86_64-unknown-linux-gnu".into())
            // .target("spirv-unknown-unknown".into())
            .build()
            .with_context(|| format!("Error compiling Sysroot: {:?}", sys))?;
        eprintln!("Sysroot {:?}, path {}", sys, sysroot.display());
    }
    Ok(())
}
