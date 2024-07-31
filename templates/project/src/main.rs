use micro_rdk::{
    common::{
        entry::RobotRepresentation,
        provisioning::server::ProvisioningInfo,
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

    let info = if cfg!(feature = "provisioning") {
        let mut info = ProvisioningInfo::default();
        info.set_manufacturer("viam".to_owned());
        info.set_model("esp32".to_owned());
        Some(info)
    } else {
        None
    };

    if info.is_none() && !storage.has_wifi_credentials() {
        log::error!("device in an unusable state");
        log::warning!("enable the `provisioning` feature or build with wifi credentials");
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
