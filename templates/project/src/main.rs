const SSID: Option<&str> = option_env!("MICRO_RDK_WIFI_SSID");
const PASS: Option<&str> = option_env!("MICRO_RDK_WIFI_PASSWORD");
const ROBOT_ID: Option<&str> = option_env!("MICRO_RDK_ROBOT_ID");
const ROBOT_SECRET: Option<&str> = option_env!("MICRO_RDK_ROBOT_SECRET");
const ROBOT_APP_ADDRESS: Option<&str> = option_env!("MICRO_RDK_ROBOT_APP_ADDRESS");

use std::rc::Rc;

use micro_rdk::{
    common::{
        conn::{server::WebRtcConfiguration, viam::ViamServerBuilder},
        credentials_storage::{RobotConfigurationStorage, RobotCredentials, WifiCredentialStorage},
        exec::Executor,
        log::initialize_logger,
        provisioning::server::ProvisioningInfo,
        registry::{ComponentRegistry, RegistryError},
        webrtc::certificate::Certificate,
    },
    esp32::{
        certificate::GeneratedWebRtcCertificateBuilder,
        conn::{mdns::Esp32Mdns, network::Esp32WifiNetwork},
        dtls::Esp32DtlsBuilder,
        esp_idf_svc::{
            self,
            log::EspLogger,
            sys::{g_wifi_feature_caps, CONFIG_FEATURE_CACHE_TX_BUF_BIT},
        },
        nvs_storage::NVSStorage,
        tcp::Esp32H2Connector,
    },
};

extern "C" {
    pub static g_spiram_ok: bool;
}

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

fn main() {
    esp_idf_svc::sys::link_patches();
    initialize_logger::<EspLogger>();

    log::info!("{} started (esp32)", env!("CARGO_PKG_NAME"));

    esp_idf_svc::sys::esp!(unsafe {
        esp_idf_svc::sys::esp_vfs_eventfd_register(&esp_idf_svc::sys::esp_vfs_eventfd_config_t {
            max_fds: 5,
        })
    })
    .unwrap();

    let storage = NVSStorage::new("nvs").unwrap();

    // At runtime, if the program does not detect credentials or configs in storage,
    // it will try to load statically compiled values.

    if !storage.has_default_network() {
        log::warn!("no default network settings found in storage");

        // check if any were statically compiled
        if SSID.is_some() && PASS.is_some() {
            log::info!(
                "storing static values from build time network settings to storage as default"
            );
            storage
                .store_default_network(SSID.unwrap(), PASS.unwrap())
                .expect("failed to store network settings to storage");
        }
    }

    if !storage.has_robot_credentials() {
        log::warn!("no machine configuration was found in storage");

        // check if any were statically compiled
        // TODO(RSDK-9148): update with app address storage logic when version is incremented
        if ROBOT_ID.is_some() && ROBOT_SECRET.is_some() && ROBOT_APP_ADDRESS.is_some() {
            log::info!("storing static values from build time machine configuration to storage");
            storage
                .store_robot_credentials(
                    &RobotCredentials::new(
                        ROBOT_ID.unwrap().to_string(),
                        ROBOT_SECRET.unwrap().to_string(),
                    )
                    .into(),
                )
                .expect("failed to store machine credentials to storage");
            storage
                .store_app_address(ROBOT_APP_ADDRESS.unwrap())
                .expect("failed to store app address to storage")
        }
    }

    unsafe {
        if !g_spiram_ok {
            log::info!("spiram not initialized disabling cache feature of the wifi driver");
            g_wifi_feature_caps &= !(CONFIG_FEATURE_CACHE_TX_BUF_BIT as u64);
        }
    }

    let mut info = ProvisioningInfo::default();
    info.set_manufacturer("viam".to_owned());
    info.set_model("esp32".to_owned());

    let mut registry = Box::<ComponentRegistry>::default();
    if let Err(e) = register_modules(&mut registry) {
        log::error!("couldn't register modules {:?}", e);
    }
    let webrtc_certs = GeneratedWebRtcCertificateBuilder::default()
        .build()
        .unwrap();
    let webrtc_certs = Rc::new(Box::new(webrtc_certs) as Box<dyn Certificate>);
    let dtls = Box::new(Esp32DtlsBuilder::new(webrtc_certs.clone()));
    let webrtc_config = WebRtcConfiguration::new(webrtc_certs, dtls);

    let mut builder = ViamServerBuilder::new(storage);
    builder
        .with_provisioning_info(info)
        .with_webrtc_configuration(webrtc_config)
        .with_http2_server(Esp32H2Connector::default(), 12346)
        .with_default_tasks()
        .with_component_registry(registry);

    let builder = { builder.with_wifi_manager(Box::new(Esp32WifiNetwork::new().unwrap())) };
    let mdns = Esp32Mdns::new("".to_owned()).unwrap();

    let mut server = { builder.build(Esp32H2Connector::default(), Executor::new(), mdns) };
    server.run_forever();
}
