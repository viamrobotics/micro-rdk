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
    use micro_rdk::common::provisioning::server::ProvisioningInfo;
    use micro_rdk::common::provisioning::storage::RAMStorage;
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

    pub(crate) fn main_esp32() {
        micro_rdk::esp32::esp_idf_svc::sys::link_patches();

        micro_rdk::esp32::esp_idf_svc::log::EspLogger::initialize_default();

        let repr = RobotRepresentation::WithRegistry(Box::<ComponentRegistry>::default());

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

        // When building esp32-server locally if a user gives a "config" (Robot credentials and Wifi Credentials)
        // then the entire provisioning step can be skipped
        #[cfg(has_robot_config)]
        {
            if SSID.is_some() && PASS.is_some() && ROBOT_ID.is_some() && ROBOT_SECRET.is_some() {
                let ram_storage = RAMStorage::new(
                    SSID.unwrap(),
                    PASS.unwrap(),
                    ROBOT_ID.unwrap(),
                    ROBOT_SECRET.unwrap(),
                );
                serve_web(None, repr, max_connection, ram_storage);
            }
        }
        #[cfg(not(has_robot_config))]
        {
	    // Pass NVS
            let mut info = ProvisioningInfo::default();
            info.set_fragment_id("d385b480-3d19-4fad-a928-b5c18a58d0ed".to_string());
            info.set_manufacturer("viam".to_owned());
            info.set_model("test-esp32".to_owned());
            let storage = RAMStorage::default();
            log::info!("Will provision");
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
