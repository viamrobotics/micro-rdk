#![allow(dead_code)]
use std::cell::RefCell;

use esp_idf_svc::{
    nvs::{EspCustomNvs, EspCustomNvsPartition, EspNvs},
    sys::EspError,
};
use thiserror::Error;

use crate::common::provisioning::storage::{
    RobotCredentialStorage, RobotCredentials, WifiCredentialStorage, WifiCredentials,
};

#[derive(Error, Debug)]
pub enum NVSStorageError {
    #[error(transparent)]
    EspError(#[from] EspError),
    #[error("nvs key {0} is absent")]
    NVSKeyAbsent(&'static str),
}

pub struct NVSStorage {
    // esp-idf-svc partition driver ensures that only one handle of a type can be created
    // so inner mutability can be achieves safely with RefCell
    nvs: RefCell<EspCustomNvs>,
}

impl NVSStorage {
    // taking partition name as argument so we can use another NVS part name if we want to.
    pub fn new(partition_name: &str) -> Result<Self, NVSStorageError> {
        let partition: EspCustomNvsPartition = EspCustomNvsPartition::take(partition_name)?;
        let nvs = EspNvs::new(partition, "VIAM_NS", true)?;

        Ok(Self { nvs: nvs.into() })
    }
    fn get_string(&self, key: &'static str) -> Result<String, NVSStorageError> {
        let nvs = self.nvs.borrow_mut();
        let len = nvs
            .str_len(key)?
            .ok_or(NVSStorageError::NVSKeyAbsent(key))?;
        let mut buf = vec![0_u8; len];
        Ok(nvs
            .get_str(key, buf.as_mut_slice())?
            .ok_or(NVSStorageError::NVSKeyAbsent(key))?
            .to_owned())
    }
    fn set_string(&self, key: &str, string: &str) -> Result<(), NVSStorageError> {
        let mut nvs = self.nvs.borrow_mut();
        Ok(nvs.set_str(key, string)?)
    }
    fn has_key(&self, key: &str) -> Result<bool, NVSStorageError> {
        let nvs = self.nvs.borrow();
        Ok(nvs.contains(key)?)
    }
}
impl RobotCredentialStorage for NVSStorage {
    type Error = NVSStorageError;
    fn has_stored_credentials(&self) -> bool {
        self.has_key("ROBOT_SECRET").unwrap_or(false) && self.has_key("ROBOT_ID").unwrap_or(false)
    }
    fn get_robot_credentials(&self) -> Result<RobotCredentials, Self::Error> {
        let robot_secret = self.get_string("ROBOT_SECRET")?;
        let robot_id = self.get_string("ROBOT_ID")?;
        Ok(RobotCredentials {
            robot_secret,
            robot_id,
        })
    }
    fn store_robot_credentials(
        &self,
        cfg: crate::proto::provisioning::v1::CloudConfig,
    ) -> Result<(), Self::Error> {
        self.set_string("ROBOT_SECRET", &cfg.secret)?;
        self.set_string("ROBOT_ID", &cfg.id)?;
        Ok(())
    }
}

impl WifiCredentialStorage for NVSStorage {
    type Error = NVSStorageError;
    fn has_wifi_credentials(&self) -> bool {
        self.has_key("WIFI_SSID").unwrap_or(false) && self.has_key("WIFI_PASSWORD").unwrap_or(false)
    }
    fn get_wifi_credentials(&self) -> Result<WifiCredentials, Self::Error> {
        let ssid = self.get_string("WIFI_SSID")?;
        let pwd = self.get_string("WIFI_PASSWORD")?;
        Ok(WifiCredentials { ssid, pwd })
    }
    fn store_wifi_credentials(&self, creds: &WifiCredentials) -> Result<(), Self::Error> {
        self.set_string("WIFI_SSID", &creds.ssid)?;
        self.set_string("WIFI_PASSWORD", &creds.pwd)?;
        Ok(())
    }
}
