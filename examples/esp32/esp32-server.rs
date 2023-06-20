#[allow(dead_code)]
#[cfg(not(feature = "qemu"))]
const SSID: &str = env!("MICRO_RDK_WIFI_SSID");
#[allow(dead_code)]
#[cfg(not(feature = "qemu"))]
const PASS: &str = env!("MICRO_RDK_WIFI_PASSWORD");

include!(concat!(env!("OUT_DIR"), "/robot_secret.rs"));

#[cfg(not(feature = "qemu"))]
use {
    esp_idf_svc::wifi::{BlockingWifi, EspWifi},
    esp_idf_sys as _,
    esp_idf_sys::esp_wifi_set_ps,
};

// webhook
use embedded_svc::{
    http::{client::Client as HttpClient, Method, Status},
    io::{Read, Write},
    utils::io,
    wifi::{ClientConfiguration, Wifi},
};

use esp_idf_svc::{
    eth::EspEth,
    http::client::{Configuration, EspHttpConnection},
    netif::EspNetif, // EspNetifWait
    //wifi::WifiWait,
};

use esp_idf_svc::eventloop::EspSystemEventLoop;

use anyhow::bail;
use log::*;
use micro_rdk::common::app_client::AppClientConfig;
use micro_rdk::esp32::certificate::WebRtcCertificate;
use std::net::Ipv4Addr;
use std::time::Duration;
//use futures_lite::future::block_on;

#[cfg(not(feature = "qemu"))]
use esp_idf_svc::wifi::EspWifi;

// webhook
use embedded_svc::{
    http::{client::Client as HttpClient, Method, Status},
    io::{Read, Write},
    utils::io,
};
#[derive(Deserialize, Debug)]
struct Response {
    response: String,
}

use esp_idf_svc::http::client::{Configuration, EspHttpConnection};

use esp_idf_hal::prelude::Peripherals;
use micro_rdk::esp32::entry::serve_web;
use micro_rdk::esp32::tls::Esp32TlsServerConfig;

