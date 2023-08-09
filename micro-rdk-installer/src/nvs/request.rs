use super::data::ViamFlashStorageData;

use rcgen::{date_time_ymd, CertificateParams, DistinguishedName};

use local_ip_address::local_ip;
use tokio::runtime::Runtime;
use viam::gen::proto::app::v1::{
    robot_service_client::RobotServiceClient, AgentInfo, CertificateRequest, ConfigRequest,
};
use viam_rust_utils::rpc::dial::{DialOptions, RPCCredentials};

/*
This module contains the logic for acquiring the security credentials for a robot from
the Viam App and preparing for flash storage
*/

pub fn populate_nvs_storage_from_app(
    storage_data: &mut ViamFlashStorageData,
) -> anyhow::Result<()> {
    populate_dtls_certificate(storage_data)?;
    let rt = Runtime::new()?;
    rt.block_on(read_cloud_config(storage_data))?;
    rt.block_on(read_certificates(storage_data))?;
    Ok(())
}

fn populate_dtls_certificate(storage_data: &mut ViamFlashStorageData) -> anyhow::Result<()> {
    let mut param: CertificateParams = CertificateParams::new(vec!["esp32".to_string()]);
    param.not_before = date_time_ymd(2021, 5, 19);
    param.not_after = date_time_ymd(4096, 1, 1);
    let mut dn = DistinguishedName::new();
    dn.push(rcgen::DnType::OrganizationName, "Viam");
    param.distinguished_name = dn;
    param.alg = &rcgen::PKCS_ECDSA_P256_SHA256;

    let kp = rcgen::KeyPair::generate(&rcgen::PKCS_ECDSA_P256_SHA256)?;
    let kp_der = kp.serialize_der();
    storage_data.robot_credentials.robot_dtls_key_pair = Some(kp_der);

    param.key_pair = Some(kp);

    let cert = rcgen::Certificate::from_params(param).unwrap();
    let cert_der = cert.serialize_der().unwrap();
    let fp = ring::digest::digest(&ring::digest::SHA256, &cert_der)
        .as_ref()
        .iter()
        .map(|b| format!("{:02X}", b))
        .collect::<Vec<String>>()
        .join(":");
    let fp = String::from("sha-256") + " " + &fp;
    storage_data.robot_credentials.robot_dtls_certificate = Some(cert_der);
    storage_data.robot_credentials.robot_dtls_certificate_fp = Some(fp);
    Ok(())
}

async fn read_cloud_config(storage_data: &mut ViamFlashStorageData) -> anyhow::Result<()> {
    // requires storage data to have already been populated with this
    // information from the robot's app config json
    let robot_id = storage_data.get_robot_id()?;
    let robot_secret = storage_data.get_robot_secret()?;
    let app_address = storage_data.get_app_address()?;

    let creds = RPCCredentials::new(
        Some(robot_id.clone()),
        "robot-secret".to_string(),
        robot_secret.clone(),
    );
    let agent = AgentInfo {
        os: "esp32-build".to_string(),
        host: gethostname::gethostname().to_str().unwrap().to_string(),
        ips: vec![local_ip().unwrap().to_string()],
        version: "0.0.1".to_string(),
        git_revision: "".to_string(),
        platform: Some("esp32-build".to_string()),
    };
    let cfg_req = ConfigRequest {
        agent_info: Some(agent),
        id: robot_id.clone(),
    };
    let dial = DialOptions::builder()
        .uri(app_address.clone().as_str())
        .with_credentials(creds)
        .disable_webrtc()
        .connect()
        .await?;
    let mut app_service = RobotServiceClient::new(dial);
    let cfg = app_service.config(cfg_req).await?.into_inner();
    let robot_config = cfg
        .config
        .ok_or(anyhow::Error::msg("no config for robot"))?;
    let cloud_cfg = robot_config.cloud.unwrap_or_default();
    storage_data.robot_credentials.robot_name = Some(
        cloud_cfg
            .local_fqdn
            .split('.')
            .next()
            .unwrap_or("")
            .to_string(),
    );
    storage_data.robot_credentials.local_fqdn =
        Some(cloud_cfg.local_fqdn.replace('.', "-").to_string());
    storage_data.robot_credentials.fqdn = Some(cloud_cfg.fqdn.replace('.', "-").to_string());
    Ok(())
}

async fn read_certificates(storage_data: &mut ViamFlashStorageData) -> anyhow::Result<()> {
    let robot_id = storage_data.get_robot_id()?;
    let robot_secret = storage_data.get_robot_secret()?;
    let app_address = storage_data.get_app_address()?;

    let creds = RPCCredentials::new(
        Some(robot_id.clone()),
        "robot-secret".to_string(),
        robot_secret.clone(),
    );
    let cert_req = CertificateRequest {
        id: robot_id.clone(),
    };
    let dial = DialOptions::builder()
        .uri(app_address.clone().as_str())
        .with_credentials(creds)
        .disable_webrtc()
        .connect()
        .await?;
    let mut app_service = RobotServiceClient::new(dial);
    let certs = app_service.certificate(cert_req).await?.into_inner();
    let tls_cert = &certs
        .tls_certificate
        .split_inclusive("----END CERTIFICATE-----");
    let srv_cert = &mut tls_cert.clone().take(2).collect::<String>();
    srv_cert.push('\0');
    let ca_cert = &mut tls_cert
        .clone()
        .take(1)
        .map(der::Document::from_pem)
        .filter(|s| s.is_ok())
        .map(|s| s.unwrap().1.to_vec())
        .collect::<Vec<Vec<u8>>>()
        .pop()
        .unwrap_or_default();
    let tls_private_key: &str = &certs.tls_private_key;
    let key = der::Document::from_pem(tls_private_key).map_or(vec![], |k| k.1.as_bytes().to_vec());
    storage_data.robot_credentials.ca_crt = Some(ca_cert.clone());
    storage_data.robot_credentials.der_key = Some(key.clone());
    Ok(())
}
