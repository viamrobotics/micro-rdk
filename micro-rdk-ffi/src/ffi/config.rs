use std::ffi::{c_char, c_int, CStr, CString};

use micro_rdk::common::config::{AttributeError, ConfigType};

use super::errors;

#[allow(non_camel_case_types)]
pub struct config_context<'a> {
    pub(crate) cfg: ConfigType<'a>,
}

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
