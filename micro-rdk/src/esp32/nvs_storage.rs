#![allow(dead_code)]
use bytes::Bytes;
use prost::{DecodeError, Message};
use std::{cell::RefCell, rc::Rc};
use thiserror::Error;

use crate::{
    common::{
        credentials_storage::{
            RobotConfigurationStorage, RobotCredentials, WifiCredentialStorage, WifiCredentials,
        },
        grpc::{GrpcError, ServerError},
    },
    esp32::esp_idf_svc::{
        nvs::{EspCustomNvs, EspCustomNvsPartition, EspNvs},
        sys::EspError,
    },
    proto::{app::v1::RobotConfig, provisioning::v1::CloudConfig},
};

#[derive(Error, Debug)]
pub enum NVSStorageError {
    #[error(transparent)]
    EspError(#[from] EspError),
    #[error("nvs key {0} is absent")]
    NVSKeyAbsent(String),
    #[error(transparent)]
    NVSValueDecodeError(#[from] DecodeError),
}

#[derive(Clone)]
pub struct NVSStorage {
    // esp-idf-svc partition driver ensures that only one handle of a type can be created
    // so inner mutability can be achieves safely with RefCell
    nvs: Rc<RefCell<EspCustomNvs>>,
}

impl NVSStorage {
    // taking partition name as argument so we can use another NVS part name if we want to.
    pub fn new(partition_name: &str) -> Result<Self, NVSStorageError> {
        let partition: EspCustomNvsPartition = EspCustomNvsPartition::take(partition_name)?;
        let nvs = EspNvs::new(partition, "VIAM_NS", true)?;

        Ok(Self {
            nvs: Rc::new(nvs.into()),
        })
    }

    fn get_string(&self, key: &str) -> Result<String, NVSStorageError> {
        let nvs = self.nvs.borrow_mut();
        let len = nvs
            .str_len(key)?
            .ok_or(NVSStorageError::NVSKeyAbsent(key.to_string()))?;
        let mut buf = vec![0_u8; len];
        Ok(nvs
            .get_str(key, buf.as_mut_slice())?
            .ok_or(NVSStorageError::NVSKeyAbsent(key.to_string()))?
            .to_owned())
    }

    fn set_string(&self, key: &str, string: &str) -> Result<(), NVSStorageError> {
        if self.has_string(key).unwrap_or_default()
            && self.get_string(key).unwrap_or_default().as_str() == string
        {
            log::debug!("no change in write to NVS key {:?}, skipping", key);
            return Ok(());
        }
        let mut nvs = self.nvs.borrow_mut();
        Ok(nvs.set_str(key, string)?)
    }

    fn has_string(&self, key: &str) -> Result<bool, NVSStorageError> {
        let nvs = self.nvs.borrow();
        Ok(nvs.str_len(key)?.is_some())
    }

    fn get_blob(&self, key: &str) -> Result<Vec<u8>, NVSStorageError> {
        let nvs = self.nvs.borrow_mut();
        let len = nvs
            .blob_len(key)?
            .ok_or(NVSStorageError::NVSKeyAbsent(key.to_string()))?;
        let mut buf = vec![0_u8; len];
        nvs.get_blob(key, buf.as_mut_slice())?
            .ok_or(NVSStorageError::NVSKeyAbsent(key.to_string()))?;
        Ok(buf)
    }

    fn set_blob(&self, key: &str, bytes: Bytes) -> Result<(), NVSStorageError> {
        if self.has_blob(key).unwrap_or_default() && self.get_blob(key).unwrap_or_default() == bytes
        {
            log::debug!("no change in write to NVS key {:?} for blob, skipping", key);
            return Ok(());
        }
        let mut nvs = self.nvs.borrow_mut();
        Ok(nvs.set_blob(key, bytes.as_ref())?)
    }

    fn has_blob(&self, key: &str) -> Result<bool, NVSStorageError> {
        let nvs = self.nvs.borrow();
        Ok(nvs.blob_len(key)?.is_some())
    }

    fn has_key(&self, key: &str) -> Result<bool, NVSStorageError> {
        let nvs = self.nvs.borrow();
        Ok(nvs.contains(key)?)
    }

    fn erase_key(&self, key: &str) -> Result<(), NVSStorageError> {
        let mut nvs = self.nvs.borrow_mut();
        let _ = nvs.remove(key)?;
        Ok(())
    }
}

const NVS_ROBOT_SECRET_KEY: &str = "ROBOT_SECRET";
const NVS_ROBOT_ID_KEY: &str = "ROBOT_ID";
const NVS_ROBOT_CONFIG_KEY: &str = "ROBOT_CONFIG";
const NVS_WIFI_SSID_KEY: &str = "WIFI_SSID";
const NVS_WIFI_PASSWORD_KEY: &str = "WIFI_PASSWORD";

impl RobotConfigurationStorage for NVSStorage {
    type Error = NVSStorageError;
    fn has_robot_credentials(&self) -> bool {
        self.has_string(NVS_ROBOT_SECRET_KEY).unwrap_or(false)
            && self.has_string(NVS_ROBOT_ID_KEY).unwrap_or(false)
    }

    fn get_robot_credentials(&self) -> Result<RobotCredentials, Self::Error> {
        let robot_secret = self.get_string(NVS_ROBOT_SECRET_KEY)?;
        let robot_id = self.get_string(NVS_ROBOT_ID_KEY)?;
        Ok(RobotCredentials {
            robot_secret,
            robot_id,
        })
    }

    fn store_robot_credentials(&self, cfg: CloudConfig) -> Result<(), Self::Error> {
        self.set_string(NVS_ROBOT_SECRET_KEY, &cfg.secret)?;
        self.set_string(NVS_ROBOT_ID_KEY, &cfg.id)?;
        Ok(())
    }

    fn reset_robot_credentials(&self) -> Result<(), Self::Error> {
        self.erase_key(NVS_ROBOT_SECRET_KEY)?;
        self.erase_key(NVS_ROBOT_ID_KEY)?;
        Ok(())
    }

    fn has_robot_configuration(&self) -> bool {
        self.has_blob(NVS_ROBOT_CONFIG_KEY).unwrap_or(false)
    }

    fn store_robot_configuration(&self, cfg: RobotConfig) -> Result<(), Self::Error> {
        self.set_blob(NVS_ROBOT_CONFIG_KEY, cfg.encode_to_vec().into())?;
        Ok(())
    }

    fn get_robot_configuration(&self) -> Result<RobotConfig, Self::Error> {
        let robot_config = self.get_blob(NVS_ROBOT_CONFIG_KEY)?;
        RobotConfig::decode(&robot_config[..]).map_err(NVSStorageError::NVSValueDecodeError)
    }

    fn reset_robot_configuration(&self) -> Result<(), Self::Error> {
        self.erase_key(NVS_ROBOT_CONFIG_KEY)?;
        Ok(())
    }
}

impl WifiCredentialStorage for NVSStorage {
    type Error = NVSStorageError;
    fn has_wifi_credentials(&self) -> bool {
        self.has_string(NVS_WIFI_SSID_KEY).unwrap_or(false)
            && self.has_string(NVS_WIFI_PASSWORD_KEY).unwrap_or(false)
    }

    fn get_wifi_credentials(&self) -> Result<WifiCredentials, Self::Error> {
        let ssid = self.get_string(NVS_WIFI_SSID_KEY)?;
        let pwd = self.get_string(NVS_WIFI_PASSWORD_KEY)?;
        Ok(WifiCredentials { ssid, pwd })
    }

    fn store_wifi_credentials(&self, creds: WifiCredentials) -> Result<(), Self::Error> {
        self.set_string(NVS_WIFI_SSID_KEY, &creds.ssid)?;
        self.set_string(NVS_WIFI_PASSWORD_KEY, &creds.pwd)?;
        Ok(())
    }

    fn reset_wifi_credentials(&self) -> Result<(), Self::Error> {
        self.erase_key(NVS_WIFI_SSID_KEY)?;
        self.erase_key(NVS_WIFI_PASSWORD_KEY)?;
        Ok(())
    }
}

impl From<NVSStorageError> for ServerError {
    fn from(value: NVSStorageError) -> Self {
        Self::new(GrpcError::RpcUnavailable, Some(value.into()))
    }
}
