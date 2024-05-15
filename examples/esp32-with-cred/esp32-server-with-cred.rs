#[cfg(target_os = "espidf")]
mod esp32 {
    use log::*;
    use thiserror::Error;

    use micro_rdk::esp32::esp_idf_svc::eventloop::EspSystemEventLoop;
    use micro_rdk::esp32::esp_idf_svc::nvs::{EspDefaultNvs, EspDefaultNvsPartition, EspNvs};
    use micro_rdk::esp32::esp_idf_svc::sys::EspError;
    use micro_rdk::esp32::esp_idf_svc::sys::{
        g_wifi_feature_caps, CONFIG_FEATURE_CACHE_TX_BUF_BIT,
    };
    use micro_rdk::{
        common::{app_client::AppClientConfig, entry::RobotRepresentation},
        esp32::{certificate::WebRtcCertificate, entry::serve_web, tls::Esp32TLSServerConfig},
    };

    extern "C" {
        pub static g_spiram_ok: bool;
    }

    #[cfg(feature = "qemu")]
    use {
        micro_rdk::{
            common::{
                board::FakeBoard,
                robot::{LocalRobot, ResourceMap, ResourceType},
            },
            esp32::conn::network::eth_configure,
            proto::common::v1::ResourceName,
        },
        std::{
            collections::HashMap,
            net::Ipv4Addr,
            sync::{Arc, Mutex},
        },
    };

    #[cfg(not(feature = "qemu"))]
    use {
        micro_rdk::common::registry::ComponentRegistry,
        micro_rdk::esp32::conn::network::Esp32WifiNetwork,
    };

    #[derive(Debug, Error)]
    pub enum ServerError {
        #[error("Error fetching NVS key: {0}")]
        NVSKeyError(String),
        #[error("{0}")]
        EspError(EspError),
    }

    impl From<EspError> for ServerError {
        fn from(value: EspError) -> ServerError {
            ServerError::EspError(value)
        }
    }

    const VIAM_NVS_NAMESPACE: &str = "VIAM_NS";

    fn get_str_from_nvs(viam_nvs: &EspDefaultNvs, key: &str) -> Result<String, ServerError> {
        let mut buffer_ref = [0_u8; 4000];
        Ok(viam_nvs
            .get_str(key, &mut buffer_ref)?
            .ok_or(ServerError::NVSKeyError(key.to_string()))?
            .trim_matches(char::from(0))
            .to_string())
    }

    fn get_blob_from_nvs(viam_nvs: &EspDefaultNvs, key: &str) -> Result<Vec<u8>, ServerError> {
        let mut buffer_ref = [0_u8; 4000];
        Ok(viam_nvs
            .get_blob(key, &mut buffer_ref)?
            .ok_or(ServerError::NVSKeyError(key.to_string()))?
            .to_vec())
    }

    struct NvsStaticVars {
        #[cfg(not(feature = "qemu"))]
        wifi_ssid: String,
        #[cfg(not(feature = "qemu"))]
        wifi_pwd: String,
        robot_secret: String,
        robot_id: String,
        robot_dtls_cert: Vec<u8>,
        robot_dtls_key_pair: Vec<u8>,
        robot_dtls_cert_fp: String,
        robot_srv_pem_chain: Vec<u8>,
        robot_srv_der_key: Vec<u8>,
    }

    impl NvsStaticVars {
        fn new() -> Result<NvsStaticVars, ServerError> {
            let nvs = EspDefaultNvsPartition::take()?;
            info!("get namespace...");
            let viam_nvs = EspNvs::new(nvs.clone(), VIAM_NVS_NAMESPACE, true)?;
            info!("loading creds...");
            Ok(NvsStaticVars {
                #[cfg(not(feature = "qemu"))]
                wifi_ssid: get_str_from_nvs(&viam_nvs, "WIFI_SSID")?,
                #[cfg(not(feature = "qemu"))]
                wifi_pwd: get_str_from_nvs(&viam_nvs, "WIFI_PASSWORD")?,
                robot_secret: get_str_from_nvs(&viam_nvs, "ROBOT_SECRET")?,
                robot_id: get_str_from_nvs(&viam_nvs, "ROBOT_ID")?,
                robot_dtls_cert: get_blob_from_nvs(&viam_nvs, "ROBOT_DTLS_CERT")?,
                robot_dtls_key_pair: get_blob_from_nvs(&viam_nvs, "DTLS_KEY_PAIR")?,
                robot_dtls_cert_fp: get_str_from_nvs(&viam_nvs, "DTLS_CERT_FP")?,
                robot_srv_pem_chain: get_blob_from_nvs(&viam_nvs, "SRV_PEM_CHAIN")?,
                robot_srv_der_key: get_blob_from_nvs(&viam_nvs, "SRV_DER_KEY")?,
            })
        }
    }

