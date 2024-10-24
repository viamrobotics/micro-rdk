const SSID: Option<&str> = option_env!("MICRO_RDK_WIFI_SSID");
const PASS: Option<&str> = option_env!("MICRO_RDK_WIFI_PASSWORD");
const ROBOT_ID: Option<&str> = option_env!("MICRO_RDK_ROBOT_ID");
const ROBOT_SECRET: Option<&str> = option_env!("MICRO_RDK_ROBOT_SECRET");
const ROBOT_APP_ADDRESS: Option<&str> = option_env!("MICRO_RDK_ROBOT_APP_ADDRESS");

use micro_rdk::{
    common::{
        credentials_storage::{
            RobotConfigurationStorage, RobotCredentials, WifiCredentialStorage, WifiCredentials,
        },
        log::initialize_logger,
        entry::RobotRepresentation,
        provisioning::server::ProvisioningInfo,
        registry::{ComponentRegistry, RegistryError},
    },
    esp32::{
        entry::serve,
        esp_idf_svc::{
            self,
            log::EspLogger,
            sys::{g_wifi_feature_caps, CONFIG_FEATURE_CACHE_TX_BUF_BIT},
        },
        nvs_storage::NVSStorage,
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

    esp_idf_svc::sys::esp!(unsafe {
        esp_idf_svc::sys::esp_vfs_eventfd_register(&esp_idf_svc::sys::esp_vfs_eventfd_config_t {
            max_fds: 5,
        })
    })
    .unwrap();

    let mut registry = Box::<ComponentRegistry>::default();
    if let Err(e) = register_modules(&mut registry) {
        log::error!("couldn't register modules {:?}", e);
    }
    let repr = RobotRepresentation::WithRegistry(registry);

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
        // TODO: update with app address storage logic when version is incremented
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
    info.set_model("esp32".to_owned());

    serve(Some(info), repr, max_connections, storage);
}
