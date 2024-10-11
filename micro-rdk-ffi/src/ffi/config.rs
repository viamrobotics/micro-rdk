use std::{
    collections::HashMap,
    ffi::{c_char, c_int, c_void, CStr, CString},
};

use micro_rdk::common::config::{AttributeError, ConfigType, Kind};

use super::errors;

#[allow(non_camel_case_types)]
pub(crate) type config_callback =
    extern "C" fn(*mut config_context, *mut c_void, *mut *mut c_void) -> c_int;

/// cbindgen:ignore
pub(crate) extern "C" fn config_noop(
    _: *mut config_context,
    _: *mut c_void,
    _: *mut *mut c_void,
) -> c_int {
    -1
}

#[allow(non_camel_case_types)]
pub struct config_context<'a> {
    pub(crate) cfg: ConfigType<'a>,
}

pub(crate) trait GenericCResourceConfig {
    fn get_user_data_and_config_callback(&mut self) -> (*mut c_void, config_callback);
    fn configure(&mut self, mut config: config_context) -> Result<*mut c_void, i32> {
        let mut obj: *mut c_void = std::ptr::null_mut();
        let (user_data, cfg_callback) = self.get_user_data_and_config_callback();
        let ret = cfg_callback(&mut config as *mut _, user_data, &mut obj as *mut *mut _);
        if ret != 0 {
            Err(ret)
        } else {
            Ok(obj)
        }
    }
}

#[allow(non_camel_case_types)]
pub struct raw_attributes(pub(crate) HashMap<String, Kind>);

/// Get a string from the attribute section of a sensor configuration
/// if found the content of the string will be written to `out`
///
/// Once your are done using the string you need to free it by calling `config_free_string`
/// # Safety
/// `ctx`, `key`, `out` must be valid pointers for the duration of the call
/// `key` must be a null terminated C string
#[no_mangle]
pub unsafe extern "C" fn config_get_string(
    ctx: *mut config_context,
    key: *const c_char,
    out: *mut *mut c_char,
) -> errors::viam_code {
    if ctx.is_null() || key.is_null() || out.is_null() {
        return errors::viam_code::VIAM_INVALID_ARG;
    }
    let key = if let Ok(s) = unsafe { CStr::from_ptr(key) }.to_str() {
        s
    } else {
        return errors::viam_code::VIAM_INVALID_ARG;
    };
    let ctx = unsafe { &mut *ctx };
    let val = match ctx.cfg.get_attribute::<String>(key) {
        Ok(val) => val,
        Err(AttributeError::KeyNotFound(_)) => return errors::viam_code::VIAM_KEY_NOT_FOUND,
        Err(_) => return errors::viam_code::VIAM_INVALID_ARG,
    };
    let c_str = CString::new(val).unwrap().into_raw(); // Assumption here is that val cannot contain a null terminating char (since it's from JSON config)
    unsafe { *out = c_str };
    errors::viam_code::VIAM_OK
}

/// Returns a pointer to the raw attribute structure of a component config
/// pointers remains valid until `config_raw_attributes_free` is called.
/// Free the structure with `config_raw_attributes_free` when done using it.
///
/// # Safety
/// `ctx` must be a valid pointer
#[no_mangle]
pub unsafe extern "C" fn config_get_raw_attributes(
    ctx: *mut config_context,
) -> *mut raw_attributes {
    if ctx.is_null() {
        return std::ptr::null_mut();
    }

    let ctx = unsafe { &mut *ctx };

    let ConfigType::Dynamic(cfg) = ctx.cfg;
    if let Some(attrs) = &cfg.attributes {
        return Box::into_raw(Box::new(raw_attributes(attrs.clone())));
    }

    std::ptr::null_mut()
}

/// Free a raw_attributes structure previously obtained with `config_get_raw_attributes`
///
/// # Safety
/// `attrs` must be a valid pointer
#[no_mangle]
pub unsafe extern "C" fn config_raw_attributes_free(
    attrs: *mut raw_attributes,
) -> errors::viam_code {
    if attrs.is_null() {
        return errors::viam_code::VIAM_INVALID_ARG;
    }

    drop(Box::from_raw(attrs));

    errors::viam_code::VIAM_OK
}

