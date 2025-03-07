#[cfg(target_os = "espidf")]
mod esp32 {
    const SSID: Option<&str> = option_env!("MICRO_RDK_WIFI_SSID");
    const PASS: Option<&str> = option_env!("MICRO_RDK_WIFI_PASSWORD");
    const ROBOT_ID: Option<&str> = option_env!("MICRO_RDK_ROBOT_ID");
    const ROBOT_SECRET: Option<&str> = option_env!("MICRO_RDK_ROBOT_SECRET");
    const ROBOT_APP_ADDRESS: Option<&str> = option_env!("MICRO_RDK_ROBOT_APP_ADDRESS");

    use std::rc::Rc;

    use micro_rdk::common::conn::server::WebRtcConfiguration;
    use micro_rdk::common::conn::viam::ViamServerBuilder;
    #[cfg(feature = "qemu")]
    use micro_rdk::common::credentials_storage::RAMStorage;
    use micro_rdk::common::exec::Executor;
    use micro_rdk::common::webrtc::certificate::Certificate;
    use micro_rdk::esp32::certificate::GeneratedWebRtcCertificateBuilder;
    use micro_rdk::esp32::conn::mdns::Esp32Mdns;
    #[cfg(not(feature = "qemu"))]
    use micro_rdk::esp32::conn::network::Esp32WifiNetwork;
    use micro_rdk::esp32::dtls::Esp32DtlsBuilder;
    #[cfg(not(feature = "qemu"))]
    use micro_rdk::esp32::nvs_storage::NVSStorage;
    use micro_rdk::esp32::tcp::Esp32H2Connector;
    use micro_rdk::{
        common::{
            credentials_storage::{
                RobotConfigurationStorage, RobotCredentials, WifiCredentialStorage,
            },
            log::initialize_logger,
            provisioning::server::ProvisioningInfo,
            registry::ComponentRegistry,
        },
        esp32::esp_idf_svc::{
            self,
            log::EspLogger,
            sys::{g_wifi_feature_caps, CONFIG_FEATURE_CACHE_TX_BUF_BIT},
        },
    };
    extern "C" {
        pub static g_spiram_ok: bool;
    }

    fn register_example_modules(r: &mut ComponentRegistry) {
        if let Err(e) = micro_rdk_modular_driver_example::free_heap_sensor::register_models(r) {
            log::error!("failed to register `free_heap_sensor`: {}", e);
        }
        if let Err(e) = micro_rdk_modular_driver_example::wifi_rssi_sensor::register_models(r) {
            log::error!("failed to register `wifi_rssi_sensor`: {}", e);
        }
    }

    pub(crate) fn main_esp32() {
        esp_idf_svc::sys::link_patches();
        initialize_logger::<EspLogger>();

        log::info!("micro-rdk-server started (esp32)");

        esp_idf_svc::sys::esp!(unsafe {
            esp_idf_svc::sys::esp_vfs_eventfd_register(
                &esp_idf_svc::sys::esp_vfs_eventfd_config_t { max_fds: 5 },
            )
        })
        .unwrap();

        #[cfg(feature = "qemu")]
        let network = {
            log::info!("creating eth object");
            let eth = micro_rdk::esp32::conn::network::esp_eth_openeth().unwrap();
            micro_rdk::esp32::conn::network::eth_configure(eth).unwrap()
        };

        let mut registry = Box::<ComponentRegistry>::default();
        register_example_modules(&mut registry);

        #[cfg(feature = "qemu")]
        let storage = { RAMStorage::new() }; //NVSStorage::new("nvs").unwrap();
        #[cfg(not(feature = "qemu"))]
        let storage = { NVSStorage::new("nvs").unwrap() };

        // At runtime, if the program does not detect credentials or configs in storage,
        // it will try to load statically compiled values.

        if !storage.has_default_network() {
            // check if any were statically compiled
            if SSID.is_some() && PASS.is_some() {
                log::info!("storing static values from build time wifi configuration to storage");
                storage
                    .store_default_network(SSID.unwrap(), PASS.unwrap())
                    .expect("Failed to store WiFi credentials to NVS");
            }
        }

        if !storage.has_robot_credentials() {
            log::warn!("no machine credentials were found in storage");

            // check if any were statically compiled
            if ROBOT_ID.is_some() && ROBOT_SECRET.is_some() && ROBOT_APP_ADDRESS.is_some() {
                log::info!(
                    "storing static values from build time machine configuration to storage"
                );
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
        info.set_model("test-esp32".to_owned());

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
        #[cfg(not(feature = "qemu"))]
        let builder = { builder.with_wifi_manager(Box::new(Esp32WifiNetwork::new().unwrap())) };
        let mdns = Esp32Mdns::new("".to_owned()).unwrap();
        #[cfg(feature = "qemu")]
        let mut server = {
            builder.build(
                Esp32H2Connector::default(),
                Executor::new(),
                mdns,
                Box::new(network),
            )
        };
        #[cfg(not(feature = "qemu"))]
        let mut server = { builder.build(Esp32H2Connector::default(), Executor::new(), mdns) };
        server.run_forever();
    }
}

fn main() {
    #[cfg(target_os = "espidf")]
    esp32::main_esp32();
}
