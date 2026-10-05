//! C entry point used directly by Swift. It has no audio or UI dependencies.
use std::ffi::{CStr, CString, c_char};

/// # Safety
/// `input` must be a terminated UTF-8 C string and `error` must hold `capacity` bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kog_policy_json(
    input: *const c_char,
    error: *mut c_char,
    capacity: usize,
) -> *mut c_char {
    unsafe { json_call(input, error, capacity, crate::bridge::dispatch_json) }
}

/// # Safety
/// The pointer requirements are the same as `kog_policy_json`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kog_session_json(
    input: *const c_char,
    error: *mut c_char,
    capacity: usize,
) -> *mut c_char {
    unsafe { json_call(input, error, capacity, crate::session::dispatch_json) }
}

unsafe fn json_call(
    input: *const c_char,
    error: *mut c_char,
    capacity: usize,
    dispatch: fn(&str) -> Result<String, String>,
) -> *mut c_char {
    let result = (|| {
        if input.is_null() {
            return Err("Missing policy command".to_owned());
        }
        let input = unsafe { CStr::from_ptr(input) }
            .to_str()
            .map_err(|e| e.to_string())?;
        CString::new(dispatch(input)?).map_err(|e| e.to_string())
    })();
    match result {
        Ok(value) => value.into_raw(),
        Err(message) => {
            if !error.is_null() && capacity > 0 {
                let count = message.len().min(capacity - 1);
                unsafe {
                    std::ptr::copy_nonoverlapping(message.as_ptr(), error.cast::<u8>(), count);
                    *error.add(count) = 0;
                }
            }
            std::ptr::null_mut()
        }
    }
}

/// # Safety
/// `value` must be null or an unfreed pointer returned by `kog_policy_json`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kog_policy_string_free(value: *mut c_char) {
    if !value.is_null() {
        drop(unsafe { CString::from_raw(value) });
    }
}
