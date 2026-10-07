use std::ffi::{CStr, CString, c_char};
use std::io::Read;
use std::path::PathBuf;
use std::ptr;
use std::slice;
use std::time::Duration;

use kog_audio::decoder::DecoderSettings;
use kog_audio::settings::MidiEngine;
use kog_audio::streaming::PcmReader;

mod catalog;
mod library;

/// Platform-neutral playback commands. The caller frees the reply with
/// `kog_audio_string_free`, just like catalog replies.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kog_playback_policy(
    input: *const c_char, error: *mut c_char, error_capacity: usize,
) -> *mut c_char {
    unsafe { kog_audio::playback_order::ffi::kog_policy_json(input, error, error_capacity) }
}

/// Keep the application session bridge linked in the native audio library.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kog_backend_session(
    input: *const c_char, error: *mut c_char, error_capacity: usize,
) -> *mut c_char {
    unsafe { kog_audio::playback_order::ffi::kog_session_json(input, error, error_capacity) }
}

pub struct KogAudioHandle {
    reader: PcmReader,
    /// A sandbox-local source, so its MML score can be recorded separately.
    score_source: Option<(PathBuf, Option<u32>, DecoderSettings)>,
}

static MML: std::sync::OnceLock<kog_server::mml::MmlJobs> = std::sync::OnceLock::new();

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
    midi_engine: *const c_char,
    soundfont_path: *const c_char,
    sc55_rom_path: *const c_char,
    mt32_rom_path: *const c_char,
    error: *mut c_char,
    error_capacity: usize,
) -> *mut KogAudioHandle {
    if path.is_null() || midi_engine.is_null() {
        unsafe { error_to_buffer("Missing audio path or MIDI synth", error, error_capacity) };
        return ptr::null_mut();
    }
    let path = match unsafe { CStr::from_ptr(path) }.to_str() {
        Ok(path) => PathBuf::from(path),
        Err(_) => {
            unsafe { error_to_buffer("Audio path is not UTF-8", error, error_capacity) };
            return ptr::null_mut();
        }
    };
    let options = (|| -> Result<DecoderSettings, String> {
        let engine = unsafe { CStr::from_ptr(midi_engine) }
            .to_str()
            .map_err(|_| "MIDI synth is not UTF-8".to_owned())?;
        let engine = MidiEngine::from_setting(engine)
            .ok_or_else(|| format!("Unknown MIDI synth: {engine}"))?;
        let optional_path = |value: *const c_char| -> Result<Option<PathBuf>, String> {
            if value.is_null() {
                return Ok(None);
            }
            let text = unsafe { CStr::from_ptr(value) }
                .to_str()
                .map_err(|_| "MIDI asset path is not UTF-8".to_owned())?;
            Ok((!text.is_empty()).then(|| PathBuf::from(text)))
        };
        Ok(DecoderSettings::new(optional_path(soundfont_path)?, engine)
            .with_sc55_rom_path(optional_path(sc55_rom_path)?)
            .with_mt32_rom_path(optional_path(mt32_rom_path)?))
    })();
    let options = match options {
        Ok(options) => options,
        Err(message) => {
            unsafe { error_to_buffer(&message, error, error_capacity) };
            return ptr::null_mut();
        }
    };
    let subsong = (subsong >= 0).then_some(subsong as u32);
    let score_source = Some((path.clone(), subsong, options.clone()));
    let reader = PcmReader::open_path_subsong(path, subsong, options);
    match reader {
        Ok(reader) => Box::into_raw(Box::new(KogAudioHandle { reader, score_source })),
        Err(message) => {
            unsafe { error_to_buffer(&message, error, error_capacity) };
            ptr::null_mut()
        }
    }
}

/// HTTP streaming through the same linked decoder and PCM output as local
/// files. This gives the native UI real audio samples even when AVPlayer's
/// audio tap is unavailable for a progressive network asset.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kog_audio_open_stream(
    location: *const c_char, headers: *const c_char, duration_ms: u64,
    error: *mut c_char, error_capacity: usize,
) -> *mut KogAudioHandle {
    let result = (|| {
        if location.is_null() || headers.is_null() { return Err("Missing stream location".to_owned()); }
        let location = unsafe { CStr::from_ptr(location) }.to_str().map_err(|e| e.to_string())?;
        let headers = unsafe { CStr::from_ptr(headers) }.to_str().map_err(|e| e.to_string())?;
        PcmReader::open_stream(location, headers, (duration_ms > 0).then(|| Duration::from_millis(duration_ms)))
    })();
    match result {
        Ok(reader) => Box::into_raw(Box::new(KogAudioHandle { reader, score_source: None })),
        Err(message) => { unsafe { error_to_buffer(&message, error, error_capacity) }; ptr::null_mut() }
    }
}

