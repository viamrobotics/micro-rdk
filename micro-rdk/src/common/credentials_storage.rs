#![allow(dead_code)]
use std::{convert::Infallible, error::Error, fmt::Debug, rc::Rc, sync::Mutex};

use crate::{common::grpc::ServerError, proto::app::v1::RobotConfig};

use crate::proto::provisioning::v1::{CloudConfig, SetNetworkCredentialsRequest};

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

impl From<CloudConfig> for RobotCredentials {
    fn from(value: CloudConfig) -> Self {
        // TODO: make ticket : ignore app_address for now but need to add it later
        Self {
            robot_id: value.id,
            robot_secret: value.secret,
        }
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

    fn has_robot_configuration(&self) -> bool;
    fn store_robot_configuration(&self, cfg: RobotConfig) -> Result<(), Self::Error>;
    fn get_robot_configuration(&self) -> Result<RobotConfig, Self::Error>;
    fn reset_robot_configuration(&self) -> Result<(), Self::Error>;
}

#[derive(Default)]
struct RAMCredentialStorageInner {
    robot_creds: Option<RobotCredentials>,
    robot_config: Option<RobotConfig>,
    wifi_creds: Option<WifiCredentials>,
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
        })))
    }
}

impl RobotConfigurationStorage for RAMStorage {
    type Error = Infallible;
    fn has_robot_credentials(&self) -> bool {
        let inner_ref = self.0.lock().unwrap();
        inner_ref.robot_creds.is_some()
    }
    fn store_robot_credentials(&self, cfg: CloudConfig) -> Result<(), Self::Error> {
        let creds: RobotCredentials = cfg.into();
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
    fn store_robot_configuration(&self, cfg: RobotConfig) -> Result<(), Self::Error> {
        let _ = self.0.lock().unwrap().robot_config.insert(cfg);
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
