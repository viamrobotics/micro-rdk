use cargo_metadata::{MetadataCommand, CargoOpt};
use const_gen::*;
use local_ip_address::local_ip;
use rcgen::{date_time_ymd, CertificateParams, DistinguishedName};
use serde::{Deserialize, Serialize};
use std::{env, fs, path::Path};
use tokio::runtime::Runtime;
use viam::gen::proto::app::v1::{
    robot_service_client::RobotServiceClient, AgentInfo, CertificateRequest, ConfigRequest,
    RobotConfig,
};
use viam_rust_utils::rpc::dial::{DialOptions, RPCCredentials};

#[derive(Serialize, Deserialize, Debug, Default)]
pub struct Config {
    pub cloud: Cloud,
}

#[derive(Serialize, Deserialize, Debug, Default)]
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

fn main() -> anyhow::Result<()> {
    println!("cargo:rerun-if-changed=viam.json");

    if std::env::var_os("IDF_PATH").is_none() {
        return Err(anyhow::anyhow!("You need to run IDF's export.sh before building"));
    }
    if std::env::var_os("MICRO_RDK_WIFI_SSID").is_none() {
        std::env::set_var("MICRO_RDK_WIFI_SSID", "{{ssid}}");
        println!("cargo:rustc-env=MICRO_RDK_WIFI_SSID={{ssid}}");
    }

    {% if pwd != ""  %}println!("cargo:rustc-env=MICRO_RDK_WIFI_PASSWORD={{pwd}}"); {% else %}
    if std::env::var_os("MICRO_RDK_WIFI_PASSWORD").is_none() {
        return Err(anyhow::anyhow!(
            "please set the password for WiFi {}",
            std::env::var_os("MICRO_RDK_WIFI_SSID")
                .unwrap()
                .to_str()
                .unwrap()
        ));
    }
    {% endif %}

    embuild::build::CfgArgs::output_propagated("MICRO_RDK")?;
    embuild::build::LinkArgs::output_propagated("MICRO_RDK")?;

    let (cert_der, kp_der, fp) = generate_dtls_certificate()?;

    let (robot_cfg, cfg) = if let Ok(content) = std::fs::read_to_string("viam.json") {
        let mut cfg: Config = serde_json::from_str(content.as_str()).map_err(anyhow::Error::msg)?;

        let rt = Runtime::new()?;
        let robot_cfg = rt.block_on(read_cloud_config(&mut cfg))?;
        rt.block_on(read_certificates(&mut cfg))?;
        (robot_cfg, cfg)
    } else {
        (RobotConfig::default(), Config::default())
    };

    let cloud_cfg = robot_cfg.cloud.unwrap_or_default();
    let robot_name = cloud_cfg.local_fqdn.split('.').next().unwrap_or("");
    let local_fqdn = cloud_cfg.local_fqdn.replace('.', "-");
    let fqdn = cloud_cfg.fqdn.replace('.', "-");

    let mut certs = cfg
        .cloud
        .tls_certificate
        .split_inclusive("----END CERTIFICATE-----");
    let mut srv_cert = (&mut certs).take(2).collect::<String>();
    srv_cert.push('\0');
    let ca_cert = certs
        .take(1)
        .map(der::Document::from_pem)
        .filter(|s| s.is_ok())
        .map(|s| s.unwrap().1.to_vec())
        .collect::<Vec<Vec<u8>>>()
        .pop()
        .unwrap_or_default();
    let key = der::Document::from_pem(&cfg.cloud.tls_private_key)
        .map_or(vec![], |k| k.1.as_bytes().to_vec());

    let out_dir = env::var_os("OUT_DIR").unwrap();
    let dest_path = Path::new(&out_dir).join("robot_secret.rs");
    let robot_decl = vec![
        const_declaration!(
            #[allow(clippy::redundant_static_lifetimes, dead_code)]
            ROBOT_ID = cfg.cloud.id.as_str()
        ),
        const_declaration!(
            #[allow(clippy::redundant_static_lifetimes, dead_code)]
            ROBOT_SECRET = cfg.cloud.secret.as_str()
        ),
        const_declaration!(
            #[allow(clippy::redundant_static_lifetimes, dead_code)]
            LOCAL_FQDN = local_fqdn.as_str()
        ),
        const_declaration!(
            #[allow(clippy::redundant_static_lifetimes, dead_code)]
            FQDN = fqdn.as_str()
        ),
        const_declaration!(
            #[allow(clippy::redundant_static_lifetimes, dead_code)]
            ROBOT_NAME = robot_name
        ),
        const_declaration!(
            #[allow(clippy::redundant_static_lifetimes, dead_code)]
            ROBOT_DTLS_CERT = cert_der
        ),
        const_declaration!(
            #[allow(clippy::redundant_static_lifetimes, dead_code)]
            ROBOT_DTLS_KEY_PAIR = kp_der
        ),
        const_declaration!(
            #[allow(clippy::redundant_static_lifetimes, dead_code)]
            ROBOT_DTLS_CERT_FP = fp
        ),
        const_declaration!(
            #[allow(clippy::redundant_static_lifetimes, dead_code)]
            ROBOT_SRV_PEM_CHAIN = srv_cert.as_bytes()
        ),
        const_declaration!(
            #[allow(clippy::redundant_static_lifetimes, dead_code)]
            ROBOT_SRV_PEM_CA = ca_cert
        ),
        const_declaration!(
            #[allow(clippy::redundant_static_lifetimes, dead_code)]
            ROBOT_SRV_DER_KEY = key
        ),
    ]
    .join("\n");
    fs::write(dest_path, robot_decl).unwrap();

    let metadata = MetadataCommand::new()
        .manifest_path("Cargo.toml")
        .features(CargoOpt::AllFeatures)
        .exec()
        .unwrap();

    let root_package_id = metadata
        .root_package()
        .ok_or(anyhow::anyhow!("Failed to get ID of root package"))?
        .id
        .clone();

    let viam_modules : Vec<_> = metadata
        // Obtain the dependency graph from the metadata and iterate its nodes
        .resolve.as_ref()
        .ok_or(anyhow::anyhow!("Dependencies were not resolved"))?
        .nodes.iter()
        // Until we find the root node..
        .find(|node| node.id == root_package_id)
        .ok_or(anyhow::anyhow!("Root package not found in dependencies"))?
        // Then iterate the root node's dependencies, selecting only those
        // that are normal dependencies.
        .deps.iter()
        .filter(|dep| dep.dep_kinds.iter().any(|dk| dk.kind == cargo_metadata::DependencyKind::Normal))
        // And which have a populated `package.metadata.com.viam` section in their Cargo.toml
        // which has `module = true`
        .filter(|dep| metadata[&dep.pkg].metadata["com"]["viam"]["module"].as_bool().unwrap_or(false))
        .collect();

    let mut modules_rs_content = String::new();
    let module_name_seq = viam_modules.iter().map(|m| m.name.replace('-', "_")).collect::<Vec<_>>().join(", \n\t");
    modules_rs_content.push_str(&format!("generate_register_modules!(\n\t{module_name_seq}\n);\n"));
    let dest_path = Path::new(&out_dir).join("modules.rs");
    fs::write(dest_path, modules_rs_content).unwrap();

    Ok(())
}

