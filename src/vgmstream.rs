//! Safe ownership wrapper around vgmstream's stable public C API.

use std::ffi::{CStr, CString, c_char, c_int};
use std::path::Path;
use std::ptr::NonNull;
use std::time::Duration;

const ERROR_INVALID_ARGUMENT: c_int = 1;
const ERROR_OPEN_FAILED: c_int = 2;
const ERROR_DECODE_FAILED: c_int = 3;

#[repr(C)]
struct NativeVgmstream {
    _private: [u8; 0],
}

unsafe extern "C" {
    fn kog_vgmstream_open(
        path: *const c_char,
        subsong: i32,
        loop_count: f64,
        fade_seconds: f64,
        error: *mut c_int,
    ) -> *mut NativeVgmstream;
    fn kog_vgmstream_free(decoder: *mut NativeVgmstream);
    fn kog_vgmstream_sample_rate(decoder: *const NativeVgmstream) -> u32;
    fn kog_vgmstream_channels(decoder: *const NativeVgmstream) -> u32;
    fn kog_vgmstream_total_frames(decoder: *const NativeVgmstream) -> u64;
    fn kog_vgmstream_subsong_count(decoder: *const NativeVgmstream) -> u32;
    fn kog_vgmstream_selected_subsong(decoder: *const NativeVgmstream) -> u32;
    fn kog_vgmstream_bitrate(decoder: *const NativeVgmstream) -> u32;
    fn kog_vgmstream_codec(decoder: *const NativeVgmstream) -> *const c_char;
    fn kog_vgmstream_title(decoder: *const NativeVgmstream) -> *const c_char;
    fn kog_vgmstream_artist(decoder: *const NativeVgmstream) -> *const c_char;
    fn kog_vgmstream_album(decoder: *const NativeVgmstream) -> *const c_char;
    fn kog_vgmstream_year(decoder: *const NativeVgmstream) -> u32;
    fn kog_vgmstream_track_number(decoder: *const NativeVgmstream) -> u32;
    fn kog_vgmstream_render(decoder: *mut NativeVgmstream, output: *mut f32, frames: usize) -> i64;
    fn kog_vgmstream_seek(decoder: *mut NativeVgmstream, frame: u64) -> u64;
    fn kog_vgmstream_supports_extension(extension: *const c_char) -> c_int;
    #[cfg(test)]
    fn kog_vgmstream_extension_count() -> usize;
    #[cfg(test)]
    fn kog_vgmstream_extension(index: usize) -> *const c_char;
    #[cfg(test)]
    fn kog_vgmstream_api_version() -> u32;
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct VgmstreamMetadata {
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub year: Option<u32>,
    pub track_number: Option<u32>,
}

pub struct Vgmstream {
    handle: NonNull<NativeVgmstream>,
    sample_rate: u32,
    channels: u16,
    total_frames: u64,
    subsong_count: u32,
    selected_subsong: u32,
    bitrate: Option<u32>,
    codec: String,
    metadata: VgmstreamMetadata,
}

impl Vgmstream {
    pub fn open(
        path: &Path,
        subsong: Option<u32>,
        loop_count: f64,
        fade: Duration,
    ) -> Result<Self, String> {
        let native_path = native_path(path)?;
        let requested_subsong = match subsong {
            Some(value) => i32::try_from(value).map_err(|_| {
                format!(
                    "{} requests vgmstream subsong {}, which exceeds the native API limit",
                    path.display(),
                    u64::from(value) + 1
                )
            })?,
            None => -1,
        };
        let mut error = 0;
        let handle = NonNull::new(unsafe {
            kog_vgmstream_open(
                native_path.as_ptr(),
                requested_subsong,
                loop_count,
                fade.as_secs_f64(),
                &mut error,
            )
        })
        .ok_or_else(|| native_error(path, subsong, error))?;

        let sample_rate = unsafe { kog_vgmstream_sample_rate(handle.as_ptr()) };
        let native_channels = unsafe { kog_vgmstream_channels(handle.as_ptr()) };
        let channels = u16::try_from(native_channels)
            .ok()
            .filter(|channels| *channels > 0)
            .ok_or_else(|| {
                unsafe { kog_vgmstream_free(handle.as_ptr()) };
                format!(
                    "vgmstream reported an invalid channel count for {}",
                    path.display()
                )
            })?;
        let total_frames = unsafe { kog_vgmstream_total_frames(handle.as_ptr()) };
        let subsong_count = unsafe { kog_vgmstream_subsong_count(handle.as_ptr()) };
        let selected_subsong = unsafe { kog_vgmstream_selected_subsong(handle.as_ptr()) };
        let bitrate = nonzero(unsafe { kog_vgmstream_bitrate(handle.as_ptr()) });
        let codec = native_text(unsafe { kog_vgmstream_codec(handle.as_ptr()) })
            .unwrap_or_else(|| "vgmstream".to_owned());
        let metadata = VgmstreamMetadata {
            title: native_text(unsafe { kog_vgmstream_title(handle.as_ptr()) }),
            artist: native_text(unsafe { kog_vgmstream_artist(handle.as_ptr()) }),
            album: native_text(unsafe { kog_vgmstream_album(handle.as_ptr()) }),
            year: nonzero(unsafe { kog_vgmstream_year(handle.as_ptr()) }),
            track_number: nonzero(unsafe { kog_vgmstream_track_number(handle.as_ptr()) }),
        };

        Ok(Self {
            handle,
            sample_rate,
            channels,
            total_frames,
            subsong_count,
            selected_subsong,
            bitrate,
            codec,
            metadata,
        })
    }

    pub fn supports_extension(extension: &str) -> bool {
        let Ok(extension) = CString::new(extension) else {
            return false;
        };
        unsafe { kog_vgmstream_supports_extension(extension.as_ptr()) != 0 }
    }

    pub fn duration(&self) -> Duration {
        duration_from_frames(self.total_frames, self.sample_rate)
    }

    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    pub fn channels(&self) -> u16 {
        self.channels
    }

    pub fn subsong_count(&self) -> u32 {
        self.subsong_count
    }

    pub fn selected_subsong(&self) -> u32 {
        self.selected_subsong
    }

    pub fn bitrate(&self) -> Option<u32> {
        self.bitrate
    }

    pub fn codec(&self) -> &str {
        &self.codec
    }

    pub fn metadata(&self) -> &VgmstreamMetadata {
        &self.metadata
    }

    pub fn render(&mut self, output: &mut [f32]) -> Result<usize, String> {
        let channels = usize::from(self.channels);
        if output.is_empty() || !output.len().is_multiple_of(channels) {
            return Err(format!(
                "vgmstream output must contain complete {}-channel frames",
                self.channels
            ));
        }
        let frames = output.len() / channels;
        let rendered =
            unsafe { kog_vgmstream_render(self.handle.as_ptr(), output.as_mut_ptr(), frames) };
        usize::try_from(rendered).map_err(|_| "vgmstream failed while rendering PCM".to_owned())
    }

    pub fn seek(&mut self, position: Duration) -> Result<Duration, String> {
        let target = frames_from_duration(position.min(self.duration()), self.sample_rate)?;
        let actual = unsafe { kog_vgmstream_seek(self.handle.as_ptr(), target) };
        Ok(duration_from_frames(actual, self.sample_rate))
    }

    #[cfg(test)]
    pub fn supported_extensions() -> Vec<String> {
        let count = unsafe { kog_vgmstream_extension_count() };
        (0..count)
            .filter_map(|index| native_text(unsafe { kog_vgmstream_extension(index) }))
            .collect()
    }

    #[cfg(test)]
    pub fn api_version() -> u32 {
        unsafe { kog_vgmstream_api_version() }
    }
}

unsafe impl Send for Vgmstream {}

impl Drop for Vgmstream {
    fn drop(&mut self) {
        unsafe { kog_vgmstream_free(self.handle.as_ptr()) };
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
        .map_err(|_| format!("vgmstream path contains a NUL byte: {}", path.display()))
}

#[cfg(not(unix))]
fn native_path(path: &Path) -> Result<CString, String> {
    let path_text = path.to_string_lossy();
    CString::new(path_text.as_bytes())
        .map_err(|_| format!("vgmstream path contains a NUL byte: {}", path.display()))
}

fn nonzero(value: u32) -> Option<u32> {
    (value != 0).then_some(value)
}

fn native_error(path: &Path, subsong: Option<u32>, error: c_int) -> String {
    match error {
        ERROR_INVALID_ARGUMENT => {
            format!("invalid vgmstream playback options for {}", path.display())
        }
        ERROR_OPEN_FAILED => match subsong {
            Some(subsong) => format!(
                "vgmstream could not open {} as subsong {} with the enabled codec set",
                path.display(),
                subsong + 1
            ),
            None => format!(
                "vgmstream could not open {} with the enabled codec set",
                path.display()
            ),
        },
        ERROR_DECODE_FAILED => format!("vgmstream ran out of memory opening {}", path.display()),
        _ => format!("vgmstream failed to open {}", path.display()),
    }
}

fn frames_from_duration(duration: Duration, sample_rate: u32) -> Result<u64, String> {
    let frames = duration.as_nanos().saturating_mul(u128::from(sample_rate)) / 1_000_000_000;
    u64::try_from(frames).map_err(|_| "vgmstream duration exceeds Kog's limit".to_owned())
}

fn duration_from_frames(frames: u64, sample_rate: u32) -> Duration {
    let seconds = frames / u64::from(sample_rate);
    let remainder = frames % u64::from(sample_rate);
    let nanos = remainder * 1_000_000_000 / u64::from(sample_rate);
    Duration::new(
        seconds,
        u32::try_from(nanos).expect("subsecond vgmstream duration fits u32"),
    )
}

#[cfg(test)]
pub fn test_vag_bytes() -> Vec<u8> {
    const FRAMES: usize = 256;
    let data_size = FRAMES * 16;
    let mut data = vec![0_u8; 0x30 + data_size];
    data[0..4].copy_from_slice(b"VAGp");
    data[4..8].copy_from_slice(&0x20_u32.to_be_bytes());
    data[0x0c..0x10].copy_from_slice(&(data_size as u32).to_be_bytes());
    data[0x10..0x14].copy_from_slice(&44_100_u32.to_be_bytes());
    data[0x20..0x30].copy_from_slice(b"Kog VAG fixture ");
    for frame in 0..FRAMES {
        let offset = 0x30 + frame * 16;
        data[offset] = 0x04;
        data[offset + 1] = if frame + 1 == FRAMES { 0x01 } else { 0x00 };
        for byte in &mut data[offset + 2..offset + 16] {
            *byte = if frame.is_multiple_of(2) { 0x17 } else { 0xf9 };
        }
    }
    data
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_dir(test_name: &str) -> std::path::PathBuf {
        let directory = std::env::temp_dir().join(format!(
            "kog-vgmstream-core-{}-{test_name}",
            std::process::id()
        ));
        std::fs::create_dir_all(&directory).expect("create vgmstream fixture directory");
        directory
    }

    #[test]
    fn generated_vag_renders_metadata_pcm_and_seek() {
        let directory = fixture_dir("render");
        let path = directory.join("fixture.vag");
        std::fs::write(&path, test_vag_bytes()).expect("write VAG fixture");
        std::fs::write(
            directory.join("!tags.m3u"),
            "# %TITLE    Kog VGMStream Test\n# %ARTIST   Kog Fixture Artist\n# %ALBUM    Kog Fixture Album\n# %DATE     1999-07-01\n# %TRACK    7\nfixture.vag\n",
        )
        .expect("write vgmstream tags fixture");

        let mut decoder =
            Vgmstream::open(&path, Some(0), 2.0, Duration::from_secs(8)).expect("open VAG fixture");
        assert_eq!(decoder.sample_rate(), 44_100);
        assert_eq!(decoder.channels(), 1);
        assert_eq!(decoder.subsong_count(), 1);
        assert_eq!(decoder.selected_subsong(), 0);
        assert_eq!(decoder.codec(), "PlayStation 4-bit ADPCM");
        assert_eq!(
            decoder.metadata().title.as_deref(),
            Some("Kog VGMStream Test")
        );
        assert_eq!(
            decoder.metadata().artist.as_deref(),
            Some("Kog Fixture Artist")
        );
        assert_eq!(
            decoder.metadata().album.as_deref(),
            Some("Kog Fixture Album")
        );
        assert_eq!(decoder.metadata().year, Some(1999));
        assert_eq!(decoder.metadata().track_number, Some(7));
        assert!(decoder.duration() > Duration::from_millis(100));

        let mut pcm = vec![0.0_f32; 2_048];
        assert_eq!(decoder.render(&mut pcm).expect("render VAG"), 2_048);
        assert!(pcm.iter().any(|sample| sample.abs() > 0.000_01));
        decoder
            .seek(Duration::from_millis(50))
            .expect("seek VAG fixture");
        pcm.fill(0.0);
        assert!(decoder.render(&mut pcm).expect("render after seek") > 0);
        assert!(pcm.iter().any(|sample| sample.abs() > 0.000_01));

        std::fs::remove_dir_all(directory).ok();
    }

    #[test]
    fn public_api_and_runtime_extension_table_match_the_pin() {
        assert_eq!(Vgmstream::api_version(), 0x0101_0000);
        let extensions = Vgmstream::supported_extensions();
        assert!(extensions.len() > 700);
        assert!(extensions.iter().any(|extension| extension == "vag"));
        assert!(Vgmstream::supports_extension("vag"));
        assert!(!Vgmstream::supports_extension("wav"));
    }
}
