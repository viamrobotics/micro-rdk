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
    common::registry::ComponentRegistry,
    esp32::{certificate::WebRtcCertificate, entry::serve_web, tls::Esp32TlsServerConfig},
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
};
use micro_rdk::common::registry::Dependency;
use micro_rdk::common::config::ConfigType;
use micro_rdk::common::motor::MotorType;
use micro_rdk::common::motor::Motor;

use std::{sync::{Arc, Mutex}, time::Duration};

struct MyMotor {
    pos: f64,
    power: f64,
    max_rpm: f64,
}


impl MyMotor {
    fn new() -> Self {
        Self {
        pos: 10.0,
        power: 0.0,
        max_rpm: 100.0

        }
    }

    fn from_config(cfg: ConfigType, _: Vec<Dependency>) -> anyhow::Result<MotorType> {
        let mut motor = MyMotor::new();
        if let Ok(pos) = cfg.get_attribute::<f64>("my_position") {
            motor.pos = pos;
        }
        if let Ok(max_rpm) = cfg.get_attribute::<f64>("my_max_rpm") {
            motor.max_rpm = max_rpm;
        }
        Ok(Arc::new(Mutex::new(motor)))
    }
}

impl Motor for MyMotor {
    fn set_power(&mut self, _pct: f64) -> anyhow::Result<()> {
        Ok(())
    }
    fn get_position(&mut self) -> anyhow::Result<i32> {
        Ok(0)
    }
    fn go_for(&mut self, _rpm: f64, _revolutions: f64) -> anyhow::Result<Option<Duration>> {
        Ok(None)
    }
}

impl micro_rdk::common::status::Status for MyMotor {
    fn get_status(&self) -> anyhow::Result<Option<prost_types::Struct>> {
        Ok(None)
    }

}

impl micro_rdk::common::stop::Stoppable for MyMotor {
    fn stop(&mut self) -> anyhow::Result<()> {
        Ok(())
    }

}

fn register_my_model<'model: 'dep, 'ctor, 'dep>(
    registry: &mut ComponentRegistry<'model, 'ctor, 'dep>,
) {
    if registry
        .register_motor("my_gpio", &MyMotor::from_config)
        .is_err()
    {
        log::error!("gpio model is already registered")
    }
}

fn main() -> anyhow::Result<()> {
    esp_idf_sys::link_patches();

    esp_idf_svc::log::EspLogger::initialize_default();
    let sys_loop_stack = EspSystemEventLoop::take().unwrap();

    #[cfg(not(feature = "qemu"))]
    let periph = Peripherals::take().unwrap();

    {
        esp_idf_sys::esp!(unsafe {
            esp_idf_sys::esp_vfs_eventfd_register(&esp_idf_sys::esp_vfs_eventfd_config_t {
                max_fds: 5,
            })
        })?;
    }

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

    let mut registry = ComponentRegistry::default();
    register_my_model(&mut registry);

    serve_web(cfg, tls_cfg, registry, ip, webrtc_certificate);
    Ok(())
}

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
