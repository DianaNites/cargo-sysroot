//! # Cargo-Sysroot
//!
//! Compiles the Rust sysroot crates, core, compiler_builtins, and alloc.
use anyhow::*;
use cargo_toml2::{
    from_path,
    to_path,
    CargoToml,
    Dependency,
    DependencyFull,
    Package,
    Patches,
    TargetConfig,
    Workspace,
};
use std::{
    collections::BTreeMap,
    env,
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
    process::Command,
};

mod util;

pub use util::get_rust_src;

/// The sysroot crates to build.
///
/// See [`build_sysroot_with`] for details.
#[derive(Debug, Copy, Clone)]
pub enum Sysroot {
    /// The core crate. Provides core functionality.
    ///
    /// This does **not** include [`Sysroot::CompilerBuiltins`],
    /// which is what you probably want unless your target
    /// needs special handling.
    Core,

    /// Compiler-builtins crate.
    ///
    /// This implies [`Sysroot::Core`].
    CompilerBuiltins,

    /// The alloc crate. Gives you a heap, and things to put on it.
    ///
    /// This implies [`Sysroot::Core`], and [`Sysroot::CompilerBuiltins`].
    Alloc,

    /// The standard library. Gives you an operating system.
    ///
    /// This implies [`Sysroot::Alloc`], [`Sysroot::Core`], and
    /// [`Sysroot::CompilerBuiltins`].
    Std,
}

/// Features to enable when building the sysroot crates
///
/// See [`SysrootBuilder::features`] for usage.
#[derive(Debug, Copy, Clone, PartialOrd, PartialEq, Ord, Eq)]
pub enum Features {
    /// This enables the `mem` feature of [`compiler_builtins`][1],
    /// which will provide memory related intrinsics such as `memcpy`.
    ///
    /// [1]: https://github.com/rust-lang/compiler-builtins
    CompilerBuiltinsMem,

    /// This enables the `c` feature of [`compiler_builtins`][1],
    /// which enables compilation of `C` code and may result in more
    /// optimized implementations, and fills in the rare unimplemented
    /// intrinsics.
    ///
    /// [1]: https://github.com/rust-lang/compiler-builtins
    CompilerBuiltinsC,

    /// This enables the `no-asm` feature of [`compiler_builtins`][1],
    /// which disables any implementations which use
    /// inline assembly and fall back to pure Rust versions (if available).
    ///
    /// [1]: https://github.com/rust-lang/compiler-builtins
    // TODO: Would only work on the [`Sysroot::CompilerBuiltins`] target,
    // as it's not exported through alloc, but could be forced by
    // adding compiler_builtins as an explicit dependency and enabling it,
    // relying on features collapsing.
    CompilerBuiltinsNoAsm,
}

/// A builder interface for constructing the Sysroot
///
/// See the individual methods for more details on what this means
/// and what defaults exist.
#[derive(Debug, Clone)]
pub struct SysrootBuilder {
    /// Manifest to use for cargo profiles
    manifest: Option<PathBuf>,

    /// Output directory, where the built sysroot will be anchored.
    output: PathBuf,

    /// Target triple/json to build for
    target: Option<PathBuf>,

    /// The rust sources to use
    rust_src: Option<PathBuf>,

    /// Which crates to include in the sysroot
    sysroot_crate: Sysroot,

    /// What custom features to enable, if any. See [`Features`] for details.
    features: Vec<Features>,

    /// Custom flags to pass to rustc.
    rustc_flags: Vec<OsString>,
}

impl SysrootBuilder {
    /// New [`SysrootBuilder`].
    ///
    /// `sysroot_crate` specifies which libraries to build as part of
    /// the sysroot. See [`Sysroot`] for more details.
    pub fn new(sysroot_crate: Sysroot) -> Self {
        Self {
            manifest: Default::default(),
            output: PathBuf::from(".").join("target").join("sysroot"),
            target: Default::default(),
            // Set in [`SysrootBuilder::build`] since `new` can't error.
            rust_src: Default::default(),
            sysroot_crate,
            features: Vec::with_capacity(3),
            rustc_flags: Default::default(),
        }
    }

