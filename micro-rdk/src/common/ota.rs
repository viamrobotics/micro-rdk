///
///
/// ```rs
/// // example config from app.viam as seen by micro-rdk
/// ServiceConfig {
///         name: "OTA",
///         namespace: "rdk",
///         r#type: "generic",
///         attributes: Some(
///             Struct {
///                 fields: {
///                     "url": Value {
///                         kind: Some(
///                             StringValue(
///                             "https://my.bucket.com/my-ota.bin",
///                             ),
///                         ),
///                     },
///                 },
///             },
///         ),
///         depends_on: [],
///         model: "rdk:builtin:ota_service",
///         api: "rdk:service:generic",
///         service_configs: [],
///         log_configuration: None,
/// }
/// ```
///
/// The sdkconfig options relevant to OTA, currently default as of esp-idf v4
/// and should be reviewed when upgrading to esp-idf v5
/// - CONFIG_BOOTLOADER_FACTORY_RESET=NO
///   - clear data partitions and boot from factory partition
/// - CONFIG_BOOTLOADER_APP_ROLLBACK_ENABLE=NO
///   - after updating the app, bootloader runs a new app with the "ESP_OTA_IMG_PENDING_VERIFY" state set. If the image is not marked as verified, will boot to previous ota slot
///
use crate::{
    common::{
        config::{AttributeError, Kind},
        conn::viam::ViamH2Connector,
        credentials_storage::OtaMetadataStorage,
        exec::Executor,
        grpc_client::H2Timer,
    },
    proto::app::v1::ServiceConfig,
};

#[cfg(feature = "esp32")]
use crate::esp32::esp_idf_svc::{
    ota::{EspFirmwareInfoLoader, EspOta},
    sys::{esp_ota_get_next_update_partition, esp_partition_t},
};
use async_io::Timer;
use http_body_util::{BodyExt, Empty};
use hyper::{body::Bytes, client::conn::http2, Request};
use once_cell::sync::Lazy;
use std::time::Duration;
use thiserror::Error;
#[cfg(not(feature = "esp32"))]
use {bincode::Decode, futures_lite::AsyncWriteExt};

const CONN_RETRY_SECS: u64 = 60;
const SIZEOF_APPDESC: usize = 256;
const MAX_VER_LEN: usize = 128;
pub const OTA_MODEL_TYPE: &str = "ota_service";
pub static OTA_MODEL_TRIPLET: Lazy<String> =
    Lazy::new(|| format!("rdk:builtin:{}", OTA_MODEL_TYPE));

/// https://github.com/espressif/esp-idf/blob/ce6085349f8d5a95fc857e28e2d73d73dd3629b5/components/esp_app_format/include/esp_app_desc.h#L42
/// https://docs.esp-rs.org/esp-idf-sys/esp_idf_sys/struct.esp_app_desc_t.html
/// typedef struct {
///     uint32_t magic_word;        /*!< Magic word ESP_APP_DESC_MAGIC_WORD */
///     uint32_t secure_version;    /*!< Secure version */
///     uint32_t reserv1[2];        /*!< reserv1 */
///     char version[32];           /*!< Application version */
///     char project_name[32];      /*!< Project name */
///     char time[16];              /*!< Compile time */
///     char date[16];              /*!< Compile date*/
///     char idf_ver[32];           /*!< Version IDF */
///     uint8_t app_elf_sha256[32]; /*!< sha256 of elf file */
///     uint32_t reserv2[19];       /*!< reserv2 */
/// } esp_app_desc_t;
///
/// should be 256 bytes in size
#[cfg(not(feature = "esp32"))]
#[repr(C)]
#[derive(Decode, Debug, PartialEq)]
struct EspAppDesc {
    //TODO(RSDK-9342): add verified, native impl of esp_app_desc_t for debugging
    /// ESP_APP_DESC_MAGIC_WORD (0xABCD5432)
    magic_word: u32,
    secure_version: u32,
    reserv1: [u32; 2],
    /// application version
    version: [u8; 32],
    project_name: [u8; 32],
    /// compile time
    time: [u8; 16],
    /// compile date
    date: [u8; 16],
    idf_ver: [u8; 32],
    app_elf_sha256: [u8; 32],
    reserv2: [u32; 20],
}

