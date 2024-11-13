#[cfg(feature = "esp32")]
use crate::esp32::{
    esp_idf_svc::{
        ota::{EspFirmwareInfoLoader, EspOta},
        sys::EspError,
    },
    tcp::Esp32H2Connector,
};
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
        credentials_storage::TlsCertificate,
        exec::Executor,
        grpc_client::H2Timer,
    },
    proto::app::v1::ServiceConfig,
};

#[cfg(not(feature = "esp32"))]
use crate::native::tcp::NativeH2Connector;
use bincode::Decode;
use futures_lite::AsyncWriteExt;
use http_body_util::{BodyExt, Empty};
use hyper::{body::Bytes, client::conn::http2, Request};
use thiserror::Error;

// TODO(RSDK-9200): set according to active partition scheme
const OTA_MAX_IMAGE_SIZE: usize = 1024 * 1024 * 4; // 4MB
pub const OTA_MODEL_TYPE: &str = "ota_service";
pub const OTA_MODEL_TRIPLET: &str = "rdk:builtin:ota_service";

/// https://github.com/espressif/esp-idf/blob/ce6085349f8d5a95fc857e28e2d73d73dd3629b5/components/esp_app_format/include/esp_app_desc.h#L42
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
///     uint16_t min_efuse_blk_rev_full; /*!< Minimal eFuse block revision supported by image, in format: major * 100 + minor */
///     uint16_t max_efuse_blk_rev_full; /*!< Maximal eFuse block revision supported by image, in format: major * 100 + minor */
///     uint32_t reserv2[19];       /*!< reserv2 */
/// } esp_app_desc_t;
///
/// should be 256 bytes in size
#[repr(C)]
#[derive(Decode, Debug, PartialEq)]
struct EspAppDesc {
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
    /// minimal eFuse block revision supported by image, in format: major * 100 + minor
    min_efuse_blk_rev_full: u16,
    /// maximal eFuse block revision supported by image, in format: major * 100 + minor
    max_efuse_blk_rev_full: u16,
    reserv2: [u32; 19],
}

// TODO(RSDK-9214)
#[derive(Error, Debug)]
pub enum OtaError {
    #[error("{0}")]
    ConfigError(String),
    #[cfg(feature = "esp32")]
    #[error(transparent)]
    EspError(#[from] EspError),
    #[error("failed to initialize ota process")]
    InitError,
    #[error("new image size is invalid: {0} bytes")]
    InvalidImageSize(usize),
    #[error("{0}")]
    Other(String),
}

pub(crate) struct OtaService {
    exec: Executor,
    #[cfg(feature = "esp32")]
    connector: Esp32H2Connector,
    #[cfg(not(feature = "esp32"))]
    connector: NativeH2Connector,
    url: String,
}

impl OtaService {
    pub(crate) fn from_config(
        ota_config: &ServiceConfig,
        cert: TlsCertificate,
        exec: Executor,
    ) -> Result<Self, OtaError> {
        // TODO(RSDK-9205)
        let kind: Kind = ota_config
            .attributes
            .as_ref()
            .ok_or_else(|| OtaError::ConfigError("config missing `attributes`".to_string()))?
            .fields
            .get("url")
            .ok_or(OtaError::ConfigError(
                "config missing `url` field".to_string(),
            ))?
            .kind
            .as_ref()
            .ok_or(OtaError::ConfigError(
                "failed to get inner `Value`".to_string(),
            ))?
            .try_into()
            .map_err(|e: AttributeError| OtaError::ConfigError(e.to_string()))?;

        let url = match kind {
            Kind::StringValue(s) => s,
            _ => {
                return Err(OtaError::ConfigError(format!(
                    "invalid url value: {:?}",
                    kind
                )))
            }
        };

        #[cfg(feature = "esp32")]
        let mut connector = Esp32H2Connector::default();
        #[cfg(not(feature = "esp32"))]
        let mut connector = NativeH2Connector::default();

        connector.set_server_certificates(cert.certificate.clone(), cert.private_key.clone());

        Ok(Self {
            url,
            connector,
            exec,
        })
    }

