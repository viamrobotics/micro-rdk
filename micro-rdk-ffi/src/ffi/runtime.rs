use std::{
    ffi::{c_char, c_void, CStr},
    marker::{PhantomData, PhantomPinned},
    sync::{Arc, Mutex},
};

use micro_rdk::common::{
    config::ConfigType,
    entry::RobotRepresentation,
    log::initialize_logger,
    provisioning::server::ProvisioningInfo,
    registry::{ComponentRegistry, Dependency},
    sensor::{SensorError, SensorType},
};

use super::{
    config::config_context,
    errors::viam_code,
    sensor::{generic_c_sensor, generic_c_sensor_config},
};

#[allow(non_camel_case_types)]
pub struct viam_server_context {
    registry: Box<ComponentRegistry>,
    provisioning_info: ProvisioningInfo,
    _marker: PhantomData<(*mut u8, PhantomPinned)>, // Non Send, Non Sync
}

#[cfg(target_os = "espidf")]
extern "C" {
    pub static g_spiram_ok: bool;
}

/// Creates a new Viam server context
///
/// Use the returned pointer to register your own components using the C API
/// The pointer is expected to be valid until the call `start_viam_server`
/// the pointer will then be consumed and any further usage is UB
#[no_mangle]
pub extern "C" fn init_viam_server_context() -> *mut viam_server_context {
    let registry = Box::<ComponentRegistry>::default();
    let mut provisioning_info = ProvisioningInfo::default();
    provisioning_info.set_manufacturer("viam".to_owned());
    provisioning_info.set_model("ffi-provisioning".to_owned());
    Box::into_raw(Box::new(viam_server_context {
        registry,
        provisioning_info,
        _marker: Default::default(),
    }))
}

/// Sets the provisioning model
///
/// returns VIAM_OK on success
/// # Safety
/// `ctx`, `model` must be valid pointers
/// `model` must be a null terminated C String
#[no_mangle]
pub unsafe extern "C" fn viam_server_set_provisioning_model(
    ctx: *mut viam_server_context,
    model: *const c_char,
) -> viam_code {
    if ctx.is_null() || model.is_null() {
        return viam_code::VIAM_INVALID_ARG;
    }
    let ctx = unsafe { &mut *ctx };
    let model = if let Ok(s) = unsafe { CStr::from_ptr(model) }.to_str() {
        s.to_owned()
    } else {
        return viam_code::VIAM_INVALID_ARG;
    };
    ctx.provisioning_info.set_model(model);
    viam_code::VIAM_OK
}

/// Sets the provisioning manufacturer
///
/// returns VIAM_OK on success
/// # Safety
/// `ctx`, `manufacturer` must be valid pointers
/// `manufacturer` must be a null terminated C String
#[no_mangle]
pub unsafe extern "C" fn viam_server_set_provisioning_manufacturer(
    ctx: *mut viam_server_context,
    manufacturer: *const c_char,
) -> viam_code {
    if ctx.is_null() || manufacturer.is_null() {
        return viam_code::VIAM_INVALID_ARG;
    }
    let ctx = unsafe { &mut *ctx };
    let manufacturer = if let Ok(s) = unsafe { CStr::from_ptr(manufacturer) }.to_str() {
        s.to_owned()
    } else {
        return viam_code::VIAM_INVALID_ARG;
    };
    ctx.provisioning_info.set_manufacturer(manufacturer);
    viam_code::VIAM_OK
}

/// Sets the provisioning fragment id
///
/// returns VIAM_OK on success
/// # Safety
/// `ctx`, `fragment_id` must be valid pointers
/// `manufacturer` must be a null terminated C String
#[no_mangle]
pub unsafe extern "C" fn viam_server_set_provisioning_fragment(
    ctx: *mut viam_server_context,
    fragment_id: *const c_char,
) -> viam_code {
    if ctx.is_null() || fragment_id.is_null() {
        return viam_code::VIAM_INVALID_ARG;
    }
    let ctx = unsafe { &mut *ctx };
    let fragment_id = if let Ok(s) = unsafe { CStr::from_ptr(fragment_id) }.to_str() {
        s.to_owned()
    } else {
        return viam_code::VIAM_INVALID_ARG;
    };
    ctx.provisioning_info.set_fragment_id(fragment_id);
    viam_code::VIAM_OK
}

