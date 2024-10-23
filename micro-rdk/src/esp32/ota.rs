///
/// example Config
/// ```rs
/// // example config from app.viam
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
/// The sdkconfig options relevant to OTA currently default as of esp-idf v4
/// and should be reviewed when upgrading to esp-idf v5
///
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
use embedded_svc::http::{client::Client, Headers, Method};

const OTA_CHUNK_SIZE: usize = 1024 * 20; // 20KB
/// bounded by partition scheme
const OTA_MAX_SIZE: usize = 1024 * 1024 * 4; // 4MB
/// The actual minimum size of an (app image)[https://github.com/espressif/esp-idf/blob/v4.4.8/components/bootloader_support/include/esp_app_format.h] would be more like:
/// min_bytes = `sizeof(esp_image_header_t) + sizeof(esp_image_segment_header_t) + sizeof(esp_app_desc_t) + sizeof(esp_image_segment_header_t) + sizeof(esp_image application) = 24 + 8 + 265 + 8 + ? `.
/// However, builds have not been beneath 2MB in a while so the minimum is set as such.
const OTA_MIN_SIZE: usize = 1024 * 1024 * 2; // 2MB
/// Determined by partition scheme, currently <4MB to support 8MB devices.
const OTA_HTTP_BUFSIZE: usize = 1024 * 4; // 4KB
pub const OTA_MODEL_TYPE: &str = "ota_service";
pub const OTA_MODEL_TRIPLET: &str = "rdk:builtin:ota_service";

use thiserror::Error;
#[derive(Error, Debug)]
pub enum OtaError {
    #[error("error downloading new firmware: ")]
    DownloadError(String),
    #[error(transparent)]
    EspError(#[from] EspError),
    #[error("failed to initialize ota process")]
    InitError,
    #[error("new image is invalid")]
    InvalidImage,
    #[error("{0}")]
    Other(String),
}

#[derive(Debug)]
pub(crate) struct OtaService {
    url: String,
}

impl OtaService {
    pub(crate) fn new(ota_config: &ServiceConfig) -> Self {
        let kind = ota_config
            .attributes
            .as_ref()
            .unwrap()
            .fields
            .get("url")
            .unwrap()
            .kind
            .clone()
            .unwrap();
        let url = match kind {
            crate::google::protobuf::value::Kind::StringValue(s) => s,
            _ => "".to_string(),
        };
        let uri = url.parse::<hyper::Uri>().unwrap();

        Self {
            url: uri.to_string(),
        }
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

        let headers = [("accept", "application/octet_stream")];
        let request = client
            .request(Method::Get, &self.url, &headers)
            .map_err(|e| OtaError::DownloadError(e.to_string()))?;

        let mut response = request
            .submit()
            .map_err(|e| OtaError::DownloadError(e.to_string()))?;
        let status = response.status();

        if status < 200 || status > 299 {
            return Err(OtaError::DownloadError(
                format!("Bad Request - Status:{}", status).to_string(),
            ));
        }

        let file_len = response.content_len().unwrap_or(0) as usize;

        if file_len <= OTA_MIN_SIZE {
            log::error!("new image too small: {} bytes", file_len);
            return Err(OtaError::InvalidImage);
        }
        if file_len > OTA_MAX_SIZE {
            log::error!("new image too big: {} bytes", file_len);
            return Err(OtaError::InvalidImage);
        }

        let mut ota = EspOta::new().map_err(OtaError::EspError)?;
        let running_fw_info = ota.get_running_slot().map_err(OtaError::EspError)?.firmware;
        let mut update_handle = ota.initiate_update().map_err(|_| OtaError::InitError)?;
        let mut buff = vec![0; OTA_CHUNK_SIZE];
        let mut total_read_len: usize = 0;
        let mut got_info = false;
        while total_read_len < file_len {
            let n = response.read(&mut buff).unwrap_or_default();
            total_read_len += n;
            if !got_info {
                let mut loader = EspFirmwareInfoLoader::new();
                loader.load(&mut buff).map_err(OtaError::EspError)?;
                let new_fw = loader.get_info().map_err(OtaError::EspError)?;

                log::info!("Firmware to be downloaded: {new_fw:?}");
                if let Some(ref running_fw) = running_fw_info {
                    if running_fw.version == new_fw.version {
                        log::info!("current firmware is up to date");
                        let _ = update_handle.abort();
                        return Ok(());
                    }
                }
                got_info = true;
            }
            if n == 0 {
                break;
            }

            if let Err(e) = update_handle.write(&buff[..n]) {
                log::error!("Failed to write to OTA. {e}");
                let _ = update_handle.abort();
                return Err(OtaError::EspError(e));
            }
        }

        if total_read_len < file_len {
            log::error!("{total_read_len} bytes downloaded, needed {file_len} bytes");
            let _ = update_handle.abort();
            return Err(OtaError::DownloadError(
                "download incomplete, aborting ota update".to_string(),
            ));
        }
        // flip ota bit
        update_handle.complete().expect("failed to complete ota");
        log::info!("will boot new firmware on reset");
        Ok(())
    }
}
