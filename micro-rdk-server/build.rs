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
    println!("cargo:rerun-if-changed=viam.json");
    println!("cargo:rerun-if-env-changed=MICRO_RDK_WIFI_SSID");
    println!("cargo:rerun-if-env-changed=MICRO_RDK_WIFI_PASSWORD");

    if Regex::new(r"\w+-esp3?2?s?\d?-espidf")
        .unwrap()
        .is_match(&env::var("TARGET").unwrap())
    {
        if !std::env::var_os("CARGO_FEATURE_QEMU").is_some() {
            if std::env::var_os("MICRO_RDK_WIFI_PASSWORD")
                .or(std::env::var_os("MICRO_RDK_WIFI_SSID"))
                .is_some()
                && std::env::var_os("MICRO_RDK_WIFI_SSID")
                    .zip(std::env::var_os("MICRO_RDK_WIFI_PASSWORD"))
                    .is_none()
            {
                panic!("Both or none of environment variables MICRO_RDK_WIFI_SSID and MICRO_RDK_WIFI_PASSWORD should be set");
            }
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
        }
    }
}