    pub(crate) async fn update(&mut self) -> Result<(), OtaError> {
        let mut uri = self
            .url
            .parse::<hyper::Uri>()
            .map_err(|_| OtaError::ConfigError(format!("invalid url: {}", self.url)))?;
        if uri.port().is_none() {
            if uri.scheme_str() != Some("https") {
                log::error!("no port found and not https");
            }

            let mut auth = uri.authority().expect("str").to_string();
            auth.push_str(":443");
            let mut parts = uri.into_parts();
            parts.authority = Some(auth.parse().unwrap());
            uri = hyper::Uri::from_parts(parts).unwrap();
        };
        log::info!("URI: {:?}", uri);
        let io = self.connector.connect_to(&uri).unwrap().await.unwrap();

        let (mut sender, conn) = {
            http2::Builder::new(self.exec.clone())
                .keep_alive_interval(Some(std::time::Duration::from_secs(120)))
                .keep_alive_timeout(std::time::Duration::from_secs(10))
                .timer(H2Timer)
                .handshake(io)
                .await
                .unwrap()
        };

        let conn = self.exec.spawn(async move {
            if let Err(err) = conn.await {
                log::error!("connection failed: {:?}", err);
            }
        });
        let request = Request::builder()
            .method("GET")
            .uri(uri)
            .body(Empty::<Bytes>::new())
            .expect("rsterntieos");
        let mut response = sender.send_request(request).await.unwrap();

        if response.status() != 200 {
            return Err(OtaError::Other(
                format!("Bad Request - Status:{}", response.status()).to_string(),
            ));
        }
        let headers = response.headers();
        log::debug!("headers: {:?}", headers);

        if !headers.contains_key(hyper::header::CONTENT_LENGTH) {
            log::error!("rsetnrien");
            return Ok(());
        }
        let file_len = headers[hyper::header::CONTENT_LENGTH]
            .to_str()
            .expect("failed to get reference to content length")
            .parse::<usize>()
            .expect("failed to parse content length");

        if file_len > OTA_MAX_IMAGE_SIZE {
            return Err(OtaError::InvalidImageSize(file_len));
        }

        #[cfg(feature = "esp32")]
        let (mut ota, running_fw_info) = {
            let mut ota = EspOta::new().map_err(OtaError::EspError)?;
            let fw_info = ota.get_running_slot().map_err(OtaError::EspError)?.firmware;
            (ota, fw_info)
        };
        #[cfg(feature = "esp32")]
        let mut update_handle = ota.initiate_update().map_err(|_| OtaError::InitError)?;
        #[cfg(not(feature = "esp32"))]
        let mut update_handle = Vec::new();
        let mut nwritten: usize = 0;
        let mut total_downloaded: usize = 0;
        let mut got_info = false;

        while let Some(next) = response.frame().await {
            let frame = next.unwrap();
            //log::info!("frame!");
            let data = frame.into_data().unwrap();
            total_downloaded += data.len();

            //log::info!("frame!");
            if !got_info {
                if total_downloaded < core::mem::size_of::<EspAppDesc>() {
                    log::error!("initial frame too small to retrieve esp_app_desc_t");
                } else {
                    log::info!("data length {}", data.len());
                    #[cfg(feature = "esp32")]
                    {
                        let mut loader = EspFirmwareInfoLoader::new();
                        loader.load(&data).map_err(OtaError::EspError)?;
                        let new_fw = loader.get_info().map_err(OtaError::EspError)?;
                        log::info!("current firmware: {:?}", running_fw_info);
                        log::info!("new firmware: {:?}", new_fw);
                        if let Some(ref running_fw) = running_fw_info {
                            if running_fw.version == new_fw.version
                                && running_fw.released == new_fw.released
                            {
                                log::info!("current firmware is up to date");
                                update_handle.abort()?;
                                return Ok(());
                            }
                        }
                    }
                    #[cfg(not(feature = "esp32"))]
                    {
                        let decoded: EspAppDesc = bincode::decode_from_slice(&data[..256], bincode::config::standard().with_big_endian()).unwrap().0;
                        log::info!("{:?}", decoded);
                    }
                    got_info = true;
                }
            }

            if data.len() + nwritten <= OTA_MAX_IMAGE_SIZE {
                #[cfg(feature = "esp32")]
                let n = update_handle.write(&data).unwrap();
                #[cfg(not(feature = "esp32"))]
                let n = update_handle.write(&data).await.unwrap();
                nwritten += n;
                // flush buffer to ota partition, clearing it
                // write frame to buffer
            } else {
                log::error!("file is larger than expected, aborting");
                #[cfg(feature = "esp32")]
                update_handle.abort()?;
            }
        }

        log::info!("download complete");

        if nwritten != file_len {
            log::error!("wrote {} bytes, expected to write {}", nwritten, file_len);
            log::error!("aborting ota");
            #[cfg(feature = "esp32")]
            update_handle.abort()?;

            return Err(OtaError::Other("download was weird".to_string()));
        }

        drop(conn);

        #[cfg(feature = "esp32")]
        {
            update_handle.complete().map_err(OtaError::EspError)?;
            log::info!("resetting now to boot from new firmware");
            esp_idf_svc::hal::reset::restart();
            unreachable!();
        }
        log::info!("ota complete");
        Ok(())
    }
}
