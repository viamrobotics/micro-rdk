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
    },
    proto::app::v1::ServiceConfig,
};
use embedded_svc::http::{client::Client, Headers};

// TODO: set according to running partition scheme
const OTA_MAX_IMAGE_SIZE: usize = 1024 * 1024 * 4; // 4MB
/// The actual minimum size of an (app image)[https://github.com/espressif/esp-idf/blob/v4.4.8/components/bootloader_support/include/esp_app_format.h] would be more like:
/// min_bytes = `sizeof(esp_image_header_t) + sizeof(esp_image_segment_header_t) + sizeof(esp_app_desc_t) + sizeof(esp_image_segment_header_t) + sizeof(esp_image application) = 24 + 8 + 265 + 8 + ? `.
/// However, builds with micro-rdk are unlikely to be <2MB so the minimum is set as sucha.
const OTA_MIN_IMAGE_SIZE: usize = 1024 * 1024 * 2; // 2MB
const OTA_CHUNK_SIZE: usize = 1024 * 20; // 20KB
const OTA_HTTP_BUFSIZE: usize = 1024 * 4; // 4KB
pub const OTA_MODEL_TYPE: &str = "ota_service";
pub const OTA_MODEL_TRIPLET: &str = "rdk:builtin:ota_service";

use thiserror::Error;
#[derive(Error, Debug)]
pub enum OtaError {
    #[error("{0}")]
    ConfigError(String),
    #[error("error downloading new firmware: {0}")]
    DownloadError(String),
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

    pub(crate) async fn update(&mut self) -> Result<(), OtaError> {
        let connection = EspHttpConnection::new(&Configuration {
            buffer_size: Some(OTA_HTTP_BUFSIZE),
            buffer_size_tx: Some(OTA_HTTP_BUFSIZE),
            use_global_ca_store: true,
            crt_bundle_attach: Some(esp_idf_svc::sys::esp_crt_bundle_attach),
            ..Default::default()
        })
        .map_err(|e| OtaError::DownloadError(e.to_string()))?;

        let mut client = Client::wrap(connection);
        let request = client
            .get(&self.url)
            .map_err(|e| OtaError::DownloadError(e.to_string()))?;
        let mut response = request
            .submit()
            .map_err(|e| OtaError::DownloadError(e.to_string()))?;
        let status = response.status();

        if !(200..=299).contains(&status) {
            return Err(OtaError::DownloadError(
                format!("Bad Request - Status:{}", status).to_string(),
            ));
        }

        let file_len = response.content_len().unwrap_or(0) as usize;
        if !(OTA_MIN_IMAGE_SIZE..OTA_MAX_IMAGE_SIZE).contains(&file_len) {
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
            return Err(OtaError::DownloadError(
                "download incomplete, update aborted".to_string(),
            ));
        }
        update_handle.complete().map_err(OtaError::EspError)?;
        log::info!("will boot new firmware on reset");
        Ok(())
    }
}
