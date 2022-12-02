mod base;
mod board;
mod camera;
mod exec;
mod grpc;
mod motor;
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

#[cfg(not(feature = "qemu"))]
use crate::base::Esp32WheelBase;
#[cfg(feature = "qemu")]
use crate::base::FakeBase;
#[cfg(not(feature = "qemu"))]
use crate::board::EspBoard;
#[cfg(feature = "qemu")]
use crate::board::FakeBoard;
#[cfg(not(feature = "qemu"))]
use crate::camera::Esp32Camera;
#[cfg(feature = "qemu")]
use crate::camera::FakeCamera;
#[cfg(feature = "qemu")]
use crate::motor::FakeMotor;
#[cfg(not(feature = "qemu"))]
use crate::motor::MotorEsp32;
use crate::robot::ResourceType;
#[cfg(feature = "qemu")]
use embedded_svc::eth;
#[cfg(feature = "qemu")]
use embedded_svc::eth::{Eth, TransitionalState};
#[cfg(not(feature = "qemu"))]
use esp_idf_hal::ledc::{config::TimerConfig, Channel, Timer};
#[cfg(not(feature = "qemu"))]
use esp_idf_hal::prelude::Peripherals;
#[cfg(not(feature = "qemu"))]
use esp_idf_hal::units::FromValueType;
#[cfg(feature = "qemu")]
use esp_idf_svc::eth::*;
use esp_idf_svc::netif::EspNetifStack;
#[cfg(not(feature = "qemu"))]
use esp_idf_svc::nvs::EspDefaultNvs;
use esp_idf_svc::sysloop::EspSysLoopStack;
#[cfg(not(feature = "qemu"))]
use esp_idf_svc::wifi::EspWifi;
#[cfg(not(feature = "qemu"))]
use esp_idf_sys::esp_wifi_set_ps;
use esp_idf_sys::{self as _}; // If using the `binstart` feature of `esp-idf-sys`, always keep this module imported
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
    let sys_loop_stack = Arc::new(EspSysLoopStack::new()?);
    let netif_stack = Arc::new(EspNetifStack::new()?);
    #[cfg(not(feature = "qemu"))]
    let nvs = Arc::new(EspDefaultNvs::new()?);

    #[cfg(not(feature = "qemu"))]
    let robot = {
        let camera = Esp32Camera::new();
        camera.setup()?;
        let camera = Arc::new(Mutex::new(camera));
        let periph = Peripherals::take().unwrap();
        // // let mut encoder = Esp32Encoder::new(
        // //     periph.pins.gpio15.into_input()?.degrade(),
        // //     periph.pins.gpio14.into_input()?.degrade(),
        // // );
        // // encoder.setup_pcnt()?;
        // // encoder.start()?;
        let tconf = TimerConfig::default().frequency(10.kHz().into());
        let timer = Arc::new(Timer::new(periph.ledc.timer0, &tconf).unwrap());
        let chan = Channel::new(periph.ledc.channel0, timer.clone(), periph.pins.gpio14)?;
        let m1 = MotorEsp32::new(
            periph.pins.gpio33.into_output()?.degrade(),
            periph.pins.gpio32.into_output()?.degrade(),
            chan,
        );

        let chan2 = Channel::new(periph.ledc.channel2, timer, periph.pins.gpio2)?;
        let m2 = MotorEsp32::new(
            periph.pins.gpio13.into_output()?.degrade(),
            periph.pins.gpio12.into_output()?.degrade(),
            chan2,
        );
        let pins = vec![periph.pins.gpio15.into_output().unwrap().degrade()];
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
    let _eth_hw = {
        info!("creating eth object");
        eth_configure(Box::new(EspEth::new_openeth(netif_stack, sys_loop_stack)?))?
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
    let _esp_idf_sys = { start_wifi(netif_stack, sys_loop_stack, nvs)? };

    if let Err(e) = robot_client::start() {
        log::error!("couldn't start robot client {:?} will start the server", e);
    }
    if let Err(e) = runserver(robot) {
        log::error!("robot server failed with error {:?}", e);
        return Err(e);
    }
    Ok(())
}

fn runserver(robot: Esp32Robot) -> anyhow::Result<()> {
    let tls = Box::new(Esp32Tls::new_server());
    let address: SocketAddr = "0.0.0.0:80".parse().unwrap();
    let mut listener = Esp32Listener::new(address.into(), Some(tls))?;
    let exec = Esp32Executor::new();
    let srv = GrpcServer::new(Arc::new(Mutex::new(robot)));
    loop {
        let stream = listener.accept()?;
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
fn eth_configure<HW>(mut eth: Box<EspEth<HW>>) -> anyhow::Result<Box<EspEth<HW>>> {
    info!("Eth created");
    eth.set_configuration(&eth::Configuration::Client(Default::default()))?;

    info!("Eth configuration set, about to get status");

    eth.wait_status_with_timeout(Duration::from_secs(10), |status| !status.is_transitional())
        .map_err(|e| anyhow::anyhow!("Unexpected Eth status: {:?}", e))?;

    let status = eth.get_status();

    if let eth::Status::Started(eth::ConnectionStatus::Connected(eth::IpStatus::Done(Some(
        ip_settings,
    )))) = status
    {
        info!("Eth connected IP {:?}", ip_settings);
    } else {
        anyhow::bail!("Unexpected Eth status: {:?}", status);
    }

    Ok(eth)
}
#[cfg(not(feature = "qemu"))]
fn start_wifi(
    nf_stack: Arc<EspNetifStack>,
    sl_stack: Arc<EspSysLoopStack>,
    defaul_nvs: Arc<EspDefaultNvs>,
) -> anyhow::Result<Box<EspWifi>> {
    use embedded_svc::wifi::*;

    let mut wifi = Box::new(EspWifi::new(nf_stack, sl_stack, defaul_nvs)?);

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
    wifi.set_configuration(&Configuration::Client(client_config))?;

    wifi.wait_status_with_timeout(Duration::from_secs(20), |s| !s.is_transitional())
        .map_err(|e| anyhow::anyhow!("unexpectedb statis {:?}", e))?;

    let status = wifi.get_status();

    if let Status(
        ClientStatus::Started(ClientConnectionStatus::Connected(ClientIpStatus::Done(ip))),
        ApStatus::Stopped,
    ) = status
    {
        info!("Connected to AP with ip {:?}", ip);
    } else {
        anyhow::bail!("Couldn't connect to Wifi {:?}", status);
    }

    esp_idf_sys::esp!(unsafe { esp_wifi_set_ps(esp_idf_sys::wifi_ps_type_t_WIFI_PS_NONE) })?;

    Ok(wifi)
}
