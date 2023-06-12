#[allow(dead_code)]
#[cfg(not(feature = "qemu"))]
const SSID: &str = env!("MICRO_RDK_WIFI_SSID");
#[allow(dead_code)]
#[cfg(not(feature = "qemu"))]
const PASS: &str = env!("MICRO_RDK_WIFI_PASSWORD");

include!(concat!(env!("OUT_DIR"), "/robot_secret.rs"));

#[allow(dead_code)]
#[cfg(feature = "qemu")]
use esp_idf_svc::eth::*;
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
use micro_rdk::common::robot::{LocalRobot, ResourceMap, ResourceType};
use micro_rdk::common::webrtc::grpc::{WebRtcGrpcBody, WebRtcGrpcServer};
use micro_rdk::esp32::certificate::WebRtcCertificate;
use micro_rdk::esp32::dtls::Esp32Dtls;
use micro_rdk::esp32::exec::Esp32Executor;
use micro_rdk::proto::common::v1::ResourceName;
use std::collections::HashMap;
use std::net::Ipv4Addr;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::time::Duration;

#[cfg(not(feature = "qemu"))]
use esp_idf_svc::wifi::EspWifi;

use esp_idf_hal::prelude::Peripherals;

fn main() -> anyhow::Result<()> {
    esp_idf_sys::link_patches();

    esp_idf_svc::log::EspLogger::initialize_default();
    let sys_loop_stack = EspSystemEventLoop::take().unwrap();
    let periph = Peripherals::take().unwrap();

    #[cfg(feature = "qemu")]
    let robot = {
        use micro_rdk::common::board::FakeBoard;
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
        Arc::new(Mutex::new(LocalRobot::new(res)))
    };
    #[cfg(not(feature = "qemu"))]
    let robot = {
        use esp_idf_hal::gpio::OutputPin;
        use esp_idf_hal::gpio::PinDriver;
        use esp_idf_hal::ledc;
        use esp_idf_hal::ledc::config::TimerConfig;
        use esp_idf_hal::units::FromValueType;
        use micro_rdk::esp32::board::EspBoard;
        use micro_rdk::esp32::motor::ABMotorEsp32;
        let tconf = TimerConfig::default().frequency(10.kHz().into());
        let timer = Arc::new(ledc::LedcTimerDriver::new(periph.ledc.timer0, &tconf).unwrap());
        let max_rpm = 100.0;
        let chan =
            ledc::LedcDriver::new(periph.ledc.channel0, timer.clone(), periph.pins.gpio14).unwrap();
        let m1 = ABMotorEsp32::new(
            PinDriver::output(periph.pins.gpio33).unwrap(),
            PinDriver::output(periph.pins.gpio32).unwrap(),
            chan,
            max_rpm,
        );
        let motor = Arc::new(Mutex::new(m1));
        let pins = vec![PinDriver::output(periph.pins.gpio15.downgrade_output()).unwrap()];
        let b = EspBoard::new(pins, vec![], HashMap::new());
        let mut res: ResourceMap = HashMap::with_capacity(2);
        res.insert(
            ResourceName {
                namespace: "rdk".to_string(),
                r#type: "component".to_string(),
                subtype: "motor".to_string(),
                name: "m1".to_string(),
            },
            ResourceType::Motor(motor),
        );
        res.insert(
            ResourceName {
                namespace: "rdk".to_string(),
                r#type: "component".to_string(),
                subtype: "board".to_string(),
                name: "b".to_string(),
            },
            ResourceType::Board(Arc::new(Mutex::new(b))),
        );
        Arc::new(Mutex::new(LocalRobot::new(res)))
    };

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

    let cfg = AppClientConfig::new(ROBOT_SECRET.to_owned(), ROBOT_ID.to_owned(), ip);
    run_server(robot, cfg);
    Ok(())
}

fn run_server(robot: Arc<Mutex<LocalRobot>>, cfg: AppClientConfig) {
    use micro_rdk::common::grpc_client::GrpcClient;
    use micro_rdk::esp32::tcp::Esp32Stream;
    use micro_rdk::esp32::tls::Esp32Tls;
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
