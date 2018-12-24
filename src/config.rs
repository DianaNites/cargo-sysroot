//! Handle configuration data
use serde_derive::{Deserialize, Serialize};
use std::{collections::BTreeMap, path::PathBuf};

#[derive(Deserialize, Debug, Serialize)]
pub struct CargoToml {
    pub package: Package,
    #[serde(default)]
    pub dependencies: BTreeMap<String, Dependency>,
    pub patch: BTreeMap<String, Patch>,
}

#[derive(Deserialize, Debug, Serialize)]
pub struct Package {
    pub metadata: Metadata,
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
    path: PathBuf,
    version: String,
    features: Vec<String>,
}

#[derive(Deserialize, Debug, Serialize, Default)]
#[serde(default)]
pub struct Patch {
    path: PathBuf,
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
