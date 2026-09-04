//! Safe ownership wrapper around the HivelyTracker 1.9 replayer bridge.

use std::ffi::{CStr, c_char, c_int};
use std::path::Path;
use std::ptr::NonNull;
use std::sync::Once;
use std::time::Duration;

const ERROR_INVALID_FILE: c_int = 1;
const ERROR_INVALID_SUBSONG: c_int = 2;
const ERROR_OUT_OF_MEMORY: c_int = 3;
const ERROR_DURATION_LIMIT: c_int = 4;

static HIVELY_INIT: Once = Once::new();

#[repr(C)]
struct NativeHively {
    _private: [u8; 0],
}

unsafe extern "C" {
    fn kog_hively_init();
    fn kog_hively_open(
        data: *const u8,
        data_size: usize,
        sample_rate: u32,
        subsong: u32,
        loop_count: u32,
        fade_frames: u64,
        error: *mut c_int,
    ) -> *mut NativeHively;
    fn kog_hively_free(decoder: *mut NativeHively);
    fn kog_hively_subsong_count(decoder: *const NativeHively) -> u32;
    fn kog_hively_selected_subsong(decoder: *const NativeHively) -> u32;
    fn kog_hively_title(decoder: *const NativeHively) -> *const c_char;
    fn kog_hively_total_frames(decoder: *const NativeHively) -> u64;
    fn kog_hively_render(decoder: *mut NativeHively, output: *mut f32, frames: usize) -> usize;
    fn kog_hively_seek(decoder: *mut NativeHively, frame: u64) -> u64;
}

pub struct Hively {
    handle: NonNull<NativeHively>,
    sample_rate: u32,
    total_frames: u64,
    title: String,
}

impl Hively {
    pub fn open(
        path: &Path,
        sample_rate: u32,
        subsong: Option<u32>,
        loop_count: u32,
        fade: Duration,
    ) -> Result<Self, String> {
        let data =
            std::fs::read(path).map_err(|error| format!("reading {}: {error}", path.display()))?;
        Self::from_bytes(&data, path, sample_rate, subsong, loop_count, fade)
    }

    fn from_bytes(
        data: &[u8],
        path: &Path,
        sample_rate: u32,
        subsong: Option<u32>,
        loop_count: u32,
        fade: Duration,
    ) -> Result<Self, String> {
        HIVELY_INIT.call_once(|| unsafe { kog_hively_init() });
        let requested_subsong = subsong.unwrap_or(0);
        let fade_frames = frames_from_duration(fade, sample_rate)?;
        let mut error = 0;
        let handle = NonNull::new(unsafe {
            kog_hively_open(
                data.as_ptr(),
                data.len(),
                sample_rate,
                requested_subsong,
                loop_count,
                fade_frames,
                &mut error,
            )
        })
        .ok_or_else(|| native_error(path, requested_subsong, error))?;
        let total_frames = unsafe { kog_hively_total_frames(handle.as_ptr()) };
        let title = unsafe { kog_hively_title(handle.as_ptr()) };
        let title = if title.is_null() {
            String::new()
        } else {
            crate::text_encoding::decode(unsafe { CStr::from_ptr(title) }.to_bytes())
                .trim()
                .to_owned()
        };
        Ok(Self {
            handle,
            sample_rate,
            total_frames,
            title,
        })
    }

    pub fn duration(&self) -> Duration {
        duration_from_frames(self.total_frames, self.sample_rate)
    }

    pub fn subsong_count(&self) -> u32 {
        unsafe { kog_hively_subsong_count(self.handle.as_ptr()) }
    }

    pub fn selected_subsong(&self) -> u32 {
        unsafe { kog_hively_selected_subsong(self.handle.as_ptr()) }
    }

    pub fn title(&self) -> &str {
        &self.title
    }

    pub fn render(&mut self, output: &mut [f32]) -> Result<usize, String> {
        if !output.len().is_multiple_of(2) {
            return Err("HivelyTracker output must contain stereo sample pairs".to_owned());
        }
        Ok(unsafe {
            kog_hively_render(self.handle.as_ptr(), output.as_mut_ptr(), output.len() / 2)
        })
    }

    pub fn seek(&mut self, position: Duration) -> Result<Duration, String> {
        let target = frames_from_duration(position.min(self.duration()), self.sample_rate)?;
        let actual = unsafe { kog_hively_seek(self.handle.as_ptr(), target) };
        Ok(duration_from_frames(actual, self.sample_rate))
    }
}

