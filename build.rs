use anyhow::{anyhow, Context};
use const_gen::*;
use std::{env, fs, path::Path};
use tokio::runtime::Runtime;

// Necessary because of this issue: https://github.com/rust-lang/cargo/issues/9641
fn main() -> anyhow::Result<()> {
    if std::env::var_os("IDF_PATH").is_none() {
        return Err(anyhow!("You need to run IDF's export.sh before building"));
    }
    if std::env::var_os("MINI_RDK_WIFI_SSID").is_none() {
        std::env::set_var("MINI_RDK_WIFI_SSID", "Viam-2G");
        println!("cargo:rustc-env=MINI_RDK_WIFI_SSID=Viam-2G");
    }
    if std::env::var_os("MINI_RDK_WIFI_PASSWORD").is_none() {
        return Err(anyhow!(
            "please set the password for WiFi {}",
            std::env::var_os("MINI_RDK_WIFI_SSID")
                .unwrap()
                .to_str()
                .unwrap()
        ));
    }

    let content = std::fs::read_to_string("viam.json").context("can't read viam.json")?;
    let mut cfg = viam::config::reader::Config::new(content.as_str())?;
    let rt = Runtime::new()?;
    rt.block_on(cfg.read_certificates());
    let out_dir = std::env::var_os("OUT_DIR").unwrap();
    let dest_path = std::path::Path::new(&out_dir).join("ca.crt");
    let ca_cert = String::from(&cfg.cloud.tls_certificate) + "\0";
    std::fs::write(dest_path, ca_cert)?;
    let dest_path = std::path::Path::new(&out_dir).join("key.key");
    let key = String::from(&cfg.cloud.tls_private_key) + "\0";
    std::fs::write(dest_path, key)?;

    let out_dir = env::var_os("OUT_DIR").unwrap();
    let dest_path = Path::new(&out_dir).join("robot_secret.rs");
    let robot_decl = vec![
        const_declaration!(
            #[allow(clippy::redundant_static_lifetimes)]
            ROBOT_ID = cfg.cloud.id.as_str()
        ),
        const_declaration!(
            #[allow(clippy::redundant_static_lifetimes)]
            ROBOT_SECRET = cfg.cloud.secret.as_str()
        ),
    ]
    .join("\n");
    fs::write(&dest_path, robot_decl).unwrap();

    embuild::build::CfgArgs::output_propagated("ESP_IDF")?;
    embuild::build::LinkArgs::output_propagated("ESP_IDF")
}
