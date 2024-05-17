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
    println!("cargo:rerun-if-changed=viam.json");
    //    let mut has_robot_credentials = false;
    if env::var("TARGET").unwrap() == "xtensa-esp32-espidf" {
        if std::env::var_os("IDF_PATH").is_none() {
            panic!("You need to run IDF's export.sh before building");
        }
        if std::env::var_os("MICRO_RDK_WIFI_SSID").is_none()
            || std::env::var_os("MICRO_RDK_WIFI_PASSWORD").is_none()
            || std::env::var_os("CARGO_FEATURE_QEMU").is_some()
        {
            println!("cargo:rustc-env=MICRO_RDK_WIFI_SSID=");
            println!("cargo:rustc-env=MICRO_RDK_WIFI_PASSWORD=");
        }

        embuild::build::CfgArgs::output_propagated("MICRO_RDK").unwrap();
        embuild::build::LinkArgs::output_propagated("MICRO_RDK").unwrap();
    }

    if let Ok(content) = std::fs::read_to_string("viam.json") {
        if let Ok(cfg) = serde_json::from_str::<Config>(content.as_str()) {
            println!(
                "cargo:rustc-env=MICRO_RDK_ROBOT_SECRET={}",
                cfg.cloud.secret
            );
            println!("cargo:rustc-env=MICRO_RDK_ROBOT_ID={}", cfg.cloud.id);
            println!("cargo:rustc-cfg=has_robot_config");
        }
    }
}
