const SSID: &str = env!("MICRO_RDK_WIFI_SSID");
const PASS: &str = env!("MICRO_RDK_WIFI_PASSWORD");

// Generated robot config during build process
include!(concat!(env!("OUT_DIR"), "/robot_secret.rs"));

use log::*;

use embedded_svc::wifi::{
    AuthMethod, ClientConfiguration as WifiClientConfiguration, Configuration as WifiConfiguration,
};
use micro_rdk::esp32::esp_idf_svc::sys::{g_wifi_feature_caps, CONFIG_FEATURE_CACHE_TX_BUF_BIT};
use micro_rdk::{
    common::{
        app_client::AppClientConfig,
        entry::RobotRepresentation,
        registry::{ComponentRegistry, RegistryError},
    },
    esp32::esp_idf_svc::{
        eventloop::EspSystemEventLoop,
        hal::{peripheral::Peripheral, prelude::Peripherals},
        sys::esp_wifi_set_ps,
        wifi::{BlockingWifi, EspWifi},
    },
    esp32::{certificate::WebRtcCertificate, entry::serve_web, tls::Esp32TLSServerConfig},
};

macro_rules! generate_register_modules {
    ($($module:ident),*) => {
        #[allow(unused_variables)]
        fn register_modules(registry: &mut ComponentRegistry) -> Result<(), RegistryError> {
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

fn main() {
    micro_rdk::esp32::esp_idf_svc::sys::link_patches();

    micro_rdk::esp32::esp_idf_svc::log::EspLogger::initialize_default();
    let sys_loop_stack = EspSystemEventLoop::take().unwrap();
    {
        micro_rdk::esp32::esp_idf_svc::sys::esp!(unsafe {
            micro_rdk::esp32::esp_idf_svc::sys::esp_vfs_eventfd_register(
                &micro_rdk::esp32::esp_idf_svc::sys::esp_vfs_eventfd_config_t { max_fds: 5 },
            )
        })
        .unwrap();
    }

    let periph = Peripherals::take().unwrap();

    let mut max_connection = 3;
    unsafe {
        if !g_spiram_ok {
            log::info!("spiram not initialized disabling cache feature of the wifi driver");
            g_wifi_feature_caps &= !(CONFIG_FEATURE_CACHE_TX_BUF_BIT as u64);
            max_connection = 1;
        }
    }

    let (ip, _wifi) = {
        let wifi = start_wifi(periph.modem, sys_loop_stack).expect("failed to start wifi");
        (
            wifi.wifi()
                .sta_netif()
                .get_ip_info()
                .expect("failed to get ip info'")
                .ip,
            wifi,
        )
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
        Esp32TLSServerConfig::new(cert, key.as_ptr(), key.len() as u32)
    };

    let mut registry = Box::<ComponentRegistry>::default();
    register_modules(&mut registry).unwrap();
    let repr = RobotRepresentation::WithRegistry(registry);

    serve_web(cfg, tls_cfg, repr, ip, webrtc_certificate, max_connection);
}

fn start_wifi(
    modem: impl Peripheral<P = micro_rdk::esp32::esp_idf_svc::hal::modem::Modem> + 'static,
    sl_stack: EspSystemEventLoop,
) -> Result<Box<BlockingWifi<EspWifi<'static>>>, micro_rdk::esp32::esp_idf_svc::sys::EspError> {
    let nvs = micro_rdk::esp32::esp_idf_svc::nvs::EspDefaultNvsPartition::take()?;
    let mut wifi = BlockingWifi::wrap(EspWifi::new(modem, sl_stack.clone(), Some(nvs))?, sl_stack)?;
    let wifi_configuration = WifiConfiguration::Client(WifiClientConfiguration {
        ssid: SSID.try_into().unwrap(),
        bssid: None,
        auth_method: AuthMethod::WPA2Personal,
        password: PASS.try_into().unwrap(),
        channel: None,
    });

    wifi.set_configuration(&wifi_configuration)?;

    wifi.start()?;
    info!("Wifi started");

    wifi.connect()?;
    info!("Wifi connected");

    wifi.wait_netif_up()?;

    micro_rdk::esp32::esp_idf_svc::sys::esp!(unsafe {
        esp_wifi_set_ps(micro_rdk::esp32::esp_idf_svc::sys::wifi_ps_type_t_WIFI_PS_NONE)
    })?;
    Ok(Box::new(wifi))
}
