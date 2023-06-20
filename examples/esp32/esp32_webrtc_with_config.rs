#[allow(dead_code)]
#[cfg(not(feature = "qemu"))]
const SSID: &str = env!("MICRO_RDK_WIFI_SSID");
#[allow(dead_code)]
#[cfg(not(feature = "qemu"))]
const PASS: &str = env!("MICRO_RDK_WIFI_PASSWORD");

include!(concat!(env!("OUT_DIR"), "/robot_secret.rs"));

#[cfg(feature = "qemu")]
use esp_idf_svc::eth::{EspEth, EthWait};
use esp_idf_svc::eventloop::EspSystemEventLoop;
use esp_idf_svc::netif::{EspNetif, EspNetifWait};
use esp_idf_sys as _;

use anyhow::bail;
#[cfg(not(feature = "qemu"))]
use esp_idf_sys::esp_wifi_set_ps;
use futures_lite::future::block_on;
use log::*;
use micro_rdk::common::app_client::{AppClientBuilder, AppClientConfig};
use micro_rdk::common::grpc::GrpcServer;
use micro_rdk::common::grpc_client::GrpcClient;
use micro_rdk::common::robot::LocalRobot;
use micro_rdk::common::webrtc::grpc::{WebRtcGrpcBody, WebRtcGrpcServer};
use micro_rdk::esp32::certificate::WebRtcCertificate;
use micro_rdk::esp32::dtls::Esp32Dtls;
use micro_rdk::esp32::exec::Esp32Executor;
use micro_rdk::esp32::tcp::Esp32Stream;
use micro_rdk::esp32::tls::Esp32Tls;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::net::Ipv4Addr;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::time::Duration;

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

fn main() -> anyhow::Result<()> {
    esp_idf_sys::link_patches();

    esp_idf_svc::log::EspLogger::initialize_default();
    let sys_loop_stack = EspSystemEventLoop::take().unwrap();
    let periph = Peripherals::take().unwrap();

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

    // webhook
    let mut client = HttpClient::wrap(EspHttpConnection::new(&Configuration {
        use_global_ca_store: true,
        crt_bundle_attach: Some(esp_idf_sys::esp_crt_bundle_attach),
        ..Default::default()
    })?);
    get_request(&mut client).unwrap();

    let cfg = AppClientConfig::new(ROBOT_SECRET.to_owned(), ROBOT_ID.to_owned(), ip);
    let executor = Esp32Executor::new();

    let mut tls = Box::new(Esp32Tls::new_client());
    let conn = tls.open_ssl_context(None).unwrap();
    let conn = Esp32Stream::TLSStream(Box::new(conn));

    let grpc_client = GrpcClient::new(conn, executor.clone(), "https://app.viam.com:443").unwrap();

    let mut app_client = AppClientBuilder::new(grpc_client, cfg);
    let jwt_token = app_client.get_jwt_token()?;
    let conf_resp = app_client.read_config(&jwt_token)?;

    let robot = LocalRobot::new_from_config_response(conf_resp)?;
    let robot = Arc::new(Mutex::new(robot));
    drop(app_client);
    let cfg = AppClientConfig::new(ROBOT_SECRET.to_owned(), ROBOT_ID.to_owned(), ip);
    run_server(robot, cfg);
    Ok(())
}

fn run_server(robot: Arc<Mutex<LocalRobot>>, cfg: AppClientConfig) {
    log::info!("Starting WebRtc ");
    let executor = Esp32Executor::new();
    let mut webrtc = {
        let mut tls = Box::new(Esp32Tls::new_client());
        let conn = tls.open_ssl_context(None).unwrap();
        let conn = Esp32Stream::TLSStream(Box::new(conn));

        let grpc_client =
            GrpcClient::new(conn, executor.clone(), "https://app.viam.com:443").unwrap();
        let mut app_client = AppClientBuilder::new(grpc_client, cfg).build().unwrap();

        let webrtc_certificate = Rc::new(WebRtcCertificate::new(
            ROBOT_DTLS_CERT,
            ROBOT_DTLS_KEY_PAIR,
            ROBOT_DTLS_CERT_FP,
        ));

        let dtls = Esp32Dtls::new(webrtc_certificate.clone()).unwrap();

        let webrtc = app_client
            .connect_webrtc(webrtc_certificate, executor.clone(), dtls)
            .unwrap();

        drop(app_client);
        webrtc
    };
    let channel = block_on(executor.run(async { webrtc.open_data_channel().await })).unwrap();
    log::info!("channel opened {:?}", channel);

    let mut webrtc_grpc =
        WebRtcGrpcServer::new(channel, GrpcServer::new(robot, WebRtcGrpcBody::default()));

    loop {
        block_on(executor.run(async { webrtc_grpc.next_request().await })).unwrap();
    }
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
    use embedded_svc::wifi::{ClientConfiguration, Wifi};
    use esp_idf_svc::wifi::WifiWait;

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

    if !WifiWait::new(&sl_stack)?
        .wait_with_timeout(Duration::from_secs(20), || wifi.is_started().unwrap())
    {
        bail!("couldn't start wifi")
    }

    wifi.connect()?;

    if !EspNetifWait::new::<EspNetif>(wifi.sta_netif(), &sl_stack)?.wait_with_timeout(
        Duration::from_secs(20),
        || {
            wifi.is_connected().unwrap()
                && wifi.sta_netif().get_ip_info().unwrap().ip != Ipv4Addr::new(0, 0, 0, 0)
        },
    ) {
        bail!("wifi couldn't connect")
    }

    let ip_info = wifi.sta_netif().get_ip_info()?;

    info!("Wifi DHCP info: {:?}", ip_info);

    esp_idf_sys::esp!(unsafe { esp_wifi_set_ps(esp_idf_sys::wifi_ps_type_t_WIFI_PS_NONE) })?;

    Ok(wifi)
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
