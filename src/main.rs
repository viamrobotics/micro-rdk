mod base;
mod board;
#[cfg(feature = "camera")]
mod camera;
mod exec;
mod grpc;
mod motor;
mod pin;
mod proto;
mod robot;
mod robot_client;
mod status;
mod tcp;
mod tls;

#[allow(dead_code)]
#[cfg(not(feature = "qemu"))]
const SSID: &str = env!("MINI_RDK_WIFI_SSID");
#[allow(dead_code)]
#[cfg(not(feature = "qemu"))]
const PASS: &str = env!("MINI_RDK_WIFI_PASSWORD");

// Generated robot config during build process
include!(concat!(env!("OUT_DIR"), "/robot_secret.rs"));

#[cfg(not(feature = "qemu"))]
use crate::base::Esp32WheelBase;
#[cfg(feature = "qemu")]
use crate::base::FakeBase;
#[cfg(not(feature = "qemu"))]
use crate::board::EspBoard;
#[cfg(feature = "qemu")]
use crate::board::FakeBoard;
#[cfg(all(not(feature = "qemu"), feature = "camera"))]
use crate::camera::Esp32Camera;
#[cfg(all(feature = "qemu", feature = "camera"))]
use crate::camera::FakeCamera;
#[cfg(feature = "qemu")]
use crate::motor::FakeMotor;
#[cfg(not(feature = "qemu"))]
use crate::motor::MotorEsp32;
use crate::robot::ResourceType;
use anyhow::bail;
#[cfg(not(feature = "qemu"))]
use esp_idf_hal::gpio::PinDriver;
#[cfg(not(feature = "qemu"))]
use esp_idf_hal::ledc::config::TimerConfig;
use esp_idf_hal::prelude::Peripherals;
use esp_idf_hal::task::notify;
#[cfg(not(feature = "qemu"))]
use esp_idf_hal::units::FromValueType;
#[cfg(not(feature = "qemu"))]
use esp_idf_hal::{ledc, peripheral};
#[cfg(feature = "qemu")]
use esp_idf_svc::eth::*;
#[cfg(feature = "qemu")]
use esp_idf_svc::eth::{EspEth, EthWait};
use esp_idf_svc::eventloop::EspSystemEventLoop;
use esp_idf_svc::netif::{EspNetif, EspNetifWait};
#[cfg(not(feature = "qemu"))]
use esp_idf_svc::wifi::EspWifi;
#[cfg(not(feature = "qemu"))]
use esp_idf_sys::esp_wifi_set_ps;
use esp_idf_sys::{self as _, TaskHandle_t}; // If using the `binstart` feature of `esp-idf-sys`, always keep this module imported
use exec::Esp32Executor;
use futures_lite::future::block_on;
use grpc::GrpcServer;
use hyper::server::conn::Http;
use log::*;
use proto::common::v1::ResourceName;
use robot::Esp32Robot;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;
use tcp::Esp32Listener;
use tls::Esp32Tls;

