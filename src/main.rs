extern crate toml;

use std::env;
use std::fs;
use std::io::prelude::*;
use std::path::{Path, PathBuf};
use toml::Value;

fn get_target() -> PathBuf {
    let cargo = Path::new("Cargo.toml");
    // let cargo = Path::new(r#"C:\_Diana\Projects\diaos\Cargo.toml"#);
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
    cargo.with_file_name(target) //.canonicalize().unwrap()
}

fn main() {
    // HACK
    let _ = env::set_current_dir(r#"C:\_Diana\Projects\diaos"#);
    let target = get_target();

    println!("{:#?}", target);
}
