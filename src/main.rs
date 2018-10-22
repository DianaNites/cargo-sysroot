//! Cargo-SysRoot
//! Automatically compiles libcore and libcompiler_builtins before running cargo.
//!
//! Cargo.toml package.metadata.cargo-sysroot.target should be set
//! to the path of a Target Specification
//!
//! The sysroot is located in target/sysroot
//!
//! Cargo will automatically rebuild the project and all dependencies
//! if the files in the sysroot change.
extern crate toml;

use std::env;
use std::ffi::OsString;
use std::fs;
use std::io::prelude::*;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::str;
use toml::Value;

mod util;
use util::*;

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
    let target = toml.parse::<Value>().unwrap();
    let target = target["package"]["metadata"]["cargo-sysroot"]["target"]
        .as_str()
        .unwrap();
    PathBuf::from(target)
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
        let target = get_target();
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
    let mut lib = cfg.rust_src.clone();
    lib.push(name);
    lib.push("Cargo.toml");
    let flags = {
        let mut x = OsString::from("-Z no-landing-pads --sysroot ");
        x.push(&cfg.local_sysroot);
        x
    };
    let features: Vec<_> = {
        match features {
            Some(fs) => fs.into_iter().collect(),
            None => Default::default(),
        }
    };

    let mut x = get_rust_cmd("cargo");
    x.arg("build")
        .arg("--out-dir")
        .arg(&cfg.output_dir)
        .arg("--target-dir")
        .arg(&cfg.target_dir)
        .arg("--target")
        .arg(&cfg.target)
        .arg("--release")
        .arg("-Z")
        .arg("unstable-options")
        .env("RUSTFLAGS", flags);
    if !features.is_empty() {
        x.arg("--features");
        let mut s = String::new();
        for f in features {
            s.push_str(f.as_ref());
        }
        x.arg(s);
    }
    x.arg("--manifest-path").arg(lib);
    let _ = x.status();
}

fn main() {
    println!("Checking libcore and libcompiler_builtins");
    // TODO: Eat output if up to date.
    // TODO: Generate .cargo/config with rustflags.
    let cfg = BuildConfig::new();
    build("libcore", None, &cfg);
    build("libcompiler_builtins", Some(&["mem"]), &cfg);

    // TODO: Process help command.
    let _ = Command::new(env::var_os("CARGO").unwrap())
        // Skip self program and our subcommand.
        .args(env::args_os().skip(2))
        .status();
}