#[derive(Error, Debug)]
pub(crate) enum ConfigError {
    #[error("version {0} has length {1}, maximum allowed characters is {2}")]
    InvalidVersionLen(String, usize, usize),
    #[error("the url `{0}`, is not valid: {1}")]
    InvalidUrl(String, String),
    #[error(transparent)]
    AttributeError(#[from] AttributeError),
    #[error("required config attribute `{0}` not found")]
    MissingAttribute(String),
    #[error("value missing for field `{0}`")]
    MissingValue(String),
    #[error("{0}")]
    Other(String),
}

#[derive(Error, Debug)]
pub(crate) enum OtaError {
    #[error("error occured during abort process: {0}")]
    AbortError(String),
    #[error("{0}")]
    ConfigError(#[from] ConfigError),
    #[error(transparent)]
    NetworkError(#[from] hyper::Error),
    #[error("{0}")]
    UpdateError(String),
    #[error("failed to initialize ota process")]
    InitError,
    #[error("new image of {0} bytes is larger than target partition of {1} bytes")]
    InvalidImageSize(usize, usize),
    #[error("failed to retrieve firmware header info from binary, firmware may not be valid for this system: {0}")]
    InvalidFirmware(String),
    #[error("error writing firmware to update partition: {0}")]
    WriteError(String),
    #[error("{0}")]
    Other(String),
}

#[cfg(feature = "esp32")]
type OtaConnector = crate::esp32::tcp::Esp32H2Connector;
#[cfg(not(feature = "esp32"))]
type OtaConnector = crate::native::tcp::NativeH2Connector;

#[derive(Clone, Default, Debug)]
pub struct OtaMetadata {
    pub(crate) version: String,
}

impl OtaMetadata {
    pub fn new(version: String) -> Self {
        Self { version }
    }
    pub(crate) fn version(&self) -> &str {
        &self.version
    }
}

pub(crate) struct OtaService<S: OtaMetadataStorage> {
    exec: Executor,
    connector: OtaConnector,
    storage: S,
    url: String,
    pending_version: String,
    max_size: usize,
    address: usize,
}

impl<S: OtaMetadataStorage> OtaService<S> {
    pub(crate) fn from_config(
        new_config: &ServiceConfig,
        storage: S,
        exec: Executor,
    ) -> Result<Self, OtaError> {
        let kind = new_config.attributes.as_ref().ok_or_else(|| {
            ConfigError::Other("ota service config has no attributes".to_string())
        })?;

        let url = kind
            .fields
            .get("url")
            .ok_or(ConfigError::MissingAttribute("url".to_string()))?
            .kind
            .as_ref()
            .ok_or(ConfigError::MissingValue("url".to_string()))?
            .try_into()
            .map_err(|e: AttributeError| ConfigError::Other(e.to_string()))?;

        let url = match url {
            Kind::StringValue(s) => Ok(s),
            _ => Err(ConfigError::Other(format!("invalid url value: {:?}", kind))),
        }?;

        let pending_version = kind
            .fields
            .get("version")
            .ok_or(ConfigError::MissingAttribute("version".to_string()))?
            .kind
            .as_ref()
            .ok_or(ConfigError::Other(
                "failed to get inner for `version`".to_string(),
            ))?
            .try_into()
            .map_err(|e: AttributeError| ConfigError::AttributeError(e))?;

        let pending_version = match pending_version {
            Kind::StringValue(s) => Ok(s),
            _ => Err(ConfigError::Other(format!(
                "invalid url value: {:?}",
                pending_version
            ))),
        }?;

        if pending_version.len() > MAX_VER_LEN {
            let len = pending_version.len();
            return Err(OtaError::ConfigError(ConfigError::InvalidVersionLen(
                pending_version,
                len,
                MAX_VER_LEN,
            )));
        }

        let connector = OtaConnector::default();

        #[cfg(not(feature = "esp32"))]
        let (max_size, address) = (1024 * 1024 * 4, 0xabcd);
        #[cfg(feature = "esp32")]
        let (max_size, address) = {
            log::debug!("getting handle to next OTA update partition");
            let ptr: *const esp_partition_t =
                unsafe { esp_ota_get_next_update_partition(std::ptr::null()) };

            if ptr.is_null() {
                let e = OtaError::UpdateError(
                    "failed to obtain a handle to the next OTA update partition, device may not be partitioned properly for OTA".to_string(),
                );
                log::warn!("{}", e.to_string());
                return Err(e);
            }
            let size = unsafe { (*ptr).size } as usize;
            let address = unsafe { (*ptr).address } as usize;

            (size, address)
        };

        Ok(Self {
            connector,
            exec,
            storage,
            url,
            pending_version,
            max_size,
            address,
        })
    }

    pub(crate) async fn update(&mut self) -> Result<(), OtaError> {
        let stored_metadata = if !self.storage.has_ota_metadata() {
            log::info!("no ota metadata currently stored in NVS");
            OtaMetadata::default()
        } else {
            self.storage
                .get_ota_metadata()
                .inspect_err(|e| log::warn!("failed to get ota metadata from nvs: {}", e))
                .unwrap_or_default()
        };

        if self.pending_version == stored_metadata.version() {
            log::info!("firmware is up-to-date: `{}`", stored_metadata.version);
            return Ok(());
        }

        log::info!(
            "firmware is out of date, proceeding with update from version `{}` to version `{}`",
            stored_metadata.version,
            self.pending_version
        );

        let mut uri = self
            .url
            .parse::<hyper::Uri>()
            .map_err(|e| ConfigError::InvalidUrl(self.url.clone(), e.to_string()))?;

        if uri.port().is_none() {
            if uri.scheme_str() != Some("https") {
                log::error!("no port found and not https");
            }

            let mut auth = uri
                .authority()
                .ok_or(OtaError::Other("no authority present in uri".to_string()))?
                .to_string();
            auth.push_str(":443");
            let mut parts = uri.into_parts();
            parts.authority = Some(
                auth.parse()
                    .map_err(|_| OtaError::Other("failed to parse authority".to_string()))?,
            );
            uri = hyper::Uri::from_parts(parts).map_err(|e| OtaError::Other(e.to_string()))?;
        };

        let (mut sender, conn) = loop {
            match self.connector.connect_to(&uri) {
                Ok(connection) => {
                    match connection.await {
                        Ok(io) => {
                            match http2::Builder::new(self.exec.clone())
                                .max_frame_size(16_384) // lowest configurable
                                .timer(H2Timer)
                                .handshake(io)
                                .await
                            {
                                Ok(pair) => break pair,
                                Err(e) => {
                                    log::error!("failed to build http request: {}", e);
                                }
                            }
                        }
                        Err(e) => {
                            log::error!("failed to create tcp stream: {}", e);
                        }
                    }
                }
                Err(e) => {
                    log::error!("failed to create http connection: {}", e);
                }
            };

            log::debug!(
                "retry establishing http connection to {} in {} seconds",
                &uri,
                CONN_RETRY_SECS
            );
            Timer::after(Duration::from_secs(CONN_RETRY_SECS)).await;
        };

        let conn = Box::new(self.exec.spawn(async move {
            if let Err(err) = conn.await {
                log::error!("connection failed: {:?}", err);
            }
        }));

        log::info!("ota connected, beginning download");
        let request = Request::builder()
            .method("GET")
            .uri(uri)
            .body(Empty::<Bytes>::new())
            .map_err(|e| OtaError::Other(e.to_string()))?;
        let mut response = sender
            .send_request(request)
            .await
            .map_err(|e| OtaError::Other(e.to_string()))?;

        // TODO(RSDK-9346): handle other success codes
        if response.status() != 200 {
            return Err(OtaError::Other(
                format!("Bad Request - Status:{}", response.status()).to_string(),
            ));
        }
        let headers = response.headers();
        log::debug!("ota response headers: {:?}", headers);

        if !headers.contains_key(hyper::header::CONTENT_LENGTH) {
            return Err(OtaError::Other(
                "response header missing content length".to_string(),
            ));
        }
        let file_len = headers[hyper::header::CONTENT_LENGTH]
            .to_str()
            .map_err(|e| OtaError::Other(e.to_string()))?
            .parse::<usize>()
            .map_err(|e| OtaError::Other(e.to_string()))?;

        if file_len > self.max_size {
            return Err(OtaError::InvalidImageSize(file_len, self.max_size));
        }

        #[cfg(feature = "esp32")]
        let (mut ota, running_fw_info) = {
            let ota = EspOta::new().map_err(|e| {
                OtaError::UpdateError(format!("failed to initiate ota partition handle: {}", e))
            })?;
            let fw_info = ota
                .get_running_slot()
                .map_err(|e| {
                    OtaError::UpdateError(format!(
                        "failed to get handle to running ota partition: {}",
                        e
                    ))
                })?
                .firmware;
            (ota, fw_info)
        };
        #[cfg(feature = "esp32")]
        let mut update_handle = ota.initiate_update().map_err(|_| OtaError::InitError)?;
        #[cfg(not(feature = "esp32"))]
        let mut update_handle = Vec::new();
        let mut nwritten: usize = 0;
        let mut total_downloaded: usize = 0;
        let mut got_info = false;

        log::info!("writing new firmware to address `{:#x}`", self.address,);

        while let Some(next) = response.frame().await {
            let frame = next.map_err(|e| OtaError::Other(e.to_string()))?;
            if !frame.is_data() {
                return Err(OtaError::Other(
                    "download contained non-data frame".to_string(),
                ));
            }

            // Err variant returns the original frame, not an impl Error
            let data = frame
                .into_data()
                .map_err(|_| OtaError::Other("failed to get data from frame".to_string()))?;
            total_downloaded += data.len();

            if !got_info {
                if total_downloaded < SIZEOF_APPDESC {
                    log::error!("initial frame too small to retrieve esp_app_desc_t");
                } else {
                    #[cfg(feature = "esp32")]
                    {
                        log::info!("verifying new ota firmware");
                        let mut loader = EspFirmwareInfoLoader::new();
                        loader
                            .load(&data)
                            .map_err(|e| OtaError::InvalidFirmware(e.to_string()))?;
                        let new_fw = loader
                            .get_info()
                            .map_err(|e| OtaError::InvalidFirmware(e.to_string()))?;
                        log::debug!("current firmware app description: {:?}", running_fw_info);
                        log::debug!("new firmware app description: {:?}", new_fw);
                    }
                    #[cfg(not(feature = "esp32"))]
                    {
                        log::debug!("deserializing app header");
                        if let Ok(decoded) = bincode::decode_from_slice::<
                            EspAppDesc,
                            bincode::config::Configuration,
                        >(
                            &data[..SIZEOF_APPDESC], bincode::config::standard()
                        ) {
                            log::debug!("{:?}", decoded.0);
                        }
                    }
                    got_info = true;
                }
            }

            if data.len() + nwritten > self.max_size {
                log::error!("file is larger than expected, aborting");
                #[cfg(feature = "esp32")]
                update_handle
                    .abort()
                    .map_err(|e| OtaError::AbortError(format!("{:?}", e)))?;
                return Err(OtaError::InvalidImageSize(
                    data.len() + nwritten,
                    self.max_size,
                ));
            }

            // TODO(RSDK-9271) add async writer for ota
            #[cfg(feature = "esp32")]
            update_handle
                .write(&data)
                .map_err(|e| OtaError::WriteError(e.to_string()))?;
            #[cfg(not(feature = "esp32"))]
            let _n = update_handle
                .write(&data)
                .await
                .map_err(|e| OtaError::Write(e.to_string()))?;
            // TODO change back to 'n' after impl async writer
            nwritten += data.len();
            log::info!(
                "updating next OTA partition at {}: {}/{} bytes written",
                self.address,
                nwritten,
                file_len
            );
        }

        drop(conn);
        log::info!("firmware download complete: {} bytes", nwritten);

        if nwritten != file_len {
            log::error!("wrote {} bytes, expected to write {}", nwritten, file_len);
            log::error!("aborting ota");
            #[cfg(feature = "esp32")]
            update_handle
                .abort()
                .map_err(|e| OtaError::AbortError(format!("{:?}", e)))?;

            return Err(OtaError::Other(
                "nbytes written did not match file size".to_string(),
            ));
        }

        #[cfg(feature = "esp32")]
        {
            log::info!(
                "setting device to use new firmware at `{:#x}`",
                self.address
            );
            update_handle
                .complete()
                .map_err(|e| OtaError::UpdateError(format!("{:?}", e)))
        }?;

        log::info!("updating firmware metadata in NVS");
        self.storage
            .store_ota_metadata(OtaMetadata {
                version: self.pending_version.clone(),
            })
            .map_err(|e| OtaError::Other(e.to_string()))?;

        log::info!("firmware update complete");

        // Test experimental ffi accesses here to be recoverable without flashing
        #[cfg(feature = "esp32")]
        {
            log::info!("rebooting to load firmware from `{}`", self.address);
            // TODO(RSDK-9464): flush logs to app.viam before restarting
            esp_idf_svc::hal::reset::restart();
        }

        Ok(())
    }
}
