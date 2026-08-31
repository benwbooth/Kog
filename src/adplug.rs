//! Safe ownership wrapper around Cog's pinned AdPlug and Nuked OPL3 core.

use std::ffi::{CStr, CString, c_char, c_int};
use std::path::Path;
use std::ptr::NonNull;
use std::time::Duration;

const ERROR_INVALID_ARGUMENT: c_int = 1;
const ERROR_OPEN_FAILED: c_int = 2;
const ERROR_INVALID_SUBSONG: c_int = 3;
const ERROR_OUT_OF_MEMORY: c_int = 4;

#[repr(C)]
struct NativeAdPlug {
    _private: [u8; 0],
}

unsafe extern "C" {
    fn kog_adplug_open(
        path: *const c_char,
        subsong: u32,
        sample_rate: u32,
        error: *mut c_int,
    ) -> *mut NativeAdPlug;
    fn kog_adplug_free(decoder: *mut NativeAdPlug);
    fn kog_adplug_sample_rate(decoder: *const NativeAdPlug) -> u32;
    fn kog_adplug_subsong_count(decoder: *const NativeAdPlug) -> u32;
    fn kog_adplug_total_frames(decoder: *const NativeAdPlug) -> u64;
    fn kog_adplug_type(decoder: *const NativeAdPlug) -> *const c_char;
    fn kog_adplug_title(decoder: *const NativeAdPlug) -> *const c_char;
    fn kog_adplug_author(decoder: *const NativeAdPlug) -> *const c_char;
    fn kog_adplug_render(decoder: *mut NativeAdPlug, output: *mut f32, frames: usize) -> i64;
    fn kog_adplug_seek(decoder: *mut NativeAdPlug, frame: u64) -> i64;
    fn kog_adplug_supports_extension(extension: *const c_char) -> c_int;
    #[cfg(test)]
    fn kog_adplug_extension_count() -> usize;
    #[cfg(test)]
    fn kog_adplug_extension(index: usize) -> *const c_char;
    #[cfg(test)]
    fn kog_adplug_version() -> *const c_char;
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AdPlugMetadata {
    pub title: Option<String>,
    pub artist: Option<String>,
}

pub struct AdPlug {
    handle: NonNull<NativeAdPlug>,
    sample_rate: u32,
    total_frames: u64,
    subsong_count: u32,
    selected_subsong: u32,
    codec: String,
    metadata: AdPlugMetadata,
}

impl AdPlug {
    pub fn open(path: &Path, subsong: u32, sample_rate: u32) -> Result<Self, String> {
        let native_path = native_path(path)?;
        let mut error = 0;
        let handle = NonNull::new(unsafe {
            kog_adplug_open(native_path.as_ptr(), subsong, sample_rate, &mut error)
        })
        .ok_or_else(|| native_error(path, subsong, error))?;

        let actual_sample_rate = unsafe { kog_adplug_sample_rate(handle.as_ptr()) };
        let total_frames = unsafe { kog_adplug_total_frames(handle.as_ptr()) };
        let subsong_count = unsafe { kog_adplug_subsong_count(handle.as_ptr()) };
        if actual_sample_rate == 0 || total_frames == 0 || subsong_count == 0 {
            unsafe { kog_adplug_free(handle.as_ptr()) };
            return Err(format!(
                "AdPlug reported invalid stream properties for {}",
                path.display()
            ));
        }
        let codec = native_text(unsafe { kog_adplug_type(handle.as_ptr()) })
            .unwrap_or_else(|| "AdPlug OPL synthesis".to_owned());
        let metadata = AdPlugMetadata {
            title: native_text(unsafe { kog_adplug_title(handle.as_ptr()) }),
            artist: native_text(unsafe { kog_adplug_author(handle.as_ptr()) }),
        };

        Ok(Self {
            handle,
            sample_rate: actual_sample_rate,
            total_frames,
            subsong_count,
            selected_subsong: subsong,
            codec,
            metadata,
        })
    }

    pub fn supports_extension(extension: &str) -> bool {
        let Ok(extension) = CString::new(extension) else {
            return false;
        };
        unsafe { kog_adplug_supports_extension(extension.as_ptr()) != 0 }
    }

    pub fn duration(&self) -> Duration {
        duration_from_frames(self.total_frames, self.sample_rate)
    }

    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    pub fn subsong_count(&self) -> u32 {
        self.subsong_count
    }

    pub fn selected_subsong(&self) -> u32 {
        self.selected_subsong
    }

    pub fn codec(&self) -> &str {
        &self.codec
    }

    pub fn metadata(&self) -> &AdPlugMetadata {
        &self.metadata
    }

    pub fn render(&mut self, output: &mut [f32]) -> Result<usize, String> {
        if output.is_empty() || !output.len().is_multiple_of(2) {
            return Err("AdPlug output must contain complete stereo frames".to_owned());
        }
        let rendered = unsafe {
            kog_adplug_render(self.handle.as_ptr(), output.as_mut_ptr(), output.len() / 2)
        };
        usize::try_from(rendered).map_err(|_| "AdPlug failed while rendering PCM".to_owned())
    }

    pub fn seek(&mut self, position: Duration) -> Result<Duration, String> {
        let target = frames_from_duration(position.min(self.duration()), self.sample_rate)?;
        let actual = unsafe { kog_adplug_seek(self.handle.as_ptr(), target) };
        let actual = u64::try_from(actual).map_err(|_| "AdPlug failed while seeking".to_owned())?;
        Ok(duration_from_frames(actual, self.sample_rate))
    }

    #[cfg(test)]
    pub fn supported_extensions() -> Vec<String> {
        let count = unsafe { kog_adplug_extension_count() };
        (0..count)
            .filter_map(|index| native_text(unsafe { kog_adplug_extension(index) }))
            .collect()
    }

    #[cfg(test)]
    pub fn version() -> Option<String> {
        native_text(unsafe { kog_adplug_version() })
    }
}

unsafe impl Send for AdPlug {}

impl Drop for AdPlug {
    fn drop(&mut self) {
        unsafe { kog_adplug_free(self.handle.as_ptr()) };
    }
}

fn native_text(value: *const c_char) -> Option<String> {
    if value.is_null() {
        return None;
    }
    let value = unsafe { CStr::from_ptr(value) }
        .to_string_lossy()
        .trim()
        .to_owned();
    (!value.is_empty()).then_some(value)
}

#[cfg(unix)]
fn native_path(path: &Path) -> Result<CString, String> {
    use std::os::unix::ffi::OsStrExt;
    CString::new(path.as_os_str().as_bytes())
        .map_err(|_| format!("AdPlug path contains a NUL byte: {}", path.display()))
}

#[cfg(not(unix))]
fn native_path(path: &Path) -> Result<CString, String> {
    let path_text = path.to_string_lossy();
    CString::new(path_text.as_bytes())
        .map_err(|_| format!("AdPlug path contains a NUL byte: {}", path.display()))
}

fn native_error(path: &Path, subsong: u32, error: c_int) -> String {
    match error {
        ERROR_INVALID_ARGUMENT => format!("invalid AdPlug options for {}", path.display()),
        ERROR_OPEN_FAILED => format!("AdPlug could not open {}", path.display()),
        ERROR_INVALID_SUBSONG => format!(
            "{} does not contain AdPlug subsong {}",
            path.display(),
            u64::from(subsong) + 1
        ),
        ERROR_OUT_OF_MEMORY => format!("AdPlug ran out of memory opening {}", path.display()),
        _ => format!("AdPlug failed to open {}", path.display()),
    }
}

fn frames_from_duration(duration: Duration, sample_rate: u32) -> Result<u64, String> {
    let frames = duration.as_nanos().saturating_mul(u128::from(sample_rate)) / 1_000_000_000;
    u64::try_from(frames).map_err(|_| "AdPlug duration exceeds Kog's limit".to_owned())
}

fn duration_from_frames(frames: u64, sample_rate: u32) -> Duration {
    let seconds = frames / u64::from(sample_rate);
    let remainder = frames % u64::from(sample_rate);
    let nanos = remainder * 1_000_000_000 / u64::from(sample_rate);
    Duration::new(
        seconds,
        u32::try_from(nanos).expect("subsecond AdPlug duration fits u32"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_path() -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("native/adplug/test/2.CMF")
    }

    #[test]
    fn cog_pinned_cmf_fixture_opens_renders_and_seeks() {
        let path = fixture_path();
        let mut decoder = AdPlug::open(&path, 0, 44_100).expect("open AdPlug CMF fixture");
        assert_eq!(decoder.sample_rate(), 44_100);
        assert_eq!(decoder.subsong_count(), 1);
        assert_eq!(decoder.selected_subsong(), 0);
        assert!(decoder.duration() > Duration::from_secs(1));
        assert_eq!(decoder.codec(), "Creative Music File (CMF)");

        let mut pcm = vec![0.0_f32; 4_096];
        assert_eq!(decoder.render(&mut pcm).expect("render CMF"), 2_048);
        assert!(pcm.iter().any(|sample| sample.abs() > 0.000_01));
        let seeked = decoder
            .seek(Duration::from_millis(500))
            .expect("seek CMF fixture");
        assert!(seeked >= Duration::from_millis(499));
        pcm.fill(0.0);
        assert!(decoder.render(&mut pcm).expect("render CMF after seek") > 0);
        assert!(pcm.iter().any(|sample| sample.abs() > 0.000_01));

        let tail = decoder.duration().saturating_sub(Duration::from_millis(10));
        decoder.seek(tail).expect("seek to CMF tail");
        let mut tail_pcm = vec![0.0_f32; 44_100 * 2];
        let tail_frames = decoder.render(&mut tail_pcm).expect("render CMF tail");
        assert!(tail_frames > 0);
        assert_eq!(decoder.render(&mut tail_pcm).expect("render CMF end"), 0);
    }

    #[test]
    fn runtime_extensions_and_version_match_the_cog_pin() {
        let extensions = AdPlug::supported_extensions();
        assert_eq!(extensions.len(), 51);
        assert!(extensions.iter().any(|extension| extension == "cmf"));
        assert!(extensions.iter().any(|extension| extension == "rad"));
        assert!(AdPlug::supports_extension("CMF"));
        assert!(!AdPlug::supports_extension("wav"));
        assert_eq!(AdPlug::version().as_deref(), Some("2.3.4-beta"));
    }
}
