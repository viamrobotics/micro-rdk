#[cfg(target_os = "espidf")]
mod esp32 {
    #[allow(dead_code)]
    #[cfg(not(feature = "qemu"))]
    const SSID: Option<&str> = option_env!("MICRO_RDK_WIFI_SSID");
    #[allow(dead_code)]
    #[cfg(not(feature = "qemu"))]
    const PASS: Option<&str> = option_env!("MICRO_RDK_WIFI_PASSWORD");
    #[allow(dead_code)]
    const ROBOT_ID: Option<&str> = option_env!("MICRO_RDK_ROBOT_ID");
    #[allow(dead_code)]
    const ROBOT_SECRET: Option<&str> = option_env!("MICRO_RDK_ROBOT_SECRET");

    use micro_rdk::common::entry::RobotRepresentation;

    #[cfg(feature = "qemu")]
    use micro_rdk::esp32::conn::network::eth_configure;
    use micro_rdk::esp32::entry::serve_web;
    #[cfg(feature = "qemu")]
    use micro_rdk::esp32::esp_idf_svc::eth::{EspEth, EthDriver};
    use micro_rdk::esp32::esp_idf_svc::sys::{
        g_wifi_feature_caps, CONFIG_FEATURE_CACHE_TX_BUF_BIT,
    };

    extern "C" {
        pub static g_spiram_ok: bool;
    }

    use micro_rdk::common::registry::ComponentRegistry;
    use micro_rdk::esp32::nvs_storage::NVSStorage;

    pub(crate) fn main_esp32() {
        micro_rdk::esp32::esp_idf_svc::sys::link_patches();

        micro_rdk::esp32::esp_idf_svc::log::EspLogger::initialize_default();

        let mut r = Box::<ComponentRegistry>::default();
        if let Err(e) = micro_rdk_modular_driver_example::register_models(&mut r) {
            log::error!("failed to load example drivers: {}", e);
        }
        let repr = RobotRepresentation::WithRegistry(r);

        {
            micro_rdk::esp32::esp_idf_svc::sys::esp!(unsafe {
                micro_rdk::esp32::esp_idf_svc::sys::esp_vfs_eventfd_register(
                    &micro_rdk::esp32::esp_idf_svc::sys::esp_vfs_eventfd_config_t { max_fds: 5 },
                )
            })
            .unwrap();
        }

        let mut max_connection = 3;
        unsafe {
            if !g_spiram_ok {
                log::info!("spiram not initialized disabling cache feature of the wifi driver");
                g_wifi_feature_caps &= !(CONFIG_FEATURE_CACHE_TX_BUF_BIT as u64);
                max_connection = 1;
            }
        }

        // When building the server locally if a user gives a "config" (Robot credentials and Wifi Credentials)
        // then the entire provisioning step can be skipped
        #[cfg(has_robot_config)]
        {
            use micro_rdk::common::credentials_storage::RobotConfigurationStorage;
            use micro_rdk::common::credentials_storage::RobotCredentials;
            use micro_rdk::common::credentials_storage::WifiCredentialStorage;
            use micro_rdk::common::credentials_storage::WifiCredentials;

            log::warn!("Unconditionally using build-time WiFi and robot configuration");

            let storage = NVSStorage::new("nvs").unwrap();
            log::info!("Storing static values from build time wifi configuration to NVS");
            storage
                .store_wifi_credentials(WifiCredentials::new(
                    SSID.expect("[cfg(has_robot_config)]: missing WiFi SSID")
                        .to_string(),
                    PASS.expect("[cfg(has_robot_config)]: missing WiFi password")
                        .to_string(),
                ))
                .expect("Failed to store WiFi credentials to NVS");

            log::info!("Storing static values from build time robot configuration to NVS");
            storage
                .store_robot_credentials(
                    RobotCredentials::new(
                        ROBOT_ID
                            .expect("[cfg(has_robot_config)]: missing robot id")
                            .to_string(),
                        ROBOT_SECRET
                            .expect("[cfg(has_robot_config)]: missing robot secret")
                            .to_string(),
                    )
                    .into(),
                )
                .expect("Failed to store robot credentials to NVS");

            serve_web(None, repr, max_connection, storage);
        }
        #[cfg(not(has_robot_config))]
        {
            use micro_rdk::common::provisioning::server::ProvisioningInfo;
            let mut info = ProvisioningInfo::default();
            info.set_manufacturer("viam".to_owned());
            info.set_model("test-esp32".to_owned());
            let storage = NVSStorage::new("nvs").unwrap();
            serve_web(Some(info), repr, max_connection, storage);
        }

        #[cfg(feature = "qemu")]
        let network = {
            use micro_rdk::esp32::esp_idf_svc::hal::prelude::Peripherals;
            log::info!("creating eth object");
            let eth = EspEth::wrap(
                EthDriver::new_openeth(Peripherals::take().unwrap().mac, sys_loop_stack.clone())
                    .unwrap(),
            )
            .unwrap();
            eth_configure(&sys_loop_stack, eth).unwrap()
        };
    }
}

fn main() {
    #[cfg(target_os = "espidf")]
    esp32::main_esp32();
}
