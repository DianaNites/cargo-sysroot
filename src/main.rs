//! Cargo-SysRoot
//! Compiles libcore and libcompiler_builtins.
//!
//! Cargo.toml package.metadata.cargo-sysroot.target should be set
//! to the path of a Target Specification
//!
//! The sysroot is located in target/sysroot
//!
//! Cargo will automatically rebuild the project and all dependencies
//! if the files in the sysroot change.
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
use structopt::{clap::AppSettings, StructOpt};

mod util;
use crate::util::*;

#[derive(StructOpt, Debug)]
#[structopt(
    bin_name = "cargo",
    global_settings(&[
        AppSettings::ColoredHelp,
]))]
enum Args {
    Sysroot(Sysroot),
}

#[derive(StructOpt, Debug)]
struct Sysroot {
    /// Path to `Cargo.toml`
    #[structopt(long, default_value = "./Cargo.toml")]
    manifest_path: PathBuf,

    /// Path to target directory.
    #[structopt(long, default_value = "./target")]
    target_dir: PathBuf,

    /// Path to sysroot directory.
    #[structopt(long, default_value = "./target/sysroot")]
    sysroot_dir: PathBuf,

    /// Target to build for.
    ///
    /// Uses the value from `package.metadata.cargo-sysroot.target` as a
    /// default.
    #[structopt(long)]
    target: Option<PathBuf>,

    /// Disable .cargo/config generation
    #[structopt(long)]
    no_config: bool,

    /// Path to the rust sources.
    ///
    /// If not specified, uses the `rust-src` component from rustup.
    #[structopt(long)]
    rust_src_dir: Option<PathBuf>,

    /// The [profile] section from `Cargo.toml`.
    /// Some use-cases require the sysroot crates be built with this matching.
    #[structopt(skip)]
    cargo_profile: Option<Profile>,

    /// Where to put the built sysroot artifacts.
    /// This should point to somewhere in the new sysroot.
    /// Example: sysroot/lib/rustlib/target-triple/lib
    #[structopt(skip)]
    sysroot_artifact_dir: Option<PathBuf>,
}

/// Create a `.cargo/config` to use our target and sysroot.
fn generate_cargo_config(target: &Path, sysroot: &Path) -> Result<()> {
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
fn generate_liballoc_cargo_toml(args: &Sysroot) -> Result<PathBuf> {
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
            path: Some(rust_src.join("liballoc").join("lib.rs")),
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
            path: Some(rust_src.join("libcore")),
            ..Default::default()
        }),
    );
    toml.dependencies.as_mut().unwrap().insert(
        "compiler_builtins".into(),
        Dependency::Full(DependencyFull {
            version: Some("0.1.0".into()),
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
                    path: Some(rust_src.join("tools").join("rustc-std-workspace-core")),
                    ..Default::default()
                }),
            );
            // Unused, causes a warning.
            //
            // x.insert(
            //     "rustc-std-workspace-alloc".to_string(),
            //     Dependency::Full(DependencyFull {
            //         path: Some(rust_src.join("tools").join("rustc-std-workspace-alloc")),
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

fn build_liballoc(liballoc_cargo_toml: &Path, args: &Sysroot) -> Result<()> {
    let path = liballoc_cargo_toml;
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
        // The rust build system only passes this for rustc? xbuild passes this for liballoc. 🤷‍♀️
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

fn main() -> Result<()> {
    // TODO: Eat output if up to date.
    let Args::Sysroot(mut args) = Args::from_args();
    let toml: CargoToml =
        from_path(&args.manifest_path).with_context(|| args.manifest_path.display().to_string())?;

    if args.target.is_none() {
        args.target = Some(
            toml.package
                .metadata
                .context("Missing package metadata")?
                .get("cargo-sysroot")
                .context("Missing cargo-sysroot metadata")?
                .get("target")
                .context("Missing cargo-sysroot target")?
                .as_str()
                .context("Cargo-sysroot target field was not a string")?
                .into(),
        );
    }

    if args.rust_src_dir.is_none() {
        // See <https://github.com/rust-lang/rustup#can-rustup-download-the-rust-source-code>
        args.rust_src_dir = Some(
            get_rustc_sysroot()?
                .join("lib")
                .join("rustlib")
                .join("src")
                .join("rust")
                .join("src"),
        )
    }

    args.cargo_profile = toml.profile;
    args.sysroot_artifact_dir = Some(
        args.sysroot_dir
            .join("lib")
            .join("rustlib")
            .join(
                args.target
                    .as_ref()
                    .context("BUG: Somehow missing target triple")?
                    .file_stem()
                    .context("Failed to parse target triple")?,
            )
            .join("lib"),
    );

    // Clean-up old artifacts
    match fs::remove_dir_all(&args.sysroot_dir) {
        Ok(_) => (),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => (),
        e => e.context("Couldn't clean sysroot artifacts")?,
    };
    fs::create_dir_all(&args.sysroot_dir).context("Couldn't create sysroot directory")?;
    fs::create_dir_all(
        args.sysroot_artifact_dir
            .as_ref()
            .expect("BUG: sysroot_artifact_dir"),
    )
    .context("Failed to setup sysroot")?;

    let args = args;

    println!("Building sysroot crates");
    if !args.no_config {
        generate_cargo_config(args.target.as_ref().unwrap(), &args.sysroot_dir)
            .context("Couldn't create .cargo/config.toml")?;
    }

    // Build liballoc, which will pull in the other sysroot crates and build them,
    // too.
    let liballoc_cargo_toml =
        generate_liballoc_cargo_toml(&args).context("Failed to generate sysroot Cargo.toml")?;
    build_liballoc(&liballoc_cargo_toml, &args).context("Failed to build sysroot")?;

    // Copy host tools to the new sysroot, so that stuff like proc-macros and
    // testing can work.
    copy_host_tools(&args.sysroot_dir).context("Couldn't copy host tools to sysroot")?;

    Ok(())
}
