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
    env, fs,
    io::prelude::*,
    path::{Path, PathBuf},
    process::Command,
    str,
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
    target.package.metadata.cargo_sysroot.target
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

/// Runs cargo build.
/// The package located at rust_src/`name`/Cargo.toml will be built.
fn build(name: &str, features: Option<&[&str]>, cfg: &BuildConfig) {
    let lib = {
        let mut x = cfg.rust_src.clone();
        x.push(name);
        x.push("Cargo.toml");
        x
    };
    let features: Vec<_> = {
        match features {
            Some(fs) => fs.into_iter().collect(),
            None => Default::default(),
        }
    };
    let mut cmd = Command::new(env::var_os("CARGO").unwrap());
    cmd.arg("rustc") //
        .arg("--release")
        .arg("--target")
        .arg(&cfg.target)
        .arg("--target-dir")
        .arg(&cfg.target_dir)
        .arg("--manifest-path")
        .arg(lib)
        .arg("-Z")
        .arg("unstable-options");
    if !features.is_empty() {
        cmd.arg("--features");
        let mut s = String::new();
        features.into_iter().for_each(|x| s.push_str(x));
        cmd.arg(s);
    }
    cmd.arg("--") // Pass to rusc directly.
        .arg("-Z")
        .arg("no-landing-pads");
    let _ = cmd.status().unwrap();
    //
    let rlib = {
        let mut x = cfg.target_dir.clone();
        x.push(cfg.target.file_stem().unwrap());
        x.push("release");
        x.push(name);
        x.set_extension("rlib");
        x
    };
    let out = {
        let mut x = cfg.output_dir.clone();
        x.push(name);
        x.set_extension("rlib");
        x
    };
    let _ = fs::copy(rlib, out).unwrap();
}

fn generate_cargo_config(cfg: &BuildConfig) {
    let path = PathBuf::from(".cargo/config");
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    if path.exists() {
        // TODO: Be smarter, update existing. Warn?
        return;
    }

    let config = CargoBuild {
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

fn main() {
    // TODO: Eat output if up to date.
    let cfg = BuildConfig::new();
    println!("Checking libcore and libcompiler_builtins");
    generate_cargo_config(&cfg);

    build("libcore", None, &cfg);
    build("libcompiler_builtins", Some(&["mem"]), &cfg);

    copy_host_tools(cfg.local_sysroot.clone());
}
