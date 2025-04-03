use secrecy::{ExposeSecret, SecretString};

use super::super::error::Error;
use super::partition::{NVSEntry, NVSKeyValuePair, NVSValue};

#[derive(Clone, Debug)]
pub struct WifiCredentials {
    pub ssid: String,
    pub password: SecretString,
}

impl Default for WifiCredentials {
    fn default() -> Self {
        Self {
            ssid: "".to_string(),
            password: "".to_string().into(),
        }
    }
}

#[derive(Default, Debug)]
pub struct RobotCredentials {
    pub robot_id: Option<String>,
    pub robot_secret: Option<SecretString>,
    pub robot_name: Option<String>,
    pub app_address: Option<String>,
}

#[derive(Default, Debug)]
pub struct ViamFlashStorageData {
    pub wifi: Option<WifiCredentials>,
    pub robot_credentials: RobotCredentials,
}

impl ViamFlashStorageData {
    fn to_nvs_key_value_pairs(&self, namespace_idx: u8) -> Result<[NVSKeyValuePair; 5], Error> {
        let wifi_cred = self
            .wifi
            .clone()
            .ok_or(Error::NVSDataProcessingError("no wifi".to_string()))?;
        Ok([
            NVSKeyValuePair {
                key: "WIFI_SSID".to_string(),
                value: NVSValue::String(wifi_cred.ssid),
                namespace_idx,
            },
            NVSKeyValuePair {
                key: "WIFI_PASSWORD".to_string(),
                value: NVSValue::String(wifi_cred.password.expose_secret().to_string()),
                namespace_idx,
            },
            NVSKeyValuePair {
                key: "ROBOT_ID".to_string(),
                value: NVSValue::String(self.robot_credentials.robot_id.clone().ok_or(
                    Error::NVSDataProcessingError("robot_id missing".to_string()),
                )?),
                namespace_idx,
            },
            NVSKeyValuePair {
                key: "ROBOT_SECRET".to_string(),
                value: NVSValue::String(
                    self.robot_credentials
                        .robot_secret
                        .clone()
                        .ok_or(Error::NVSDataProcessingError(
                            "robot_secret missing".to_string(),
                        ))?
                        .expose_secret()
                        .to_string(),
                ),
                namespace_idx,
            },
            NVSKeyValuePair {
                key: "ROBOT_APP_ADDR".to_string(),
                value: NVSValue::String(self.robot_credentials.app_address.clone().ok_or(
                    Error::NVSDataProcessingError("app_address missing".to_string()),
                )?),
                namespace_idx,
            },
        ])
    }

    pub fn to_entries(&self, namespace_idx: u8) -> Result<Vec<NVSEntry>, Error> {
        self.to_nvs_key_value_pairs(namespace_idx)?
            .iter()
            .map(|p| p.try_into())
            .collect()
    }

    pub fn get_robot_id(&self) -> Result<String, Error> {
        self.robot_credentials
            .robot_id
            .clone()
            .ok_or(Error::MissingConfigInfo("robot_id not set".to_string()))
    }

    pub fn get_robot_secret(&self) -> Result<String, Error> {
        Ok(self
            .robot_credentials
            .robot_secret
            .clone()
            .ok_or(Error::MissingConfigInfo("robot_secret not set".to_string()))?
            .expose_secret()
            .to_string())
    }

    pub fn get_app_address(&self) -> Result<String, Error> {
        self.robot_credentials
            .app_address
            .clone()
            .ok_or(Error::MissingConfigInfo("app address not set".to_string()))
    }
}
