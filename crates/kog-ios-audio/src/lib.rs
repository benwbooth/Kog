use std::ffi::{CStr, c_char};
use std::io::Read;
use std::path::PathBuf;
use std::ptr;
use std::slice;
use std::time::Duration;

use kog_audio::decoder::DecoderSettings;
use kog_audio::streaming::PcmReader;

mod catalog;

pub struct KogAudioHandle {
    reader: PcmReader,
}

pub(crate) unsafe fn error_to_buffer(message: &str, output: *mut c_char, capacity: usize) {
    if output.is_null() || capacity == 0 {
        return;
    }
    let bytes = message.as_bytes();
    let count = bytes.len().min(capacity - 1);
    unsafe {
        ptr::copy_nonoverlapping(bytes.as_ptr(), output.cast::<u8>(), count);
        *output.add(count) = 0;
    }
}

/// Open a sandbox-local file. `subsong` is -1 for normal expansion.
/// The returned pointer is owned by the caller and must be closed once.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kog_audio_open(
    path: *const c_char,
    subsong: i32,
    error: *mut c_char,
    error_capacity: usize,
) -> *mut KogAudioHandle {
    if path.is_null() {
        unsafe { error_to_buffer("Missing audio path", error, error_capacity) };
        return ptr::null_mut();
    }
    let path = match unsafe { CStr::from_ptr(path) }.to_str() {
        Ok(path) => PathBuf::from(path),
        Err(_) => {
            unsafe { error_to_buffer("Audio path is not UTF-8", error, error_capacity) };
            return ptr::null_mut();
        }
    };
    let reader = PcmReader::open_path_subsong(
        path,
        (subsong >= 0).then_some(subsong as u32),
        DecoderSettings::default(),
    );
    match reader {
        Ok(reader) => Box::into_raw(Box::new(KogAudioHandle { reader })),
        Err(message) => {
            unsafe { error_to_buffer(&message, error, error_capacity) };
            ptr::null_mut()
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn kog_audio_duration_ms(handle: *const KogAudioHandle) -> i64 {
    let Some(handle) = (unsafe { handle.as_ref() }) else {
        return -1;
    };
    handle.reader.duration().map_or(-1, |duration| {
        duration.as_millis().min(i64::MAX as u128) as i64
    })
}

/// Read interleaved 48 kHz stereo f32 samples as native-endian bytes.
/// Returns a byte count, 0 at end of stream, or -1 on failure.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kog_audio_read(
    handle: *mut KogAudioHandle,
    output: *mut u8,
    capacity: usize,
    error: *mut c_char,
    error_capacity: usize,
) -> isize {
    if output.is_null() || capacity == 0 || capacity % 8 != 0 {
        unsafe {
            error_to_buffer(
                "PCM buffer must hold whole stereo frames",
                error,
                error_capacity,
            )
        };
        return -1;
    }
    let Some(handle) = (unsafe { handle.as_mut() }) else {
        unsafe { error_to_buffer("Decoder is closed", error, error_capacity) };
        return -1;
    };
    let output = unsafe { slice::from_raw_parts_mut(output, capacity) };
    match handle.reader.read(output) {
        Ok(bytes) => bytes as isize,
        Err(failure) => {
            unsafe { error_to_buffer(&failure.to_string(), error, error_capacity) };
            -1
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn kog_audio_seek(
    handle: *mut KogAudioHandle,
    position_ms: u64,
    error: *mut c_char,
    error_capacity: usize,
) -> bool {
    let Some(handle) = (unsafe { handle.as_mut() }) else {
        unsafe { error_to_buffer("Decoder is closed", error, error_capacity) };
        return false;
    };
    match handle.reader.seek(Duration::from_millis(position_ms)) {
        Ok(()) => true,
        Err(message) => {
            unsafe { error_to_buffer(&message, error, error_capacity) };
            false
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn kog_audio_close(handle: *mut KogAudioHandle) {
    if !handle.is_null() {
        drop(unsafe { Box::from_raw(handle) });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn c_abi_reads_and_seeks_a_local_wav() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let file = directory.path().join("tone.wav");
        let sample_count = 48_000_u32 / 5;
        let data_bytes = sample_count * 2;
        let mut wav = Vec::with_capacity(44 + data_bytes as usize);
        wav.extend_from_slice(b"RIFF");
        wav.extend_from_slice(&(36 + data_bytes).to_le_bytes());
        wav.extend_from_slice(b"WAVEfmt ");
        wav.extend_from_slice(&16_u32.to_le_bytes());
        wav.extend_from_slice(&1_u16.to_le_bytes());
        wav.extend_from_slice(&1_u16.to_le_bytes());
        wav.extend_from_slice(&48_000_u32.to_le_bytes());
        wav.extend_from_slice(&(48_000_u32 * 2).to_le_bytes());
        wav.extend_from_slice(&2_u16.to_le_bytes());
        wav.extend_from_slice(&16_u16.to_le_bytes());
        wav.extend_from_slice(b"data");
        wav.extend_from_slice(&data_bytes.to_le_bytes());
        for index in 0..sample_count {
            let value: i16 = if index % 200 < 100 { 8_000 } else { -8_000 };
            wav.extend_from_slice(&value.to_le_bytes());
        }
        fs::write(&file, wav).expect("write audio fixture");
        let path = std::ffi::CString::new(file.to_str().expect("UTF-8 temp path"))
            .expect("no NUL in temp path");
        let mut error = [0_i8; 256];
        let handle = unsafe { kog_audio_open(path.as_ptr(), -1, error.as_mut_ptr(), error.len()) };
        assert!(
            !handle.is_null(),
            "{}",
            unsafe { CStr::from_ptr(error.as_ptr()) }.to_string_lossy()
        );
        assert_eq!(unsafe { kog_audio_duration_ms(handle) }, 200);
        let mut output = [0_u8; 8192];
        let read = unsafe {
            kog_audio_read(
                handle,
                output.as_mut_ptr(),
                output.len(),
                error.as_mut_ptr(),
                error.len(),
            )
        };
        assert!(read > 0 && read % 8 == 0);
        assert!(output[..read as usize].iter().any(|byte| *byte != 0));
        assert!(unsafe { kog_audio_seek(handle, 100, error.as_mut_ptr(), error.len()) });
        let read = unsafe {
            kog_audio_read(
                handle,
                output.as_mut_ptr(),
                output.len(),
                error.as_mut_ptr(),
                error.len(),
            )
        };
        assert!(read > 0);
        unsafe { kog_audio_close(handle) };
    }
}
