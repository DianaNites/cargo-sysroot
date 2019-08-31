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
use cargo_toml2::{
    from_path, Build, CargoConfig, CargoToml, Dependency, DependencyFull, Package, Patches,
    Profile, TargetConfig,
};
use std::{collections::BTreeMap, env, fs, io::prelude::*, path::PathBuf, process::Command};
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
}

/// Stuff the build command needs.
struct BuildConfig {
    local_sysroot: PathBuf,
    target: PathBuf,
    target_dir: PathBuf,
    output_dir: PathBuf,
}

impl BuildConfig {
    fn new() -> Self {
        let Args::Sysroot(args) = Args::from_args();
        //
        let sysroot = get_local_sysroot_dir();
        let toml: CargoToml = from_path(args.manifest_path).expect("Failed to read Cargo.toml");

        let target = args.target.unwrap_or_else(|| {
            toml.package
                .metadata
                .as_ref()
                .expect("Missing cargo-sysroot metadata")
                .get("cargo-sysroot")
                .expect("Missing cargo-sysroot metadata")["target"]
                .as_str()
                .expect("Invalid cargo-sysroot metadata")
                .into()
        });

        Self {
            target_dir: get_target_dir(&sysroot),
            output_dir: get_output_dir(&sysroot, &target),
            local_sysroot: sysroot,
            target,
        }
    }
}

/// Create a `.cargo/config` to use our target and sysroot.
///
/// ## Arguments:
/// * `target`, path to the target json file.
fn generate_cargo_config(args: &Sysroot) {
    let cargo_config = PathBuf::from(".cargo/config");
    fs::create_dir_all(cargo_config.parent().unwrap()).unwrap();
    if cargo_config.exists() {
        // TODO: Be smarter, update existing. Warn?
        return;
    }
    let target = args
        .target
        .as_ref()
        .expect("Missing target triple")
        .canonicalize()
        .expect("Couldn't get path to target.json")
        .to_str()
        .expect("Failed to convert target.json path to utf-8")
        .to_string();
    let sysroot_dir = args
        .sysroot_dir
        .to_str()
        .expect("Failed to convert sysroot path to utf-8");

    let config = CargoConfig {
        build: Some(Build {
            target: Some(target),
            rustflags: Some(vec!["--sysroot".to_owned(), format!("{}", sysroot_dir)]),
            ..Default::default()
        }),
        ..Default::default()
    };
    let toml = toml::to_string(&config).unwrap();

    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(cargo_config)
        .unwrap();
    file.write_all(toml.as_bytes()).unwrap();
}

/// The `Cargo.toml` for building the `alloc` crate.
///
/// Returns the full path to the manifest
fn generate_liballoc_cargo_toml(args: &Sysroot) -> PathBuf {
    let rust_src = args.rust_src_dir.as_ref().expect("BUG: Missing rust-src");

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
            features: Some(vec!["core".into(), "mem".into()]),
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
            x
        });
    //
    let t = toml::to_string(&toml).expect("Failed creating temp Cargo.toml");
    let path = args.sysroot_dir.join("Cargo.toml");
    std::fs::create_dir_all(path.parent().expect("Impossible")).expect("Failed to create temp dir");
    fs::write(&path, t).expect("Failed writing temp Cargo.toml");
    path
}

fn build_liballoc(cfg: &BuildConfig) {
    // let path = generate_liballoc_cargo_toml(cfg);
    let path = PathBuf::new();
    //
    let exit = Command::new(env::var_os("CARGO").unwrap())
        .arg("rustc")
        .arg("--release")
        .arg("--target")
        .arg(&cfg.target)
        .arg("--target-dir")
        .arg(&cfg.target_dir)
        .arg("--manifest-path")
        .arg(path)
        .arg("--") // Pass to rustc directly.
        .arg("-Z")
        // Should this be configurable? Would anyone want unwinding for the sysroot crates?
        .arg("no-landing-pads")
        .arg("-Z")
        // The rust build system only passes this for rustc, but xbuild passes this for liballoc. 🤷
        .arg("force-unstable-if-unmarked")
        .status()
        .expect("Build failed.");
    assert!(exit.success(), "Build failed.");
    //
    for entry in fs::read_dir(
        cfg.target_dir
            .join(&cfg.target.file_stem().unwrap())
            .join("release")
            .join("deps"),
    )
    .expect("Failure to read directory")
    {
        let entry = entry.expect("Failure to read entry");
        let name = entry
            .file_name()
            .into_string()
            .expect("Invalid Unicode in path");
        if name.starts_with("lib") {
            let out = cfg.output_dir.join(name);
            fs::copy(entry.path(), out).expect("Copying failed");
        }
    }
}

#[allow(unreachable_code, unused_variables)]
fn main() {
    // TODO: Eat output if up to date.
    let Args::Sysroot(mut args) = Args::from_args();
    let toml: CargoToml = from_path(&args.manifest_path).expect("Failed to read Cargo.toml");

    args.target = Some(args.target.unwrap_or_else(|| {
        toml.package
            .metadata
            .as_ref()
            .expect("Missing cargo-sysroot metadata")
            .get("cargo-sysroot")
            .expect("Missing cargo-sysroot metadata")["target"]
            .as_str()
            .expect("Invalid cargo-sysroot metadata")
            .into()
    }));
    args.rust_src_dir = Some(args.rust_src_dir.unwrap_or_else(|| {
        let rustc = Command::new("rustc")
            .arg("--print")
            .arg("sysroot")
            .output()
            .expect("Failed to run `rustc` and get sysroot");
        let sysroot = PathBuf::from(
            std::str::from_utf8(&rustc.stdout)
                .expect("Failed to convert sysroot path to utf-8")
                .trim(),
        );
        sysroot
            .join("lib")
            .join("rustlib")
            .join("src")
            .join("rust")
            .join("src")
    }));
    args.cargo_profile = toml.profile;

    let args = args;
    //
    println!("Building sysroot crates");
    if !args.no_config {
        generate_cargo_config(&args);
    }

    let liballoc_cargo_toml = generate_liballoc_cargo_toml(&args);
    // build_liballoc();

    //
    return;
    let cfg = BuildConfig::new();

    build_liballoc(&cfg);

    copy_host_tools(cfg.local_sysroot.clone());
}
