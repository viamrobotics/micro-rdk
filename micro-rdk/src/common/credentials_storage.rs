#![allow(dead_code)]
#![allow(clippy::manual_try_fold)]
use crate::common::grpc::GrpcError;
use hyper::{http::uri::InvalidUri, Uri};
use std::str::FromStr;
use std::{error::Error, fmt::Debug, rc::Rc, sync::Mutex};

use crate::{
    common::{config::NetworkSetting, grpc::ServerError},
    proto::app::v1::RobotConfig,
};

#[cfg(feature = "ota")]
use crate::common::ota::OtaMetadata;
use crate::proto::{
    app::v1::CertificateResponse,
    provisioning::v1::{CloudConfig, SetNetworkCredentialsRequest},
};
use thiserror::Error;

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
            priority: 0,
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
    pub(crate) priority: i32,
}

impl WifiCredentials {
    pub fn new(ssid: String, pwd: String) -> Self {
        Self {
            ssid,
            pwd,
            priority: 0,
        }
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

pub trait NetworkSettingsStorage {
    type Error: Error + Debug + Into<ServerError>;
    fn has_network_settings(&self) -> bool;
    fn store_network_settings(&self, networks: &[NetworkSetting]) -> Result<(), Self::Error>;
    fn get_network_settings(&self) -> Result<Vec<NetworkSetting>, Self::Error>;
    fn reset_network_settings(&self) -> Result<(), Self::Error>;

    fn has_wifi_credentials(&self) -> bool;
    fn store_wifi_credentials(&self, creds: &WifiCredentials) -> Result<(), Self::Error>;
    fn get_wifi_credentials(&self) -> Result<WifiCredentials, Self::Error>;
    fn reset_wifi_credentials(&self) -> Result<(), Self::Error>;
    fn get_all_networks(&self) -> Result<Vec<WifiCredentials>, Self::Error>;
}

pub trait RobotConfigurationStorage {
    type Error: Error + Debug + Into<ServerError>;
    fn has_robot_credentials(&self) -> bool;
    fn store_robot_credentials(&self, cfg: &CloudConfig) -> Result<(), Self::Error>;
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
    fn store_tls_certificate(&self, creds: &TlsCertificate) -> Result<(), Self::Error>;
    fn get_tls_certificate(&self) -> Result<TlsCertificate, Self::Error>;
    fn reset_tls_certificate(&self) -> Result<(), Self::Error>;
}

#[cfg(feature = "ota")]
pub trait OtaMetadataStorage {
    type Error: Error + Debug + Into<ServerError>;
    fn has_ota_metadata(&self) -> bool;
    fn get_ota_metadata(&self) -> Result<OtaMetadata, Self::Error>;
    fn store_ota_metadata(&self, ota_metadata: &OtaMetadata) -> Result<(), Self::Error>;
    fn reset_ota_metadata(&self) -> Result<(), Self::Error>;
}

pub trait StorageDiagnostic {
    fn log_space_diagnostic(&self);
}

#[derive(Error, Debug)]
#[error("empty storage collection")]
pub struct EmptyStorageCollectionError;

#[derive(Default)]
struct RAMCredentialStorageInner {
    robot_creds: Option<RobotCredentials>,
    robot_config: Option<RobotConfig>,
    wifi_creds: Option<WifiCredentials>,
    network_settings: Option<Vec<NetworkSetting>>,
    tls_cert: Option<TlsCertificate>,
    app_address: Option<String>,
    #[cfg(feature = "ota")]
    ota_metadata: Option<OtaMetadata>,
}

/// Simple CrendentialStorage made for testing purposes
#[derive(Default, Clone)]
pub struct RAMStorage(Rc<Mutex<RAMCredentialStorageInner>>);

#[derive(Error, Debug)]
pub enum RAMStorageError {
    #[error(transparent)]
    ParseUriError(#[from] InvalidUri),
    #[error(transparent)]
    NVSCollectionEmpty(#[from] EmptyStorageCollectionError),
    #[error("object not found")]
    NotFound,
}

impl From<RAMStorageError> for ServerError {
    fn from(value: RAMStorageError) -> Self {
        Self::new(GrpcError::RpcUnavailable, Some(value.into()))
    }
}

impl RAMStorage {
    pub fn new() -> Self {
        Self(Rc::new(Mutex::new(RAMCredentialStorageInner {
            robot_creds: None,
            robot_config: None,
            wifi_creds: None,
            tls_cert: None,
            app_address: None,
            network_settings: None,
            #[cfg(feature = "ota")]
            ota_metadata: None,
        })))
    }
}

impl NetworkSettingsStorage for RAMStorage {
    type Error = RAMStorageError;
    fn has_network_settings(&self) -> bool {
        let inner_ref = self.0.lock().unwrap();
        inner_ref.network_settings.is_some()
    }
    fn get_network_settings(&self) -> Result<Vec<NetworkSetting>, Self::Error> {
        let inner_ref = self.0.lock().unwrap();
        inner_ref
            .network_settings
            .clone()
            .ok_or(RAMStorageError::NotFound)
    }
    fn store_network_settings(&self, networks: &[NetworkSetting]) -> Result<(), Self::Error> {
        let mut inner_ref = self.0.lock().unwrap();
        let _ = inner_ref.network_settings.insert(networks.into());
        Ok(())
    }
    fn reset_network_settings(&self) -> Result<(), Self::Error> {
        let _ = self.0.lock().unwrap().network_settings.take();
        Ok(())
    }
    fn get_wifi_credentials(&self) -> Result<WifiCredentials, Self::Error> {
        let inner_ref = self.0.lock().unwrap();
        inner_ref
            .wifi_creds
            .clone()
            .ok_or(RAMStorageError::NotFound)
    }
    fn has_wifi_credentials(&self) -> bool {
        let inner_ref = self.0.lock().unwrap();
        inner_ref.wifi_creds.is_some()
    }
    fn store_wifi_credentials(&self, creds: &WifiCredentials) -> Result<(), Self::Error> {
        let mut inner_ref = self.0.lock().unwrap();
        let _ = inner_ref.wifi_creds.insert(creds.clone());
        Ok(())
    }
    fn reset_wifi_credentials(&self) -> Result<(), Self::Error> {
        let mut inner_ref = self.0.lock().unwrap();
        let _ = inner_ref.wifi_creds.take();
        Ok(())
    }
}

impl<Iterable, Storage: NetworkSettingsStorage> NetworkSettingsStorage for Iterable
where
    for<'a> &'a Iterable: IntoIterator<Item = &'a Storage>,
    Storage::Error: From<EmptyStorageCollectionError>,
{
    type Error = Storage::Error;
    fn has_network_settings(&self) -> bool {
        self.into_iter()
            .any(NetworkSettingsStorage::has_network_settings)
    }
    fn get_network_settings(&self) -> Result<Vec<NetworkSetting>, Self::Error> {
        self.into_iter().fold(
            Err::<_, Self::Error>(EmptyStorageCollectionError.into()),
            |val, s| val.or_else(|_| s.get_network_settings()),
        )
    }
    fn store_network_settings(&self, networks: &[NetworkSetting]) -> Result<(), Self::Error> {
        self.into_iter().fold(
            Err::<_, Self::Error>(EmptyStorageCollectionError.into()),
            |val, s| val.or_else(|_| s.store_network_settings(networks)),
        )
    }
    fn reset_network_settings(&self) -> Result<(), Self::Error> {
        self.into_iter().fold(
            Err::<_, Self::Error>(EmptyStorageCollectionError.into()),
            |val, s| val.or(s.reset_network_settings()),
        )
    }
    fn has_wifi_credentials(&self) -> bool {
        self.into_iter()
            .any(NetworkSettingsStorage::has_wifi_credentials)
    }
    fn get_wifi_credentials(&self) -> Result<WifiCredentials, Self::Error> {
        self.into_iter().fold(
            Err::<_, Self::Error>(EmptyStorageCollectionError.into()),
            |val, s| val.or_else(|_| s.get_wifi_credentials()),
        )
    }
    fn store_wifi_credentials(&self, creds: &WifiCredentials) -> Result<(), Self::Error> {
        self.into_iter().fold(
            Err::<_, Self::Error>(EmptyStorageCollectionError.into()),
            |val, s| val.or_else(|_| s.store_wifi_credentials(creds)),
        )
    }
    fn reset_wifi_credentials(&self) -> Result<(), Self::Error> {
        self.into_iter().fold(
            Err::<_, Self::Error>(EmptyStorageCollectionError.into()),
            |val, s| val.or(s.reset_wifi_credentials()),
        )
    }
}

#[cfg(feature = "ota")]
impl OtaMetadataStorage for RAMStorage {
    type Error = RAMStorageError;
    fn has_ota_metadata(&self) -> bool {
        let inner_ref = self.0.lock().unwrap();
        inner_ref.ota_metadata.is_some()
    }
    fn store_ota_metadata(&self, ota_metadata: &OtaMetadata) -> Result<(), Self::Error> {
        let mut inner_ref = self.0.lock().unwrap();
        let _ = inner_ref.ota_metadata.insert(ota_metadata.clone());
        Ok(())
    }
    fn get_ota_metadata(&self) -> Result<OtaMetadata, Self::Error> {
        let inner_ref = self.0.lock().unwrap();
        inner_ref
            .ota_metadata
            .clone()
            .ok_or(RAMStorageError::NotFound)
    }
    fn reset_ota_metadata(&self) -> Result<(), Self::Error> {
        let _ = self.0.lock().unwrap().ota_metadata.take();
        Ok(())
    }
}
#[cfg(feature = "ota")]
impl<Iterable, Storage: OtaMetadataStorage> OtaMetadataStorage for Iterable
where
    for<'a> &'a Iterable: IntoIterator<Item = &'a Storage>,
    Storage::Error: From<EmptyStorageCollectionError>,
{
    type Error = Storage::Error;
    fn has_ota_metadata(&self) -> bool {
        self.into_iter().any(OtaMetadataStorage::has_ota_metadata)
    }
    fn get_ota_metadata(&self) -> Result<OtaMetadata, Self::Error> {
        self.into_iter().fold(
            Err::<_, Self::Error>(EmptyStorageCollectionError.into()),
            |val, s| val.or_else(|_| s.get_ota_metadata()),
        )
    }
    fn reset_ota_metadata(&self) -> Result<(), Self::Error> {
        self.into_iter().fold(
            Err::<_, Self::Error>(EmptyStorageCollectionError.into()),
            |val, s| val.or(s.reset_ota_metadata()),
        )
    }
    fn store_ota_metadata(&self, ota_metadata: &OtaMetadata) -> Result<(), Self::Error> {
        self.into_iter().fold(
            Err::<_, Self::Error>(EmptyStorageCollectionError.into()),
            |val, s| val.or_else(|_| s.store_ota_metadata(ota_metadata)),
        )
    }
}

impl RobotConfigurationStorage for RAMStorage {
    type Error = RAMStorageError;
    fn has_robot_credentials(&self) -> bool {
        let inner_ref = self.0.lock().unwrap();
        inner_ref.robot_creds.is_some()
    }
    fn store_robot_credentials(&self, cfg: &CloudConfig) -> Result<(), Self::Error> {
        let creds: RobotCredentials = cfg.clone().try_into()?;
        let mut inner_ref = self.0.lock().unwrap();
        let _ = inner_ref.robot_creds.insert(creds);
        Ok(())
    }
    fn get_robot_credentials(&self) -> Result<RobotCredentials, Self::Error> {
        let inner_ref = self.0.lock().unwrap();
        inner_ref
            .robot_creds
            .clone()
            .ok_or(RAMStorageError::NotFound)
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
        self.0
            .lock()
            .unwrap()
            .robot_config
            .clone()
            .ok_or(RAMStorageError::NotFound)
    }
    fn reset_robot_configuration(&self) -> Result<(), Self::Error> {
        let _ = self.0.lock().unwrap().robot_config.take();
        Ok(())
    }
    fn get_tls_certificate(&self) -> Result<TlsCertificate, Self::Error> {
        let inner_ref = self.0.lock().unwrap();
        inner_ref.tls_cert.clone().ok_or(RAMStorageError::NotFound)
    }
    fn has_tls_certificate(&self) -> bool {
        let inner_ref = self.0.lock().unwrap();
        inner_ref.tls_cert.is_some()
    }
    fn store_tls_certificate(&self, creds: &TlsCertificate) -> Result<(), Self::Error> {
        let mut inner_ref = self.0.lock().unwrap();
        let _ = inner_ref.tls_cert.insert(creds.clone());
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
        Ok(inner_ref
            .app_address
            .clone()
            .unwrap_or_default()
            .parse::<Uri>()?)
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

impl<Iterable, Storage: RobotConfigurationStorage> RobotConfigurationStorage for Iterable
where
    for<'a> &'a Iterable: IntoIterator<Item = &'a Storage>,
    Storage::Error: From<EmptyStorageCollectionError>,
{
    type Error = Storage::Error;
    fn has_robot_credentials(&self) -> bool {
        self.into_iter()
            .any(RobotConfigurationStorage::has_robot_credentials)
    }
    fn has_tls_certificate(&self) -> bool {
        self.into_iter()
            .any(RobotConfigurationStorage::has_tls_certificate)
    }
    fn has_app_address(&self) -> bool {
        self.into_iter()
            .any(RobotConfigurationStorage::has_app_address)
    }
    fn has_robot_configuration(&self) -> bool {
        self.into_iter()
            .any(RobotConfigurationStorage::has_robot_configuration)
    }
    fn get_robot_credentials(&self) -> Result<RobotCredentials, Self::Error> {
        self.into_iter().fold(
            Err::<_, Self::Error>(EmptyStorageCollectionError.into()),
            |val, s| val.or_else(|_| s.get_robot_credentials()),
        )
    }
    fn get_tls_certificate(&self) -> Result<TlsCertificate, Self::Error> {
        self.into_iter().fold(
            Err::<_, Self::Error>(EmptyStorageCollectionError.into()),
            |val, s| val.or_else(|_| s.get_tls_certificate()),
        )
    }
    fn get_app_address(&self) -> Result<Uri, Self::Error> {
        self.into_iter().fold(
            Err::<_, Self::Error>(EmptyStorageCollectionError.into()),
            |val, s| val.or_else(|_| s.get_app_address()),
        )
    }
    fn store_app_address(&self, uri: &str) -> Result<(), Self::Error> {
        self.into_iter().fold(
            Err::<_, Self::Error>(EmptyStorageCollectionError.into()),
            |val, s| val.or_else(|_| s.store_app_address(uri)),
        )
    }
    fn store_tls_certificate(&self, creds: &TlsCertificate) -> Result<(), Self::Error> {
        self.into_iter().fold(
            Err::<_, Self::Error>(EmptyStorageCollectionError.into()),
            |val, s| val.or_else(|_| s.store_tls_certificate(creds)),
        )
    }
    fn store_robot_configuration(&self, cfg: &RobotConfig) -> Result<(), Self::Error> {
        self.into_iter().fold(
            Err::<_, Self::Error>(EmptyStorageCollectionError.into()),
            |val, s| val.or_else(|_| s.store_robot_configuration(cfg)),
        )
    }
    fn store_robot_credentials(&self, cfg: &CloudConfig) -> Result<(), Self::Error> {
        self.into_iter().fold(
            Err::<_, Self::Error>(EmptyStorageCollectionError.into()),
            |val, s| val.or_else(|_| s.store_robot_credentials(cfg)),
        )
    }
    fn get_robot_configuration(&self) -> Result<RobotConfig, Self::Error> {
        self.into_iter().fold(
            Err::<_, Self::Error>(EmptyStorageCollectionError.into()),
            |val, s| val.or_else(|_| s.get_robot_configuration()),
        )
    }
    fn reset_app_address(&self) -> Result<(), Self::Error> {
        self.into_iter().fold(
            Err::<_, Self::Error>(EmptyStorageCollectionError.into()),
            |val, s| val.or(s.reset_app_address()),
        )
    }
    fn reset_robot_configuration(&self) -> Result<(), Self::Error> {
        self.into_iter().fold(
            Err::<_, Self::Error>(EmptyStorageCollectionError.into()),
            |val, s| val.or(s.reset_robot_configuration()),
        )
    }
    fn reset_tls_certificate(&self) -> Result<(), Self::Error> {
        self.into_iter().fold(
            Err::<_, Self::Error>(EmptyStorageCollectionError.into()),
            |val, s| val.or(s.reset_tls_certificate()),
        )
    }
    fn reset_robot_credentials(&self) -> Result<(), Self::Error> {
        self.into_iter().fold(
            Err::<_, Self::Error>(EmptyStorageCollectionError.into()),
            |val, s| val.or(s.reset_robot_credentials()),
        )
    }
}

impl WifiCredentialStorage for RAMStorage {
    type Error = RAMStorageError;
    fn get_wifi_credentials(&self) -> Result<WifiCredentials, Self::Error> {
        let inner_ref = self.0.lock().unwrap();
        inner_ref
            .wifi_creds
            .clone()
            .ok_or(RAMStorageError::NotFound)
    }
    fn get_all_networks(&self) -> Result<Vec<WifiCredentials>, Self::Error> {
        Ok(vec![self.get_wifi_credentials()?])
    }
    fn has_wifi_credentials(&self) -> bool {
        let inner_ref = self.0.lock().unwrap();
        inner_ref.wifi_creds.is_some()
    }
    fn store_wifi_credentials(&self, creds: &WifiCredentials) -> Result<(), Self::Error> {
        let mut inner_ref = self.0.lock().unwrap();
        let _ = inner_ref.wifi_creds.insert(creds.clone());
        Ok(())
    }
    fn reset_wifi_credentials(&self) -> Result<(), Self::Error> {
        let mut inner_ref = self.0.lock().unwrap();
        let _ = inner_ref.wifi_creds.take();
        Ok(())
    }
}

impl<Iterable, Storage: WifiCredentialStorage> WifiCredentialStorage for Iterable
where
    for<'a> &'a Iterable: IntoIterator<Item = &'a Storage>,
    Storage::Error: From<EmptyStorageCollectionError>,
{
    type Error = Storage::Error;
    fn has_wifi_credentials(&self) -> bool {
        self.into_iter()
            .any(WifiCredentialStorage::has_wifi_credentials)
    }
    fn get_wifi_credentials(&self) -> Result<WifiCredentials, Self::Error> {
        self.into_iter().fold(
            Err::<_, Self::Error>(EmptyStorageCollectionError.into()),
            |val, s| val.or_else(|_| s.get_wifi_credentials()),
        )
    }
    fn get_all_networks(&self) -> Result<Vec<WifiCredentials>, Self::Error> {
        self.into_iter().fold(
            Err::<_, Self::Error>(EmptyStorageCollectionError.into()),
            |val, s| val.or_else(|_| s.get_all_networks()),
        )
    }
    fn store_wifi_credentials(&self, creds: &WifiCredentials) -> Result<(), Self::Error> {
        self.into_iter().fold(
            Err::<_, Self::Error>(EmptyStorageCollectionError.into()),
            |val, s| val.or_else(|_| s.store_wifi_credentials(creds)),
        )
    }
    fn reset_wifi_credentials(&self) -> Result<(), Self::Error> {
        self.into_iter().fold(
            Err::<_, Self::Error>(EmptyStorageCollectionError.into()),
            |val, s| val.or(s.reset_wifi_credentials()),
        )
    }
}

impl StorageDiagnostic for RAMStorage {
    fn log_space_diagnostic(&self) {}
}

impl<Iterable, Storage: StorageDiagnostic> StorageDiagnostic for Iterable
where
    for<'a> &'a Iterable: IntoIterator<Item = &'a Storage>,
{
    fn log_space_diagnostic(&self) {
        self.into_iter()
            .for_each(StorageDiagnostic::log_space_diagnostic);
    }
}

#[cfg(test)]
mod tests {
    use crate::common::credentials_storage::{
        RAMStorage, RAMStorageError, RobotConfigurationStorage,
    };
    use crate::proto::provisioning::v1::CloudConfig;
    use std::collections::HashSet;
    #[test_log::test]
    fn test_vec_storage_empty() {
        let v: Vec<RAMStorage> = vec![];

        assert!(!v.has_robot_credentials());
        let err = v.get_robot_credentials();
        assert!(err.is_err());
        assert!(matches!(
            err.unwrap_err(),
            RAMStorageError::NVSCollectionEmpty(_)
        ));
    }

    #[test_log::test]
    fn test_trait_impls() {
        // compile time check for trait implementation over collections
        fn is_robot_configuration_storage<T: RobotConfigurationStorage>() {}
        is_robot_configuration_storage::<Vec<RAMStorage>>();
        is_robot_configuration_storage::<[RAMStorage; 2]>();
        is_robot_configuration_storage::<HashSet<RAMStorage>>();
    }

    #[test_log::test]
    fn test_vec_storage() {
        let ram1 = RAMStorage::new();
        let ram2 = RAMStorage::new();
        let ram3 = RAMStorage::new();
        let v: Vec<RAMStorage> = vec![ram1.clone(), ram2.clone(), ram3.clone()];

        assert!(!v.has_robot_credentials());
        assert!(ram2
            .store_robot_credentials(&CloudConfig {
                app_address: "http://downloadramstorage.org".to_owned(),
                id: "ram2".to_owned(),
                secret: "secret".to_owned()
            })
            .is_ok());
        assert!(v.has_robot_credentials());
        let cred = v.get_robot_credentials();
        assert!(cred.is_ok());
        let cred = cred.unwrap();
        assert_eq!(cred.robot_id, "ram2");

        assert!(ram1
            .store_robot_credentials(&CloudConfig {
                app_address: "http://downloadramstorage.org".to_owned(),
                id: "ram1".to_owned(),
                secret: "secret".to_owned()
            })
            .is_ok());
        assert!(ram2
            .store_robot_credentials(&CloudConfig {
                app_address: "http://downloadramstorage.org".to_owned(),
                id: "ram2".to_owned(),
                secret: "secret".to_owned()
            })
            .is_ok());
        assert!(v.has_robot_credentials());
        let cred = v.get_robot_credentials();
        assert!(cred.is_ok());
        let cred = cred.unwrap();
        // if multiple credentials stored first one should be returned
        assert_eq!(cred.robot_id, "ram1");

        // reset always remove credentials from all storage in array
        assert!(v.reset_robot_credentials().is_ok());
        assert!(!v.has_robot_credentials());

        assert!(v
            .store_robot_credentials(&CloudConfig {
                app_address: "http://downloadramstorage.org".to_owned(),
                id: "vec".to_owned(),
                secret: "secret".to_owned()
            })
            .is_ok());
        assert!(v.has_robot_credentials());
        let cred = v.get_robot_credentials();
        assert!(cred.is_ok());
        let cred = cred.unwrap();
        assert_eq!(cred.robot_id, "vec");
        let cred = ram1.get_robot_credentials();
        assert!(cred.is_ok());
        let cred = cred.unwrap();
        assert_eq!(cred.robot_id, "vec");
    }
}
