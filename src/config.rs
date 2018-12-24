//! Handle configuration data
use serde_derive::{Deserialize, Serialize};
use std::{collections::BTreeMap, path::PathBuf};
// TODO: Factor this shit out into a seperate crate.

#[derive(Deserialize, Debug, Serialize, Default)]
pub struct CargoToml {
    pub package: Option<Package>,
    #[serde(skip_deserializing)]
    pub dependencies: BTreeMap<String, Dependency>,
    #[serde(skip_deserializing)]
    pub patch: BTreeMap<String, BTreeMap<String, Patch>>,
    #[serde(skip_deserializing)]
    pub lib: Lib,
}

#[derive(Deserialize, Debug, Serialize, Default)]
pub struct Lib {
    pub name: String,
    pub path: PathBuf,
}

#[derive(Deserialize, Debug, Serialize)]
pub struct Package {
    #[serde(skip_deserializing)]
    pub name: String,
    #[serde(skip_deserializing)]
    pub version: String,
    #[serde(skip_serializing)]
    pub metadata: Option<Metadata>,
}

#[derive(Deserialize, Debug, Serialize)]
pub struct Metadata {
    #[serde(rename = "cargo-sysroot")]
    pub cargo_sysroot: CargoSysroot,
}

#[derive(Deserialize, Debug, Serialize)]
pub struct CargoSysroot {
    pub target: PathBuf,
}

#[derive(Deserialize, Debug, Serialize, Default)]
#[serde(default)]
pub struct Dependency {
    pub path: Option<PathBuf>,
    pub version: Option<String>,
    pub features: Option<Vec<String>>,
}

#[derive(Deserialize, Debug, Serialize, Default, Clone)]
#[serde(default)]
pub struct Patch {
    pub path: PathBuf,
}

//

#[derive(Serialize, Deserialize, Debug)]
pub struct CargoConfig {
    pub build: Build,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Build {
    pub rustflags: Vec<String>,
    pub target: PathBuf,
}
