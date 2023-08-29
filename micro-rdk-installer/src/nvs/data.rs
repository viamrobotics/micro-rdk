use secrecy::{ExposeSecret, Secret};

use super::super::error::Error;
use super::partition::{NVSEntry, NVSKeyValuePair, NVSValue};

#[derive(Clone, Debug)]
pub struct WifiCredentials {
    pub ssid: String,
    pub password: Secret<String>,
}

impl Default for WifiCredentials {
    fn default() -> Self {
        Self {
            ssid: "".to_string(),
            password: Secret::new("".to_string()),
        }
    }
}

#[derive(Default, Debug)]
pub struct RobotCredentials {
    pub robot_id: Option<String>,
    pub robot_secret: Option<Secret<String>>,
    pub robot_name: Option<String>,
    pub app_address: Option<String>,
    pub local_fqdn: Option<String>,
    pub fqdn: Option<String>,
    pub ca_crt: Option<Vec<u8>>,
    pub der_key: Option<Vec<u8>>,
    pub robot_dtls_certificate: Option<Vec<u8>>,
    pub robot_dtls_key_pair: Option<Vec<u8>>,
    pub robot_dtls_certificate_fp: Option<String>,
    pub pem_chain: Option<Vec<u8>>,
}

#[derive(Default, Debug)]
pub struct ViamFlashStorageData {
    pub wifi: Option<WifiCredentials>,
    pub robot_credentials: RobotCredentials,
}

impl ViamFlashStorageData {
    fn to_nvs_key_value_pairs(&self, namespace_idx: u8) -> Result<[NVSKeyValuePair; 14], Error> {
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
                key: "LOCAL_FQDN".to_string(),
                value: NVSValue::String(self.robot_credentials.local_fqdn.clone().ok_or(
                    Error::NVSDataProcessingError("local_fqdn missing".to_string()),
                )?),
                namespace_idx,
            },
            NVSKeyValuePair {
                key: "FQDN".to_string(),
                value: NVSValue::String(
                    self.robot_credentials
                        .fqdn
                        .clone()
                        .ok_or(Error::NVSDataProcessingError("fqdn missing".to_string()))?,
                ),
                namespace_idx,
            },
            NVSKeyValuePair {
                key: "ROBOT_NAME".to_string(),
                value: NVSValue::String(self.robot_credentials.robot_name.clone().ok_or(
                    Error::NVSDataProcessingError("robot_name missing".to_string()),
                )?),
                namespace_idx,
            },
            NVSKeyValuePair {
                key: "DTLS_CERT_FP".to_string(),
                value: NVSValue::String(
                    self.robot_credentials
                        .robot_dtls_certificate_fp
                        .clone()
                        .ok_or(Error::NVSDataProcessingError(
                            "robot_dtls_certificate_fp missing".to_string(),
                        ))?,
                ),
                namespace_idx,
            },
            NVSKeyValuePair {
                key: "SRV_DER_KEY".to_string(),
                value: NVSValue::Bytes(
                    self.robot_credentials
                        .der_key
                        .clone()
                        .ok_or(Error::NVSDataProcessingError("der_key missing".to_string()))?,
                ),
                namespace_idx,
            },
            NVSKeyValuePair {
                key: "SRV_PEM_CHAIN".to_string(),
                value: NVSValue::Bytes(self.robot_credentials.pem_chain.clone().ok_or(
                    Error::NVSDataProcessingError("pem_chain missing".to_string()),
                )?),
                namespace_idx,
            },
            NVSKeyValuePair {
                key: "CA_CRT".to_string(),
                value: NVSValue::Bytes(
                    self.robot_credentials
                        .ca_crt
                        .clone()
                        .ok_or(Error::NVSDataProcessingError("ca_crt missing".to_string()))?,
                ),
                namespace_idx,
            },
            NVSKeyValuePair {
                key: "APP_ADDRESS".to_string(),
                value: NVSValue::String(self.robot_credentials.app_address.clone().ok_or(
                    Error::NVSDataProcessingError("app_address missing".to_string()),
                )?),
                namespace_idx,
            },
            NVSKeyValuePair {
                key: "DTLS_KEY_PAIR".to_string(),
                value: NVSValue::Bytes(self.robot_credentials.robot_dtls_key_pair.clone().ok_or(
                    Error::NVSDataProcessingError("robot_dtls_key_pair missing".to_string()),
                )?),
                namespace_idx,
            },
            NVSKeyValuePair {
                key: "ROBOT_DTLS_CERT".to_string(),
                value: NVSValue::Bytes(
                    self.robot_credentials
                        .robot_dtls_certificate
                        .clone()
                        .ok_or(Error::NVSDataProcessingError(
                            "robot_dtls_certificate missing".to_string(),
                        ))?,
                ),
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
