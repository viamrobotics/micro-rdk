use std::{
    ffi::{c_char, c_void, CStr},
    marker::{PhantomData, PhantomPinned},
    rc::Rc,
    sync::{Arc, Mutex},
};

use micro_rdk::common::{
    config::ConfigType,
    conn::{server::WebRtcConfiguration, viam::ViamServerBuilder},
    exec::Executor,
    log::initialize_logger,
    provisioning::server::ProvisioningInfo,
    registry::{ComponentRegistry, Dependency, RegistryError},
    sensor::{SensorError, SensorType},
    webrtc::certificate::Certificate,
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
    storage: Vec<String>,
}

// TODO(RSDK-9963): Move to micro-RDK
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

/// Creates a new Viam server context
///
/// Use the returned pointer to register your own components using the C API
/// The pointer is expected to be valid until the call `start_viam_server`
/// the pointer will then be consumed and any further usage is UB
#[no_mangle]
pub extern "C" fn init_viam_server_context() -> *mut viam_server_context {
    let registry = Box::<ComponentRegistry>::default();
    #[cfg(target_os = "espidf")]
    initialize_logger::<micro_rdk::esp32::esp_idf_svc::log::EspLogger>();
    #[cfg(not(target_os = "espidf"))]
    initialize_logger::<env_logger::Logger>();
    let mut provisioning_info = ProvisioningInfo::default();
    provisioning_info.set_manufacturer("viam".to_owned());
    provisioning_info.set_model("ffi-provisioning".to_owned());
    Box::into_raw(Box::new(viam_server_context {
        registry,
        provisioning_info,
        _marker: Default::default(),
        storage: Default::default(),
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

/// Add an nvs partition to the storage collection
///
/// When the viam server starts each partitions (if they exists) will be added to a storage collection
/// and made available to the server
///
/// # Safety
/// `ctx`, `storage_name` must be valid pointers
/// may panic if the storage_name cannot be pushed on the vector
#[no_mangle]
pub unsafe extern "C" fn viam_server_add_nvs_storage(
    ctx: *mut viam_server_context,
    storage_name: *const c_char,
) -> viam_code {
    if ctx.is_null() || storage_name.is_null() {
        return viam_code::VIAM_INVALID_ARG;
    }

    let ctx = unsafe { &mut *ctx };
    let name = if let Ok(s) = unsafe { CStr::from_ptr(storage_name) }.to_str() {
        s
    } else {
        return viam_code::VIAM_INVALID_ARG;
    };

    ctx.storage.push(name.to_owned());

    viam_code::VIAM_OK
}

#[allow(dead_code)]
const ROBOT_ID: Option<&str> = option_env!("MICRO_RDK_ROBOT_ID");
#[allow(dead_code)]
const ROBOT_SECRET: Option<&str> = option_env!("MICRO_RDK_ROBOT_SECRET");
#[allow(dead_code)]
const ROBOT_APP_ADDRESS: Option<&str> = option_env!("MICRO_RDK_ROBOT_APP_ADDRESS");

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

    let mut ctx = unsafe { Box::from_raw(ctx) };

    #[cfg(target_os = "espidf")]
    {
        micro_rdk::esp32::esp_idf_svc::sys::link_patches();
        micro_rdk::esp32::esp_idf_svc::sys::esp!(unsafe {
            micro_rdk::esp32::esp_idf_svc::sys::esp_vfs_eventfd_register(
                &micro_rdk::esp32::esp_idf_svc::sys::esp_vfs_eventfd_config_t { max_fds: 5 },
            )
        })
        .unwrap();
    }

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

    #[cfg(has_robot_config)]
    let storage = {
        use micro_rdk::common::credentials_storage::RAMStorage;
        use micro_rdk::common::credentials_storage::RobotConfigurationStorage;
        use micro_rdk::proto::provisioning::v1::CloudConfig;
        //TODO(RSDK-9715)
        let ram_storage = RAMStorage::new();
        let cloud_conf = if ROBOT_ID.is_some() && ROBOT_SECRET.is_some()  {
            Some(CloudConfig {
                id: ROBOT_ID.unwrap().to_string(),
                secret: ROBOT_SECRET.unwrap().to_string(),
		app_address: "".to_string(),
            })
        } else {
            None
        }.expect("has_robot_config set in cfg, but build-time configuration failed to set robot credentials");
        ram_storage
            .store_robot_credentials(&cloud_conf)
            .expect("Failed to store cloud config");
        if ROBOT_APP_ADDRESS.is_some() {
            ram_storage
                .store_app_address(ROBOT_APP_ADDRESS.unwrap())
                .expect("Failed to store app address")
        }
        ram_storage
    };

    #[cfg(not(has_robot_config))]
    let storage = {
        #[cfg(not(target_os = "espidf"))]
        let storage = micro_rdk::common::credentials_storage::RAMStorage::default();
        #[cfg(target_os = "espidf")]
        let storage: Vec<micro_rdk::esp32::nvs_storage::NVSStorage> = ctx
            .storage
            .iter()
            .map(|s| {
                micro_rdk::esp32::nvs_storage::NVSStorage::new(s).inspect_err(|err| {
                    log::error!(
                        "storage {} cannot be built reason {} continuing without",
                        s,
                        err
                    )
                })
            })
            .filter(|r| r.is_ok())
            .flatten()
            .collect();
        storage
    };

    if let Err(e) = register_modules(&mut ctx.registry) {
        log::error!("couldn't register modules {:?}", e);
    }

    let mut builder = ViamServerBuilder::new(storage);
    builder
        .with_provisioning_info(ctx.provisioning_info)
        .with_component_registry(ctx.registry)
        .with_default_tasks();

    #[cfg(not(target_os = "espidf"))]
    let mut server = {
        use micro_rdk::common::conn::network::Network;
        use micro_rdk::native::{
            certificate::WebRtcCertificate, conn::mdns::NativeMdns, dtls::NativeDtls,
            tcp::NativeH2Connector,
        };
        let webrtc_certs = WebRtcCertificate::new();
        let webrtc_certs = Rc::new(Box::new(webrtc_certs) as Box<dyn Certificate>);
        let dtls = Box::new(NativeDtls::new(webrtc_certs.clone()));
        let webrtc_config = WebRtcConfiguration::new(webrtc_certs, dtls);

        builder
            .with_webrtc_configuration(webrtc_config)
            .with_http2_server(NativeH2Connector::default(), 12346);
        builder.build(
            NativeH2Connector::default(),
            Executor::new(),
            NativeMdns::new("".to_owned(), network.get_ip()).unwrap(),
            Box::new(network),
        )
    };

    #[cfg(target_os = "espidf")]
    let mut server = {
        use micro_rdk::esp32::{
            certificate::GeneratedWebRtcCertificateBuilder, conn::mdns::Esp32Mdns,
            dtls::Esp32DtlsBuilder, tcp::Esp32H2Connector,
        };
        let webrtc_certs = GeneratedWebRtcCertificateBuilder::default()
            .build()
            .unwrap();
        let webrtc_certs = Rc::new(Box::new(webrtc_certs) as Box<dyn Certificate>);
        let dtls = Box::new(Esp32DtlsBuilder::new(webrtc_certs.clone()));
        let webrtc_config = WebRtcConfiguration::new(webrtc_certs, dtls);
        builder
            .with_webrtc_configuration(webrtc_config)
            .with_http2_server(Esp32H2Connector::default(), 12346);

        builder.build(
            Esp32H2Connector::default(),
            Executor::new(),
            Esp32Mdns::new("".to_owned()).unwrap(),
            Box::new(network),
        )
    };
    server.run_forever();
}
