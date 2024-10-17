use std::{
    collections::HashMap,
    ffi::c_void,
    sync::{Arc, Mutex},
};

use micro_rdk::common::{
    config::ConfigType,
    movement_sensor::{MovementSensor, MovementSensorType},
    registry::Dependency,
    sensor::{GenericReadingsResult, Readings, SensorError},
    status::Status,
};
use micro_rdk::DoCommand;

use super::{
    config::{config_callback, config_context, config_noop, configure, GenericCResourceConfig},
    errors::viam_code,
    sensor::{get_readings_callback, get_readings_context, get_readings_noop},
};

#[allow(non_camel_case_types)]
pub struct generic_c_movement_sensor_config {
    pub(crate) user_data: *mut c_void,
    pub(crate) config_callback: config_callback,
    pub(crate) get_readings_callback: get_readings_callback,
}

impl GenericCResourceConfig for generic_c_movement_sensor_config {
    fn get_user_data_and_config_callback(&mut self) -> (*mut c_void, config_callback) {
        (self.user_data, self.config_callback)
    }
    fn register(
        m_sensor: *mut Self,
        name: &'static str,
        ctx: &mut super::runtime::viam_server_context,
    ) -> viam_code {
        let constructor = Box::new(move |cfg: ConfigType<'_>, _: Vec<Dependency>| {
            let sensor_config = unsafe { &mut *m_sensor };
            let config = config_context { cfg };
            configure(sensor_config, config)
                .map(|obj| {
                    let s = generic_c_movement_sensor {
                        user_data: obj,
                        get_readings_callback: sensor_config.get_readings_callback,
                    };
                    Arc::new(Mutex::new(s)) as MovementSensorType
                })
                .map_err(|_| SensorError::ConfigError(name))
        });
        match ctx
            .registry
            .register_movement_sensor(name, Box::leak(constructor))
        {
            Ok(_) => viam_code::VIAM_OK,
            Err(e) => {
                log::error!("couldn't register movement sensor {:?}", e);
                viam_code::VIAM_REGISTRY_ERROR
            }
        }
    }
}

/// Creates an new generic movement sensor config to be used for registering a generic C movement sensor with
/// the Robot's registry
///
/// The configure and readings functions should be set with
/// `generic_c_sensor_config_set_config_callback` and `generic_c_sensor_config_set_readings_callback`
///
/// Optionally you can set a pointer to some data to be passed during the configuration step with
/// `generic_c_sensor_config_set_user_data`
#[no_mangle]
pub extern "C" fn generic_c_movement_sensor_config_new() -> *mut generic_c_movement_sensor_config {
    Box::into_raw(Box::new(generic_c_movement_sensor_config {
        user_data: std::ptr::null_mut(),
        config_callback: config_noop,
        get_readings_callback: get_readings_noop,
    }))
}

/// Set the user data pointer, the value will then be passed to the `config_callback` during the configuration step
///
/// # Safety
/// `ctx` must be a valid pointer
#[no_mangle]
pub unsafe extern "C" fn generic_c_movement_sensor_config_set_user_data(
    ctx: *mut generic_c_movement_sensor_config,
    data: *mut c_void,
) -> viam_code {
    if !ctx.is_null() {
        let ctx = unsafe { &mut *ctx };
        ctx.user_data = data;
        return viam_code::VIAM_OK;
    }
    viam_code::VIAM_INVALID_ARG
}

/// Set the config callback, which will be called when this sensor is configured
///
/// # Safety
/// `ctx` must be a valid pointer
#[no_mangle]
pub unsafe extern "C" fn generic_c_movement_sensor_config_set_config_callback(
    ctx: *mut generic_c_movement_sensor_config,
    cb: config_callback,
) -> viam_code {
    if !ctx.is_null() {
        let ctx = unsafe { &mut *ctx };
        ctx.config_callback = cb;
        return viam_code::VIAM_OK;
    }
    viam_code::VIAM_INVALID_ARG
}

/// Set the get readings callback, which will be called when GetReadings is called on a properly
/// configured sensor
///
/// # Safety
/// `ctx` must be a valid pointer
#[no_mangle]
pub unsafe extern "C" fn generic_c_movement_sensor_config_set_readings_callback(
    ctx: *mut generic_c_movement_sensor_config,
    cb: get_readings_callback,
) -> viam_code {
    if !ctx.is_null() {
        let ctx = unsafe { &mut *ctx };
        ctx.get_readings_callback = cb;
        return viam_code::VIAM_OK;
    }
    viam_code::VIAM_INVALID_ARG
}

#[allow(non_camel_case_types)]
#[derive(DoCommand)]
pub struct generic_c_movement_sensor {
    pub(crate) user_data: *mut c_void,
    pub(crate) get_readings_callback: get_readings_callback,
}

unsafe impl Send for generic_c_movement_sensor {}
unsafe impl Sync for generic_c_movement_sensor {}

impl Readings for generic_c_movement_sensor {
    fn get_generic_readings(
        &mut self,
    ) -> Result<GenericReadingsResult, micro_rdk::common::sensor::SensorError> {
        let mut ctx = get_readings_context {
            readings: GenericReadingsResult::default(),
        };

        let ret = (self.get_readings_callback)(&mut ctx as *mut _, self.user_data);
        if ret != 0 {
            return Err(SensorError::SensorCodeError(ret));
        }
        Ok(ctx.readings)
    }
}

impl Status for generic_c_movement_sensor {
    fn get_status(
        &self,
    ) -> Result<Option<micro_rdk::google::protobuf::Struct>, micro_rdk::common::status::StatusError>
    {
        Ok(Some(micro_rdk::google::protobuf::Struct {
            fields: HashMap::new(),
        }))
    }
}

impl MovementSensor for generic_c_movement_sensor {
    fn get_angular_velocity(
        &mut self,
    ) -> Result<micro_rdk::common::math_utils::Vector3, SensorError> {
        Err(SensorError::SensorMethodUnimplemented(
            "generic_c_movement_sensor does not implement get_angular_velocity",
        ))
    }
    fn get_compass_heading(&mut self) -> Result<f64, SensorError> {
        Err(SensorError::SensorMethodUnimplemented(
            "generic_c_movement_sensor does not implement get_compass_heading",
        ))
    }
    fn get_linear_acceleration(
        &mut self,
    ) -> Result<micro_rdk::common::math_utils::Vector3, SensorError> {
        Err(SensorError::SensorMethodUnimplemented(
            "generic_c_movement_sensor does not implement get_linear_acceleration",
        ))
    }
    fn get_linear_velocity(
        &mut self,
    ) -> Result<micro_rdk::common::math_utils::Vector3, SensorError> {
        Err(SensorError::SensorMethodUnimplemented(
            "generic_c_movement_sensor does not implement get_linear_velocity",
        ))
    }
    fn get_position(
        &mut self,
    ) -> Result<micro_rdk::common::movement_sensor::GeoPosition, SensorError> {
        Err(SensorError::SensorMethodUnimplemented(
            "generic_c_movement_sensor does not implement get_position",
        ))
    }
    fn get_properties(&self) -> micro_rdk::common::movement_sensor::MovementSensorSupportedMethods {
        Default::default()
    }
}