unsafe impl Send for Hively {}

impl Drop for Hively {
    fn drop(&mut self) {
        unsafe { kog_hively_free(self.handle.as_ptr()) };
    }
}

fn native_error(path: &Path, subsong: u32, error: c_int) -> String {
    let detail = match error {
        ERROR_INVALID_FILE => "the file is not valid AHX/HVL data",
        ERROR_INVALID_SUBSONG => {
            return format!(
                "{} requests HivelyTracker subsong {}, but it does not exist",
                path.display(),
                subsong + 1
            );
        }
        ERROR_OUT_OF_MEMORY => "the native replayer ran out of memory",
        ERROR_DURATION_LIMIT => "the song did not finish within the two-hour safety limit",
        _ => "the native replayer failed",
    };
    format!("opening {} with HivelyTracker: {detail}", path.display())
}

fn frames_from_duration(duration: Duration, sample_rate: u32) -> Result<u64, String> {
    let frames = duration.as_nanos().saturating_mul(u128::from(sample_rate)) / 1_000_000_000;
    u64::try_from(frames).map_err(|_| "HivelyTracker duration exceeds Kog's limit".to_owned())
}

fn duration_from_frames(frames: u64, sample_rate: u32) -> Duration {
    let seconds = frames / u64::from(sample_rate);
    let remainder = frames % u64::from(sample_rate);
    let nanos = remainder * 1_000_000_000 / u64::from(sample_rate);
    Duration::new(
        seconds,
        u32::try_from(nanos).expect("subsecond Hively duration fits u32"),
    )
}

#[cfg(test)]
pub fn test_multisubsong_hvl_bytes() -> Vec<u8> {
    let mut data = std::fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("native/hivelytracker/Songs/chiprolled.hvl"),
    )
    .expect("read official HivelyTracker fixture");
    assert_eq!(&data[..3], b"HVL");
    assert_eq!(data[13], 0);
    let name_offset = u16::from_be_bytes([data[4], data[5]])
        .checked_add(2)
        .expect("HVL fixture name offset fits u16");
    data[4..6].copy_from_slice(&name_offset.to_be_bytes());
    data[13] = 1;
    data.splice(16..16, [0_u8, 0]);
    data
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_path(test_name: &str) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!(
            "kog-hively-core-{}-{test_name}.hvl",
            std::process::id()
        ));
        std::fs::write(&path, test_multisubsong_hvl_bytes()).expect("write HVL fixture");
        path
    }

    #[test]
    fn official_hvl_renders_subsongs_non_silent_pcm_and_seeks() {
        let path = fixture_path("render");
        let mut decoder = Hively::open(&path, 44_100, Some(1), 2, Duration::from_secs(8))
            .expect("open HVL fixture");

        assert_eq!(decoder.subsong_count(), 2);
        assert_eq!(decoder.selected_subsong(), 1);
        assert!(!decoder.title().is_empty());
        assert!(decoder.duration() > Duration::from_secs(8));
        let mut pcm = vec![0.0_f32; 4_410 * 2];
        assert_eq!(decoder.render(&mut pcm).expect("render HVL"), 4_410);
        assert!(pcm.iter().any(|sample| sample.abs() > 0.000_01));

        assert!(decoder.seek(Duration::from_secs(1)).is_ok());
        pcm.fill(0.0);
        assert_eq!(decoder.render(&mut pcm).expect("render after seek"), 4_410);
        assert!(pcm.iter().any(|sample| sample.abs() > 0.000_01));

        decoder.seek(decoder.duration()).expect("seek to HVL end");
        assert_eq!(decoder.render(&mut pcm).expect("render at HVL end"), 0);

        std::fs::remove_file(path).ok();
    }

    #[test]
    fn official_ahx_fixture_opens_and_renders() {
        let path =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("native/hivelytracker/Songs/karma.ahx");
        let mut decoder = Hively::open(&path, 44_100, Some(0), 2, Duration::from_secs(8))
            .expect("open official AHX fixture");
        assert_eq!(decoder.subsong_count(), 1);
        assert!(decoder.duration() > Duration::from_secs(8));
        let mut pcm = vec![0.0_f32; 4_410 * 2];
        assert_eq!(decoder.render(&mut pcm).expect("render AHX"), 4_410);
        assert!(pcm.iter().any(|sample| sample.abs() > 0.000_01));
    }
}