    /// Set path to the `Cargo.toml` of the project requiring a custom sysroot.
    ///
    /// If provided, any [Cargo Profile's][1] in the provided manifest
    /// will be copied into the sysroot crate being compiled.
    ///
    /// If not provided, profiles use their default settings.
    ///
    /// By default this will be `None`.
    ///
    /// [1]: https://doc.rust-lang.org/stable/cargo/reference/profiles.html
    pub fn manifest(&mut self, manifest: PathBuf) -> &mut Self {
        self.manifest = Some(manifest);
        self
    }

    /// Set where the sysroot directory will be placed.
    ///
    /// By default this is `./target/sysroot`.
    pub fn output(&mut self, output: PathBuf) -> &mut Self {
        self.output = output;
        self
    }

    /// The target to compile *for*. This can be a target-triple,
    /// or a [JSON Target Specification][1].
    ///
    /// By default this is `None`, and if not set when
    /// [`SysrootBuilder::build`] is called, will cause an error.
    ///
    /// [1]: https://doc.rust-lang.org/rustc/targets/custom.html
    pub fn target(&mut self, target: PathBuf) -> &mut Self {
        self.target = Some(target);
        self
    }

    /// The rust source directory. These are used to compile the sysroot.
    ///
    /// By default this uses the `rust-src` component from the
    /// current `rustup` toolchain.
    pub fn rust_src(&mut self, rust_src: PathBuf) -> &mut Self {
        self.rust_src = Some(rust_src);
        self
    }

    /// Which features to enable.
    ///
    /// This *adds* to, not *replaces*, any previous calls to this method.
    ///
    /// By default this is empty.
    ///
    /// See [`Features`] for details.
    pub fn features(&mut self, features: &[Features]) -> &mut Self {
        self.features.extend_from_slice(features);
        // TODO: Should?? Not??
        self.features.sort_unstable();
        self.features.dedup();
        self
    }

    /// Custom flags to pass to **all** `rustc` compiler invocations.
    ///
    /// This *adds* to, not *replaces*, any previous calls to this method.
    ///
    /// By default this is empty.
    ///
    /// # Internal
    ///
    /// This will use the `RUSTFLAGS` to ensure flags are set for all
    /// invocations.
    ///
    /// Flags passed to this method will be *appended*
    /// to any existing `RUSTFLAGS`.
    pub fn rustc_flags<I, S>(&mut self, flags: I) -> &mut Self
    where
        I: IntoIterator<Item = S>,
        S: Into<OsString>,
    {
        self.rustc_flags.extend(flags.into_iter().map(Into::into));
        self
    }

