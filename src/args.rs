use cargo_toml2::Profile;
use std::path::PathBuf;
use structopt::{clap::AppSettings, StructOpt};

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
}

#[derive(StructOpt, Debug)]
#[structopt(
    bin_name = "cargo",
    global_settings(&[
        AppSettings::ColoredHelp,
]))]
pub enum Args {
    Sysroot(Sysroot),
}