/// Decode a platform-provided stream (URLSession on iOS) with the shared codec.
/// Ownership of context always transfers to the decoder's close callback.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kog_audio_open_reader(
    read: kog_audio::ffmpeg::StreamRead, close: kog_audio::ffmpeg::StreamClose,
    context: *mut std::ffi::c_void, duration_ms: u64,
    error: *mut c_char, error_capacity: usize,
) -> *mut KogAudioHandle {
    match unsafe { kog_audio::ffmpeg::Ffmpeg::open_reader(read, close, context) } {
        Ok(decoder) => Box::into_raw(Box::new(KogAudioHandle {
            reader: PcmReader::from_stream_decoder(decoder, (duration_ms > 0).then(|| Duration::from_millis(duration_ms))),
            score_source: None,
        })),
        Err(message) => { unsafe { error_to_buffer(&message, error, error_capacity) }; ptr::null_mut() }
    }
}

/// The caller serializes this with read/seek and supplies the audible position.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kog_audio_channel_snapshot(handle: *const KogAudioHandle, position_ms: u64, playing: bool) -> *mut c_char {
    let snapshot = (unsafe { handle.as_ref() }).map(|handle| handle.reader.channel_snapshot(Duration::from_millis(position_ms), playing)).unwrap_or_default();
    serde_json::to_string(&snapshot).ok().and_then(|json| CString::new(json).ok()).map_or(ptr::null_mut(), CString::into_raw)
}

/// The local track's MML score as `/api/mml`-style JSON, or null for a stream.
/// Recording starts on the first call; `have` is the revision already held,
/// or -1. Free the reply with `kog_audio_string_free`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kog_audio_mml(handle: *const KogAudioHandle, have: i64, bars: u32) -> *mut c_char {
    let Some((path, subsong, settings)) =
        (unsafe { handle.as_ref() }).and_then(|handle| handle.score_source.clone())
    else {
        return ptr::null_mut();
    };
    let key = format!("{}\0{subsong:?}\0{}", path.display(), settings.midi_engine().setting_value());
    let title = path.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_default();
    let status = MML.get_or_init(Default::default).status_with(
        key,
        u64::try_from(have).ok(),
        (bars as usize).clamp(1, 64),
        move |progress, partial| {
            let mut pcm = PcmReader::open_path_subsong(path, subsong, settings)?;
            kog_audio::inspection::score::record(
                &mut pcm,
                &title,
                progress,
                kog_audio::inspection::score::MAX_SECONDS,
                partial,
            )
        },
    );
    serde_json::to_string(&status).ok().and_then(|json| CString::new(json).ok()).map_or(ptr::null_mut(), CString::into_raw)
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
        let engine = std::ffi::CString::new("opl3windows").unwrap();
        let empty = std::ffi::CString::new("").unwrap();
        let mut error = [0_i8; 256];
        let handle = unsafe {
            kog_audio_open(
                path.as_ptr(), -1, engine.as_ptr(), empty.as_ptr(), empty.as_ptr(),
                empty.as_ptr(), error.as_mut_ptr(), error.len(),
            )
        };
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

#[cfg(test)]
mod mml_tests {
    use super::*;

    #[test]
    fn local_handles_record_an_mml_score() {
        let path = CString::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../native/game-music-emu/test.nsf")).unwrap();
        let engine = CString::new("opl3windows").unwrap();
        let mut error = [0 as c_char; 256];
        let handle = unsafe {
            kog_audio_open(path.as_ptr(), -1, engine.as_ptr(), ptr::null(), ptr::null(), ptr::null(), error.as_mut_ptr(), error.len())
        };
        assert!(!handle.is_null());
        let deadline = std::time::Instant::now() + Duration::from_secs(120);
        let reply = loop {
            let json = unsafe { kog_audio_mml(handle, -1, 4) };
            assert!(!json.is_null());
            let reply: serde_json::Value = serde_json::from_str(unsafe { CStr::from_ptr(json) }.to_str().unwrap()).unwrap();
            unsafe { crate::catalog::kog_audio_string_free(json) };
            if reply["document"].is_object() || std::time::Instant::now() > deadline {
                break reply;
            }
            std::thread::sleep(Duration::from_millis(50));
        };
        assert!(reply["document"]["text"].as_str().unwrap().starts_with("#KOG-MML 1"), "{reply}");
        unsafe { kog_audio_close(handle) };
    }
}
