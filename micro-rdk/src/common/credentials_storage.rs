#![allow(dead_code)]
use std::str::FromStr;
use std::{convert::Infallible, error::Error, fmt::Debug, rc::Rc, sync::Mutex};

use hyper::Uri;

use crate::{common::grpc::ServerError, proto::app::v1::RobotConfig};

use crate::proto::{
    app::v1::CertificateResponse,
    provisioning::v1::{CloudConfig, SetNetworkCredentialsRequest},
};

#[cfg(feature = "ota")]
use crate::common::ota::OtaMetadata;

#[derive(Clone, Default, Debug)]
pub struct RobotCredentials {
    pub(crate) robot_id: String,
    pub(crate) robot_secret: String,
}

impl RobotCredentials {
    pub fn new(robot_id: String, robot_secret: String) -> Self {
        Self {
            robot_secret,
            robot_id,
        }
    }

    pub(crate) fn robot_id(&self) -> &str {
        &self.robot_id
    }

    pub(crate) fn robot_secret(&self) -> &str {
        &self.robot_secret
    }
}

impl From<SetNetworkCredentialsRequest> for WifiCredentials {
    fn from(value: SetNetworkCredentialsRequest) -> Self {
        Self {
            ssid: value.ssid,
            pwd: value.psk,
        }
    }
}

impl TryFrom<CloudConfig> for RobotCredentials {
    type Error = <Uri as FromStr>::Err;
    fn try_from(value: CloudConfig) -> Result<Self, Self::Error> {
        Ok(Self {
            robot_id: value.id,
            robot_secret: value.secret,
        })
    }
}

impl From<RobotCredentials> for CloudConfig {
    fn from(value: RobotCredentials) -> Self {
        Self {
            app_address: "".to_string(),
            id: value.robot_id,
            secret: value.robot_secret,
        }
    }
}

#[derive(Clone, Default)]
pub struct WifiCredentials {
    pub(crate) ssid: String,
    pub(crate) pwd: String,
}

impl WifiCredentials {
    pub fn new(ssid: String, pwd: String) -> Self {
        Self { ssid, pwd }
    }
    pub(crate) fn wifi_ssid(&self) -> &str {
        &self.ssid
    }
    pub(crate) fn wifi_pwd(&self) -> &str {
        &self.pwd
    }
}

#[derive(Clone, Debug, Default)]
pub struct TlsCertificate {
    pub(crate) certificate: Vec<u8>,
    pub(crate) private_key: Vec<u8>,
}

impl From<CertificateResponse> for TlsCertificate {
    fn from(resp: CertificateResponse) -> Self {
        Self {
            certificate: resp.tls_certificate.into_bytes(),
            private_key: resp.tls_private_key.into_bytes(),
        }
    }
}

pub trait WifiCredentialStorage {
    type Error: Error + Debug + Into<ServerError>;
    fn has_wifi_credentials(&self) -> bool;
    fn store_wifi_credentials(&self, creds: WifiCredentials) -> Result<(), Self::Error>;
    fn get_wifi_credentials(&self) -> Result<WifiCredentials, Self::Error>;
    fn reset_wifi_credentials(&self) -> Result<(), Self::Error>;
}

pub trait RobotConfigurationStorage {
    type Error: Error + Debug + Into<ServerError>;
    fn has_robot_credentials(&self) -> bool;
    fn store_robot_credentials(&self, cfg: CloudConfig) -> Result<(), Self::Error>;
    fn get_robot_credentials(&self) -> Result<RobotCredentials, Self::Error>;
    fn reset_robot_credentials(&self) -> Result<(), Self::Error>;

    fn has_app_address(&self) -> bool;
    fn store_app_address(&self, uri: &str) -> Result<(), Self::Error>;
    fn get_app_address(&self) -> Result<Uri, Self::Error>;
    fn reset_app_address(&self) -> Result<(), Self::Error>;

    fn has_robot_configuration(&self) -> bool;
    fn store_robot_configuration(&self, cfg: &RobotConfig) -> Result<(), Self::Error>;
    fn get_robot_configuration(&self) -> Result<RobotConfig, Self::Error>;
    fn reset_robot_configuration(&self) -> Result<(), Self::Error>;

    fn has_tls_certificate(&self) -> bool;
    fn store_tls_certificate(&self, creds: TlsCertificate) -> Result<(), Self::Error>;
    fn get_tls_certificate(&self) -> Result<TlsCertificate, Self::Error>;
    fn reset_tls_certificate(&self) -> Result<(), Self::Error>;
}

#[cfg(feature = "ota")]
pub trait OtaMetadataStorage {
    type Error: Error + Debug + Into<ServerError>;
    fn has_ota_metadata(&self) -> bool;
    fn get_ota_metadata(&self) -> Result<OtaMetadata, Self::Error>;
    fn store_ota_metadata(&self, ota_metadata: OtaMetadata) -> Result<(), Self::Error>;
    fn reset_ota_metadata(&self) -> Result<(), Self::Error>;
}

#[derive(Default)]
struct RAMCredentialStorageInner {
    robot_creds: Option<RobotCredentials>,
    robot_config: Option<RobotConfig>,
    wifi_creds: Option<WifiCredentials>,
    tls_cert: Option<TlsCertificate>,
    app_address: Option<String>,
    #[cfg(feature = "ota")]
    ota_metadata: Option<OtaMetadata>,
}

/// Simple CrendentialStorage made for testing purposes
#[derive(Default, Clone)]
pub struct RAMStorage(Rc<Mutex<RAMCredentialStorageInner>>);

