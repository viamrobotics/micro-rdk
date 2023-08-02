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
    common::{app_client::AppClientConfig, entry::RobotRepresentation},
    esp32::{certificate::WebRtcCertificate, entry::serve_web, tls::Esp32TlsServerConfig},
};

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
    esp_idf_hal::{peripheral::Peripheral, prelude::Peripherals},
    esp_idf_svc::wifi::{BlockingWifi, EspWifi},
    esp_idf_sys as _,
    esp_idf_sys::esp_wifi_set_ps,
    micro_rdk::common::registry::ComponentRegistry,
};

fn main() -> anyhow::Result<()> {
    esp_idf_sys::link_patches();

    esp_idf_svc::log::EspLogger::initialize_default();
    let sys_loop_stack = EspSystemEventLoop::take().unwrap();

    #[cfg(not(feature = "qemu"))]
    let periph = Peripherals::take().unwrap();

    #[cfg(feature = "qemu")]
    let srv_config = {
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
    let srv_cfg = RobotRepresentation::WithRegistry(ComponentRegistry::default());

    {
        esp_idf_sys::esp!(unsafe {
            esp_idf_sys::esp_vfs_eventfd_register(&esp_idf_sys::esp_vfs_eventfd_config_t {
                max_fds: 5,
            })
        })?;
    }

    #[cfg(feature = "qemu")]
    let (ip, _block_eth) = {
        use esp_idf_hal::prelude::Peripherals;
        info!("creating eth object");
        let mut eth = Box::new(esp_idf_svc::eth::EspEth::wrap(
            esp_idf_svc::eth::EthDriver::new_openeth(
                Peripherals::take().unwrap().mac,
                sys_loop_stack.clone(),
            )
            .unwrap(),
        )?);
        let _ = eth_configure(&sys_loop_stack, &mut eth)?;
        let ip = Ipv4Addr::new(10, 1, 12, 187);
        (ip, eth)
    };

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
        let cert = &[ROBOT_SRV_PEM_CHAIN, ROBOT_SRV_PEM_CA];
        let key = ROBOT_SRV_DER_KEY;
        Esp32TlsServerConfig::new(cert, key.as_ptr(), key.len() as u32)
    };

    serve_web(cfg, tls_cfg, srv_cfg, ip, webrtc_certificate);
    Ok(())
}

#[cfg(feature = "qemu")]
fn eth_configure<'d, T>(
    sl_stack: &EspSystemEventLoop,
    eth: &mut esp_idf_svc::eth::EspEth<'d, T>,
) -> anyhow::Result<Ipv4Addr> {
    let mut eth = esp_idf_svc::eth::BlockingEth::wrap(eth, sl_stack.clone())?;
    eth.start()?;
    let ip_info = eth.eth().netif().get_ip_info()?;

    info!("ETH IP {:?}", ip_info.ip);
    Ok(ip_info.ip)
}

#[cfg(not(feature = "qemu"))]
fn start_wifi(
    modem: impl Peripheral<P = esp_idf_hal::modem::Modem> + 'static,
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
