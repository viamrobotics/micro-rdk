const SSID: Option<&str> = option_env!("MICRO_RDK_WIFI_SSID");
const PASS: Option<&str> = option_env!("MICRO_RDK_WIFI_PASSWORD");
const ROBOT_ID: Option<&str> = option_env!("MICRO_RDK_ROBOT_ID");
const ROBOT_SECRET: Option<&str> = option_env!("MICRO_RDK_ROBOT_SECRET");

use micro_rdk::{
    common::{
        credentials_storage::{RobotConfigurationStorage, WifiCredentialStorage, WifiCredentials},
        entry::RobotRepresentation,
        provisioning::ProvisioningInfo,
        registry::{ComponentRegistry, RegistryError},
    },
    esp32::{
        entry::serve_web,
        esp_idf_svc::{
            log::EspLogger,
            sys::{self, esp, g_wifi_feature_caps, CONFIG_FEATURE_CACHE_TX_BUF_BIT},
        },
        nvs_storage::NVSStorage,
    },
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
    sys::link_patches();
    EspLogger::initialize_default();

    esp!(unsafe { sys::esp_vfs_eventfd_register(&sys::esp_vfs_eventfd_config_t { max_fds: 5 },) })
        .unwrap();

    let max_connections = unsafe {
        if !g_spiram_ok {
            log::info!("spiram not initialized disabling cache feature of the wifi driver");
            g_wifi_feature_caps &= !(CONFIG_FEATURE_CACHE_TX_BUF_BIT as u64);
            1
        } else {
            3
        }
    };

    let storage = NVSStorage::new("nvs").unwrap();

    if SSID.is_some() && PASS.is_some() {
        log::info!("Storing static values from build time wifi configuration to NVS");
        storage
            .store_wifi_credentials(WifiCredentials::new(
                SSID.unwrap().to_string(),
                PASS.unwrap().to_string(),
            ))
            .expect("Failed to store WiFi credentials to NVS");
    }

    if cfg!(has_robot_config) {
        use micro_rdk::common::credentials_storage::RobotCredentials;

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
    }

    let info = if cfg!(feature = "provisioning") {
        let mut info = ProvisioningInfo::default();
        info.set_manufacturer("viam".to_owned());
        info.set_model("esp32".to_owned());
        Some(info)
    } else {
        None
    };

    // TODO: RSDK-8445
    if info.is_none() && !(storage.has_wifi_credentials() && storage.has_robot_credentials()) {
        log::error!("device in an unusable state");
        log::warn!("enable the `provisioning` feature or build with wifi and robot credentials");
        log::error!("sleeping indefinitely...");
        unsafe {
            sys::esp_deep_sleep_start();
        }
    }

    let mut registry = Box::<ComponentRegistry>::default();
    if let Err(e) = register_modules(&mut registry) {
        log::error!("couldn't register modules {:?}", e);
    }
    let repr = RobotRepresentation::WithRegistry(registry);
    serve_web(info, repr, max_connections, storage);
}