    /// Build the Sysroot, and return a path suitable to pass to rustc.
    ///
    /// # Errors
    ///
    /// - [`SysrootBuilder::target`] was not called
    /// - If `manifest` is provided and does not exist
    /// - If `target` is a JSON specification, but doesn't exist.
    /// - If the `rust_src` directory does not exist, or could not be detected.
    /// - If the sysroot cannot be setup, or fails to compile
    pub fn build(&self) -> Result<PathBuf> {
        let target = match &self.target {
            Some(t) => t,
            None => return Err(anyhow!("SysrootBuilder::target was not called")),
        };
        if let Some(manifest) = &self.manifest {
            if !manifest.exists() {
                return Err(anyhow!(
                    "Provided manifest did not exist. Path: {}",
                    manifest.display()
                ));
            }
        }
        // If `target` has an extension, assume target spec...
        if target.extension().is_some() {
            // ...and check if it exists.
            if !target.exists() {
                return Err(anyhow!(
                    "Provided JSON Target Specification did not exist: {}",
                    target.display()
                ));
            }
        }
        let rust_src = match &self.rust_src {
            Some(s) => {
                if !s.exists() {
                    return Err(anyhow!(
                        "The provided rust-src directory did not exist: {}",
                        s.display()
                    ));
                }
                s.clone()
            }
            None => {
                let src = util::get_rust_src().context("Could not detect appropriate rust-src")?;
                if !src.exists() {
                    return Err(anyhow!("Rust-src component not installed?"));
                }
                src
            }
        };
        fs::create_dir_all(&self.output).context("Couldn't create sysroot output directory")?;
        fs::create_dir_all(artifact_dir(&self.output, target)?)
            .context("Failed to setup sysroot directory structure")?;

        let sysroot_cargo_toml = generate_sysroot_cargo_toml(&SysrootBuilder {
            // HACK: So it can see auto-detected rust-src.
            rust_src: Some(rust_src),
            ..self.clone()
        })?;
        build_alloc(&sysroot_cargo_toml, &self).context("Failed to build sysroot")?;

        // Copy host tools to the new sysroot, so that stuff like proc-macros and
        // testing can work.
        util::copy_host_tools(&self.output).context("Couldn't copy host tools to sysroot")?;
        Ok(self.output.canonicalize().with_context(|| {
            format!(
                "Couldn't get canonical path to sysroot: {}",
                self.output.display()
            )
        })?)
    }
}

/// Generate a Cargo.toml for building the sysroot crates
///
/// Should ONLY be called by [`SysrootBuilder::build`].
///
/// See [`build_sysroot_with`].
fn generate_sysroot_cargo_toml(builder: &SysrootBuilder) -> Result<PathBuf> {
    fs::write(
        builder.output.join("lib.rs"),
        "#![feature(no_core)]\n#![no_core]",
    )?;
    let toml = CargoToml {
        package: Package {
            name: "Sysroot".into(),
            version: "0.0.0".into(),
            authors: vec!["The Rust Project Developers".into(), "DianaNites".into()],
            edition: Some("2018".into()),
            autotests: Some(false),
            autobenches: Some(false),
            ..Default::default()
        },
        lib: Some(TargetConfig {
            name: Some("sysroot".into()),
            path: Some("lib.rs".into()),
            ..Default::default()
        }),
        workspace: Some(Workspace::default()),
        dependencies: Some({
            let mut deps = BTreeMap::new();
            match builder.sysroot_crate {
                Sysroot::Core => {
                    deps.insert(
                        "core".into(),
                        Dependency::Full(DependencyFull {
                            path: Some(builder.rust_src.as_ref().unwrap().join("core")),
                            ..Default::default()
                        }),
                    );
                }

                Sysroot::CompilerBuiltins => {
                    deps.insert(
                        "compiler_builtins".into(),
                        Dependency::Full(DependencyFull {
                            version: Some("0.1".into()),
                            features: {
                                let mut f = vec!["rustc-dep-of-std".into()];
                                if builder.features.contains(&Features::CompilerBuiltinsMem) {
                                    f.push("mem".into());
                                }
                                Some(f)
                            },
                            ..Default::default()
                        }),
                    );
                }

                Sysroot::Alloc => {
                    deps.insert(
                        "alloc".into(),
                        Dependency::Full(DependencyFull {
                            path: Some(builder.rust_src.as_ref().unwrap().join("alloc")),
                            features: if builder.features.contains(&Features::CompilerBuiltinsMem) {
                                Some(vec!["compiler-builtins-mem".into()])
                            } else {
                                None
                            },
                            ..Default::default()
                        }),
                    );
                }

                Sysroot::Std => {
                    deps.insert(
                        "std".into(),
                        Dependency::Full(DependencyFull {
                            path: Some(builder.rust_src.as_ref().unwrap().join("std")),
                            features: if builder.features.contains(&Features::CompilerBuiltinsMem) {
                                Some(vec!["compiler-builtins-mem".into()])
                            } else {
                                None
                            },
                            ..Default::default()
                        }),
                    );
                }
            }
            deps
        }),
        patch: Some(Patches {
            sources: if let Sysroot::Core = builder.sysroot_crate {
                BTreeMap::new()
            } else {
                let mut sources = BTreeMap::new();
                sources.insert("crates-io".into(), {
                    let mut x = BTreeMap::new();
                    x.insert(
                        "rustc-std-workspace-core".to_string(),
                        Dependency::Full(DependencyFull {
                            path: Some(
                                builder
                                    .rust_src
                                    .as_ref()
                                    .unwrap()
                                    .join("rustc-std-workspace-core"),
                            ),
                            ..Default::default()
                        }),
                    );
                    x
                });
                sources
            },
        }),
        profile: {
            match &builder.manifest {
                Some(manifest) => {
                    let toml: CargoToml =
                        from_path(manifest).with_context(|| manifest.display().to_string())?;
                    toml.profile
                }
                None => None,
            }
        },
        ..Default::default()
    };
    let path = builder.output.join("Cargo.toml");
    to_path(&path, &toml).context("Failed writing sysroot Cargo.toml")?;
    Ok(path)
}

