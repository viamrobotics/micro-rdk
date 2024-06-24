use micro_rdk::esp32::esp_idf_svc::sys::{g_wifi_feature_caps, CONFIG_FEATURE_CACHE_TX_BUF_BIT};
use micro_rdk::{
    common::{
        entry::RobotRepresentation,
        registry::{ComponentRegistry, RegistryError},
    },
    esp32::entry::serve_web,
};

use micro_rdk::common::provisioning::server::ProvisioningInfo;
use micro_rdk::esp32::nvs_storage::NVSStorage;
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

    let mut registry = Box::<ComponentRegistry>::default();
    if let Err(e) = register_modules(&mut registry) {
        log::error!("couldn't register modules {:?}", e);
    }
    let repr = RobotRepresentation::WithRegistry(registry);
    let mut info = ProvisioningInfo::default();
    info.set_manufacturer("viam".to_owned());
    info.set_model("esp32".to_owned());
    let storage = NVSStorage::new("nvs").unwrap();
    serve_web(Some(info), repr, max_connection, storage);
}
