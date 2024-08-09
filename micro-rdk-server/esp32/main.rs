#[cfg(target_os = "espidf")]
mod esp32 {
    const SSID: Option<&str> = option_env!("MICRO_RDK_WIFI_SSID");
    const PASS: Option<&str> = option_env!("MICRO_RDK_WIFI_PASSWORD");
    const ROBOT_ID: Option<&str> = option_env!("MICRO_RDK_ROBOT_ID");
    const ROBOT_SECRET: Option<&str> = option_env!("MICRO_RDK_ROBOT_SECRET");

    #[cfg(feature = "qemu")]
    use micro_rdk::esp32::{
        conn::network::eth_configure,
        esp_idf_svc::eth::{EspEth, EthDriver},
    };

    use micro_rdk::{
        common::{
            credentials_storage::{
                RobotConfigurationStorage, RobotCredentials, WifiCredentialStorage, WifiCredentials,
            },
            entry::RobotRepresentation,
            provisioning::server::ProvisioningInfo,
            registry::ComponentRegistry,
        },
        esp32::{
            entry::serve_web,
            esp_idf_svc::{
                self,
                sys::{g_wifi_feature_caps, CONFIG_FEATURE_CACHE_TX_BUF_BIT},
            },
            nvs_storage::NVSStorage,
        },
    };

    extern "C" {
        pub static g_spiram_ok: bool;
    }

    fn register_examples(r: &mut ComponentRegistry) {
        if let Err(e) = micro_rdk_modular_driver_example::free_heap_sensor::register_models(r) {
            log::error!("failed to register `free_heap_sensor`: {}", e);
        }
        if let Err(e) = micro_rdk_modular_driver_example::wifi_rssi_sensor::register_models(r) {
            log::error!("failed to register `wifi_rssi_sensor`: {}", e);
        }
    }

    pub(crate) fn main_esp32() {
        esp_idf_svc::sys::link_patches();
        esp_idf_svc::log::EspLogger::initialize_default();

        esp_idf_svc::sys::esp!(unsafe {
            esp_idf_svc::sys::esp_vfs_eventfd_register(
                &esp_idf_svc::sys::esp_vfs_eventfd_config_t { max_fds: 5 },
            )
        })
        .unwrap();

        #[cfg(feature = "qemu")]
        let _network = {
            use micro_rdk::esp32::esp_idf_svc::hal::prelude::Peripherals;
            log::info!("creating eth object");
            let sys_loop = esp_idf_svc::eventloop::EspEventLoop::take().unwrap();
            let eth = EspEth::wrap(
                EthDriver::new_openeth(Peripherals::take().unwrap().mac, sys_loop.clone()).unwrap(),
            )
            .unwrap();
            eth_configure(&sys_loop, eth).unwrap()
        };

        let mut r = Box::<ComponentRegistry>::default();
        register_examples(&mut r);
        let repr = RobotRepresentation::WithRegistry(r);

        let storage = NVSStorage::new("nvs").unwrap();

        // At runtime, if the program does not detect credentials or configs in storage,
        // it will try to load statically compiled values.
        if !storage.has_wifi_credentials() {
            // check if any were statically compiled
            if SSID.is_some() && PASS.is_some() {
                log::info!("Storing static values from build time wifi configuration to NVS");
                storage
                    .store_wifi_credentials(WifiCredentials::new(
                        SSID.unwrap().to_string(),
                        PASS.unwrap().to_string(),
                    ))
                    .expect("Failed to store WiFi credentials to NVS");
            }
        }

        if !storage.has_robot_configuration() {
            // check if any were statically compiled
            if ROBOT_ID.is_some() && ROBOT_SECRET.is_some() {
                log::info!("Storing static values from build time robot configuration to NVS");
                storage
                    .store_robot_credentials(
                        RobotCredentials::new(
                            ROBOT_ID.unwrap().to_string(),
                            ROBOT_SECRET.unwrap().to_string(),
                        )
                        .into(),
                    )
                    .expect("Failed to store robot credentials to NVS");
            }
        }

        let max_connections = unsafe {
            if !g_spiram_ok {
                log::info!("spiram not initialized disabling cache feature of the wifi driver");
                g_wifi_feature_caps &= !(CONFIG_FEATURE_CACHE_TX_BUF_BIT as u64);
                1
            } else {
                3
            }
        };

        let mut info = ProvisioningInfo::default();
        info.set_manufacturer("viam".to_owned());
        info.set_model("test-esp32".to_owned());

        serve_web(Some(info), repr, max_connections, storage);
    }
}

fn main() {
    #[cfg(target_os = "espidf")]
    esp32::main_esp32();
}
