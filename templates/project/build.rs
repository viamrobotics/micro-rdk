use cargo_metadata::{CargoOpt, DependencyKind, MetadataCommand};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::env;

#[derive(Serialize, Deserialize, Debug, Default)]
pub struct Config {
    pub cloud: Cloud,
}

#[derive(Serialize, Deserialize, Debug, Default)]
pub struct Cloud {
    pub id: String,
    pub secret: String,
    pub app_address: String,
}

fn main() {
    println!("cargo:rerun-if-env-changed=MICRO_RDK_WIFI_SSID");
    println!("cargo:rerun-if-env-changed=MICRO_RDK_WIFI_PASSWORD");
    println!("cargo:rerun-if-changed=viam.json");

    if std::env::var_os("MICRO_RDK_WIFI_PASSWORD")
        .or(std::env::var_os("MICRO_RDK_WIFI_SSID"))
        .is_none()
        && std::env::var_os("MICRO_RDK_WIFI_SSID")
            .zip(std::env::var_os("MICRO_RDK_WIFI_PASSWORD"))
            .is_none()
    {
        panic!("Both environment variables MICRO_RDK_WIFI_SSID and MICRO_RDK_WIFI_PASSWORD should be set");
    }

    embuild::build::CfgArgs::output_propagated("MICRO_RDK").unwrap();
    embuild::build::LinkArgs::output_propagated("MICRO_RDK").unwrap();

    if let Ok(content) = std::fs::read_to_string("viam.json") {
        if let Ok(cfg) = serde_json::from_str::<Config>(content.as_str()) {
            println!(
                "cargo:rustc-env=MICRO_RDK_ROBOT_SECRET={}",
                cfg.cloud.secret
            );
            println!("cargo:rustc-env=MICRO_RDK_ROBOT_ID={}", cfg.cloud.id);
        }
    } else {
        panic!("`viam.json` configuration file not found in project root directory");
    }

    // Dynamic Module Magic
    let metadata = MetadataCommand::new()
        .manifest_path("Cargo.toml")
        .features(CargoOpt::AllFeatures)
        .exec()
        .expect("cannot load Cargo.toml metadata");

    let root_package_id = metadata
        .root_package()
        .expect("Failed to get ID of root package")
        .id
        .clone();

    let viam_modules: Vec<_> = metadata
        // Obtain the dependency graph from the metadata and iterate its nodes
        .resolve
        .as_ref()
        .expect("Dependencies were not resolved")
        .nodes
        .iter()
        // Until we find the root node..
        .find(|node| node.id == root_package_id)
        .expect("Root package not found in dependencies")
        // Then iterate the root node's dependencies, selecting only those
        // that are normal dependencies.
        .deps
        .iter()
        .filter(|dep| {
            dep.dep_kinds
                .iter()
                .any(|dk| dk.kind == DependencyKind::Normal)
        })
        // And which have a populated `package.metadata.com.viam` section in their Cargo.toml
        // which has `module = true`
        .filter(|dep| {
            metadata[&dep.pkg].metadata["com"]["viam"]["module"]
                .as_bool()
                .unwrap_or(false)
        })
        .collect();
    let out_dir = env::var_os("OUT_DIR").expect("OUT_DIR environment variable unset");

    let mut modules_rs_content = String::new();
    let module_name_seq = viam_modules
        .iter()
        .map(|m| m.name.replace('-', "_"))
        .collect::<Vec<_>>()
        .join(", \n\t");
    modules_rs_content.push_str(&format!(
        "generate_register_modules!(\n\t{module_name_seq}\n);\n"
    ));
    let dest_path = std::path::Path::new(&out_dir).join("modules.rs");
    std::fs::write(dest_path, modules_rs_content).expect("couldn't write modules.rs file");
}
