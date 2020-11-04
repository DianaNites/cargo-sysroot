//! # Cargo-Sysroot
//!
//! Compiles the Rust sysroot crates, core, compiler_builtins, and alloc.
//!
//! Cargo.toml package.metadata.cargo-sysroot.target should be set
//! to the path of a Target Specification
//!
//! The sysroot is located in `.target/sysroot`
use anyhow::*;
use cargo_toml2::{from_path, CargoToml};
use std::fs;
use structopt::{clap::AppSettings, StructOpt};

mod util;
use crate::util::*;
use cargo_sysroot::*;

#[derive(StructOpt, Debug)]
#[structopt(
    bin_name = "cargo",
    global_settings(&[
        AppSettings::ColoredHelp,
]))]
enum Args {
    Sysroot(Sysroot),
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
        args.rust_src_dir = Some(get_rust_src()?)
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
    let alloc_cargo_toml =
        generate_alloc_cargo_toml(&args).context("Failed to generate sysroot Cargo.toml")?;
    build_alloc(&alloc_cargo_toml, &args).context("Failed to build sysroot")?;

    // Copy host tools to the new sysroot, so that stuff like proc-macros and
    // testing can work.
    copy_host_tools(&args.sysroot_dir).context("Couldn't copy host tools to sysroot")?;

    Ok(())
}
