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
use structopt::StructOpt;

#[allow(dead_code)]
mod util;
use crate::{args::*, util::get_rust_src};
use cargo_sysroot::*;

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
    args.sysroot_artifact_dir = Some(artifact_dir(
        &args.sysroot_dir,
        args.target
            .as_ref()
            .context("BUG: Somehow missing target triple")?,
    )?);

    clean_artifacts(&args.sysroot_dir)?;

    let args = args;

    println!("Building sysroot crates");
    if !args.no_config {
        generate_cargo_config(args.target.as_ref().unwrap(), &args.sysroot_dir)
            .context("Couldn't create .cargo/config.toml")?;
    }

    build_sysroot_with(
        &args.manifest_path,
        &args.sysroot_dir,
        args.target
            .as_ref()
            .context("BUG: Somehow missing target triple")?,
        args.rust_src_dir.as_ref().unwrap(),
    )?;

    Ok(())
}
