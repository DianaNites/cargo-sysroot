//! # Cargo-Sysroot
//!
//! Compiles the Rust sysroot crates, core, compiler_builtins, and alloc.
//!
//! Cargo.toml package.metadata.cargo-sysroot.target should be set
//! to the path of a Target Specification
//!
//! The sysroot is located in `.target/sysroot`
//!
//! Build the Rust sysroot crates
//!
//! # Example
//!
//! ```rust
//! ```
use anyhow::*;
use cargo_toml2::{
    from_path,
    to_path,
    CargoToml,
    Dependency,
    DependencyFull,
    Package,
    Patches,
    TargetConfig,
};
use std::{
    collections::BTreeMap,
    env,
    fs,
    path::{Path, PathBuf},
    process::Command,
};

mod util;

/// The sysroot crates to build.
///
/// See [`build_sysroot_with`] for details.
#[derive(Debug)]
pub enum Sysroot {
    /// The core crate. Provides.. core functionality.
    Core,

    /// Compiler-builtins crate.
    ///
    /// This implies [`Sysroot::Core`].
    CompilerBuiltins,

    /// The alloc crate. Gives you a heap, and things to put on it.
    ///
    /// This implies [`Sysroot::Core`], and `compiler_builtins`.
    Alloc,

    /// The standard library. Gives you an operating system.
    ///
    /// This implies [`Sysroot::Alloc`], [`Sysroot::Core`], and
    /// `compiler_builtins`.
    Std,
}

/// Generate a Cargo.toml for building the sysroot crates
///
/// See [`build_sysroot_with`].
fn generate_sysroot_cargo_toml(
    manifest: Option<&Path>,
    sysroot_dir: &Path,
    rust_src: &Path,
    sysroot: Sysroot,
    compiler_builtins_mem: bool,
) -> Result<PathBuf> {
    fs::write(
        sysroot_dir.join("lib.rs"),
        "#![feature(no_core)]\n#![no_core]",
    )?;
    let toml = CargoToml {
        package: Package {
            name: "Sysroot".into(),
            version: "0.0.0".into(),
            authors: vec!["The Rust Project Developers".into(), "DianaNites".into()],
            edition: Some("2018".into()),
            autotests: Some(false),
            autobenches: Some(false),
            ..Default::default()
        },
        lib: Some(TargetConfig {
            name: Some("sysroot".into()),
            path: Some("lib.rs".into()),
            ..Default::default()
        }),
        dependencies: Some({
            let mut deps = BTreeMap::new();
            match sysroot {
                Sysroot::Core => {
                    deps.insert(
                        "core".into(),
                        Dependency::Full(DependencyFull {
                            path: Some(rust_src.join("core")),
                            ..Default::default()
                        }),
                    );
                }

                Sysroot::CompilerBuiltins => {
                    deps.insert(
                        "core".into(),
                        Dependency::Full(DependencyFull {
                            path: Some(rust_src.join("core")),
                            ..Default::default()
                        }),
                    );
                    deps.insert(
                        "compiler_builtins".into(),
                        Dependency::Full(DependencyFull {
                            version: Some("0.1".into()),
                            ..Default::default()
                        }),
                    );
                }

                Sysroot::Alloc => {
                    deps.insert(
                        "alloc".into(),
                        Dependency::Full(DependencyFull {
                            path: Some(rust_src.join("alloc")),
                            features: if compiler_builtins_mem {
                                Some(vec!["compiler-builtins-mem".into()])
                            } else {
                                None
                            },
                            ..Default::default()
                        }),
                    );
                }

                Sysroot::Std => {
                    deps.insert(
                        "std".into(),
                        Dependency::Full(DependencyFull {
                            path: Some(rust_src.join("std")),
                            features: if compiler_builtins_mem {
                                Some(vec!["compiler-builtins-mem".into()])
                            } else {
                                None
                            },
                            ..Default::default()
                        }),
                    );
                }
            }
            deps
        }),
        patch: Some(Patches {
            sources: if let Sysroot::Core = sysroot {
                let mut sources = BTreeMap::new();
                sources.insert("crates-io".into(), {
                    let mut x = BTreeMap::new();
                    x.insert(
                        "rustc-std-workspace-core".to_string(),
                        Dependency::Full(DependencyFull {
                            path: Some(rust_src.join("rustc-std-workspace-core")),
                            ..Default::default()
                        }),
                    );
                    x
                });
                sources
            } else {
                BTreeMap::new()
            },
        }),
        profile: {
            match manifest {
                Some(manifest) => {
                    let toml: CargoToml =
                        from_path(manifest).with_context(|| manifest.display().to_string())?;
                    toml.profile
                }
                None => None,
            }
        },
        ..Default::default()
    };
    let path = sysroot_dir.join("Cargo.toml");
    to_path(&path, &toml).context("Failed writing sysroot Cargo.toml")?;
    Ok(path)
}