fn main() -> anyhow::Result<()> {
    esp_idf_sys::link_patches();

    esp_idf_svc::log::EspLogger::initialize_default();
    let sys_loop_stack = EspSystemEventLoop::take().unwrap();
    let periph = Peripherals::take().unwrap();

    #[cfg(not(feature = "qemu"))]
    let robot = {
        #[cfg(feature = "camera")]
        let camera = {
            Esp32Camera::new();
            camera.setup()?;
            Arc::new(Mutex::new(camera))
        };
        // // let mut encoder = Esp32Encoder::new(
        // //     periph.pins.gpio15.into_input()?.degrade(),
        // //     periph.pins.gpio14.into_input()?.degrade(),
        // // );
        // // encoder.setup_pcnt()?;
        // // encoder.start()?;
        let tconf = TimerConfig::default().frequency(10.kHz().into());
        let timer = Arc::new(ledc::LedcTimerDriver::new(periph.ledc.timer0, &tconf).unwrap());
        let chan = ledc::LedcDriver::new(
            periph.ledc.channel0,
            timer.clone(),
            periph.pins.gpio14,
            &tconf,
        )?;
        let m1 = MotorEsp32::new(
            PinDriver::output(periph.pins.gpio33)?,
            PinDriver::output(periph.pins.gpio32)?,
            chan,
        );
        let chan2 = ledc::LedcDriver::new(
            periph.ledc.channel2,
            timer.clone(),
            periph.pins.gpio2,
            &tconf,
        )?;
        let m2 = MotorEsp32::new(
            PinDriver::output(periph.pins.gpio13)?,
            PinDriver::output(periph.pins.gpio12)?,
            chan2,
        );

        let pins = vec![PinDriver::output(periph.pins.gpio15)?];
        let b = EspBoard::new(pins);
        let motor = Arc::new(Mutex::new(m1));
        let m2 = Arc::new(Mutex::new(m2));
        let board = Arc::new(Mutex::new(b));
        let base = Arc::new(Mutex::new(Esp32WheelBase::new(motor.clone(), m2.clone())));

        let mut res: robot::ResourceMap = HashMap::with_capacity(5);
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
                subtype: "motor".to_string(),
                name: "m2".to_string(),
            },
            ResourceType::Motor(m2),
        );
        res.insert(
            ResourceName {
                namespace: "rdk".to_string(),
                r#type: "component".to_string(),
                subtype: "board".to_string(),
                name: "b".to_string(),
            },
            ResourceType::Board(board),
        );
        res.insert(
            ResourceName {
                namespace: "rdk".to_string(),
                r#type: "component".to_string(),
                subtype: "base".to_string(),
                name: "base".to_string(),
            },
            ResourceType::Base(base),
        );
        #[cfg(feature = "camera")]
        res.insert(
            ResourceName {
                namespace: "rdk".to_string(),
                r#type: "component".to_string(),
                subtype: "camera".to_string(),
                name: "c".to_string(),
            },
            ResourceType::Camera(camera),
        );
        Esp32Robot::new(res)
    };

    #[cfg(feature = "qemu")]
    let robot = {
        let motor = Arc::new(Mutex::new(FakeMotor::new()));
        let base = Arc::new(Mutex::new(FakeBase::new()));
        let board = Arc::new(Mutex::new(FakeBoard::new()));
        #[cfg(feature = "camera")]
        let camera = Arc::new(Mutex::new(FakeCamera::new()));
        let mut res: robot::ResourceMap = HashMap::with_capacity(1);
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
            ResourceType::Board(board),
        );
        res.insert(
            ResourceName {
                namespace: "rdk".to_string(),
                r#type: "component".to_string(),
                subtype: "base".to_string(),
                name: "base".to_string(),
            },
            ResourceType::Base(base),
        );
        #[cfg(feature = "camera")]
        res.insert(
            ResourceName {
                namespace: "rdk".to_string(),
                r#type: "component".to_string(),
                subtype: "camera".to_string(),
                name: "c".to_string(),
            },
            ResourceType::Camera(camera),
        );
        Esp32Robot::new(res)
    };

    #[cfg(feature = "qemu")]
    let (ip, _eth) = {
        use std::net::Ipv4Addr;
        info!("creating eth object");
        let eth = eth_configure(
            &sys_loop_stack,
            Box::new(esp_idf_svc::eth::EspEth::wrap(EthDriver::new_openeth(
                periph.mac,
                sys_loop_stack.clone(),
            )?)?),
        )?;
        (Ipv4Addr::new(0, 0, 0, 0), eth)
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

    let hnd = match robot_client::start(ip) {
        Err(e) => {
            log::error!("couldn't start robot client {:?} will start the server", e);
            None
        }
        Ok(hnd) => Some(hnd),
    };
    // start mdns service
    {
        unsafe {
            match esp_idf_sys::mdns_init() {
                esp_idf_sys::ESP_OK => {}
                err => log::error!("couldn't start mdns service: '{}'", err),
            };
            match esp_idf_sys::mdns_hostname_set(LOCAL_FQDN.as_ptr() as *const i8) {
                esp_idf_sys::ESP_OK => {}
                err => log::error!("couldn't sey mdns hostname: '{}'", err),
            };
        }
    }
    if let Err(e) = runserver(robot, hnd) {
        log::error!("robot server failed with error {:?}", e);
        return Err(e);
    }
    Ok(())
}

fn runserver(robot: Esp32Robot, client_handle: Option<TaskHandle_t>) -> anyhow::Result<()> {
    let tls = Box::new(Esp32Tls::new_server());
    let address: SocketAddr = "0.0.0.0:80".parse().unwrap();
    let mut listener = Esp32Listener::new(address.into(), Some(tls))?;
    let exec = Esp32Executor::new();
    let srv = GrpcServer::new(Arc::new(Mutex::new(robot)));
    loop {
        let stream = listener.accept()?;
        if let Some(hnd) = client_handle {
            if unsafe { notify(hnd, 1) } {
                log::info!("successfully notified client task")
            }
        }
        block_on(exec.run(async {
            let err = Http::new()
                .with_executor(exec.clone())
                .http2_max_concurrent_streams(1)
                .serve_connection(stream, srv.clone())
                .await;
            if err.is_err() {
                log::error!("server error {}", err.err().unwrap());
            }
        }));
    }
}
#[cfg(feature = "qemu")]
fn eth_configure(
    sl_stack: &EspSystemEventLoop,
    mut eth: Box<EspEth<'static>>,
) -> anyhow::Result<Box<EspEth<'static>>> {
    use std::net::Ipv4Addr;

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
    modem: impl peripheral::Peripheral<P = esp_idf_hal::modem::Modem> + 'static,
    sl_stack: EspSystemEventLoop,
) -> anyhow::Result<Box<EspWifi<'static>>> {
    use embedded_svc::wifi::{ClientConfiguration, Wifi};
    use esp_idf_svc::wifi::WifiWait;
    use std::net::Ipv4Addr;

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
    wifi.set_configuration(&embedded_svc::wifi::Configuration::Client(client_config))?; //&Configuration::Client(client_config)

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
