#![allow(dead_code)]
use bytes::Bytes;
use hyper::{http::uri::InvalidUri, Uri};
use prost::Message;
use std::{cell::RefCell, rc::Rc};
use thiserror::Error;

use crate::{
    common::{
        config::NetworkSetting,
        credentials_storage::{
            EmptyStorageCollectionError, NetworkSettingsStorage, RobotConfigurationStorage,
            RobotCredentials, StorageDiagnostic, TlsCertificate, WifiCredentials,
        },
        grpc::{GrpcError, ServerError},
    },
    esp32::esp_idf_svc::{
        handle::RawHandle,
        nvs::{EspCustomNvs, EspCustomNvsPartition, EspNvs},
        sys::{esp, nvs_get_stats, nvs_stats_t, EspError},
    },
    proto::{app::v1::RobotConfig, provisioning::v1::CloudConfig},
};

const MAX_NVS_KEY_SIZE: usize = 15;

#[derive(Error, Debug)]
pub enum NVSDecodeError {
    #[error(transparent)]
    Postcard(#[from] postcard::Error),
    #[error(transparent)]
    Prost(#[from] prost::DecodeError),
}

#[derive(Error, Debug)]
pub enum NVSStorageError {
    #[error(transparent)]
    EspError(#[from] EspError),
    #[error("nvs key {0} is absent")]
    NVSKeyAbsent(String),
    #[error("nvs key {0} exceeds {1} characters")]
    NVSKeyTooLong(String, usize),
    #[error(transparent)]
    NVSValueDecodeError(#[from] NVSDecodeError),
    #[error(transparent)]
    NVSValueEncodeError(#[from] postcard::Error),
    #[error(transparent)]
    NVSUriParseError(#[from] InvalidUri),
    #[error("nvs collection empty")]
    NVSCollectionEmpty(#[from] EmptyStorageCollectionError),
}

#[derive(Clone)]
pub struct NVSStorage {
    // esp-idf-svc partition driver ensures that only one handle of a type can be created
    // so inner mutability can be achieves safely with RefCell
    nvs: Rc<RefCell<EspCustomNvs>>,
    part: EspCustomNvsPartition,
}

impl NVSStorage {
    // taking partition name as argument so we can use another NVS part name if we want to.
    pub fn new(partition_name: &str) -> Result<Self, NVSStorageError> {
        let partition: EspCustomNvsPartition = EspCustomNvsPartition::take(partition_name)?;
        let nvs = EspNvs::new(partition.clone(), "VIAM_NS", true)?;

        Ok(Self {
            nvs: Rc::new(nvs.into()),
            part: partition,
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
        if key.len() > MAX_NVS_KEY_SIZE {
            return Err(NVSStorageError::NVSKeyTooLong(key.to_string(), key.len()));
        }
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
        if key.len() > MAX_NVS_KEY_SIZE {
            return Err(NVSStorageError::NVSKeyTooLong(key.to_string(), key.len()));
        }
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

const BYTES_PER_ENTRY: usize = 32;

impl StorageDiagnostic for NVSStorage {
    fn log_space_diagnostic(&self) {
        let mut stats: nvs_stats_t = Default::default();
        if let Err(err) =
            esp!(unsafe { nvs_get_stats(self.part.handle() as _, &mut stats as *mut _) })
        {
            log::error!("could not acquire NVS stats: {:?}", err);
            return;
        }

        let used_entries = stats.used_entries;
        let used_space = used_entries * BYTES_PER_ENTRY;
        let total_space = stats.total_entries * BYTES_PER_ENTRY;

        // From experimentation we have found that NVS requires 4000 bytes of
        // unused space for reasons unknown. The percentage portion of the calculation (0.976)
        // comes from the blob size restriction as stated in the ESP32 documentation
        // on NVS
        let total_usable_space = (0.976 * (total_space as f64)) - 4000.0;
        let fraction_used = (used_space as f64) / total_usable_space;
        log::log!(
            if fraction_used > 0.9 {
                log::Level::Warn
            } else {
                log::Level::Info
            },
            "NVS stats: {:?} bytes used of {:?} available",
            used_space,
            total_space
        );
    }
}

const NVS_ROBOT_SECRET_KEY: &str = "ROBOT_SECRET";
const NVS_ROBOT_ID_KEY: &str = "ROBOT_ID";
const NVS_ROBOT_APP_ADDRESS: &str = "ROBOT_APP_ADDR";
const NVS_ROBOT_CONFIG_KEY: &str = "ROBOT_CONFIG";
const NVS_WIFI_SSID_KEY: &str = "WIFI_SSID";
const NVS_WIFI_PASSWORD_KEY: &str = "WIFI_PASSWORD";
const NVS_TLS_CERTIFICATE_KEY: &str = "TLS_CERT";
const NVS_TLS_PRIVATE_KEY_KEY: &str = "TLS_PRIV_KEY";
const NVS_NETWORK_SETTINGS_KEY: &str = "NETWORKS";

#[cfg(feature = "ota")]
const NVS_OTA_VERSION_KEY: &str = "OTA_VERSION";
#[cfg(feature = "ota")]
use crate::common::{credentials_storage::OtaMetadataStorage, ota::OtaMetadata};

#[cfg(feature = "ota")]
impl OtaMetadataStorage for NVSStorage {
    type Error = NVSStorageError;
    fn has_ota_metadata(&self) -> bool {
        self.has_string(NVS_OTA_VERSION_KEY).unwrap_or(false)
    }
    fn get_ota_metadata(&self) -> Result<OtaMetadata, Self::Error> {
        let version = self.get_string(NVS_OTA_VERSION_KEY)?;
        Ok(OtaMetadata { version })
    }
    fn store_ota_metadata(&self, ota_metadata: &OtaMetadata) -> Result<(), Self::Error> {
        self.set_string(NVS_OTA_VERSION_KEY, &ota_metadata.version)
    }
    fn reset_ota_metadata(&self) -> Result<(), Self::Error> {
        self.erase_key(NVS_OTA_VERSION_KEY)
    }
}

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

    fn get_app_address(&self) -> Result<Uri, Self::Error> {
        Ok(self.get_string(NVS_ROBOT_APP_ADDRESS)?.parse::<Uri>()?)
    }

    fn has_app_address(&self) -> bool {
        self.has_string(NVS_ROBOT_APP_ADDRESS).unwrap_or(false)
    }

    fn store_app_address(&self, uri: &str) -> Result<(), Self::Error> {
        self.set_string(NVS_ROBOT_APP_ADDRESS, uri)
    }
    fn reset_app_address(&self) -> Result<(), Self::Error> {
        self.erase_key(NVS_ROBOT_APP_ADDRESS)
    }

    fn store_robot_credentials(&self, cfg: &CloudConfig) -> Result<(), Self::Error> {
        self.set_string(NVS_ROBOT_SECRET_KEY, &cfg.secret)?;
        self.set_string(NVS_ROBOT_ID_KEY, &cfg.id)
            .inspect_err(|_| {
                let _ = self.erase_key(NVS_ROBOT_SECRET_KEY);
            })?;
        self.set_string(NVS_ROBOT_APP_ADDRESS, &cfg.app_address)
            .inspect_err(|_| {
                let _ = self.erase_key(NVS_ROBOT_SECRET_KEY);
                let _ = self.erase_key(NVS_ROBOT_ID_KEY);
            })?;
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

    fn store_robot_configuration(&self, cfg: &RobotConfig) -> Result<(), Self::Error> {
        self.set_blob(NVS_ROBOT_CONFIG_KEY, cfg.encode_to_vec().into())?;
        Ok(())
    }

    fn get_robot_configuration(&self) -> Result<RobotConfig, Self::Error> {
        let robot_config = self.get_blob(NVS_ROBOT_CONFIG_KEY)?;
        let config = RobotConfig::decode(&robot_config[..]).map_err(NVSDecodeError::Prost)?;
        Ok(config)
    }

    fn reset_robot_configuration(&self) -> Result<(), Self::Error> {
        self.erase_key(NVS_ROBOT_CONFIG_KEY)?;
        Ok(())
    }

    fn has_tls_certificate(&self) -> bool {
        self.has_blob(NVS_TLS_CERTIFICATE_KEY).unwrap_or(false)
            && self.has_blob(NVS_TLS_PRIVATE_KEY_KEY).unwrap_or(false)
    }

    fn get_tls_certificate(&self) -> Result<TlsCertificate, Self::Error> {
        let certificate = self.get_blob(NVS_TLS_CERTIFICATE_KEY)?;
        let private_key = self.get_blob(NVS_TLS_PRIVATE_KEY_KEY)?;
        Ok(TlsCertificate {
            certificate,
            private_key,
        })
    }

    fn store_tls_certificate(&self, creds: &TlsCertificate) -> Result<(), Self::Error> {
        self.set_blob(
            NVS_TLS_CERTIFICATE_KEY,
            Bytes::from(creds.certificate.clone()),
        )?;
        self.set_blob(
            NVS_TLS_PRIVATE_KEY_KEY,
            Bytes::from(creds.private_key.clone()),
        )
        .inspect_err(|_| {
            let _ = self.erase_key(NVS_TLS_CERTIFICATE_KEY);
        })?;
        Ok(())
    }

    fn reset_tls_certificate(&self) -> Result<(), Self::Error> {
        self.erase_key(NVS_TLS_CERTIFICATE_KEY)?;
        self.erase_key(NVS_TLS_PRIVATE_KEY_KEY)?;
        Ok(())
    }
}

impl NetworkSettingsStorage for NVSStorage {
    type Error = NVSStorageError;
    fn has_network_settings(&self) -> bool {
        self.has_blob(NVS_NETWORK_SETTINGS_KEY).unwrap_or(false)
    }

    fn get_network_settings(&self) -> Result<Vec<NetworkSetting>, Self::Error> {
        let blob: Vec<u8> = self.get_blob(NVS_NETWORK_SETTINGS_KEY)?;
        let networks: Vec<NetworkSetting> =
            postcard::from_bytes(&blob).map_err(NVSDecodeError::Postcard)?;
        Ok(networks)
    }

    fn store_network_settings(
        &self,
        network_settings: &[NetworkSetting],
    ) -> Result<(), Self::Error> {
        let bytes: Vec<u8> = postcard::to_allocvec(network_settings)?;
        self.set_blob(NVS_NETWORK_SETTINGS_KEY, bytes.into())?;
        Ok(())
    }

    fn reset_network_settings(&self) -> Result<(), Self::Error> {
        self.erase_key(NVS_NETWORK_SETTINGS_KEY)?;
        Ok(())
    }
    fn has_wifi_credentials(&self) -> bool {
        self.has_string(NVS_WIFI_SSID_KEY).unwrap_or(false)
            && self.has_string(NVS_WIFI_PASSWORD_KEY).unwrap_or(false)
    }

    fn get_wifi_credentials(&self) -> Result<WifiCredentials, Self::Error> {
        let ssid = self.get_string(NVS_WIFI_SSID_KEY)?;
        let pwd = self.get_string(NVS_WIFI_PASSWORD_KEY)?;
        Ok(WifiCredentials {
            ssid,
            pwd,
            priority: 0,
        })
    }

    fn get_all_networks(&self) -> Result<Vec<WifiCredentials>, Self::Error> {
        Ok(vec![self.get_wifi_credentials()?])
    }

    fn store_wifi_credentials(&self, creds: &WifiCredentials) -> Result<(), Self::Error> {
        self.set_string(NVS_WIFI_SSID_KEY, &creds.ssid)?;
        self.set_string(NVS_WIFI_PASSWORD_KEY, &creds.pwd)
            .inspect_err(|_| {
                let _ = self.erase_key(NVS_WIFI_SSID_KEY);
            })?;
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