fn generate_dtls_certificate() -> anyhow::Result<(Vec<u8>, Vec<u8>, String)> {
    let mut param: CertificateParams = CertificateParams::new(vec!["esp32".to_string()]);
    param.not_before = date_time_ymd(2021, 5, 19);
    param.not_after = date_time_ymd(4096, 1, 1);
    let mut dn = DistinguishedName::new();
    dn.push(rcgen::DnType::OrganizationName, "Viam");
    param.distinguished_name = dn;
    param.alg = &rcgen::PKCS_ECDSA_P256_SHA256;

    let kp = rcgen::KeyPair::generate(&rcgen::PKCS_ECDSA_P256_SHA256)?;
    let kp_der = kp.serialize_der();

    param.key_pair = Some(kp);

    let cert = rcgen::Certificate::from_params(param).unwrap();
    let cert_der = cert.serialize_der().unwrap();
    let fp = ring::digest::digest(&ring::digest::SHA256, &cert_der)
        .as_ref()
        .iter()
        .map(|b| format!("{b:02X}"))
        .collect::<Vec<String>>()
        .join(":");
    let fp = String::from("sha-256") + " " + &fp;

    Ok((cert_der, kp_der, fp))
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

async fn read_cloud_config(config: &mut Config) -> anyhow::Result<RobotConfig> {
    let creds = RPCCredentials::new(
        Some(config.cloud.id.clone()),
        "robot-secret".to_string(),
        config.cloud.secret.clone(),
    );
    let agent = AgentInfo {
        os: "esp32-build".to_string(),
        host: gethostname::gethostname().to_str().unwrap().to_string(),
        ips: vec![local_ip().unwrap().to_string()],
        version: "0.0.1".to_string(),
        git_revision: "".to_string(),
        ..Default::default()
    };
    let cfg_req = ConfigRequest {
        agent_info: Some(agent),
        id: config.cloud.id.clone(),
    };
    let dial = DialOptions::builder()
        .uri(config.cloud.app_address.clone().as_str())
        .with_credentials(creds)
        .disable_webrtc()
        .connect()
        .await?;
    let mut app_service = RobotServiceClient::new(dial);
    let cfg = app_service.config(cfg_req).await?.into_inner();
    match cfg.config {
        Some(cfg) => Ok(cfg),
        None => anyhow::bail!("no config for robot"),
    }
}
