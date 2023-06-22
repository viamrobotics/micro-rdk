#[allow(dead_code)]
#[cfg(not(feature = "qemu"))]
const SSID: &str = env!("MICRO_RDK_WIFI_SSID");
#[allow(dead_code)]
#[cfg(not(feature = "qemu"))]
const PASS: &str = env!("MICRO_RDK_WIFI_PASSWORD");

include!(concat!(env!("OUT_DIR"), "/robot_secret.rs"));

use log::*;
#[allow(unused_imports)]
use std::{
    collections::HashMap,
    net::Ipv4Addr,
    time::Duration,
    sync::{Arc, Mutex},
};

// micro-rdk
#[allow(unused_imports)]
use micro_rdk::common::{
    app_client::AppClientConfig,
    robot::{LocalRobot, ResourceMap, ResourceType},
    board::FakeBoard,
};
#[allow(unused_imports)]
use micro_rdk::proto::common::v1::ResourceName;

#[allow(unused_imports)]
use micro_rdk::esp32::{
    certificate::WebRtcCertificate, dtls::Esp32Dtls, entry::serve_web, exec::Esp32Executor,
    tcp::Esp32Stream, tls::Esp32Tls, tls::Esp32TlsServerConfig,
};

/* webhook
use embedded_svc::{
    http::{client::Client as HttpClient, Method, Status},
    io::{Read, Write},
    utils::io,
};
*/

// esp_idf_svc

#[allow(unused_imports)]
use esp_idf_svc::{
    eventloop::EspSystemEventLoop,
    eth::{EspEth, EthDriver},
    http::client::{Configuration, EspHttpConnection},
    netif::{EspNetif, NetifStatus, AsyncNetif}, // EspNetifWait
    timer::EspTimerService,
                     //wifi::WifiWait,
};
use esp_idf_hal::prelude::Peripherals;

#[cfg(not(feature = "qemu"))]
use {
    esp_idf_svc::{
        wifi::{AsyncWifi, EspWifi},
    },
    esp_idf_sys::esp_wifi_set_ps,
    esp_idf_sys as _,
};

#[cfg(feature = "qemu")]
use embedded_svc::ipv4::{ClientSettings, ClientConfiguration, Subnet, Mask};


fn main() -> anyhow::Result<()> {
    esp_idf_sys::link_patches();

    esp_idf_svc::log::EspLogger::initialize_default();
    let sys_loop_stack = EspSystemEventLoop::take().unwrap();
    let periph = Peripherals::take().unwrap();

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
    let (ip, _blocking_eth) = {
        info!("creating eth object");

        let ip =  Ipv4Addr::new(10, 1, 12, 187);
        let ip_configuration = embedded_svc::ipv4::Configuration::Client(ClientConfiguration::Fixed(ClientSettings{
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
        let timer = EspTimerService::new()?;
        // check eth
        let (ip, eth) = eth_configure(
    &sys_loop_stack, 
    netif,
        )?;
        (ip, Box::new(eth))
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
        "".to_string(),
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
) -> anyhow::Result<(Ipv4Addr, AsyncNetif<EspNetif>)> {
    use futures_lite::future::block_on;

    let timer = EspTimerService::new()?;
    let ip_info = eth.get_ip_info()?;
    let async_eth = AsyncNetif::wrap(eth, sl_stack.clone(), timer);

    block_on( async {
            //eth.wifi_wait(|| async_wifi.wifi().is_started(), Some(Duration::from_secs(20)))
            // .await
            //.expect("couldn't start wifi");
            //async_wifi.connect().await.unwrap();

        async_eth.ip_wait_while(
                || Ok(async_eth.is_up().unwrap()), 
                Some(Duration::from_secs(20)))
            .await
            .expect("ethernet couldn't connect");

    });

    /*
    if !EthWait::new(eth.driver(), sl_stack)?
        .wait_with_timeout(Duration::from_secs(30), || eth.is_started().unwrap())
    {
        anyhow::bail!("couldn't start eth driver")
    }

    if !EspNetifWait::new::<EspNetif>(eth.netif(), sl_stack)?
        .wait_with_timeout(Duration::from_secs(20), || {
            eth.netif().get_ip_info().unwrap().ip != Ipv4Addr::new(0, 0, 0, 0)
        })
    {
        anyhow::bail!("didn't get an ip")
    }
        */
    info!("ETH IP {:?}", ip_info.ip);
    Ok((ip_info.ip, async_eth))
}

#[cfg(not(feature = "qemu"))]
fn start_wifi(
    modem: impl esp_idf_hal::peripheral::Peripheral<P = esp_idf_hal::modem::Modem> + 'static,
    sl_stack: EspSystemEventLoop,
) -> anyhow::Result<AsyncWifi<EspWifi<'static>>> {
    use futures_lite::future::block_on;
    use esp_idf_svc::wifi::config::{ScanConfig,ScanType};
    use embedded_svc::wifi::{AuthMethod, ClientConfiguration, Configuration};

    let timer = EspTimerService::new()?;
    let nvs = esp_idf_svc::nvs::EspDefaultNvsPartition::take()?;
    let mut wifi = AsyncWifi::wrap(EspWifi::new(modem, sl_stack.clone(), Some(nvs))?, sl_stack, timer)?;
    let wifi_configuration: Configuration = Configuration::Client(ClientConfiguration {
        ssid: SSID.into(),
        bssid: None,
        auth_method: AuthMethod::WPA2Personal,
        password: PASS.into(),
        channel: None,
    });

    wifi.set_configuration(&wifi_configuration)?;

    block_on( async {
    wifi.start().await.unwrap();
    info!("Wifi started");

    wifi.connect().await.unwrap();
    info!("Wifi connected");

    wifi.wait_netif_up().await.unwrap();
    info!("Wifi netif up");

    });

    /*
    info!("mattjperez - W");
    let ap = wifi.start_scan(&scan_conf, false)?;
    info!("mattjperez - Z");
    let ap = &wifi.get_scan_result_n::<1>()?.0[0];
    info!("{} channel is {}", "Viam", ap.channel);
    let channel = Some(ap.channel);
    info!("mattjperez - X");
    let client_config = embedded_svc::wifi::ClientConfiguration {
        ssid: SSID.into(),
        password: PASS.into(),
        channel,
        ..Default::default()
    };
    wifi.set_configuration(&embedded_svc::wifi::Configuration::Client(client_config))?;

    wifi.start()?;

    info!("mattjperez - A");
    let mut async_wifi = AsyncWifi::wrap(wifi, sl_stack.clone(), timer)?;
    info!("mattjperez - B");
*/

    /*
    block_on(async {
        async_wifi
            .wifi_wait(|| async_wifi.wifi().is_started(), Some(Duration::from_secs(20)))
            .await
            .expect("couldn't start wifi");
            async_wifi.connect().await.unwrap();
    info!("mattjperez - C");

        async_wifi
            .ip_wait_while(
                || Ok(async_wifi.wifi().is_connected().unwrap() && async_wifi.wifi().sta_netif().get_ip_info().unwrap().ip != Ipv4Addr::new(0,0,0,0)), 
                Some(Duration::from_secs(20)))
            .await
            .expect("wifi couldn't connect");
            async_wifi.connect().await.unwrap();
    info!("mattjperez - D");
    });

    info!("mattjperez - E");
    let ip_info = async_wifi.wifi().sta_netif().get_ip_info()?;

    info!("Wifi DHCP info: {:?}", ip_info);

    esp_idf_sys::esp!(unsafe { esp_wifi_set_ps(esp_idf_sys::wifi_ps_type_t_WIFI_PS_NONE) })?;

    */
    Ok(wifi)
}

