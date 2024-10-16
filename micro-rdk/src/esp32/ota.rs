use crate::{
    esp32::esp_idf_svc::{
        http::client::{Configuration, EspHttpConnection},
        ota::{EspFirmwareInfoLoader, EspOta, FirmwareInfo},
        sys::{EspError, ESP_ERR_IMAGE_INVALID},
    },
    proto::app::v1::ServiceConfig,
};
/// The following are sdkconfig options relevant to OTA
/// They reflect what is currently default as of esp-idf v4.4.3 and should be reviewed when upgrading to esp-idf v5
///
/// - CONFIG_BOOTLOADER_FACTORY_RESET=NO
///   - clear data partitions and boot from factory partition
/// - CONFIG_BOOTLOADER_APP_ROLLBACK_ENABLE=NO
///   - after updating the app, bootloader runs a new app with the "ESP_OTA_IMG_PENDING_VERIFY" state set. If the image is not marked as verified
use core::mem::size_of;
use embedded_svc::http::{client::Client, Headers, Method};

/*
ServiceConfig {
        name: "OTA",
        namespace: "rdk",
        r#type: "generic",
        attributes: Some(
            Struct {
                fields: {
                    "url": Value {
                        kind: Some(
                            StringValue(
                            "https://my.bucket.com/my-ota.bin",
                            ),
                        ),
                    },
                },
            },
        ),
        depends_on: [],
        model: "rdk:builtin:ota_service",
        api: "rdk:service:generic",
        service_configs: [],
        log_configuration: None,
}
 */

const OTA_CHUNK_SIZE: usize = 1024 * 20; // 20KB
const OTA_MAX_SIZE: usize = 1024 * 1024 * 4; // 4MB
const OTA_MIN_SIZE: usize = size_of::<FirmwareInfo>() + 1024;

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

    // TODO
    pub(crate) fn needs_update(&self) -> bool {
        // check config for version/metadata
        // get metadata from NVS storage
        // compare hash or firmware data
        // check hash/version/metadata compare to active ota hash/version/metadata
        true
    }

    fn get_firmware_info(&self, buff: &[u8]) -> Result<FirmwareInfo, EspError> {
        let mut loader = EspFirmwareInfoLoader::new();
        loader.load(buff)?;
        loader.get_info()
    }
    pub(crate) fn update(&mut self) -> Result<(), String> {
        let connection = EspHttpConnection::new(&Configuration {
            buffer_size: Some(1024 * 4),
            buffer_size_tx: Some(1024 * 4),
            use_global_ca_store: true,
            crt_bundle_attach: Some(esp_idf_svc::sys::esp_crt_bundle_attach),
            ..Default::default()
        })
        .map_err(|e| e.to_string())?;

        let mut client = Client::wrap(connection);

        let headers = [("accept", "application/octet_stream")];
        let request = client
            .request(Method::Get, &self.url, &headers)
            .map_err(|e| e.to_string())?;

        let mut response = request.submit().map_err(|e| e.to_string())?;
        let status = response.status();

        if status < 200 || status > 299 {
            return Err(format!("Bad Request - Status:{}", status).to_string());
        }

        let file_len = response.content_len().unwrap_or(0) as usize;

        if file_len <= OTA_MIN_SIZE {
            log::info!(
                "File size is {file_len}, too small to be a firmware! No need to proceed further."
            );
            return Err(ESP_ERR_IMAGE_INVALID.to_string());
        }
        if file_len > OTA_MAX_SIZE {
            log::info!("File is too big ({file_len} bytes).");
            return Err(ESP_ERR_IMAGE_INVALID.to_string());
        }
        // get handle to inactive slot
        // start ota process on inactive slot, get write buffer
        // loop through request to url with ota buffer as target
        //   check for FirmwareInfo mid-loop
        //   break when num_read > file_size
        // check firmware info and hash
        let mut ota = EspOta::new().expect("failed to initialize EspOta");
        let mut work = ota
            .initiate_update()
            .expect("failed to initiate ota update");
        let mut buff = vec![0; OTA_CHUNK_SIZE];
        let mut total_read_len: usize = 0;
        let mut got_info = false;
        let dl_result = loop {
            let n = response.read(&mut buff).unwrap_or_default();
            total_read_len += n;
            if !got_info {
                match self.get_firmware_info(&buff[..n]) {
                    Ok(info) => log::info!("Firmware to be downloaded: {info:?}"),
                    Err(e) => {
                        log::error!("Failed to get firmware info");
                        break Err(e);
                    }
                };
                got_info = true;
            }
            if n > 0 {
                if let Err(e) = work.write(&buff[..n]) {
                    log::error!("Failed to write to OTA. {e}");
                    break Err(e);
                }
            }
            if total_read_len >= file_len {
                break Ok(());
            }
        };
        if dl_result.is_err() {
            let _ = work.abort();
            return Err("download error, aborting ota".to_string());
        }
        if total_read_len < file_len {
            log::error!("{total_read_len} bytes downloaded, needed {file_len} bytes");
            let _ = work.abort();
            return Err("download incomplete, aborting ota update".to_string());
        }
        // flip ota bit
        work.complete().expect("failed to complete ota");
        log::info!("will boot new firmware on reset");
        Ok(())
    }
}
