use micro_rdk::{
    common::{
        config::Kind,
        sensor::{GenericReadingsResult, Readings, Sensor, SensorError},
        status::Status,
    },
    google::protobuf::{value, ListValue, Value},
    DoCommand,
};
use std::{
    collections::HashMap,
    ffi::{c_char, c_int, c_uchar, c_uint, c_void, CStr},
};

use super::{
    config::{config_context, raw_attributes},
    errors::viam_code,
};

#[allow(non_camel_case_types)]
type config_callback = extern "C" fn(*mut config_context, *mut c_void, *mut *mut c_void) -> c_int;

#[allow(non_camel_case_types)]
type get_readings_callback = extern "C" fn(*mut get_readings_context, *mut c_void) -> c_int;

#[allow(non_camel_case_types)]
pub struct generic_c_sensor_config {
    pub(crate) user_data: *mut c_void,
    pub(crate) config_callback: config_callback,
    pub(crate) get_readings_callback: get_readings_callback,
}

#[allow(non_camel_case_types)]
#[derive(DoCommand)]
pub struct generic_c_sensor {
    pub(crate) user_data: *mut c_void,
    pub(crate) get_readings_callback: get_readings_callback,
}

unsafe impl Send for generic_c_sensor {}
unsafe impl Sync for generic_c_sensor {}

/// Creates an new generic sensor config to be used for registering a generic C sensor with the Robot's registry
///
/// The configure and readings functions should be set with
/// `generic_c_sensor_config_set_config_callback` and `generic_c_sensor_config_set_readings_callback`
///
/// Optionally you can set a pointer to some data to be passed during the configuration step with
/// `generic_c_sensor_config_set_user_data`
#[no_mangle]
pub extern "C" fn generic_c_sensor_config_new() -> *mut generic_c_sensor_config {
    Box::into_raw(Box::new(generic_c_sensor_config {
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
pub unsafe extern "C" fn generic_c_sensor_config_set_user_data(
    ctx: *mut generic_c_sensor_config,
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
pub unsafe extern "C" fn generic_c_sensor_config_set_config_callback(
    ctx: *mut generic_c_sensor_config,
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
pub unsafe extern "C" fn generic_c_sensor_config_set_readings_callback(
    ctx: *mut generic_c_sensor_config,
    cb: get_readings_callback,
) -> viam_code {
    if !ctx.is_null() {
        let ctx = unsafe { &mut *ctx };
        ctx.get_readings_callback = cb;
        return viam_code::VIAM_OK;
    }
    viam_code::VIAM_INVALID_ARG
}

/// cbindgen:ignore
extern "C" fn config_noop(_: *mut config_context, _: *mut c_void, _: *mut *mut c_void) -> c_int {
    -1
}

/// cbindgen:ignore
extern "C" fn get_readings_noop(_: *mut get_readings_context, _: *mut c_void) -> c_int {
    -1
}

impl Sensor for generic_c_sensor {}
impl Readings for generic_c_sensor {
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

impl Status for generic_c_sensor {
    fn get_status(
        &self,
    ) -> Result<Option<micro_rdk::google::protobuf::Struct>, micro_rdk::common::status::StatusError>
    {
        Ok(Some(micro_rdk::google::protobuf::Struct {
            fields: HashMap::new(),
        }))
    }
}

#[allow(non_camel_case_types)]
pub struct get_readings_context {
    readings: GenericReadingsResult,
}

/// This function can be use by a sensor during the call to `get_readings_callback` to add binary data to a response
/// The content of `array` will be encoded to BASE64.
///
/// # Safety
/// `ctx`, `key` and `array` must be valid pointers for the duration of the call
/// `key` must be a null terminated C string
#[no_mangle]
pub unsafe extern "C" fn get_readings_add_binary_blob(
    ctx: *mut get_readings_context,
    key: *const c_char,
    array: *const c_uchar,
    len: c_uint,
) -> viam_code {
    use base64::{engine::general_purpose::STANDARD, Engine as _};
    if ctx.is_null() || array.is_null() || key.is_null() {
        return viam_code::VIAM_INVALID_ARG;
    }
    let ctx = unsafe { &mut *ctx };
    if len == 0 {
        return viam_code::VIAM_INVALID_ARG;
    }
    let key = if let Ok(s) = unsafe { CStr::from_ptr(key) }.to_str() {
        s
    } else {
        return viam_code::VIAM_INVALID_ARG;
    };
    let array = unsafe { core::slice::from_raw_parts(array, len as usize) };
    let enc = STANDARD.encode(array);

    let _ = ctx.readings.insert(
        key.to_owned(),
        Value {
            kind: Some(micro_rdk::google::protobuf::value::Kind::StringValue(enc)),
        },
    );

    viam_code::VIAM_OK
}

/// This function can be use by a sensor during the call to `get_readings_callback` to add a string to a response
///
/// # Safety
/// `ctx`, and `key` and `value` must be valid pointers for the duration of the call
/// `key` and `value` must be null terminated C string
#[no_mangle]
pub unsafe extern "C" fn get_readings_add_string(
    ctx: *mut get_readings_context,
    key: *const c_char,
    value: *const c_char,
) -> viam_code {
    if ctx.is_null() || key.is_null() || value.is_null() {
        return viam_code::VIAM_INVALID_ARG;
    }
    let ctx = unsafe { &mut *ctx };
    let key = if let Ok(s) = unsafe { CStr::from_ptr(key) }.to_str() {
        s
    } else {
        return viam_code::VIAM_INVALID_ARG;
    };
    let value = if let Ok(s) = unsafe { CStr::from_ptr(value) }.to_str() {
        s
    } else {
        return viam_code::VIAM_INVALID_ARG;
    };

    let _ = ctx.readings.insert(
        key.to_owned(),
        Value {
            kind: Some(micro_rdk::google::protobuf::value::Kind::StringValue(
                value.to_owned(),
            )),
        },
    );

    viam_code::VIAM_OK
}

// converts a config::Kind to value::Kind purposefully skipping "nested" Kinds
fn into_value(kind: Kind) -> value::Kind {
    match kind {
        Kind::BoolValue(b) => value::Kind::BoolValue(b),
        Kind::NullValue(n) => value::Kind::NullValue(n),
        Kind::NumberValue(f) => value::Kind::NumberValue(f),
        Kind::StringValue(s) => value::Kind::StringValue(s),
        Kind::VecValue(v) => value::Kind::ListValue(ListValue {
            values: v
                .into_iter()
                .map(|v| Value {
                    kind: Some(into_value(v)),
                })
                .collect(),
        }),
        _ => value::Kind::NullValue(0),
    }
}

/// This function can be use by a sensor during the call to `get_readings_callback` to add a `raw_attributes` struct
/// to get_readings
///
/// # Safety
/// `ctx`, and `raw_attrs` and `value` must be valid pointers for the duration of the call
#[no_mangle]
pub unsafe extern "C" fn get_readings_add_raw_attributes(
    ctx: *mut get_readings_context,
    raw_attrs: *const raw_attributes,
) -> viam_code {
    if ctx.is_null() || raw_attrs.is_null() {
        return viam_code::VIAM_INVALID_ARG;
    }
    let ctx = unsafe { &mut *ctx };
    let attrs = unsafe { &*raw_attrs };
    for attr in &attrs.0 {
        let _ = ctx.readings.insert(
            attr.0.clone(),
            Value {
                kind: Some(into_value(attr.1.clone())),
            },
        );
    }

    viam_code::VIAM_OK
}
