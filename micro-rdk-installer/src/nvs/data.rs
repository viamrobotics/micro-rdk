use super::partition::{NVSEntry, NVSKeyValuePair, NVSValue};

#[derive(Clone, Default, Debug)]
pub struct WifiCredentials {
    pub ssid: String,
    pub password: String,
}

#[derive(Default, Debug)]
pub struct RobotCredentials {
    pub robot_id: Option<String>,
    pub robot_secret: Option<String>,
    pub robot_name: Option<String>,
    pub app_address: Option<String>,
    pub local_fqdn: Option<String>,
    pub fqdn: Option<String>,
    pub ca_crt: Option<Vec<u8>>,
    pub der_key: Option<Vec<u8>>,
    pub robot_dtls_certificate: Option<Vec<u8>>,
    pub robot_dtls_key_pair: Option<Vec<u8>>,
    pub robot_dtls_certificate_fp: Option<String>,
}

#[derive(Default, Debug)]
pub struct ViamFlashStorageData {
    pub wifi: Option<WifiCredentials>,
    pub robot_credentials: RobotCredentials,
}

impl ViamFlashStorageData {
    fn to_nvs_key_value_pairs(&self, namespace_idx: u8) -> anyhow::Result<[NVSKeyValuePair; 13]> {
        let wifi_cred = self.wifi.clone().ok_or(anyhow::Error::msg("no wifi"))?;
        Ok([
            NVSKeyValuePair {
                key: "WIFI_SSID".to_string(),
                value: NVSValue::String(wifi_cred.ssid),
                namespace_idx: namespace_idx,
            },
            NVSKeyValuePair {
                key: "PASSWORD".to_string(),
                value: NVSValue::String(wifi_cred.password),
                namespace_idx: namespace_idx,
            },
            NVSKeyValuePair {
                key: "ROBOT_ID".to_string(),
                value: NVSValue::String(
                    self.robot_credentials
                        .robot_id
                        .clone()
                        .ok_or(anyhow::Error::msg("robot_id missing"))?,
                ),
                namespace_idx: namespace_idx,
            },
            NVSKeyValuePair {
                key: "ROBOT_SECRET".to_string(),
                value: NVSValue::String(
                    self.robot_credentials
                        .robot_secret
                        .clone()
                        .ok_or(anyhow::Error::msg("robot_secret missing"))?,
                ),
                namespace_idx: namespace_idx,
            },
            NVSKeyValuePair {
                key: "LOCAL_FQDN".to_string(),
                value: NVSValue::String(
                    self.robot_credentials
                        .local_fqdn
                        .clone()
                        .ok_or(anyhow::Error::msg("local_fqdn missing"))?,
                ),
                namespace_idx: namespace_idx,
            },
            NVSKeyValuePair {
                key: "FQDN".to_string(),
                value: NVSValue::String(
                    self.robot_credentials
                        .fqdn
                        .clone()
                        .ok_or(anyhow::Error::msg("fqdn missing"))?,
                ),
                namespace_idx: namespace_idx,
            },
            NVSKeyValuePair {
                key: "ROBOT_NAME".to_string(),
                value: NVSValue::String(
                    self.robot_credentials
                        .robot_name
                        .clone()
                        .ok_or(anyhow::Error::msg("robot_name missing"))?,
                ),
                namespace_idx: namespace_idx,
            },
            NVSKeyValuePair {
                key: "DTLS_CERT_FP".to_string(),
                value: NVSValue::String(
                    self.robot_credentials
                        .robot_dtls_certificate_fp
                        .clone()
                        .ok_or(anyhow::Error::msg("robot_dtls_certificate_fp missing"))?,
                ),
                namespace_idx: namespace_idx,
            },
            NVSKeyValuePair {
                key: "ROBOT_SRV_DER_KEY".to_string(),
                value: NVSValue::Bytes(
                    self.robot_credentials
                        .der_key
                        .clone()
                        .ok_or(anyhow::Error::msg("der_key missing"))?,
                ),
                namespace_idx: namespace_idx,
            },
            NVSKeyValuePair {
                key: "CA_CRT".to_string(),
                value: NVSValue::Bytes(
                    self.robot_credentials
                        .ca_crt
                        .clone()
                        .ok_or(anyhow::Error::msg("ca_crt missing"))?,
                ),
                namespace_idx: namespace_idx,
            },
            NVSKeyValuePair {
                key: "APP_ADDRESS".to_string(),
                value: NVSValue::String(
                    self.robot_credentials
                        .app_address
                        .clone()
                        .ok_or(anyhow::Error::msg("app_address missing"))?,
                ),
                namespace_idx: namespace_idx,
            },
            NVSKeyValuePair {
                key: "ROBOT_DTLS_KEY_PAIR".to_string(),
                value: NVSValue::Bytes(
                    self.robot_credentials
                        .robot_dtls_key_pair
                        .clone()
                        .ok_or(anyhow::Error::msg("robot_dtls_key_pair missing"))?,
                ),
                namespace_idx: namespace_idx,
            },
            NVSKeyValuePair {
                key: "ROBOT_DTLS_CERT".to_string(),
                value: NVSValue::Bytes(
                    self.robot_credentials
                        .robot_dtls_certificate
                        .clone()
                        .ok_or(anyhow::Error::msg("robot_dtls_certificate missing"))?,
                ),
                namespace_idx: namespace_idx,
            },
        ])
    }

    pub fn to_entries(&self, namespace_idx: u8) -> anyhow::Result<Vec<NVSEntry>> {
        self.to_nvs_key_value_pairs(namespace_idx)?
            .iter()
            .map(|p| p.try_into())
            .collect()
    }

    pub fn get_robot_id(&self) -> anyhow::Result<String> {
        self.robot_credentials
            .robot_id
            .clone()
            .ok_or(anyhow::Error::msg("robot_id not set"))
    }

    pub fn get_robot_secret(&self) -> anyhow::Result<String> {
        self.robot_credentials
            .robot_secret
            .clone()
            .ok_or(anyhow::Error::msg("robot_secret not set"))
    }

    pub fn get_app_address(&self) -> anyhow::Result<String> {
        self.robot_credentials
            .app_address
            .clone()
            .ok_or(anyhow::Error::msg("app address not set"))
    }
}