/// The entry-point for building the alloc crate, which builds all the others
fn build_alloc(alloc_cargo_toml: &Path, sysroot_dir: &Path, target: &Path) -> Result<()> {
    let path = alloc_cargo_toml;
    let triple = target;
    let target_dir = sysroot_dir.join("target");

    // TODO: Eat output if up to date? Always?
    let _exit = Command::new(env::var_os("CARGO").context("Couldn't find cargo command")?)
        .arg("rustc")
        .arg("--release")
        .arg("--target")
        // If it doesn't work, assume it's a builtin path?
        .arg(&triple.canonicalize().unwrap_or_else(|_| triple.into()))
        .arg("--target-dir")
        .arg(&target_dir)
        .arg("--manifest-path")
        .arg(path)
        .arg("--") // Pass to rustc directly.
        .arg("-Z")
        // The rust build system only passes this for rustc? xbuild passes this for alloc. 🤷‍♀️
        .arg("force-unstable-if-unmarked")
        .status()
        .context("Build failed")?;

    // Copy artifacts to sysroot.
    for entry in fs::read_dir(
        target_dir
            .join(
                &triple
                    .file_stem()
                    .context("Failed to parse target triple")?,
            )
            .join("release")
            .join("deps"),
    )
    .context("Failure to read artifact directory")?
    {
        let entry = entry?;
        let name = entry
            .file_name()
            .into_string()
            .map_err(|e| Error::msg(e.to_string_lossy().to_string()))
            .context("Invalid Unicode in path")?;
        if name.starts_with("lib") {
            let out = artifact_dir(sysroot_dir, target)?.join(name);
            fs::copy(entry.path(), &out).with_context(|| {
                format!(
                    "Copying sysroot artifact from {} to {} failed",
                    entry.path().display(),
                    out.display()
                )
            })?;
        }
    }

    Ok(())
}

/// Not part of the public API.
#[doc(hidden)]
pub fn artifact_dir(sysroot_dir: &Path, target: &Path) -> Result<PathBuf> {
    Ok(sysroot_dir
        .join("lib")
        .join("rustlib")
        .join(target.file_stem().context("Invalid Target Specification")?)
        .join("lib"))
}

/// Clean up generated sysroot artifacts.
/// Should be called before [`build_sysroot`] if you want this behavior.
pub fn clean_artifacts(sysroot_dir: &Path) -> Result<()> {
    // Clean-up old artifacts
    match fs::remove_dir_all(sysroot_dir) {
        Ok(_) => (),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => (),
        e => e.context("Couldn't clean sysroot artifacts")?,
    };
    Ok(())
}

/// Build the Rust sysroot crates, using
/// `manifest`, `sysroot`, `target`, and `rust_src`.
///
/// `target` may be a path to a JSON Target Specification
///
/// If `manifest` is provided, the sysroot crates will be built
/// with any profile overrides specified in it.
///
/// `sysroot_crate` specifies which sysroot crate to build.
/// Only one option can be picked, because they imply each other.
///
/// If `compiler_builtins_mem` is true, the `compiler_builtins`
/// mem feature will be enabled.
/// This only applies to [`Sysroot::Alloc`] and [`Sysroot::Std`].
///
/// You may want the simpler `build_sysroot`.
pub fn build_sysroot_with(
    manifest: Option<&Path>,
    sysroot: &Path,
    target: &Path,
    rust_src: &Path,
    sysroot_crate: Sysroot,
    compiler_builtins_mem: bool,
) -> Result<PathBuf> {
    fs::create_dir_all(sysroot).context("Couldn't create sysroot directory")?;
    fs::create_dir_all(artifact_dir(sysroot, target)?).context("Failed to setup sysroot")?;

    let sysroot_cargo_toml = generate_sysroot_cargo_toml(
        manifest,
        sysroot,
        rust_src,
        sysroot_crate,
        compiler_builtins_mem,
    )?;
    build_alloc(&sysroot_cargo_toml, sysroot, target).context("Failed to build sysroot")?;

    // Copy host tools to the new sysroot, so that stuff like proc-macros and
    // testing can work.
    util::copy_host_tools(sysroot).context("Couldn't copy host tools to sysroot")?;
    Ok(sysroot.canonicalize().with_context(|| {
        format!(
            "Couldn't get canonical path to sysroot: {}",
            sysroot.display()
        )
    })?)
}

/// Build the Rust sysroot crates.
///
/// Returns the path to use for the sysroot.
///
/// This will build the sysroot crates, using:
/// - any profiles from `./Cargo.toml`
/// - `./target/sysroot` as the sysroot directory
/// - `package.metadata.cargo-sysroot.target` as the target triple
/// - The current rustup `rust_src` component.
/// - [`Sysroot::Alloc`]
/// - The `compiler_builtins_mem` feature enabled.
pub fn build_sysroot() -> Result<PathBuf> {
    let sysroot = Path::new("target").join("sysroot");
    let manifest_path = Path::new("Cargo.toml");
    let toml: CargoToml =
        from_path(manifest_path).with_context(|| manifest_path.display().to_string())?;
    let target: PathBuf = toml
        .package
        .metadata
        .context("Missing package metadata")?
        .get("cargo-sysroot")
        .context("Missing cargo-sysroot metadata")?
        .get("target")
        .context("Missing cargo-sysroot target")?
        .as_str()
        .context("Cargo-sysroot target field was not a string")?
        .into();
    build_sysroot_with(
        Some(manifest_path),
        &sysroot,
        &target,
        &util::get_rust_src()?,
        Sysroot::Alloc,
        true,
    )
}