    pub(crate) fn main_esp32() {
        micro_rdk::esp32::esp_idf_svc::sys::link_patches();

        micro_rdk::esp32::esp_idf_svc::log::EspLogger::initialize_default();
        let sys_loop_stack = EspSystemEventLoop::take().unwrap();

        #[cfg(feature = "qemu")]
        let repr = {
            let board = Arc::new(Mutex::new(FakeBoard::new(vec![])));
            let mut res: ResourceMap = HashMap::with_capacity(1);
            res.insert(
                ResourceName {
                    namespace: "rdk".to_string(),
                    r#type: "component".to_string(),
                    subtype: "board".to_string(),
                    name: "b".to_string(),
                },
                ResourceType::Board(board),
            );
            RobotRepresentation::WithRobot(LocalRobot::new(res))
        };
        #[cfg(not(feature = "qemu"))]
        let repr = RobotRepresentation::WithRegistry(Box::new(ComponentRegistry::default()));

        micro_rdk::esp32::esp_idf_svc::sys::esp!(unsafe {
            micro_rdk::esp32::esp_idf_svc::sys::esp_vfs_eventfd_register(
                &micro_rdk::esp32::esp_idf_svc::sys::esp_vfs_eventfd_config_t { max_fds: 5 },
            )
        })
        .unwrap();

        info!("load vars from NVS...");
        let nvs_vars = NvsStaticVars::new().unwrap();

        #[cfg(feature = "qemu")]
        let network = {
            use micro_rdk::esp32::esp_idf_svc::hal::prelude::Peripherals;
            info!("creating eth object");
            let mut eth = Box::new(
                micro_rdk::esp32::esp_idf_svc::eth::EspEth::wrap(
                    micro_rdk::esp32::esp_idf_svc::eth::EthDriver::new_openeth(
                        Peripherals::take()
                            .ok_or(ServerError::PeripheralsError)
                            .unwrap()
                            .mac,
                        sys_loop_stack.clone(),
                    )
                    .unwrap(),
                )
                .unwrap(),
            );
            eth_configure(&sys_loop_stack, &mut eth).unwrap()
        };

        let mut max_connection = 3;
        unsafe {
            if !g_spiram_ok {
                log::info!("spiram not initialized disabling cache feature of the wifi driver");
                g_wifi_feature_caps &= !(CONFIG_FEATURE_CACHE_TX_BUF_BIT as u64);
                max_connection = 1;
            }
        }

        info!("starting wifi...");
        #[allow(clippy::redundant_clone)]
        #[cfg(not(feature = "qemu"))]
        let network = {
            let mut wifi = Esp32WifiNetwork::new(
                sys_loop_stack.clone(),
                nvs_vars.wifi_ssid.clone(),
                nvs_vars.wifi_pwd.clone(),
            )
            .expect("could not configure wifi");
            loop {
                match wifi.connect() {
                    Ok(()) => break,
                    Err(_) => {
                        log::info!(
                            "wifi could not connect to SSID: {:?}, retrying...",
                            nvs_vars.wifi_ssid
                        );
                        std::thread::sleep(std::time::Duration::from_millis(300));
                    }
                }
            }
            wifi
        };

        let webrtc_certificate = WebRtcCertificate::new(
            nvs_vars.robot_dtls_cert,
            nvs_vars.robot_dtls_key_pair,
            &nvs_vars.robot_dtls_cert_fp,
        );

        let cert: Vec<u8> = nvs_vars.robot_srv_pem_chain;
        let key = nvs_vars.robot_srv_der_key;
        let tls_cfg = Esp32TLSServerConfig::new(cert, key.as_ptr(), key.len() as u32);

        let cfg = AppClientConfig::new(nvs_vars.robot_secret, nvs_vars.robot_id, "".to_owned());

        serve_web(
            cfg,
            tls_cfg,
            repr,
            webrtc_certificate,
            max_connection,
            network,
        );
    }
}
fn main() {
    #[cfg(target_os = "espidf")]
    esp32::main_esp32();
}
