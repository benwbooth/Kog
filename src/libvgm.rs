//! Safe ownership wrapper around Kog's small libvgm C ABI.

use std::ffi::{CStr, c_char, c_void};
use std::path::Path;
use std::ptr::NonNull;
use std::time::Duration;

const ERROR_BUFFER_SIZE: usize = 512;

unsafe extern "C" {
    fn kog_libvgm_create(
        data: *const u8,
        data_size: usize,
        yrw801_rom: *const u8,
        yrw801_rom_size: usize,
        sample_rate: u32,
        loop_count: u32,
        fade_samples: u32,
        end_silence_samples: u32,
        error: *mut c_char,
        error_size: usize,
    ) -> *mut c_void;
    fn kog_libvgm_destroy(decoder: *mut c_void);
    fn kog_libvgm_total_frames(decoder: *const c_void) -> u64;
    fn kog_libvgm_title(decoder: *const c_void) -> *const c_char;
    fn kog_libvgm_artist(decoder: *const c_void) -> *const c_char;
    fn kog_libvgm_album(decoder: *const c_void) -> *const c_char;
    fn kog_libvgm_date(decoder: *const c_void) -> *const c_char;
    fn kog_libvgm_codec(decoder: *const c_void) -> *const c_char;
    fn kog_libvgm_warning(decoder: *const c_void) -> *const c_char;
    fn kog_libvgm_last_error(decoder: *const c_void) -> *const c_char;
    fn kog_libvgm_render(decoder: *mut c_void, output: *mut f32, frames: usize) -> usize;
    fn kog_libvgm_seek(decoder: *mut c_void, frame: u64) -> i32;
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LibVgmMetadata {
    pub title: String,
    pub artist: String,
    pub album: String,
    pub date: String,
    pub codec: String,
}

pub struct LibVgm {
    handle: NonNull<c_void>,
    sample_rate: u32,
    total_frames: u64,
    metadata: LibVgmMetadata,
    warning: Option<String>,
}

impl LibVgm {
    pub fn open(
        path: &Path,
        sample_rate: u32,
        loop_count: u32,
        fade: Duration,
        end_silence: Duration,
    ) -> Result<Self, String> {
        let data =
            std::fs::read(path).map_err(|error| format!("reading {}: {error}", path.display()))?;
        let rom_path = path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join("yrw801.rom");
        let (rom, rom_warning) = match std::fs::read(&rom_path) {
            Ok(data) => (data, None),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => (Vec::new(), None),
            Err(error) => (
                Vec::new(),
                Some(format!("reading optional {}: {error}", rom_path.display())),
            ),
        };
        let fade_samples = duration_samples(fade, sample_rate)?;
        let end_silence_samples = duration_samples(end_silence, sample_rate)?;
        let mut error = [0 as c_char; ERROR_BUFFER_SIZE];
        let rom_pointer = if rom.is_empty() {
            std::ptr::null()
        } else {
            rom.as_ptr()
        };
        let handle = NonNull::new(unsafe {
            kog_libvgm_create(
                data.as_ptr(),
                data.len(),
                rom_pointer,
                rom.len(),
                sample_rate,
                loop_count,
                fade_samples,
                end_silence_samples,
                error.as_mut_ptr(),
                error.len(),
            )
        })
        .ok_or_else(|| c_string(error.as_ptr()))?;

        let metadata = LibVgmMetadata {
            title: copy_native_string(unsafe { kog_libvgm_title(handle.as_ptr()) }),
            artist: copy_native_string(unsafe { kog_libvgm_artist(handle.as_ptr()) }),
            album: copy_native_string(unsafe { kog_libvgm_album(handle.as_ptr()) }),
            date: copy_native_string(unsafe { kog_libvgm_date(handle.as_ptr()) }),
            codec: copy_native_string(unsafe { kog_libvgm_codec(handle.as_ptr()) }),
        };
        let native_warning = copy_native_string(unsafe { kog_libvgm_warning(handle.as_ptr()) });
        let warning = combine_warnings(rom_warning, nonempty(native_warning));
        let total_frames = unsafe { kog_libvgm_total_frames(handle.as_ptr()) };
        Ok(Self {
            handle,
            sample_rate,
            total_frames,
            metadata,
            warning,
        })
    }

    pub fn total_frames(&self) -> u64 {
        self.total_frames
    }

    pub fn total_duration(&self) -> Duration {
        let seconds = self.total_frames() / u64::from(self.sample_rate);
        let remainder = self.total_frames() % u64::from(self.sample_rate);
        let nanoseconds = u32::try_from(
            u128::from(remainder) * 1_000_000_000_u128 / u128::from(self.sample_rate),
        )
        .expect("subsecond libvgm duration fits u32");
        Duration::new(seconds, nanoseconds)
    }

    pub fn metadata(&self) -> &LibVgmMetadata {
        &self.metadata
    }

    pub fn warning(&self) -> Option<&str> {
        self.warning.as_deref()
    }

    pub fn render(&mut self, output: &mut [f32]) -> Result<usize, String> {
        if !output.len().is_multiple_of(2) {
            return Err("libvgm output must contain stereo sample pairs".to_owned());
        }
        let frames = unsafe {
            kog_libvgm_render(self.handle.as_ptr(), output.as_mut_ptr(), output.len() / 2)
        };
        if frames == 0 {
            let error = self.last_error();
            if !error.is_empty() {
                return Err(format!("rendering libvgm audio: {error}"));
            }
        }
        Ok(frames)
    }

    pub fn seek(&mut self, position: Duration) -> Result<u64, String> {
        let position = position.min(self.total_duration());
        let frame = duration_frames(position, self.sample_rate)?;
        let result = unsafe { kog_libvgm_seek(self.handle.as_ptr(), frame) };
        if result != 0 {
            return Err(format!("seeking libvgm audio: {}", self.last_error()));
        }
        Ok(frame)
    }

    fn last_error(&self) -> String {
        copy_native_string(unsafe { kog_libvgm_last_error(self.handle.as_ptr()) })
    }
}

unsafe impl Send for LibVgm {}

impl Drop for LibVgm {
    fn drop(&mut self) {
        unsafe { kog_libvgm_destroy(self.handle.as_ptr()) };
    }
}

fn duration_samples(duration: Duration, sample_rate: u32) -> Result<u32, String> {
    let frames = duration_frames(duration, sample_rate)?;
    u32::try_from(frames).map_err(|_| "libvgm timing exceeds the native API limit".to_owned())
}

fn duration_frames(duration: Duration, sample_rate: u32) -> Result<u64, String> {
    let frames = duration.as_nanos().saturating_mul(u128::from(sample_rate)) / 1_000_000_000_u128;
    u64::try_from(frames).map_err(|_| "libvgm timing exceeds Kog's limit".to_owned())
}

fn combine_warnings(first: Option<String>, second: Option<String>) -> Option<String> {
    match (first, second) {
        (Some(first), Some(second)) => Some(format!("{first}; {second}")),
        (Some(warning), None) | (None, Some(warning)) => Some(warning),
        (None, None) => None,
    }
}

fn nonempty(value: String) -> Option<String> {
    (!value.is_empty()).then_some(value)
}

fn c_string(value: *const c_char) -> String {
    if value.is_null() {
        return "native libvgm error".to_owned();
    }
    unsafe { CStr::from_ptr(value) }
        .to_string_lossy()
        .into_owned()
}

fn copy_native_string(value: *const c_char) -> String {
    c_string(value)
}

#[cfg(test)]
pub fn test_vgm_bytes() -> Vec<u8> {
    let mut vgm = vec![0_u8; 0x40];
    vgm[0..4].copy_from_slice(b"Vgm ");
    vgm[8..12].copy_from_slice(&0x0000_0150_u32.to_le_bytes());
    vgm[12..16].copy_from_slice(&3_579_545_u32.to_le_bytes());
    vgm[24..28].copy_from_slice(&44_100_u32.to_le_bytes());
    vgm[36..40].copy_from_slice(&60_u32.to_le_bytes());
    vgm[52..56].copy_from_slice(&12_u32.to_le_bytes());

    vgm.extend_from_slice(&[
        0x50, 0x80, // SN76489 channel 0 tone period, low nibble
        0x50, 0x10, // SN76489 channel 0 tone period, high bits
        0x50, 0x90, // channel 0 at full volume
        0x50, 0xBF, // mute channel 1
        0x50, 0xDF, // mute channel 2
        0x50, 0xFF, // mute noise channel
    ]);
    vgm.extend(std::iter::repeat_n(0x62, 60));
    vgm.extend_from_slice(&[0x50, 0x9F, 0x66]);
    let eof_offset = u32::try_from(vgm.len() - 4).expect("test VGM fits u32");
    vgm[4..8].copy_from_slice(&eof_offset.to_le_bytes());
    vgm
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_vgm_renders_non_silent_pcm_and_seeks() {
        let path = std::env::temp_dir().join(format!("kog-libvgm-{}.vgm", std::process::id()));
        std::fs::write(&path, test_vgm_bytes()).expect("write VGM fixture");
        let mut decoder = LibVgm::open(
            &path,
            44_100,
            2,
            Duration::from_secs(8),
            Duration::from_millis(500),
        )
        .expect("open VGM fixture");

        assert_eq!(decoder.total_frames(), 44_100);
        assert_eq!(decoder.total_duration(), Duration::from_secs(1));
        assert_eq!(decoder.metadata().codec, "VGM v1.50");
        let mut pcm = vec![0.0_f32; 4_410 * 2];
        assert_eq!(decoder.render(&mut pcm).expect("render VGM"), 2_048);
        assert!(pcm.iter().any(|sample| sample.abs() > 0.000_01));

        assert_eq!(
            decoder.seek(Duration::from_millis(500)).expect("seek VGM"),
            22_050
        );
        pcm.fill(0.0);
        assert!(decoder.render(&mut pcm).expect("render after seek") > 0);
        assert!(pcm.iter().any(|sample| sample.abs() > 0.000_01));

        std::fs::remove_file(path).ok();
    }
}
