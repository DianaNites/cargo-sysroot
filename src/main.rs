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
use clap::{crate_description, crate_name, crate_version, App, AppSettings, Arg, SubCommand};
use std::{
    collections::BTreeMap,
    env,
    fs,
    io::prelude::*,
    path::{Path, PathBuf},
    process::Command,
};

mod config;
mod util;
use crate::{config::*, util::*};

/// Returns Some is target was passed on the commandline, None otherwise.
fn parse_args() -> Option<String> {
    let args = App::new(crate_name!())
        .version(crate_version!())
        .about(crate_description!())
        .bin_name("cargo")
        .setting(AppSettings::SubcommandRequiredElseHelp)
        .setting(AppSettings::GlobalVersion)
        .subcommand(
            SubCommand::with_name("sysroot")
                .about(crate_description!())
                .arg(
                    Arg::with_name("target")
                        .long("target")
                        .empty_values(false)
                        .takes_value(true),
                ),
        )
        .get_matches();
    args.subcommand_matches("sysroot")
        .and_then(|x| x.value_of("target").map(|s| s.to_string()))
}

/// Read the target specification to use.
/// This is located in Cargo.toml.
/// target can be a relative or absolute path.
/// Relative paths will be relative to the directory containing Cargo.toml.
/// ```toml
/// [package.metadata.cargo-sysroot]
/// target = "path"
/// ```
fn get_target() -> PathBuf {
    let cargo = Path::new("Cargo.toml");
    let toml = {
        let mut s = String::new();
        fs::File::open(cargo)
            .unwrap()
            .read_to_string(&mut s)
            .unwrap();
        s
    };
    let target: CargoToml = toml::from_str(&toml).unwrap();
    target
        .package
        .expect("Missing cargo-sysroot metadata")
        .metadata
        .expect("Missing cargo-sysroot metadata")
        .cargo_sysroot
        .target
}

/// Stuff the build command needs.
struct BuildConfig {
    rust_src: PathBuf,
    local_sysroot: PathBuf,
    target: PathBuf,
    target_dir: PathBuf,
    output_dir: PathBuf,
}

impl BuildConfig {
    fn new() -> Self {
        let sysroot = get_local_sysroot_dir();
        let target = match parse_args() {
            Some(x) => PathBuf::from(x),
            None => get_target(),
        };
        Self {
            rust_src: get_rust_src_dir(),
            target_dir: get_target_dir(sysroot.clone()),
            output_dir: get_output_dir(sysroot.clone(), &target),
            local_sysroot: sysroot,
            target: target,
        }
    }
}

#[allow(dead_code)]
fn generate_cargo_config(cfg: &BuildConfig) {
    let path = PathBuf::from(".cargo/config");
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    if path.exists() {
        // TODO: Be smarter, update existing. Warn?
        return;
    }

    let config = CargoConfig {
        build: Build {
            target: cfg.target.canonicalize().unwrap(),
            rustflags: vec![
                "--sysroot".to_owned(),
                format!("{}", cfg.local_sysroot.to_str().unwrap()),
            ],
        },
    };
    let toml = toml::to_string(&config).unwrap();

    let mut f = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .unwrap();
    f.write_all(toml.as_bytes()).unwrap();
}

fn build_liballoc(cfg: &BuildConfig) {
    let core = cfg.rust_src.join("libcore");
    let mut deps: BTreeMap<String, Dependency> = BTreeMap::new();
    let mut patch: BTreeMap<String, BTreeMap<String, Patch>> = BTreeMap::new();
    let coredep = Dependency {
        path: Some(core),
        ..Default::default()
    };
    let builtinsdep = Dependency {
        version: Some("0.1.0".into()),
        features: Some(vec!["core".into(), "mem".into()]),
        ..Default::default()
    };
    deps.insert("core".to_string(), coredep);
    deps.insert("compiler_builtins".to_string(), builtinsdep);
    patch.insert(
        "crates-io".into(),
        [(
            "rustc-std-workspace-core".into(),
            Patch {
                path: cfg.rust_src.join("tools").join("rustc-std-workspace-core"),
            },
        )]
        .iter()
        .cloned()
        .collect(),
    );
    let t = CargoToml {
        dependencies: deps,
        patch: patch,
        package: Some(Package {
            name: "alloc_builder".into(),
            version: "0.0.0".into(),
            metadata: None,
        }),
        lib: Lib {
            name: "alloc".into(),
            path: cfg.rust_src.join("liballoc").join("lib.rs"),
        },
    };
    let t = toml::to_string(&t).expect("Failed creating temp Cargo.toml");
    let path = cfg.target_dir.join("Cargo.toml");
    std::fs::create_dir_all(path.parent().expect("Impossible")).expect("Failed to create temp dir");
    fs::write(&path, t).expect("Failed writing temp Cargo.toml");
    //
    Command::new(env::var_os("CARGO").unwrap())
        .arg("rustc")
        .arg("--release")
        .arg("--target")
        .arg(&cfg.target)
        .arg("--target-dir")
        .arg(&cfg.target_dir)
        .arg("--manifest-path")
        .arg(path)
        .arg("--") // Pass to rusc directly.
        .arg("-Z")
        .arg("no-landing-pads")
        .status()
        .expect("Build failed.");
}

fn main() {
    // TODO: Eat output if up to date.
    let cfg = BuildConfig::new();
    println!("Checking libcore and libcompiler_builtins");
    // generate_cargo_config(&cfg);

    build_liballoc(&cfg);

    // copy_host_tools(cfg.local_sysroot.clone());
}