/// The entry-point for building the alloc crate, which builds all the others
///
/// Should ONLY be called by [`SysrootBuilder::build`].
fn build_alloc(alloc_cargo_toml: &Path, builder: &SysrootBuilder) -> Result<()> {
    let path = alloc_cargo_toml;
    let triple = builder.target.as_ref().unwrap();
    let target_dir = builder.output.join("target");

    // TODO: Eat output if up to date? Always? On error?
    let exit = Command::new(env::var_os("CARGO").unwrap_or_else(|| "cargo".into()))
        .arg("rustc")
        .arg("--release")
        .arg("--target")
        // If it doesn't work, assume it's a builtin path?
        .arg(&triple.canonicalize().unwrap_or_else(|_| triple.into()))
        .arg("--target-dir")
        .arg(&target_dir)
        .arg("--manifest-path")
        .arg(path)
        .arg("--") // Pass to rustc directly.
        .arg("-Z")
        // The rust build system only passes this for rustc? xbuild passes this for alloc. 🤷‍♀️
        .arg("force-unstable-if-unmarked")
        .env("RUSTFLAGS", {
            let mut env = OsString::from("-Cembed-bitcode=yes");
            if let Some(exist) = std::env::var_os("RUSTFLAGS") {
                env.push(" ");
                env.push(exist);
            }
            for flag in &builder.rustc_flags {
                env.push(" ");
                env.push(flag)
            }
            env
        })
        .status()
        .context("Couldn't find/run cargo command")?;
    if !exit.success() {
        return Err(anyhow!(
            "Failed to build sysroot: Exit code {}",
            exit.code()
                .map(|i| i.to_string())
                .unwrap_or_else(|| "Killed by signal".to_string())
        ));
    }

    // Copy artifacts to sysroot.
    for entry in fs::read_dir(
        target_dir
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
            let out = artifact_dir(&builder.output, &triple)?.join(name);
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

/// The output artifact directory
///
/// Not part of the public API.
fn artifact_dir(sysroot_dir: &Path, target: &Path) -> Result<PathBuf> {
    Ok(sysroot_dir
        .join("lib")
        .join("rustlib")
        .join(target.file_stem().context("Invalid Target Specification")?)
        .join("lib"))
}

/// Clean up generated sysroot artifacts.
///
/// Should be called before [`build_sysroot`] if you want this behavior.
pub fn clean_artifacts(sysroot_dir: &Path) -> Result<()> {
    // Clean-up old artifacts
    match fs::remove_dir_all(sysroot_dir) {
        Ok(_) => (),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => (),
        e => e.context("Couldn't clean sysroot artifacts")?,
    };
    Ok(())
}
