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
    esp32::esp_idf_svc::{
        http::client::{Configuration, EspHttpConnection},
        ota::{EspFirmwareInfoLoader, EspOta},
        sys::EspError,
        hal::io::EspIOError,
    },
    proto::app::v1::ServiceConfig,
};
use embedded_svc::http::{client::Client, Headers};

// TODO(RSDK-9200): set according to active partition scheme
const OTA_MAX_IMAGE_SIZE: usize = 1024 * 1024 * 4; // 4MB
const OTA_CHUNK_SIZE: usize = 1024 * 16; // 16KB
const OTA_HTTP_BUFSIZE: usize = 1024 * 16; // 16KB
pub const OTA_MODEL_TYPE: &str = "ota_service";
pub const OTA_MODEL_TRIPLET: &str = "rdk:builtin:ota_service";

use thiserror::Error;
#[derive(Error, Debug)]
pub enum OtaError {
    #[error("{0}")]
    ConfigError(String),
    #[error(transparent)]
    DownloadError(#[from] EspIOError),
    #[error(transparent)]
    EspError(#[from] EspError),
    #[error("failed to initialize ota process")]
    InitError,
    #[error("new image size is invalid: {0} bytes")]
    InvalidImageSize(usize),
    #[error("{0}")]
    Other(String),
}

#[derive(Debug)]
pub(crate) struct OtaService {
    url: String,
}

impl OtaService {
    pub(crate) fn from_config(ota_config: &ServiceConfig) -> Result<Self, OtaError> {
        let kind = ota_config
            .attributes
            .as_ref()
            .unwrap()
            .fields
            .get("url")
            .ok_or_else(|| OtaError::ConfigError("`url` not found in config".to_string()))?
            .kind
            .clone()
            .ok_or_else(|| OtaError::ConfigError("failed to get inner value".to_string()))?;

        let url = match kind {
            crate::google::protobuf::value::Kind::StringValue(s) => s,
            _ => {
                return Err(OtaError::ConfigError(format!(
                    "invalid url value: {:?}",
                    kind
                )))
            }
        };

        let _ = url
            .parse::<hyper::Uri>()
            .map_err(|_| OtaError::ConfigError(format!("invalid url: {}", url)))?;

        Ok(Self { url })
    }

    pub(crate) fn update(&mut self) -> Result<(), OtaError> {
        let connection = EspHttpConnection::new(&Configuration {
            buffer_size: Some(OTA_HTTP_BUFSIZE),
            buffer_size_tx: Some(OTA_HTTP_BUFSIZE),
            use_global_ca_store: true,
            crt_bundle_attach: Some(esp_idf_svc::sys::esp_crt_bundle_attach),
            ..Default::default()
        })
            .map_err(OtaError::EspError)?;

        let mut client = Client::wrap(connection);
        let request = client
            .get(&self.url)
            .map_err(OtaError::DownloadError)?;
        let mut response = request
            .submit()
            .map_err(OtaError::DownloadError)?;
        let status = response.status();

        if status!= 200 {
            return Err(OtaError::Other(
                format!("Bad Request - Status:{}", status).to_string(),
            ));
        }

        let file_len = response.content_len().unwrap_or(0) as usize;
        if file_len > OTA_MAX_IMAGE_SIZE {
            return Err(OtaError::InvalidImageSize(file_len));
        }

        let mut ota = EspOta::new().map_err(OtaError::EspError)?;
        let running_fw_info = ota.get_running_slot().map_err(OtaError::EspError)?.firmware;
        let mut update_handle = ota.initiate_update().map_err(|_| OtaError::InitError)?;
        let mut buff = vec![0; OTA_CHUNK_SIZE];
        let mut total_read: usize = 0;
        let mut got_info = false;
        while total_read < file_len {
            let num_read = response.read(&mut buff).unwrap_or_default();
            total_read += num_read;
            if !got_info {
                let mut loader = EspFirmwareInfoLoader::new();
                loader.load(&mut buff).map_err(OtaError::EspError)?;
                let new_fw = loader.get_info().map_err(OtaError::EspError)?;
                log::info!("current firmware: {:?}", running_fw_info);
                log::info!("new firmware: {:?}", new_fw);
                if let Some(ref running_fw) = running_fw_info {
                    if running_fw.version == new_fw.version
                        && running_fw.released == new_fw.released
                    {
                        log::info!("current firmware is up to date");
                        let _ = update_handle.abort();
                        return Ok(());
                    }
                }
                got_info = true;
            }

            if num_read == 0 {
                break;
            }

            if let Err(e) = update_handle.write(&buff[..num_read]) {
                log::error!("failed to write OTA partition, update aborted: {}", e);
                let _ = update_handle.abort();
                return Err(OtaError::EspError(e));
            }
        }

        if total_read < file_len {
            log::error!("{} bytes downloaded, needed {} bytes", total_read, file_len);
            let _ = update_handle.abort();
            return Err(OtaError::Other(
                "download incomplete, update aborted".to_string(),
            ));
        }
        update_handle.complete().map_err(OtaError::EspError)?;

        log::info!("resetting now to boot from new firmware");

        esp_idf_svc::hal::reset::restart();
        unreachable!();
    }
}
