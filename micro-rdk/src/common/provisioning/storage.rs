#![allow(dead_code)]
use std::{convert::Infallible, rc::Rc, sync::Mutex};

use crate::proto::provisioning::v1::CloudConfig;

#[derive(Clone, Default)]
pub struct RobotCredentials {
    pub(crate) robot_secret: String,
    pub(crate) robot_id: String,
}

impl RobotCredentials {
    pub(crate) fn robot_secret(&self) -> &str {
        &self.robot_secret
    }
    pub(crate) fn robot_id(&self) -> &str {
        &self.robot_id
    }
}
#[derive(Clone, Default)]
pub struct WifiCredentials {
    pub(crate) ssid: String,
    pub(crate) pwd: String,
}

impl WifiCredentials {
    pub(crate) fn wifi_ssid(&self) -> &str {
        &self.ssid
    }
    pub(crate) fn wifi_pwd(&self) -> &str {
        &self.pwd
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

pub trait WifiCredentialStorage {
    type Error;
    fn has_wifi_credentials(&self) -> bool;
    fn store_wifi_credentials(&self, creds: &WifiCredentials) -> Result<(), Self::Error>;
    fn get_wifi_credentials(&self) -> Result<WifiCredentials, Self::Error>;
}

pub trait RobotCredentialStorage {
    type Error;
    fn has_stored_credentials(&self) -> bool;
    fn store_robot_credentials(&self, cfg: CloudConfig) -> Result<(), Self::Error>;
    fn get_robot_credentials(&self) -> Result<RobotCredentials, Self::Error>;
}

#[derive(Default)]
struct MemoryCredentialStorageInner {
    config: Option<RobotCredentials>,
    ssid: Option<String>,
    pwd: Option<String>,
}

/// Simple CrendentialStorage made for testing purposes
#[derive(Default, Clone)]
pub(crate) struct MemoryCredentialStorage(Rc<Mutex<MemoryCredentialStorageInner>>);

impl RobotCredentialStorage for MemoryCredentialStorage {
    type Error = Infallible;
    fn has_stored_credentials(&self) -> bool {
        let this = self.0.lock().unwrap();
        this.config.is_some()
    }
    fn store_robot_credentials(&self, cfg: CloudConfig) -> Result<(), Self::Error> {
        let creds: RobotCredentials = cfg.into();
        let mut this = self.0.lock().unwrap();
        let _ = this.config.insert(creds);
        Ok(())
    }
    fn get_robot_credentials(&self) -> Result<RobotCredentials, Self::Error> {
        let this = self.0.lock().unwrap();
        let cfg = this.config.clone().unwrap_or_default().clone();
        Ok(cfg)
    }
}

impl WifiCredentialStorage for MemoryCredentialStorage {
    type Error = Infallible;
    fn get_wifi_credentials(&self) -> Result<WifiCredentials, Self::Error> {
        let this = self.0.lock().unwrap();
        let creds = WifiCredentials {
            ssid: this.ssid.clone().unwrap_or_default(),
            pwd: this.pwd.clone().unwrap_or_default(),
        };
        Ok(creds)
    }
    fn has_wifi_credentials(&self) -> bool {
        let this = self.0.lock().unwrap();
        this.ssid.is_some() && this.pwd.is_none()
    }
    fn store_wifi_credentials(&self, creds: &WifiCredentials) -> Result<(), Self::Error> {
        let mut this = self.0.lock().unwrap();
        let _ = this.ssid.insert(creds.ssid.clone());
        let _ = this.pwd.insert(creds.pwd.clone());
        Ok(())
    }
}