impl RAMStorage {
    pub fn new() -> Self {
        Self(Rc::new(Mutex::new(RAMCredentialStorageInner {
            robot_creds: None,
            robot_config: None,
            wifi_creds: None,
            tls_cert: None,
            app_address: None,
            #[cfg(feature = "ota")]
            ota_metadata: None,
        })))
    }
}

#[cfg(feature = "ota")]
impl OtaMetadataStorage for RAMStorage {
    type Error = Infallible;
    fn has_ota_metadata(&self) -> bool {
        let inner_ref = self.0.lock().unwrap();
        inner_ref.ota_metadata.is_some()
    }
    fn store_ota_metadata(&self, ota_metadata: OtaMetadata) -> Result<(), Self::Error> {
        let mut inner_ref = self.0.lock().unwrap();
        let _ = inner_ref.ota_metadata.insert(ota_metadata);
        Ok(())
    }
    fn get_ota_metadata(&self) -> Result<OtaMetadata, Self::Error> {
        let inner_ref = self.0.lock().unwrap();
        Ok(inner_ref.ota_metadata.clone().unwrap_or_default())
    }
    fn reset_ota_metadata(&self) -> Result<(), Self::Error> {
        let _ = self.0.lock().unwrap().ota_metadata.take();
        Ok(())
    }
}

impl RobotConfigurationStorage for RAMStorage {
    type Error = <RobotCredentials as TryFrom<CloudConfig>>::Error;
    fn has_robot_credentials(&self) -> bool {
        let inner_ref = self.0.lock().unwrap();
        inner_ref.robot_creds.is_some()
    }
    fn store_robot_credentials(&self, cfg: CloudConfig) -> Result<(), Self::Error> {
        let creds: RobotCredentials = cfg.try_into()?;
        let mut inner_ref = self.0.lock().unwrap();
        let _ = inner_ref.robot_creds.insert(creds);
        Ok(())
    }
    fn get_robot_credentials(&self) -> Result<RobotCredentials, Self::Error> {
        let inner_ref = self.0.lock().unwrap();
        let cfg = inner_ref.robot_creds.clone().unwrap_or_default().clone();
        Ok(cfg)
    }
    fn reset_robot_credentials(&self) -> Result<(), Self::Error> {
        let mut inner_ref = self.0.lock().unwrap();
        let _ = inner_ref.robot_creds.take();
        Ok(())
    }

    fn has_robot_configuration(&self) -> bool {
        self.0.lock().unwrap().robot_config.is_some()
    }
    fn store_robot_configuration(&self, cfg: &RobotConfig) -> Result<(), Self::Error> {
        let _ = self.0.lock().unwrap().robot_config.insert(cfg.clone());
        Ok(())
    }
    fn get_robot_configuration(&self) -> Result<RobotConfig, Self::Error> {
        Ok(self
            .0
            .lock()
            .unwrap()
            .robot_config
            .clone()
            .unwrap_or_default()
            .clone())
    }
    fn reset_robot_configuration(&self) -> Result<(), Self::Error> {
        let _ = self.0.lock().unwrap().robot_config.take();
        Ok(())
    }
    fn get_tls_certificate(&self) -> Result<TlsCertificate, Self::Error> {
        let inner_ref = self.0.lock().unwrap();
        let creds = inner_ref.tls_cert.clone().unwrap_or_default();
        Ok(creds)
    }
    fn has_tls_certificate(&self) -> bool {
        let inner_ref = self.0.lock().unwrap();
        inner_ref.tls_cert.is_some()
    }
    fn store_tls_certificate(&self, creds: TlsCertificate) -> Result<(), Self::Error> {
        let mut inner_ref = self.0.lock().unwrap();
        let _ = inner_ref.tls_cert.insert(creds);
        Ok(())
    }
    fn reset_tls_certificate(&self) -> Result<(), Self::Error> {
        let mut inner_ref = self.0.lock().unwrap();
        let _ = inner_ref.tls_cert.take();
        Ok(())
    }
    fn store_app_address(&self, uri: &str) -> Result<(), Self::Error> {
        let mut inner_ref = self.0.lock().unwrap();
        let _ = inner_ref.app_address.insert(uri.to_string());
        Ok(())
    }
    fn get_app_address(&self) -> Result<Uri, Self::Error> {
        let inner_ref = self.0.lock().unwrap();
        inner_ref
            .app_address
            .clone()
            .unwrap_or_default()
            .parse::<Uri>()
    }
    fn reset_app_address(&self) -> Result<(), Self::Error> {
        let mut inner_ref = self.0.lock().unwrap();
        let _ = inner_ref.app_address.take();
        Ok(())
    }
    fn has_app_address(&self) -> bool {
        let inner_ref = self.0.lock().unwrap();
        inner_ref.app_address.is_some()
    }
}

impl WifiCredentialStorage for RAMStorage {
    type Error = Infallible;
    fn get_wifi_credentials(&self) -> Result<WifiCredentials, Self::Error> {
        let inner_ref = self.0.lock().unwrap();
        let creds = inner_ref.wifi_creds.clone().unwrap_or_default();
        Ok(creds)
    }
    fn has_wifi_credentials(&self) -> bool {
        let inner_ref = self.0.lock().unwrap();
        inner_ref.wifi_creds.is_some()
    }
    fn store_wifi_credentials(&self, creds: WifiCredentials) -> Result<(), Self::Error> {
        let mut inner_ref = self.0.lock().unwrap();
        let _ = inner_ref.wifi_creds.insert(creds);
        Ok(())
    }
    fn reset_wifi_credentials(&self) -> Result<(), Self::Error> {
        let mut inner_ref = self.0.lock().unwrap();
        let _ = inner_ref.wifi_creds.take();
        Ok(())
    }
}

impl From<Infallible> for ServerError {
    fn from(_: Infallible) -> Self {
        unreachable!()
    }
}
