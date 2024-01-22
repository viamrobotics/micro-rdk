const SSID: &str = env!("MICRO_RDK_WIFI_SSID");
const PASS: &str = env!("MICRO_RDK_WIFI_PASSWORD");

// Generated robot config during build process
include!(concat!(env!("OUT_DIR"), "/robot_secret.rs"));

use log::*;

use esp_idf_svc::eventloop::EspSystemEventLoop;
use esp_idf_sys::{g_wifi_feature_caps, CONFIG_FEATURE_CACHE_TX_BUF_BIT};
use micro_rdk::{
    common::{
        app_client::AppClientConfig,
        entry::RobotRepresentation,
        registry::{ComponentRegistry, RegistryError},
    },
    esp32::{certificate::WebRtcCertificate, entry::serve_web, tls::Esp32TlsServerConfig},
};
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

macro_rules! generate_register_modules {
    ($($module:ident),*) => {
        #[allow(unused_variables)]
        fn register_modules(registry: &mut ComponentRegistry) -> anyhow::Result<(), RegistryError> {
            $(
                log::info!("registering micro-rdk module '{}'", stringify!($module));
                $module::register_models(registry)?;
            )*
                Ok(())
        }
    }
}

include!(concat!(env!("OUT_DIR"), "/modules.rs"));

extern "C" {
    pub static g_spiram_ok: bool;
}

fn main() -> anyhow::Result<()> {
    esp_idf_sys::link_patches();

    esp_idf_svc::log::EspLogger::initialize_default();
    let sys_loop_stack = EspSystemEventLoop::take().unwrap();
    {
        esp_idf_sys::esp!(unsafe {
            esp_idf_sys::esp_vfs_eventfd_register(&esp_idf_sys::esp_vfs_eventfd_config_t {
                max_fds: 5,
            })
        })?;
    }

    let periph = Peripherals::take().unwrap();

    unsafe {
        if !g_spiram_ok {
            log::info!("spiram not initialized disabling cache feature of the wifi driver");
            g_wifi_feature_caps &= !(CONFIG_FEATURE_CACHE_TX_BUF_BIT as u64);
        }
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

    let mut registry = Box::<ComponentRegistry>::default();
    register_modules(&mut registry)?;
    let repr = RobotRepresentation::WithRegistry(registry);

    serve_web(cfg, tls_cfg, repr, ip, webrtc_certificate);
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

    esp_idf_sys::esp!(unsafe { esp_wifi_set_ps(esp_idf_sys::wifi_ps_type_t_WIFI_PS_NONE) })?;
    Ok(Box::new(wifi))
}
