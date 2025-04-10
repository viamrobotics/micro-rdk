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
use crate::proto::{app::v1::CertificateResponse, provisioning::v1::CloudConfig};
use thiserror::Error;

#[derive(Clone, Default, Debug)]
pub struct RobotCredentials {
    pub(crate) robot_id: String,
    pub(crate) robot_secret: String,
    pub(crate) app_address: Uri,
}

impl RobotCredentials {
    pub fn new(
        robot_id: String,
        robot_secret: String,
        app_address: String,
    ) -> Result<Self, <Uri as FromStr>::Err> {
        Ok(Self {
            robot_secret,
            robot_id,
            app_address: app_address.parse::<Uri>()?,
        })
    }

    pub(crate) fn robot_id(&self) -> &str {
        &self.robot_id
    }

    pub(crate) fn robot_secret(&self) -> &str {
        &self.robot_secret
    }

    pub(crate) fn app_address(&self) -> Uri {
        self.app_address.clone()
    }
}

impl TryFrom<CloudConfig> for RobotCredentials {
    type Error = <Uri as FromStr>::Err;
    fn try_from(value: CloudConfig) -> Result<Self, Self::Error> {
        Ok(Self {
            robot_id: value.id,
            robot_secret: value.secret,
            app_address: value.app_address.parse::<Uri>()?,
        })
    }
}

impl From<RobotCredentials> for CloudConfig {
    fn from(value: RobotCredentials) -> Self {
        Self {
            app_address: value.app_address.to_string(),
            id: value.robot_id,
            secret: value.robot_secret,
        }
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
    fn has_default_network(&self) -> bool;
    fn store_default_network(&self, ssid: &str, password: &str) -> Result<(), Self::Error>;
    fn get_default_network(&self) -> Result<NetworkSetting, Self::Error>;
    fn reset_default_network(&self) -> Result<(), Self::Error>;
    fn has_network_settings(&self) -> bool;
    fn store_network_settings(&self, networks: &[NetworkSetting]) -> Result<(), Self::Error>;
    fn get_network_settings(&self) -> Result<Vec<NetworkSetting>, Self::Error>;
    fn reset_network_settings(&self) -> Result<(), Self::Error>;
    fn get_all_networks(&self) -> Result<Vec<NetworkSetting>, Self::Error>;

    // TODO(RSDK-10105): remove deprecated methods
    #[deprecated(
        since = "0.5.0",
        note = "method has been deprecated in favor of has_default_network()"
    )]
    fn has_wifi_credentials(&self) -> bool {
        self.has_default_network()
    }
    #[deprecated(
        since = "0.5.0",
        note = "method has been deprecated in favor of store_default_network()"
    )]
    fn store_wifi_credentials(&self, creds: &WifiCredentials) -> Result<(), Self::Error> {
        self.store_default_network(&creds.ssid, &creds.pwd)
    }
    #[deprecated(
        since = "0.5.0",
        note = "method has been deprecated in favor of get_default_network()"
    )]
    fn get_wifi_credentials(&self) -> Result<WifiCredentials, Self::Error> {
        self.get_default_network().map(|network| WifiCredentials {
            ssid: network.ssid,
            pwd: network.password,
        })
    }
    #[deprecated(
        since = "0.5.0",
        note = "method has been deprecated in favor of reset_default_network()"
    )]
    fn reset_wifi_credentials(&self) -> Result<(), Self::Error> {
        self.reset_default_network()
    }
}

pub trait RobotConfigurationStorage {
    type Error: Error + Debug + Into<ServerError>;
    fn has_robot_credentials(&self) -> bool;
    fn store_robot_credentials(&self, cfg: &CloudConfig) -> Result<(), Self::Error>;
    fn get_robot_credentials(&self) -> Result<RobotCredentials, Self::Error>;
    fn reset_robot_credentials(&self) -> Result<(), Self::Error>;

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
    default_network: Option<NetworkSetting>,
    network_settings: Option<Vec<NetworkSetting>>,
    tls_cert: Option<TlsCertificate>,
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
            default_network: None,
            tls_cert: None,
            network_settings: None,
            #[cfg(feature = "ota")]
            ota_metadata: None,
        })))
    }
}

impl WifiCredentialStorage for RAMStorage {
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
    fn get_default_network(&self) -> Result<NetworkSetting, Self::Error> {
        let inner_ref = self.0.lock().unwrap();
        inner_ref
            .default_network
            .clone()
            .ok_or(RAMStorageError::NotFound)
    }

    fn has_default_network(&self) -> bool {
        let inner_ref = self.0.lock().unwrap();
        inner_ref.default_network.is_some()
    }
    fn store_default_network(&self, ssid: &str, password: &str) -> Result<(), Self::Error> {
        let mut inner_ref = self.0.lock().unwrap();
        let _ = inner_ref.default_network.insert(NetworkSetting {
            ssid: ssid.to_string(),
            password: password.to_string(),
            priority: 0,
        });
        Ok(())
    }
    fn reset_default_network(&self) -> Result<(), Self::Error> {
        let mut inner_ref = self.0.lock().unwrap();
        let _ = inner_ref.default_network.take();
        Ok(())
    }
    fn get_all_networks(&self) -> Result<Vec<NetworkSetting>, Self::Error> {
        let default_network: NetworkSetting = self.get_default_network()?;
        let mut networks = self.get_network_settings()?;
        networks.push(default_network);
        Ok(networks)
    }
}

impl<Iterable, Storage: WifiCredentialStorage> WifiCredentialStorage for Iterable
where
    for<'a> &'a Iterable: IntoIterator<Item = &'a Storage>,
    Storage::Error: From<EmptyStorageCollectionError>,
{
    type Error = Storage::Error;
    fn has_network_settings(&self) -> bool {
        self.into_iter()
            .any(WifiCredentialStorage::has_network_settings)
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
            .any(WifiCredentialStorage::has_default_network)
    }
    fn store_wifi_credentials(&self, creds: &WifiCredentials) -> Result<(), Self::Error> {
        self.into_iter().fold(
            Err::<_, Self::Error>(EmptyStorageCollectionError.into()),
            |val, s| val.or_else(|_| s.store_default_network(&creds.ssid, &creds.pwd)),
        )
    }
    fn has_default_network(&self) -> bool {
        self.into_iter()
            .any(WifiCredentialStorage::has_default_network)
    }
    fn get_default_network(&self) -> Result<NetworkSetting, Self::Error> {
        self.into_iter().fold(
            Err::<_, Self::Error>(EmptyStorageCollectionError.into()),
            |val, s| val.or_else(|_| s.get_default_network()),
        )
    }
    fn store_default_network(&self, ssid: &str, password: &str) -> Result<(), Self::Error> {
        self.into_iter().fold(
            Err::<_, Self::Error>(EmptyStorageCollectionError.into()),
            |val, s| val.or_else(|_| s.store_default_network(ssid, password)),
        )
    }
    fn reset_default_network(&self) -> Result<(), Self::Error> {
        self.into_iter().fold(
            Err::<_, Self::Error>(EmptyStorageCollectionError.into()),
            |val, s| val.or(s.reset_default_network()),
        )
    }
    fn get_all_networks(&self) -> Result<Vec<NetworkSetting>, Self::Error> {
        self.into_iter().fold(
            Err::<_, Self::Error>(EmptyStorageCollectionError.into()),
            |val, s| val.or(s.get_all_networks()),
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
