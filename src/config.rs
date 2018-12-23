//! Handle configuration data
use serde_derive::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Deserialize, Debug, Serialize)]
pub struct CargoToml {
    pub package: Package,
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

//

#[derive(Serialize, Deserialize, Debug)]
pub struct CargoBuild {
    pub build: Build,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Build {
    pub rustflags: Vec<String>,
    pub target: PathBuf,
}
