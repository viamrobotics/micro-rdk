use std::{
    collections::HashMap,
    ffi::{c_char, c_void, CStr, CString},
};

use super::errors::viam_code;

#[allow(non_camel_case_types)]
type hashmap_cstring_ptr_callback = extern "C" fn(*mut c_void, *const c_char, *const c_void);

/// An helper type which stores key value pairs where key is a Cstring and value
/// is a pointer to user data
#[allow(non_camel_case_types)]
#[derive(Default)]
pub struct hashmap_cstring_ptr {
    inner: HashMap<CString, *const c_void>,
}

/// Creates an new `hashmap_ctsring_ptr`
///
/// Use the returned pointer to store key value pairs using the C API
/// the pointer returned will remain valid until the call to `hashmap_cstring_ptr_destroy` after which
/// the pointer shouldn't be used again
#[no_mangle]
pub extern "C" fn hashmap_cstring_ptr_new() -> *mut hashmap_cstring_ptr {
    Box::into_raw(Box::default())
}

/// Destroys an  `hashmap_cstring_ptr`
///
/// After this call the pointer shouldn't be used again
/// If callback is not null it will be called on each key value pairs
/// # Safety
/// `ctx` must be a valid pointer
#[no_mangle]
pub unsafe extern "C" fn hashmap_cstring_ptr_destroy(
    ctx: *mut hashmap_cstring_ptr,
    user_data: *mut c_void,
    callback: Option<hashmap_cstring_ptr_callback>,
) -> viam_code {
    if ctx.is_null() {
        return viam_code::VIAM_INVALID_ARG;
    }
    let ctx = unsafe { Box::from_raw(ctx) };
    if let Some(callback) = callback {
        for (key, value) in &ctx.inner {
            callback(user_data, key.as_ptr(), *value);
        }
    }
    viam_code::VIAM_OK
}

/// Returns a previously stored value if it exists, otherwise returns a null pointer
///
/// The returned pointer (if not null) shouldn't be freed, to remove a key value pair use
/// hashmap_cstring_ptr_remove
/// # Safety
/// `ctx` and `key` must be valid pointers
#[no_mangle]
pub unsafe extern "C" fn hashmap_cstring_ptr_get(
    ctx: *mut hashmap_cstring_ptr,
    key: *const c_char,
) -> *const c_void {
    if ctx.is_null() || key.is_null() {
        return std::ptr::null();
    }
    let key = unsafe { CStr::from_ptr(key) }.to_owned();
    let ctx = unsafe { &mut *ctx };
    ctx.inner.get(&key).map_or(std::ptr::null(), |ptr| *ptr)
}

/// Iterate through each key value pair calling callback on each pairs
///
/// Any pointer passed into the callback function should not be freed
/// # Safety
/// `ctx` must be a valid pointer
/// `callback` must be a valid callback
#[no_mangle]
pub unsafe extern "C" fn hashmap_cstring_ptr_for_each_kv(
    ctx: *mut hashmap_cstring_ptr,
    user_data: *mut c_void,
    callback: hashmap_cstring_ptr_callback,
) {
    if ctx.is_null() {
        return;
    }

    let ctx = unsafe { &mut *ctx };
    for (key, value) in &ctx.inner {
        callback(user_data, key.as_ptr(), *value);
    }
}

/// Removes and returns a previously stored value if it exists, otherwise returns a null pointer
///
/// The returned pointer (if not null) must be freed, to get a key value pair without removal use
/// hashmap_cstring_ptr_get
/// # Safety
/// `ctx` and `key` must be valid pointers
#[no_mangle]
pub unsafe extern "C" fn hashmap_cstring_ptr_remove(
    ctx: *mut hashmap_cstring_ptr,
    key: *const c_char,
) -> *const c_void {
    if ctx.is_null() || key.is_null() {
        return std::ptr::null();
    }
    let key = unsafe { CStr::from_ptr(key) }.to_owned();
    let ctx = unsafe { &mut *ctx };
    ctx.inner.remove(&key).map_or(std::ptr::null(), |ptr| ptr)
}

/// Inserts a key-value pair into the hash map.
///
/// The returned pointer (if not null) must be freed
/// # Safety
/// `ctx`, `key` and ptr must be valid pointers
/// `ptr` must remain valid until a call to hashmap_cstring_ptr_remove or hashmap_cstring_ptr_destroy
#[no_mangle]
pub unsafe extern "C" fn hashmap_cstring_ptr_insert(
    ctx: *mut hashmap_cstring_ptr,
    key: *const c_char,
    ptr: *const c_void,
) -> *const c_void {
    if ctx.is_null() || key.is_null() || ptr.is_null() {
        return std::ptr::null();
    }
    let key = unsafe { CStr::from_ptr(key) }.to_owned();
    let ctx = unsafe { &mut *ctx };
    ctx.inner
        .insert(key, ptr)
        .map_or(std::ptr::null(), |ptr| ptr)
}
