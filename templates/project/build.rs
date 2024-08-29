use cargo_metadata::{CargoOpt, DependencyKind, MetadataCommand};
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

fn main() -> Result<(), &'static str> {
    println!("cargo:rerun-if-changed=Cargo.toml");
    println!("cargo:rerun-if-env-changed=MICRO_RDK_WIFI_SSID");
    println!("cargo:rerun-if-env-changed=MICRO_RDK_WIFI_PASSWORD");
    println!("cargo:rerun-if-changed=viam.json");

    if std::env::var_os("MICRO_RDK_WIFI_PASSWORD").is_none() {
        let pass: &'static str = "{{pwd}}";
        if !(pass.len() > 0) {
            return Err("WiFi password wasn't set during the template generation. You can set with MICRO_RDK_WIFI_PASSWORD environment variable");
        }
        println!("cargo:rustc-env=MICRO_RDK_WIFI_PASSWORD={}", pass);
    }

    if std::env::var_os("MICRO_RDK_WIFI_SSID").is_none() {
        let ssid: &'static str = "{{ssid}}";
        if !(ssid.len() > 0) {
            return Err("WiFi ssid wasn't set during the template generation. You can set with MICRO_RDK_WIFI_SSID environment variable");
        }
        println!("cargo:rustc-env=MICRO_RDK_WIFI_SSID={}", ssid);
    }

    if let Ok(content) = std::fs::read_to_string("viam.json") {
        if let Ok(cfg) = serde_json::from_str::<Config>(content.as_str()) {
            println!(
                "cargo:rustc-env=MICRO_RDK_ROBOT_SECRET={}",
                cfg.cloud.secret
            );
            println!("cargo:rustc-env=MICRO_RDK_ROBOT_ID={}", cfg.cloud.id);
        } else {
            return Err("`viam.json` is empty or it's content are invalid, re download it from app.viam.com");
        }
    } else {
        return Err("`viam.json` configuration file not found in project root directory");
    }

    embuild::build::CfgArgs::output_propagated("MICRO_RDK").unwrap();
    embuild::build::LinkArgs::output_propagated("MICRO_RDK").unwrap();

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
    Ok(())
}
