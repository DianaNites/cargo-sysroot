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
    Build,
    CargoConfig,
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
    io::prelude::*,
    path::{Path, PathBuf},
    process::Command,
};

#[doc(hidden)]
pub mod args;
#[allow(dead_code)]
mod util;

/// Create a `.cargo/config` to use our target and sysroot.
///
/// Not part of the public API.
#[doc(hidden)]
pub fn generate_cargo_config(target: &Path, sysroot: &Path) -> Result<()> {
    let cargo = Path::new(".cargo");
    let cargo_config = cargo.join("config.toml");
    fs::create_dir_all(cargo)?;

    if cargo_config.exists() {
        // TODO: Be smarter, update existing. Warn?
        return Ok(());
    }

    let target = target
        // .canonicalize()
        // .with_context(|| {
        //     format!(
        //         "Couldn't get absolute path to custom target: {}",
        //         target.display()
        //     )
        // })?
        .to_str()
        .context("Failed to convert target.json path to utf-8")?
        .to_string();
    let sysroot_dir = sysroot
        .canonicalize()
        .context("Couldn't get canonical path to sysroot")?
        .to_str()
        .context("Failed to convert sysroot path to utf-8")?
        .to_string();

    let config = CargoConfig {
        build: Some(Build {
            target: Some(target),
            rustflags: Some(vec!["--sysroot".to_owned(), sysroot_dir]),
            ..Default::default()
        }),
        ..Default::default()
    };
    let toml = toml::to_string(&config).unwrap();

    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(cargo_config)?;
    file.write_all(toml.as_bytes())?;
    Ok(())
}

/// The `Cargo.toml` for building the `alloc` crate.
///
/// Returns the full path to the manifest
fn generate_alloc_cargo_toml(
    manifest: &Path,
    sysroot_dir: &Path,
    rust_src: &Path,
) -> Result<PathBuf> {
    let mut toml = CargoToml {
        package: Package {
            name: "alloc".into(),
            version: "0.0.0".into(),
            authors: vec!["The Rust Project Developers".into()],
            edition: Some("2018".into()),
            ..Default::default()
        },
        lib: Some(TargetConfig {
            name: Some("alloc".into()),
            path: Some(rust_src.join("alloc").join("src").join("lib.rs")),
            ..Default::default()
        }),
        dependencies: Some(BTreeMap::new()),
        patch: Some(Patches {
            sources: BTreeMap::new(),
        }),
        profile: {
            let toml: CargoToml =
                from_path(manifest).with_context(|| manifest.display().to_string())?;
            toml.profile
        },
        ..Default::default()
    };
    toml.dependencies.as_mut().unwrap().insert(
        "core".into(),
        Dependency::Full(DependencyFull {
            path: Some(rust_src.join("core")),
            ..Default::default()
        }),
    );
    toml.dependencies.as_mut().unwrap().insert(
        "compiler_builtins".into(),
        Dependency::Full(DependencyFull {
            version: Some("0.1.10".into()),
            features: Some(vec!["rustc-dep-of-std".into(), "mem".into()]),
            ..Default::default()
        }),
    );
    toml.patch
        .as_mut()
        .unwrap()
        .sources
        .insert("crates-io".into(), {
            let mut x = BTreeMap::new();
            x.insert(
                "rustc-std-workspace-core".to_string(),
                Dependency::Full(DependencyFull {
                    path: Some(rust_src.join("rustc-std-workspace-core")),
                    ..Default::default()
                }),
            );
            // Unused, causes a warning.
            //
            // x.insert(
            //     "rustc-std-workspace-alloc".to_string(),
            //     Dependency::Full(DependencyFull {
            //         path: Some(rust_src.join("rustc-std-workspace-alloc")),
            //         ..Default::default()
            //     }),
            // );
            x
        });

    let t = toml::to_string(&toml).context("Failed creating sysroot Cargo.toml")?;
    let path = sysroot_dir.join("Cargo.toml");
    fs::write(&path, t).context("Failed writing sysroot Cargo.toml")?;
    Ok(path)
}

/// The entry-point for building the alloc crate, which builds all the others
fn build_alloc(alloc_cargo_toml: &Path, sysroot_dir: &Path, target: &Path) -> Result<()> {
    let path = alloc_cargo_toml;
    let triple = target;
    let target_dir = sysroot_dir.join("target");

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
/// Should be called before `build_sysroot` if you want this behavior.
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
/// You may want the simpler `build_sysroot`.
pub fn build_sysroot_with(
    manifest: &Path,
    sysroot: &Path,
    target: &Path,
    rust_src: &Path,
) -> Result<()> {
    fs::create_dir_all(sysroot).context("Couldn't create sysroot directory")?;
    fs::create_dir_all(artifact_dir(sysroot, target)?).context("Failed to setup sysroot")?;

    let alloc_cargo_toml = generate_alloc_cargo_toml(manifest, sysroot, rust_src)
        .context("Failed to generate sysroot Cargo.toml")?;
    build_alloc(&alloc_cargo_toml, sysroot, target).context("Failed to build sysroot")?;

    // Copy host tools to the new sysroot, so that stuff like proc-macros and
    // testing can work.
    util::copy_host_tools(sysroot).context("Couldn't copy host tools to sysroot")?;
    Ok(())
}

/// Build the Rust sysroot crates.
///
/// This will build the sysroot crates, using:
/// - any profiles from `./Cargo.toml`
/// - `./target/sysroot` as the sysroot directory
/// - `package.metadata.cargo-sysroot.target` as the target triple
/// - The current rustup `rust_src` component.
pub fn build_sysroot() -> Result<()> {
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
        manifest_path,
        &Path::new("target").join("sysroot"),
        &target,
        &util::get_rust_src()?,
    )?;
    Ok(())
}
