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
use clap::{crate_description, crate_name, crate_version, App, AppSettings, Arg, SubCommand};
use std::{collections::BTreeMap, env, fs, io::prelude::*, path::PathBuf, process::Command};

mod util;
use crate::util::*;

/// Returns Some is target was passed on the command line, None otherwise.
fn parse_args() -> (Option<String>, bool) {
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
                )
                .arg(
                    Arg::with_name("no_config")
                        .help("Disable .cargo/config generation")
                        .long("no-config"),
                ),
        )
        .get_matches();
    let matches = args.subcommand_matches("sysroot").expect("Impossible");
    (
        matches.value_of("target").map(|s| s.to_string()),
        matches.is_present("no_config"),
    )
}

/// Stuff the build command needs.
struct BuildConfig {
    rust_src: PathBuf,
    local_sysroot: PathBuf,
    target: PathBuf,
    target_dir: PathBuf,
    output_dir: PathBuf,
    no_config: bool,
    profile: Option<Profile>,
}

impl BuildConfig {
    fn new() -> Self {
        let sysroot = get_local_sysroot_dir();
        let toml: CargoToml = from_path("Cargo.toml").expect("Failed to read Cargo.toml");
        let (target, no_config) = parse_args();
        let target = match target {
            Some(x) => PathBuf::from(x),
            None => toml
                .package
                .metadata
                .expect("Missing cargo-sysroot metadata")
                .get("cargo-sysroot")
                .expect("Missing cargo-sysroot metadata")["target"]
                .as_str()
                .expect("Invalid cargo-sysroot metadata")
                .into(),
        };
        Self {
            rust_src: get_rust_src_dir(),
            target_dir: get_target_dir(&sysroot),
            output_dir: get_output_dir(&sysroot, &target),
            local_sysroot: sysroot,
            target: target,
            no_config: no_config,
            profile: toml.profile,
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
        build: Some(Build {
            target: Some(cfg.target.canonicalize().unwrap().to_str().unwrap().into()),
            rustflags: Some(vec![
                "--sysroot".to_owned(),
                format!("{}", cfg.local_sysroot.to_str().unwrap()),
            ]),
            ..Default::default()
        }),
        ..Default::default()
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
    let mut t = CargoToml {
        package: Package {
            name: "alloc".into(),
            version: "0.0.0".into(),
            authors: vec!["The Rust Project Developers".into()],
            edition: Some("2018".into()),
            ..Default::default()
        },
        lib: Some(TargetConfig {
            //.
            name: Some("alloc".into()),
            path: Some(cfg.rust_src.join("liballoc").join("lib.rs")),
            ..Default::default()
        }),
        dependencies: Some(BTreeMap::new()),
        patch: Some(Patches {
            sources: BTreeMap::new(),
        }),
        profile: cfg.profile.clone(),
        ..Default::default()
    };
    t.dependencies.as_mut().unwrap().insert(
        "core".into(),
        Dependency::Full(DependencyFull {
            path: Some(cfg.rust_src.join("libcore")),
            ..Default::default()
        }),
    );
    t.dependencies.as_mut().unwrap().insert(
        "compiler_builtins".into(),
        Dependency::Full(DependencyFull {
            version: Some("0.1.0".into()),
            features: Some(vec!["core".into(), "mem".into()]),
            ..Default::default()
        }),
    );
    t.patch
        .as_mut()
        .unwrap()
        .sources
        .insert("crates-io".into(), {
            let mut x = BTreeMap::new();
            x.insert(
                "rustc-std-workspace-core".to_string(),
                Dependency::Full(DependencyFull {
                    path: Some(cfg.rust_src.join("tools").join("rustc-std-workspace-core")),
                    ..Default::default()
                }),
            );
            x
        });
    //
    let t = toml::to_string(&t).expect("Failed creating temp Cargo.toml");
    let path = cfg.target_dir.join("Cargo.toml");
    std::fs::create_dir_all(path.parent().expect("Impossible")).expect("Failed to create temp dir");
    fs::write(&path, t).expect("Failed writing temp Cargo.toml");
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
        .arg("--") // Pass to rusc directly.
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

fn main() {
    // TODO: Eat output if up to date.
    let cfg = BuildConfig::new();
    println!("Building sysroot crates");
    if !cfg.no_config {
        generate_cargo_config(&cfg);
    }

    build_liballoc(&cfg);

    copy_host_tools(cfg.local_sysroot.clone());
}
