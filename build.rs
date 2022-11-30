use anyhow::{anyhow, Context};
use const_gen::*;
use serde::{Deserialize, Serialize};
use std::{env, fs, path::Path};
use tokio::runtime::Runtime;
use viam::gen::proto::app::v1::{robot_service_client::RobotServiceClient, CertificateRequest};
use viam_rust_utils::rpc::dial::{DialOptions, RPCCredentials};

#[derive(Serialize, Deserialize, Debug)]
pub struct Config {
    pub cloud: Cloud,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Cloud {
    pub id: String,
    pub secret: String,
    #[serde(default)]
    pub location_secret: String,
    #[serde(default)]
    managed_by: String,
    #[serde(default)]
    fqdn: String,
    #[serde(default)]
    local_fqdn: String,
    #[serde(default)]
    signaling_address: String,
    #[serde(default)]
    signaling_insecure: bool,
    #[serde(default)]
    path: String,
    #[serde(default)]
    log_path: String,
    app_address: String,
    #[serde(default)]
    refresh_interval: String,
    #[serde(default)]
    pub tls_certificate: String,
    #[serde(default)]
    pub tls_private_key: String,
}

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

    let mut cfg: Config = serde_json::from_str(content.as_str()).map_err(anyhow::Error::msg)?;

    let rt = Runtime::new()?;
    rt.block_on(read_certificates(&mut cfg))?;
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

async fn read_certificates(config: &mut Config) -> anyhow::Result<()> {
    let creds = RPCCredentials::new(
        Some(config.cloud.id.clone()),
        "robot-secret".to_string(),
        config.cloud.secret.clone(),
    );
    let cert_req = CertificateRequest {
        id: config.cloud.id.clone(),
    };
    let dial = DialOptions::builder()
        .uri(config.cloud.app_address.clone().as_str())
        .with_credentials(creds)
        .disable_webrtc()
        .connect()
        .await?;
    let mut app_service = RobotServiceClient::new(dial);
    let certs = app_service.certificate(cert_req).await?.into_inner();
    config.cloud.tls_certificate = certs.tls_certificate;
    config.cloud.tls_private_key = certs.tls_private_key;
    Ok(())
}
