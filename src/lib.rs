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
    Build,
    CargoConfig,
    CargoToml,
    Dependency,
    DependencyFull,
    Package,
    Patches,
    Profile,
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
use structopt::StructOpt;

#[derive(StructOpt, Debug)]
pub struct Sysroot {
    /// Path to `Cargo.toml`
    #[structopt(long, default_value = "./Cargo.toml")]
    pub manifest_path: PathBuf,

    /// Path to target directory.
    #[structopt(long, default_value = "./target/sysroot/target")]
    pub target_dir: PathBuf,

    /// Path to sysroot directory.
    #[structopt(long, default_value = "./target/sysroot")]
    pub sysroot_dir: PathBuf,

    /// Target to build for.
    ///
    /// Uses the value from `package.metadata.cargo-sysroot.target` as a
    /// default.
    #[structopt(long)]
    pub target: Option<PathBuf>,

    /// Disable .cargo/config generation
    #[structopt(long)]
    pub no_config: bool,

    /// Path to the rust sources.
    ///
    /// If not specified, uses the `rust-src` component from rustup.
    #[structopt(long)]
    pub rust_src_dir: Option<PathBuf>,

    /// The [profile] section from `Cargo.toml`.
    /// Some use-cases require the sysroot crates be built with this matching.
    #[structopt(skip)]
    pub cargo_profile: Option<Profile>,

    /// Where to put the built sysroot artifacts.
    /// This should point to somewhere in the new sysroot.
    /// Example: sysroot/lib/rustlib/target-triple/lib
    #[structopt(skip)]
    pub sysroot_artifact_dir: Option<PathBuf>,
}

/// Create a `.cargo/config` to use our target and sysroot.
pub(crate) fn generate_cargo_config(target: &Path, sysroot: &Path) -> Result<()> {
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
pub(crate) fn generate_alloc_cargo_toml(args: &Sysroot) -> Result<PathBuf> {
    let rust_src = args
        .rust_src_dir
        .as_ref()
        .context("BUG: Missing rust-src")?;

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
        profile: args.cargo_profile.clone(),
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
    let path = args.sysroot_dir.join("Cargo.toml");
    fs::write(&path, t).context("Failed writing sysroot Cargo.toml")?;
    Ok(path)
}

pub(crate) fn build_alloc(alloc_cargo_toml: &Path, args: &Sysroot) -> Result<()> {
    let path = alloc_cargo_toml;
    let triple = args.target.as_ref().context("BUG: Missing target triple")?;

    let _exit = Command::new(env::var_os("CARGO").context("Couldn't find cargo command")?)
        .arg("rustc")
        .arg("--release")
        .arg("--target")
        // If it doesn't work, assume it's a builtin path?
        .arg(&triple.canonicalize().unwrap_or_else(|_| triple.into()))
        .arg("--target-dir")
        .arg(&args.target_dir)
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
        args.target_dir
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
            let out = args
                .sysroot_artifact_dir
                .as_ref()
                .context("BUG: Missing sysroot_artifact_dir")?
                .join(name);
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
