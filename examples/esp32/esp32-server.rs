#[allow(dead_code)]
#[cfg(not(feature = "qemu"))]
const SSID: &str = env!("MICRO_RDK_WIFI_SSID");
#[allow(dead_code)]
#[cfg(not(feature = "qemu"))]
const PASS: &str = env!("MICRO_RDK_WIFI_PASSWORD");

include!(concat!(env!("OUT_DIR"), "/robot_secret.rs"));

use log::*;

use esp_idf_svc::eventloop::EspSystemEventLoop;
use micro_rdk::{
    common::app_client::AppClientConfig,
    esp32::{certificate::WebRtcCertificate, entry::serve_web, tls::Esp32TlsServerConfig},
};

#[cfg(feature = "qemu")]
use {
    embedded_svc::ipv4::{ClientConfiguration, ClientSettings, Mask, Subnet},
    esp_idf_svc::netif::{BlockingNetif, EspNetif},
    micro_rdk::{
        common::{
            board::FakeBoard,
            robot::{LocalRobot, ResourceMap, ResourceType},
        },
        proto::common::v1::ResourceName,
    },
    std::{
        collections::HashMap,
        sync::{Arc, Mutex},
        time::Duration,
        net::Ipv4Addr,
    },
};

#[cfg(not(feature = "qemu"))]
use {
    embedded_svc::wifi::{
        AuthMethod, ClientConfiguration as WifiClientConfiguration,
        Configuration as WifiConfiguration,
    },
    esp_idf_svc::wifi::{BlockingWifi, EspWifi},
    esp_idf_sys as _,
    esp_idf_sys::esp_wifi_set_ps,
};

fn main() -> anyhow::Result<()> {
    esp_idf_sys::link_patches();

    esp_idf_svc::log::EspLogger::initialize_default();
    let sys_loop_stack = EspSystemEventLoop::take().unwrap();
    #[cfg(not(feature = "qemu"))]
    let periph = esp_idf_hal::prelude::Peripherals::take().unwrap();

    #[cfg(feature = "qemu")]
    let robot = {
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
        Some(LocalRobot::new(res))
    };
    #[cfg(not(feature = "qemu"))]
    let robot = None;

    #[cfg(feature = "qemu")]
    let (ip, _block_eth) = {
        info!("creating eth object");

        let ip = Ipv4Addr::new(10, 1, 12, 187);
        let ip_configuration =
            embedded_svc::ipv4::Configuration::Client(ClientConfiguration::Fixed(ClientSettings {
                ip,
                subnet: Subnet {
                    gateway: Ipv4Addr::new(10, 1, 12, 1),
                    mask: Mask(24),
                },
                ..Default::default()
            }));

        let mut eth_config = esp_idf_svc::netif::NetifConfiguration::eth_default_client();
        // netif_config.custom_mac = Some(periph.mac.into_ref()); // need to get a reference to [u8:6]
        eth_config.ip_configuration = ip_configuration;

        let netif = esp_idf_svc::netif::EspNetif::new_with_conf(&eth_config)?;
        let (ip, block_eth) = eth_configure(&sys_loop_stack, netif)?;
        (ip, block_eth)
    };

    {
        esp_idf_sys::esp!(unsafe {
            esp_idf_sys::esp_vfs_eventfd_register(&esp_idf_sys::esp_vfs_eventfd_config_t {
                max_fds: 5,
            })
        })?;
    }

    #[allow(clippy::redundant_clone)]
    #[cfg(not(feature = "qemu"))]
    let (ip, _wifi) = {
        let wifi = start_wifi(periph.modem, sys_loop_stack)?;
        (wifi.wifi().sta_netif().get_ip_info()?.ip, wifi)
    };

    let cfg = AppClientConfig::new(
        ROBOT_SECRET.to_owned(),
        ROBOT_ID.to_owned(),
        ip,
        "".to_owned(),
    );
    let webrtc_certificate =
        WebRtcCertificate::new(ROBOT_DTLS_CERT, ROBOT_DTLS_KEY_PAIR, ROBOT_DTLS_CERT_FP);

    let tls_cfg = {
        let cert = include_bytes!(concat!(env!("OUT_DIR"), "/ca.crt"));
        let key = include_bytes!(concat!(env!("OUT_DIR"), "/key.key"));
        Esp32TlsServerConfig::new(
            cert.as_ptr(),
            cert.len() as u32,
            key.as_ptr(),
            key.len() as u32,
        )
    };

    serve_web(cfg, tls_cfg, robot, ip, webrtc_certificate);
    Ok(())
}

#[cfg(feature = "qemu")]
fn eth_configure(
    sl_stack: &EspSystemEventLoop,
    eth: EspNetif,
) -> anyhow::Result<(Ipv4Addr, Box<BlockingNetif<EspNetif>>)> {
    let ip_info = eth.get_ip_info()?;
    let block_eth = BlockingNetif::wrap(eth, sl_stack.clone());

    block_eth
        .ip_wait_while(
            || Ok(block_eth.is_up().unwrap()),
            Some(Duration::from_secs(20)),
        )
        .expect("ethernet couldn't connect");

    info!("ETH IP {:?}", ip_info.ip);
    Ok((ip_info.ip, Box::new(block_eth)))
}

#[cfg(not(feature = "qemu"))]
fn start_wifi(
    modem: impl esp_idf_hal::peripheral::Peripheral<P = esp_idf_hal::modem::Modem> + 'static,
    sl_stack: EspSystemEventLoop,
) -> anyhow::Result<Box<BlockingWifi<EspWifi<'static>>>> {
    let nvs = esp_idf_svc::nvs::EspDefaultNvsPartition::take()?;
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

    esp_idf_sys::esp!(unsafe { esp_wifi_set_ps(esp_idf_sys::wifi_ps_type_t_WIFI_PS_NONE) })?;
    Ok(Box::new(wifi))
}
