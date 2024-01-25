use log::*;
use thiserror::Error;

use micro_rdk::esp_idf_svc::eventloop::EspSystemEventLoop;
use micro_rdk::esp_idf_svc::nvs::{EspDefaultNvs, EspDefaultNvsPartition, EspNvs};
use micro_rdk::esp_idf_svc::sys::EspError;
use micro_rdk::esp_idf_svc::sys::{g_wifi_feature_caps, CONFIG_FEATURE_CACHE_TX_BUF_BIT};
use micro_rdk::{
    common::{app_client::AppClientConfig, entry::RobotRepresentation},
    esp32::{certificate::WebRtcCertificate, entry::serve_web, tls::Esp32TlsServerConfig},
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
    embedded_svc::wifi::{
        AuthMethod, ClientConfiguration as WifiClientConfiguration,
        Configuration as WifiConfiguration,
    },
    micro_rdk::common::registry::ComponentRegistry,
    micro_rdk::esp_idf_svc::hal::{peripheral::Peripheral, prelude::Peripherals},
    micro_rdk::esp_idf_svc::sys::esp_wifi_set_ps,
    micro_rdk::esp_idf_svc::wifi::{BlockingWifi, EspWifi},
};

#[derive(Debug, Error)]
pub enum ServerError {
    #[error("Error fetching NVS key: {0}")]
    NVSKeyError(String),
    #[error("{0}")]
    EspError(EspError),
    #[error("Error obtaining peripherals")]
    PeripheralsError,
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
    robot_srv_pem_ca: Vec<u8>,
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
            robot_srv_pem_ca: get_blob_from_nvs(&viam_nvs, "CA_CRT")?,
            robot_srv_der_key: get_blob_from_nvs(&viam_nvs, "SRV_DER_KEY")?,
        })
    }
}

fn main() {
    micro_rdk::esp_idf_svc::sys::link_patches();

    micro_rdk::esp_idf_svc::log::EspLogger::initialize_default();
    let sys_loop_stack = EspSystemEventLoop::take().unwrap();

    #[cfg(not(feature = "qemu"))]
    let periph = Peripherals::take()
        .map_err(|_| ServerError::PeripheralsError)
        .unwrap();

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

    micro_rdk::esp_idf_svc::sys::esp!(unsafe {
        micro_rdk::esp_idf_svc::sys::esp_vfs_eventfd_register(
            &micro_rdk::esp_idf_svc::sys::esp_vfs_eventfd_config_t { max_fds: 5 },
        )
    })
    .unwrap();

    info!("load vars from NVS...");
    let nvs_vars = NvsStaticVars::new().unwrap();

    #[cfg(feature = "qemu")]
    let (ip, _block_eth) = {
        use micro_rdk::esp_idf_svc::hal::prelude::Peripherals;
        info!("creating eth object");
        let mut eth = Box::new(
            micro_rdk::esp_idf_svc::eth::EspEth::wrap(
                micro_rdk::esp_idf_svc::eth::EthDriver::new_openeth(
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
        let _ = eth_configure(&sys_loop_stack, &mut eth).unwrap();
        let ip = Ipv4Addr::new(10, 1, 12, 187);
        (ip, eth)
    };

    unsafe {
        if !g_spiram_ok {
            log::info!("spiram not initialized disabling cache feature of the wifi driver");
            g_wifi_feature_caps &= !(CONFIG_FEATURE_CACHE_TX_BUF_BIT as u64);
        }
    }

    info!("starting wifi...");
    #[allow(clippy::redundant_clone)]
    #[cfg(not(feature = "qemu"))]
    let (ip, _wifi) = {
        let wifi = start_wifi(
            periph.modem,
            sys_loop_stack,
            &nvs_vars.wifi_ssid,
            &nvs_vars.wifi_pwd,
        )
        .unwrap();
        (wifi.wifi().sta_netif().get_ip_info().unwrap().ip, wifi)
    };

    let webrtc_certificate = WebRtcCertificate::new(
        nvs_vars.robot_dtls_cert,
        nvs_vars.robot_dtls_key_pair,
        &nvs_vars.robot_dtls_cert_fp,
    );

    let cert: [Vec<u8>; 2] = [nvs_vars.robot_srv_pem_chain, nvs_vars.robot_srv_pem_ca];
    let key = nvs_vars.robot_srv_der_key;
    let tls_cfg = Esp32TlsServerConfig::new(cert, key.as_ptr(), key.len() as u32);

    let cfg = AppClientConfig::new(nvs_vars.robot_secret, nvs_vars.robot_id, ip, "".to_owned());

    serve_web(cfg, tls_cfg, repr, ip, webrtc_certificate);
}

#[cfg(feature = "qemu")]
fn eth_configure<'d, T>(
    sl_stack: &EspSystemEventLoop,
    eth: &mut micro_rdk::esp_idf_svc::eth::EspEth<'d, T>,
) -> Result<Ipv4Addr, ServerError> {
    let mut eth = micro_rdk::esp_idf_svc::eth::BlockingEth::wrap(eth, sl_stack.clone())?;
    eth.start()?;
    let ip_info = eth.eth().netif().get_ip_info()?;

    info!("ETH IP {:?}", ip_info.ip);
    Ok(ip_info.ip)
}

#[cfg(not(feature = "qemu"))]
fn start_wifi(
    modem: impl Peripheral<P = micro_rdk::esp_idf_svc::hal::modem::Modem> + 'static,
    sl_stack: EspSystemEventLoop,
    ssid: &str,
    password: &str,
) -> Result<Box<BlockingWifi<EspWifi<'static>>>, ServerError> {
    let nvs = EspDefaultNvsPartition::take()?;
    let mut wifi = BlockingWifi::wrap(
        EspWifi::new(modem, sl_stack.clone(), Some(nvs.clone()))?,
        sl_stack,
    )?;
    let ssid_heapless = ssid.into();
    let password_heapless = password.into();
    let wifi_configuration = WifiConfiguration::Client(WifiClientConfiguration {
        ssid: ssid_heapless,
        bssid: None,
        auth_method: AuthMethod::WPA2Personal,
        password: password_heapless,
        channel: None,
    });
    debug!("setting wifi configuration...");
    wifi.set_configuration(&wifi_configuration)?;

    wifi.start()?;
    info!("Wifi started");

    wifi.connect()?;
    info!("Wifi connected");

    wifi.wait_netif_up()?;
    info!("Wifi netif up");

    micro_rdk::esp_idf_svc::sys::esp!(unsafe {
        esp_wifi_set_ps(micro_rdk::esp_idf_svc::sys::wifi_ps_type_t_WIFI_PS_NONE)
    })?;
    Ok(Box::new(wifi))
}
