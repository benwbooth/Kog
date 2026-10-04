//! C adapter to the shared in-process library service.
use std::ffi::{c_char, CStr, CString};
use std::ptr;
use kog_server::local_api::request;

#[unsafe(no_mangle)]
pub unsafe extern "C" fn kog_library_request(input: *const c_char, error: *mut c_char, capacity: usize) -> *mut c_char {
    let result = (|| {
        if input.is_null() { return Err("Missing library request".to_owned()); }
        let input = unsafe { CStr::from_ptr(input) }.to_bytes();
        request(serde_json::from_slice(input).map_err(|e| e.to_string())?)
    })();
    match result {
        Ok(value) => CString::new(value.to_string()).unwrap().into_raw(),
        Err(message) => { unsafe { crate::error_to_buffer(&message, error, capacity) }; ptr::null_mut() }
    }
}