/// Register a generic sensor in the Registry making configurable via Viam config
///
/// `model` is the model name the sensor should be referred to in the Viam config
/// for example calling `viam_server_register_c_generic_sensor(ctx,"my_sensor", config)` will make the generic sensor
/// configurable with `{
///      "name": "sensor1",
///      "namespace": "rdk",
///      "type": "sensor",
///      "model": "my_sensor",
///    }`
///
/// Sensor specific data structure to be used in by the readings callback can be written to out
/// returns VIAM_OK on success
/// # Safety
/// `ctx`, `model` must be valid pointers
#[no_mangle]
pub unsafe extern "C" fn viam_server_register_c_generic_sensor(
    ctx: *mut viam_server_context,
    model: *const c_char,
    sensor: *mut generic_c_sensor_config,
) -> viam_code {
    if ctx.is_null() || model.is_null() {
        return viam_code::VIAM_INVALID_ARG;
    }

    let ctx = unsafe { &mut *ctx };
    let name = if let Ok(s) = unsafe { CStr::from_ptr(model) }.to_str() {
        s
    } else {
        return viam_code::VIAM_INVALID_ARG;
    };

    // Because registry expects a &'static str for its key, we have to copy the name passed
    // as an argument and leak it so it remains valid for the duration of the program.
    let name: &'static str = Box::leak(name.to_owned().into_boxed_str());

    let f = Box::new(move |cfg: ConfigType<'_>, _: Vec<Dependency>| {
        let sensor_config = unsafe { &mut *sensor };
        let mut config = config_context { cfg };
        // obj will hold sensor specific data
        let mut obj: *mut c_void = std::ptr::null_mut();
        let ret = (sensor_config.config_callback)(
            &mut config as *mut _,
            sensor_config.user_data,
            &mut obj as *mut *mut _,
        );
        if ret != 0 {
            return Err(SensorError::ConfigError(name));
        }
        let s = generic_c_sensor {
            user_data: obj,
            get_readings_callback: sensor_config.get_readings_callback,
        };
        Ok::<SensorType, SensorError>(Arc::new(Mutex::new(s)))
    });

    if let Err(e) = ctx.registry.register_sensor(name, Box::leak(f)) {
        log::error!("couldn't register sensor {:?}", e);
        return viam_code::VIAM_REGISTRY_ERROR;
    }

    viam_code::VIAM_OK
}

#[allow(dead_code)]
const ROBOT_ID: Option<&str> = option_env!("MICRO_RDK_ROBOT_ID");
#[allow(dead_code)]
const ROBOT_SECRET: Option<&str> = option_env!("MICRO_RDK_ROBOT_SECRET");

/// Starts the viam server, the function will take ownership of `ctx` therefore future call
/// to other viam_server_* CAPI will be undefined behavior
///
/// The function will returns once the viam server is shutdown
///
/// # Safety
/// `ctx` must be a valid pointer
#[no_mangle]
pub unsafe extern "C" fn viam_server_start(ctx: *mut viam_server_context) -> viam_code {
    if ctx.is_null() {
        return viam_code::VIAM_INVALID_ARG;
    }

    let ctx = unsafe { Box::from_raw(ctx) };

    #[cfg(not(target_os = "espidf"))]
    {
        initialize_logger::<env_logger::Logger>();
    }
    #[cfg(target_os = "espidf")]
    {
        micro_rdk::esp32::esp_idf_svc::sys::link_patches();
        initialize_logger::<micro_rdk::esp32::esp_idf_svc::log::EspLogger>();
        micro_rdk::esp32::esp_idf_svc::sys::esp!(unsafe {
            micro_rdk::esp32::esp_idf_svc::sys::esp_vfs_eventfd_register(
                &micro_rdk::esp32::esp_idf_svc::sys::esp_vfs_eventfd_config_t { max_fds: 5 },
            )
        })
        .unwrap();
    }

    let max_connection = {
        #[cfg(not(target_os = "espidf"))]
        {
            10
        }
        #[cfg(target_os = "espidf")]
        {
            use micro_rdk::esp32::esp_idf_svc::hal::sys::g_wifi_feature_caps;
            use micro_rdk::esp32::esp_idf_svc::hal::sys::CONFIG_FEATURE_CACHE_TX_BUF_BIT;
            if !g_spiram_ok {
                log::info!("spiram not initialized disabling cache feature of the wifi driver");
                g_wifi_feature_caps &= !(CONFIG_FEATURE_CACHE_TX_BUF_BIT as u64);
                1
            } else {
                3
            }
        }
    };

    let repr = RobotRepresentation::WithRegistry(ctx.registry);

    let network = {
        #[cfg(not(target_os = "espidf"))]
        {
            use micro_rdk::common::conn::network::ExternallyManagedNetwork;
            match local_ip_address::local_ip().expect("error parsing local IP") {
                std::net::IpAddr::V4(ip) => ExternallyManagedNetwork::new(ip),
                _ => panic!("oops expected ipv4"),
            }
        }
        #[cfg(target_os = "espidf")]
        {
            use micro_rdk::esp32::conn::network::Esp32ExternallyManagedNetwork;
            Esp32ExternallyManagedNetwork::default()
        }
    };

    use micro_rdk::common::entry::serve_with_network;

    #[cfg(has_robot_config)]
    {
        use micro_rdk::common::credentials_storage::RAMStorage;
        let ram_storage = RAMStorage::new(
            "",
            "",
            ROBOT_ID.expect("Provided build-time configuration failed to set `ROBOT_ID`"),
            ROBOT_SECRET.expect("Provided build-time configuration failed to set `ROBOT_SECRET`"),
        );
        log::info!("Robot configuration information was provided at build time - bypassing Viam provisioning flow");
        serve_with_network(None, repr, max_connection, ram_storage, network);
    }

    #[cfg(not(has_robot_config))]
    {
        #[cfg(not(target_os = "espidf"))]
        let storage = micro_rdk::common::credentials_storage::RAMStorage::default();
        #[cfg(target_os = "espidf")]
        let storage = micro_rdk::esp32::nvs_storage::NVSStorage::new("nvs").unwrap();

        serve_with_network(
            Some(ctx.provisioning_info),
            repr,
            max_connection,
            storage,
            network,
        )
    }

    viam_code::VIAM_OK
}