/// Free a string allocated by a successful call to `config_get_string`
///
/// # Safety
/// `ptr` must be a pointer to a string previously allocated by `config_get_string`
#[no_mangle]
pub unsafe extern "C" fn config_free_string(
    _: *mut config_context,
    ptr: *mut c_char,
) -> errors::viam_code {
    if !ptr.is_null() {
        unsafe {
            let _ = CString::from_raw(ptr);
            return errors::viam_code::VIAM_OK;
        }
    }
    errors::viam_code::VIAM_INVALID_ARG
}

/// Get an int32 from the attribute section of a sensor configuration
/// if found the value will be written to `out`
///
/// # Safety
/// `ctx`, `key`, `out` must be valid pointers for the duration of the call
/// `key` must be a null terminated C string
#[no_mangle]
pub unsafe extern "C" fn config_get_i32(
    ctx: *mut config_context,
    key: *const c_char,
    out: *mut c_int,
) -> errors::viam_code {
    if ctx.is_null() || key.is_null() || out.is_null() {
        return errors::viam_code::VIAM_INVALID_ARG;
    }
    let key = if let Ok(s) = unsafe { CStr::from_ptr(key) }.to_str() {
        s
    } else {
        return errors::viam_code::VIAM_KEY_NOT_FOUND;
    };
    let ctx = unsafe { &mut *ctx };
    let val = match ctx.cfg.get_attribute::<i32>(key) {
        Ok(val) => val,
        Err(AttributeError::KeyNotFound(_)) => return errors::viam_code::VIAM_KEY_NOT_FOUND,
        Err(_) => return errors::viam_code::VIAM_INVALID_ARG,
    };
    unsafe { *out = val };
    errors::viam_code::VIAM_OK
}

/// Get a vector of int32s from the attribute section of a sensor configuration,
/// if found the values will be copied into `out`. The length should be obtained
/// first, using `config_get_i32_vec_len` in order to allocate the proper amount of memory
/// pointed to by `out`
///
/// # Safety
/// `ctx`, `key`, `out` must be valid pointers for the duration of the call
/// `key` must be a null terminated C string. Additionally the external process calling
/// the function is responsible for managing the memory allocated for `out`
#[no_mangle]
pub unsafe extern "C" fn config_get_i32_vec(
    ctx: *mut config_context,
    key: *const c_char,
    out: *mut i32,
) -> errors::viam_code {
    if ctx.is_null() || key.is_null() || out.is_null() {
        return errors::viam_code::VIAM_INVALID_ARG;
    }
    let key = if let Ok(s) = unsafe { CStr::from_ptr(key) }.to_str() {
        s
    } else {
        return errors::viam_code::VIAM_KEY_NOT_FOUND;
    };
    let ctx = unsafe { &mut *ctx };
    let val = match ctx.cfg.get_attribute::<Vec<i32>>(key) {
        Ok(val) => val,
        Err(AttributeError::KeyNotFound(_)) => return errors::viam_code::VIAM_KEY_NOT_FOUND,
        Err(_) => return errors::viam_code::VIAM_INVALID_ARG,
    };
    let copy_ptr = out;
    for (i, elem) in val.iter().enumerate() {
        *copy_ptr.add(i) = *elem;
    }
    errors::viam_code::VIAM_OK
}

/// Get the length of a vector of int32s from the attribute section of a sensor configuration.
/// If found the value will be copied into `out`.
///
/// # Safety
/// `ctx`, `key`, `out` must be valid pointers for the duration of the call
/// `key` must be a null terminated C string.
#[no_mangle]
pub unsafe extern "C" fn config_get_i32_vec_len(
    ctx: *mut config_context,
    key: *const c_char,
    out: *mut i32,
) -> errors::viam_code {
    if ctx.is_null() || key.is_null() || out.is_null() {
        return errors::viam_code::VIAM_INVALID_ARG;
    }
    let key = if let Ok(s) = unsafe { CStr::from_ptr(key) }.to_str() {
        s
    } else {
        return errors::viam_code::VIAM_KEY_NOT_FOUND;
    };
    let ctx = unsafe { &mut *ctx };
    let val = match ctx.cfg.get_attribute::<Vec<i32>>(key) {
        Ok(val) => val,
        Err(AttributeError::KeyNotFound(_)) => return errors::viam_code::VIAM_KEY_NOT_FOUND,
        Err(_) => return errors::viam_code::VIAM_INVALID_ARG,
    };
    let len = val.len() as i32;
    unsafe { *out = len };
    errors::viam_code::VIAM_OK
}