fn main() -> anyhow::Result<()> {
    esp_idf_sys::link_patches();

    esp_idf_svc::log::EspLogger::initialize_default();
    let sys_loop_stack = EspSystemEventLoop::take().unwrap();
    let periph = esp_idf_hal::prelude::Peripherals::take().unwrap();

    #[cfg(feature = "qemu")]
    let robot = {
        use micro_rdk::common::board::FakeBoard;
        use micro_rdk::common::robot::{LocalRobot, ResourceMap, ResourceType};
        use micro_rdk::proto::common::v1::ResourceName;
        use std::collections::HashMap;
        use std::sync::{Arc, Mutex};
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
    let (ip, _eth) = {
        info!("creating eth object");
        let eth = eth_configure(
            &sys_loop_stack,
            Box::new(esp_idf_svc::eth::EspEth::wrap(EthDriver::new_openeth(
                periph.mac,
                sys_loop_stack.clone(),
            )?)?),
        )?;
        (Ipv4Addr::new(10, 1, 12, 187), eth)
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
        (wifi.sta_netif().get_ip_info()?.ip, wifi)
    };

    use micro_rdk::common::app_client::AppClientConfig;
    let cfg = AppClientConfig::new(
        ROBOT_SECRET.to_owned(),
        ROBOT_ID.to_owned(),
        ip,
        "".to_string()
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
    mut eth: Box<EspEth<'static>>,
) -> anyhow::Result<Box<EspEth<'static>>> {
    eth.start()?;

    if !EthWait::new(eth.driver(), sl_stack)?
        .wait_with_timeout(Duration::from_secs(30), || eth.is_started().unwrap())
    {
        bail!("couldn't start eth driver")
    }

    if !EspNetifWait::new::<EspNetif>(eth.netif(), sl_stack)?
        .wait_with_timeout(Duration::from_secs(20), || {
            eth.netif().get_ip_info().unwrap().ip != Ipv4Addr::new(0, 0, 0, 0)
        })
    {
        bail!("didn't get an ip")
    }
    let ip_info = eth.netif().get_ip_info()?;
    info!("ETH IP {:?}", ip_info);
    Ok(eth)
}

#[cfg(not(feature = "qemu"))]
fn start_wifi(
    modem: impl esp_idf_hal::peripheral::Peripheral<P = esp_idf_hal::modem::Modem> + 'static,
    sl_stack: EspSystemEventLoop,
) -> anyhow::Result<Box<EspWifi<'static>>> {
    let mut wifi = Box::new(EspWifi::new(modem, sl_stack.clone(), None)?);

    info!("scanning");
    let aps = wifi.scan()?;
    let foundap = aps.into_iter().find(|x| x.ssid == SSID);

    let channel = if let Some(foundap) = foundap {
        info!("{} channel is {}", "Viam", foundap.channel);
        Some(foundap.channel)
    } else {
        None
    };
    let client_config = ClientConfiguration {
        ssid: SSID.into(),
        password: PASS.into(),
        channel,
        ..Default::default()
    };
    wifi.set_configuration(&embedded_svc::wifi::Configuration::Client(client_config))?;

    wifi.start()?;

    /*   // refactor to use updated api
    if !WifiWait::new(&sl_stack)?
        .wait_with_timeout(Duration::from_secs(20), || wifi.is_started().unwrap())
    {
        bail!("couldn't start wifi")
    }
        */

    wifi.connect()?;

    /*   // refactor to use updated api
    if !EspNetifWait::new::<EspNetif>(wifi.sta_netif(), &sl_stack)?.wait_with_timeout( 
        Duration::from_secs(20),
        || {
            wifi.is_connected().unwrap()
                && wifi.sta_netif().get_ip_info().unwrap().ip != Ipv4Addr::new(0, 0, 0, 0)
        },
    ) {
        bail!("wifi couldn't connect")
    }
        */

    let ip_info = wifi.sta_netif().get_ip_info()?;

    info!("Wifi DHCP info: {:?}", ip_info);

    esp_idf_sys::esp!(unsafe { esp_wifi_set_ps(esp_idf_sys::wifi_ps_type_t_WIFI_PS_NONE) })?;

    Ok(wifi)
}
#[derive(Deserialize, Debug)]
struct Response {
    response: String,
}

/// Send a HTTP GET request.
fn get_request(client: &mut HttpClient<EspHttpConnection>) -> anyhow::Result<()> {
    // Prepare headers and URL
    //let content_length_header = format!("{}", payload.len());

    let payload = json!({
        /*
        "location": "<ROBOT LOCATION>",
        "secret": "<ROBOT SECRET>",
        "target": "<COMPONENT BOARD NAME>",
        "pin": pin-no
        */
        "delete": "this"
    })
    .to_string();
    let payload = payload.as_bytes();
    // Prepare headers and URL
    let content_length_header = format!("{}", payload.len());
    let headers = [
        ("accept", "text/plain"),
        ("content-type", "application/json"),
        ("connection", "close"),
        ("content-length", &*content_length_header),
    ];
    let url = "https://restless-shape-1762.fly.dev/esp";

    // Send request
    //let mut request = client.get(&url, &headers)?;
    let mut request = client.request(Method::Get, &url, &headers)?;
    request.write_all(payload)?;
    request.flush()?;
    info!("-> GET {}", url);
    let mut response = request.submit()?;

    // Process response
    let status = response.status();
    info!("<- {}", status);
    let (_headers, mut body) = response.split();
    let mut buf = [0u8; 4096];
    let bytes_read = io::try_read_full(&mut body, &mut buf).map_err(|e| e.0)?;
    info!("Read {} bytes", bytes_read);
    let response: Response = serde_json::from_slice(&buf[0..bytes_read])?;
    info!("Response body: {:?} bytes", response);

    // Drain the remaining response bytes
    while body.read(&mut buf)? > 0 {}

    //let bytes_read = io::try_read_full(&mut body, &mut buf).map_err(|e| e.0)?;
    //info!("Read {} bytes", bytes_read);

    // Drain the remaining response bytes
    //while body.read(&mut buf)? > 0 {}

    Ok(())
}
