//! Linked FFmpeg encoder for the server's progressive AAC, Opus, and FLAC
//! streams. The same libav libraries already decode desktop audio.

use std::ffi::{CStr, c_char, c_int, c_void};
use std::io::{Read, Write};
use std::ptr::NonNull;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AudioEncoding {
    Aac,
    Opus,
    Flac,
}

impl AudioEncoding {
    fn native_id(self) -> c_int {
        match self {
            Self::Aac => 0,
            Self::Opus => 1,
            Self::Flac => 2,
        }
    }
}

type WriteCallback = unsafe extern "C" fn(*mut c_void, *const u8, c_int) -> c_int;

unsafe extern "C" {
    fn kog_ffmpeg_encoder_open(
        codec: c_int,
        bitrate_kbps: c_int,
        input_rate: c_int,
        channels: c_int,
        write: WriteCallback,
        opaque: *mut c_void,
        error: *mut c_char,
        error_capacity: usize,
    ) -> *mut c_void;
    fn kog_ffmpeg_encoder_push(
        encoder: *mut c_void,
        interleaved: *const f32,
        frames: c_int,
    ) -> c_int;
    fn kog_ffmpeg_encoder_finish(encoder: *mut c_void) -> c_int;
    fn kog_ffmpeg_encoder_error(encoder: *const c_void) -> *const c_char;
    fn kog_ffmpeg_encoder_close(encoder: *mut c_void);
}

struct Encoder(NonNull<c_void>);

impl Encoder {
    fn error(&self) -> String {
        unsafe { CStr::from_ptr(kog_ffmpeg_encoder_error(self.0.as_ptr())) }
            .to_string_lossy()
            .into_owned()
    }
}

impl Drop for Encoder {
    fn drop(&mut self) {
        unsafe { kog_ffmpeg_encoder_close(self.0.as_ptr()) }
    }
}

struct OutputSink<'a, W: Write> {
    writer: &'a mut W,
    error: Option<String>,
}

unsafe extern "C" fn write_output<W: Write>(
    opaque: *mut c_void,
    bytes: *const u8,
    count: c_int,
) -> c_int {
    if opaque.is_null() || count < 0 || (count > 0 && bytes.is_null()) {
        return -1;
    }
    if count == 0 {
        return 0;
    }
    let sink = unsafe { &mut *opaque.cast::<OutputSink<'_, W>>() };
    let data = unsafe { std::slice::from_raw_parts(bytes, count as usize) };
    match sink.writer.write_all(data) {
        Ok(()) => count,
        Err(error) => {
            sink.error = Some(error.to_string());
            -1
        }
    }
}

/// Encode interleaved little-endian f32 PCM, writing the selected container
/// progressively. The callback runs synchronously on this calling thread.
pub fn encode_to_writer<W: Write>(
    codec: AudioEncoding,
    bitrate_kbps: u16,
    sample_rate: u32,
    channels: u16,
    mut input: impl Read,
    mut output: W,
) -> Result<(), String> {
    let sample_rate = c_int::try_from(sample_rate).map_err(|_| "sample rate is too large")?;
    let channels = c_int::from(channels);
    if !(1..=8).contains(&channels) {
        return Err("invalid audio channel count".to_owned());
    }
    let mut sink = OutputSink {
        writer: &mut output,
        error: None,
    };
    let mut message = [0 as c_char; 1024];
    let raw = unsafe {
        kog_ffmpeg_encoder_open(
            codec.native_id(),
            c_int::from(bitrate_kbps),
            sample_rate,
            channels,
            write_output::<W>,
            (&raw mut sink).cast(),
            message.as_mut_ptr(),
            message.len(),
        )
    };
    let encoder = NonNull::new(raw).map(Encoder).ok_or_else(|| {
        sink.error.take().unwrap_or_else(|| {
            unsafe { CStr::from_ptr(message.as_ptr()) }
                .to_string_lossy()
                .into_owned()
        })
    })?;

    let frame_bytes = channels as usize * 4;
    let mut bytes = vec![0_u8; 64 * 1024 + frame_bytes];
    let mut pending = 0;
    let mut samples = Vec::with_capacity(64 * 1024 / 4);
    loop {
        let count = input
            .read(&mut bytes[pending..])
            .map_err(|error| format!("reading decoded PCM: {error}"))?;
        if count == 0 {
            if pending != 0 {
                return Err("decoded PCM ends in a partial frame".to_owned());
            }
            break;
        }
        let total = pending + count;
        let complete = total / frame_bytes * frame_bytes;
        samples.clear();
        samples.extend(
            bytes[..complete]
                .chunks_exact(4)
                .map(|chunk| f32::from_le_bytes(chunk.try_into().unwrap())),
        );
        if complete != 0 {
            let frames =
                c_int::try_from(complete / frame_bytes).map_err(|_| "PCM buffer is too large")?;
            let status =
                unsafe { kog_ffmpeg_encoder_push(encoder.0.as_ptr(), samples.as_ptr(), frames) };
            if status != 0 {
                return Err(sink.error.take().unwrap_or_else(|| encoder.error()));
            }
        }
        pending = total - complete;
        bytes.copy_within(complete..total, 0);
    }
    let status = unsafe { kog_ffmpeg_encoder_finish(encoder.0.as_ptr()) };
    if status != 0 {
        return Err(sink.error.take().unwrap_or_else(|| encoder.error()));
    }
    Ok(())
}
