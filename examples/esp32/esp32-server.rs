#[allow(dead_code)]
#[cfg(not(feature = "qemu"))]
const SSID: &str = env!("MICRO_RDK_WIFI_SSID");
#[allow(dead_code)]
#[cfg(not(feature = "qemu"))]
const PASS: &str = env!("MICRO_RDK_WIFI_PASSWORD");

include!(concat!(env!("OUT_DIR"), "/robot_secret.rs"));

use log::*;
#[cfg(feature = "qemu")]
use micro_rdk::esp_idf_svc::eth::EspEth;
use micro_rdk::esp_idf_svc::eventloop::EspSystemEventLoop;
use micro_rdk::esp_idf_svc::sys::{g_wifi_feature_caps, CONFIG_FEATURE_CACHE_TX_BUF_BIT};
use micro_rdk::{
    common::{app_client::AppClientConfig, entry::RobotRepresentation},
    esp32::{certificate::WebRtcCertificate, entry::serve_web, tls::Esp32TlsServerConfig},
};
#[cfg(feature = "qemu")]
use std::net::Ipv4Addr;

extern "C" {
    pub static g_spiram_ok: bool;
}

use micro_rdk::common::registry::ComponentRegistry;

#[cfg(not(feature = "qemu"))]
use {
    embedded_svc::wifi::{
        AuthMethod, ClientConfiguration as WifiClientConfiguration,
        Configuration as WifiConfiguration,
    },
    micro_rdk::esp_idf_svc::hal::{peripheral::Peripheral, prelude::Peripherals},
    micro_rdk::esp_idf_svc::sys::esp_wifi_set_ps,
    micro_rdk::esp_idf_svc::wifi::{BlockingWifi, EspWifi},
};

fn main() {
    micro_rdk::esp_idf_svc::sys::link_patches();

    micro_rdk::esp_idf_svc::log::EspLogger::initialize_default();
    let sys_loop_stack = EspSystemEventLoop::take().unwrap();

    #[cfg(not(feature = "qemu"))]
    let periph = Peripherals::take().unwrap();

    let repr = RobotRepresentation::WithRegistry(Box::<ComponentRegistry>::default());

    {
        micro_rdk::esp_idf_svc::sys::esp!(unsafe {
            micro_rdk::esp_idf_svc::sys::esp_vfs_eventfd_register(
                &micro_rdk::esp_idf_svc::sys::esp_vfs_eventfd_config_t { max_fds: 5 },
            )
        })
        .unwrap();
    }

    #[cfg(feature = "qemu")]
    let (ip, _block_eth) = {
        use micro_rdk::esp_idf_svc::hal::prelude::Peripherals;
        info!("creating eth object");
        let eth = micro_rdk::esp_idf_svc::eth::EspEth::wrap(
            micro_rdk::esp_idf_svc::eth::EthDriver::new_openeth(
                Peripherals::take().unwrap().mac,
                sys_loop_stack.clone(),
            )
            .unwrap(),
        )
        .unwrap();
        let (_, eth) = eth_configure(&sys_loop_stack, eth).unwrap();
        let ip = Ipv4Addr::new(10, 1, 12, 187);
        (ip, eth)
    };

    unsafe {
        if !g_spiram_ok {
            log::info!("spiram not initialized disabling cache feature of the wifi driver");
            g_wifi_feature_caps &= !(CONFIG_FEATURE_CACHE_TX_BUF_BIT as u64);
        }
    }
    #[allow(clippy::redundant_clone)]
    #[cfg(not(feature = "qemu"))]
    let (ip, _wifi) = {
        let wifi = start_wifi(periph.modem, sys_loop_stack).unwrap();
        (wifi.wifi().sta_netif().get_ip_info().unwrap().ip, wifi)
    };

    let cfg = AppClientConfig::new(
        ROBOT_SECRET.to_owned(),
        ROBOT_ID.to_owned(),
        ip,
        "".to_owned(),
    );
    let webrtc_certificate = WebRtcCertificate::new(
        ROBOT_DTLS_CERT.to_vec(),
        ROBOT_DTLS_KEY_PAIR.to_vec(),
        ROBOT_DTLS_CERT_FP,
    );

    let tls_cfg = {
        let cert = [ROBOT_SRV_PEM_CHAIN.to_vec(), ROBOT_SRV_PEM_CA.to_vec()];
        let key = ROBOT_SRV_DER_KEY;
        Esp32TlsServerConfig::new(cert, key.as_ptr(), key.len() as u32)
    };

    serve_web(cfg, tls_cfg, repr, ip, webrtc_certificate);
}

#[cfg(feature = "qemu")]
use micro_rdk::esp_idf_svc::eth::BlockingEth;
#[cfg(feature = "qemu")]
fn eth_configure<'d, T>(
    sl_stack: &EspSystemEventLoop,
    eth: micro_rdk::esp_idf_svc::eth::EspEth<'d, T>,
) -> anyhow::Result<(Ipv4Addr, Box<BlockingEth<EspEth<'d, T>>>)> {
    let mut eth = micro_rdk::esp_idf_svc::eth::BlockingEth::wrap(eth, sl_stack.clone())?;
    eth.start()?;
    eth.wait_netif_up()?;

    let ip_info = eth.eth().netif().get_ip_info()?;

    info!("ETH IP {:?}", ip_info.ip);
    Ok((ip_info.ip, Box::new(eth)))
}

#[cfg(not(feature = "qemu"))]
fn start_wifi(
    modem: impl Peripheral<P = micro_rdk::esp_idf_svc::hal::modem::Modem> + 'static,
    sl_stack: EspSystemEventLoop,
) -> anyhow::Result<Box<BlockingWifi<EspWifi<'static>>>> {
    let nvs = micro_rdk::esp_idf_svc::nvs::EspDefaultNvsPartition::take()?;
    let mut wifi = BlockingWifi::wrap(EspWifi::new(modem, sl_stack.clone(), Some(nvs))?, sl_stack)?;
    let wifi_configuration = WifiConfiguration::Client(WifiClientConfiguration {
        ssid: SSID.into(),
        bssid: None,
        auth_method: AuthMethod::WPA2Personal,
        password: PASS.into(),
        channel: None,
    });

    wifi.set_configuration(&wifi_configuration)?;

    wifi.start().unwrap();
    info!("Wifi started");

    wifi.connect().unwrap();
    info!("Wifi connected");

    wifi.wait_netif_up().unwrap();
    info!("Wifi netif up");

    micro_rdk::esp_idf_svc::sys::esp!(unsafe {
        esp_wifi_set_ps(micro_rdk::esp_idf_svc::sys::wifi_ps_type_t_WIFI_PS_NONE)
    })?;
    Ok(Box::new(wifi))
}
