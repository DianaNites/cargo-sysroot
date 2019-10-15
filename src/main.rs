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
use std::{
    collections::BTreeMap, env, fs, io::prelude::*, path::Path, path::PathBuf, process::Command,
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
    // canonicalize requires the directory exist, after all.
    fs::create_dir_all(&args.sysroot_dir).unwrap();
    let sysroot_dir = args
        .sysroot_dir
        .canonicalize()
        .expect("Failed to canonicalize `sysroot_dir`");
    let sysroot_dir = sysroot_dir
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

fn build_liballoc(liballoc_cargo_toml: &Path, args: &Sysroot) {
    let path = liballoc_cargo_toml;
    let triple = args.target.as_ref().expect("BUG: Missing target triple");
    //
    let exit = Command::new(env::var_os("CARGO").unwrap())
        .arg("rustc")
        .arg("--release")
        .arg("--target")
        .arg(&triple)
        .arg("--target-dir")
        .arg(&args.target_dir)
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
        args.target_dir
            .join(&triple.file_stem().expect("Failed to parse target triple"))
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
            let out = args
                .sysroot_artifact_dir
                .as_ref()
                .expect("BUG: Missing sysroot_artifact_dir")
                .join(name);
            fs::copy(entry.path(), out).expect("Copying failed");
        }
    }
}

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
    args.sysroot_artifact_dir = Some(
        args.sysroot_dir
            .join("lib")
            .join("rustlib")
            .join(
                args.target
                    .as_ref()
                    .expect("BUG: Somehow missing target triple")
                    .file_stem()
                    .expect("Failed to parse target triple"),
            )
            .join("lib"),
    );
    // Clean-up old artifacts
    match fs::remove_dir_all(&args.sysroot_dir) {
        Ok(_) => (),
        Err(ref e) if e.kind() == std::io::ErrorKind::NotFound => (),
        _ => panic!("Couldn't clear sysroot"),
    };
    fs::create_dir_all(&args.sysroot_dir).expect("Couldn't create sysroot directory");
    fs::create_dir_all(args.sysroot_artifact_dir.as_ref().unwrap())
        .expect("Failed to create sysroot_artifact_dir directory");

    let args = args;
    //
    println!("Building sysroot crates");
    if !args.no_config {
        generate_cargo_config(&args);
    }

    // Build liballoc, which will pull in the other sysroot crates and build them, too.
    let liballoc_cargo_toml = generate_liballoc_cargo_toml(&args);
    build_liballoc(&liballoc_cargo_toml, &args);

    // Copy host tools to the new sysroot, so that stuff like proc-macros and testing can work.
    copy_host_tools(&args.sysroot_dir);
}
