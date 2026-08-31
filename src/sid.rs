//! Safe ownership wrapper around Cog's pinned libsidplayfp and reSIDfp core.

use std::ffi::{CStr, c_char};
use std::path::Path;
use std::ptr::NonNull;
use std::time::Duration;

#[repr(C)]
struct NativeSid {
    _private: [u8; 0],
}

unsafe extern "C" {
    fn kog_sid_open(
        data: *const u8,
        data_size: usize,
        subsong: u32,
        sample_rate: u32,
        play_seconds: u32,
        fade_milliseconds: u32,
    ) -> *mut NativeSid;
    fn kog_sid_free(decoder: *mut NativeSid);
    fn kog_sid_sample_rate(decoder: *const NativeSid) -> u32;
    fn kog_sid_channels(decoder: *const NativeSid) -> u32;
    fn kog_sid_subsong_count(decoder: *const NativeSid) -> u32;
    fn kog_sid_selected_subsong(decoder: *const NativeSid) -> u32;
    fn kog_sid_total_frames(decoder: *const NativeSid) -> u64;
    fn kog_sid_title(decoder: *const NativeSid) -> *const c_char;
    fn kog_sid_artist(decoder: *const NativeSid) -> *const c_char;
    fn kog_sid_released(decoder: *const NativeSid) -> *const c_char;
    fn kog_sid_format(decoder: *const NativeSid) -> *const c_char;
    fn kog_sid_render(decoder: *mut NativeSid, output: *mut f32, frames: usize) -> i64;
    fn kog_sid_seek(decoder: *mut NativeSid, frame: u64) -> i64;
    fn kog_sid_last_error() -> *const c_char;
    #[cfg(test)]
    fn kog_sid_version() -> *const c_char;
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SidMetadata {
    pub title: Option<String>,
    pub artist: Option<String>,
    pub released: Option<String>,
}

pub struct Sid {
    handle: NonNull<NativeSid>,
    sample_rate: u32,
    channels: u16,
    total_frames: u64,
    subsong_count: u32,
    selected_subsong: u32,
    codec: String,
    metadata: SidMetadata,
}

impl Sid {
    pub fn open(
        path: &Path,
        subsong: u32,
        sample_rate: u32,
        play_length: Duration,
        fade: Duration,
    ) -> Result<Self, String> {
        let data =
            std::fs::read(path).map_err(|error| format!("reading {}: {error}", path.display()))?;
        Self::from_bytes(&data, path, subsong, sample_rate, play_length, fade)
    }

    fn from_bytes(
        data: &[u8],
        path: &Path,
        subsong: u32,
        sample_rate: u32,
        play_length: Duration,
        fade: Duration,
    ) -> Result<Self, String> {
        let play_seconds = u32::try_from(play_length.as_secs())
            .map_err(|_| "SID play length exceeds the native API limit".to_owned())?;
        if play_seconds == 0 || play_length.subsec_nanos() != 0 {
            return Err("SID play length must be a whole positive number of seconds".to_owned());
        }
        let fade_milliseconds = u32::try_from(fade.as_millis())
            .map_err(|_| "SID fade length exceeds the native API limit".to_owned())?;
        let handle = NonNull::new(unsafe {
            kog_sid_open(
                data.as_ptr(),
                data.len(),
                subsong,
                sample_rate,
                play_seconds,
                fade_milliseconds,
            )
        })
        .ok_or_else(|| {
            let error = last_error();
            format!("opening {} with libsidplayfp: {error}", path.display())
        })?;

        let actual_sample_rate = unsafe { kog_sid_sample_rate(handle.as_ptr()) };
        let channels = u16::try_from(unsafe { kog_sid_channels(handle.as_ptr()) })
            .ok()
            .filter(|channels| matches!(channels, 1 | 2))
            .ok_or_else(|| {
                unsafe { kog_sid_free(handle.as_ptr()) };
                format!(
                    "libsidplayfp reported an invalid channel count for {}",
                    path.display()
                )
            })?;
        let total_frames = unsafe { kog_sid_total_frames(handle.as_ptr()) };
        let subsong_count = unsafe { kog_sid_subsong_count(handle.as_ptr()) };
        if actual_sample_rate == 0 || total_frames == 0 || subsong_count == 0 {
            unsafe { kog_sid_free(handle.as_ptr()) };
            return Err(format!(
                "libsidplayfp reported invalid stream properties for {}",
                path.display()
            ));
        }
        let selected_subsong = unsafe { kog_sid_selected_subsong(handle.as_ptr()) };
        let codec = native_text(unsafe { kog_sid_format(handle.as_ptr()) })
            .unwrap_or_else(|| "SID through reSIDfp".to_owned());
        let metadata = SidMetadata {
            title: native_text(unsafe { kog_sid_title(handle.as_ptr()) }),
            artist: native_text(unsafe { kog_sid_artist(handle.as_ptr()) }),
            released: native_text(unsafe { kog_sid_released(handle.as_ptr()) }),
        };

        Ok(Self {
            handle,
            sample_rate: actual_sample_rate,
            channels,
            total_frames,
            subsong_count,
            selected_subsong,
            codec,
            metadata,
        })
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

    pub fn codec(&self) -> &str {
        &self.codec
    }

    pub fn metadata(&self) -> &SidMetadata {
        &self.metadata
    }

    pub fn render(&mut self, output: &mut [f32]) -> Result<usize, String> {
        let channels = usize::from(self.channels);
        if output.is_empty() || !output.len().is_multiple_of(channels) {
            return Err(format!(
                "SID output must contain complete {}-channel frames",
                self.channels
            ));
        }
        let rendered = unsafe {
            kog_sid_render(
                self.handle.as_ptr(),
                output.as_mut_ptr(),
                output.len() / channels,
            )
        };
        usize::try_from(rendered)
            .map_err(|_| format!("libsidplayfp render failed: {}", last_error()))
    }

    pub fn seek(&mut self, position: Duration) -> Result<Duration, String> {
        let target = frames_from_duration(position.min(self.duration()), self.sample_rate)?;
        let actual = unsafe { kog_sid_seek(self.handle.as_ptr(), target) };
        let actual = u64::try_from(actual)
            .map_err(|_| format!("libsidplayfp seek failed: {}", last_error()))?;
        Ok(duration_from_frames(actual, self.sample_rate))
    }

    #[cfg(test)]
    fn version() -> Option<String> {
        native_text(unsafe { kog_sid_version() })
    }
}

unsafe impl Send for Sid {}

impl Drop for Sid {
    fn drop(&mut self) {
        unsafe { kog_sid_free(self.handle.as_ptr()) };
    }
}

fn native_text(value: *const c_char) -> Option<String> {
    if value.is_null() {
        return None;
    }
    let value = unsafe { CStr::from_ptr(value) }
        .to_string_lossy()
        .trim_matches(|character: char| character.is_whitespace() || character == '\0')
        .to_owned();
    (!value.is_empty()).then_some(value)
}

fn last_error() -> String {
    native_text(unsafe { kog_sid_last_error() }).unwrap_or_else(|| "unknown SID error".to_owned())
}

fn frames_from_duration(duration: Duration, sample_rate: u32) -> Result<u64, String> {
    let frames = duration.as_nanos().saturating_mul(u128::from(sample_rate)) / 1_000_000_000;
    u64::try_from(frames).map_err(|_| "SID duration exceeds Kog's limit".to_owned())
}

fn duration_from_frames(frames: u64, sample_rate: u32) -> Duration {
    let seconds = frames / u64::from(sample_rate);
    let remainder = frames % u64::from(sample_rate);
    let nanos = remainder * 1_000_000_000 / u64::from(sample_rate);
    Duration::new(
        seconds,
        u32::try_from(nanos).expect("subsecond SID duration fits u32"),
    )
}

#[cfg(test)]
pub fn test_psid_bytes(real_c64: bool) -> Vec<u8> {
    let code = [
        0xa9, 0x34, 0x8d, 0x00, 0xd4, 0xa9, 0x12, 0x8d, 0x01, 0xd4, 0xa9, 0x00, 0x8d, 0x02, 0xd4,
        0xa9, 0x08, 0x8d, 0x03, 0xd4, 0xa9, 0x00, 0x8d, 0x05, 0xd4, 0xa9, 0xf0, 0x8d, 0x06, 0xd4,
        0xa9, 0x21, 0x8d, 0x04, 0xd4, 0xa9, 0x0f, 0x8d, 0x18, 0xd4, 0x60, 0xee, 0x00, 0xd4, 0x60,
    ];
    let mut data = vec![0_u8; 124];
    data[0..4].copy_from_slice(if real_c64 { b"RSID" } else { b"PSID" });
    data[4..6].copy_from_slice(&2_u16.to_be_bytes());
    data[6..8].copy_from_slice(&124_u16.to_be_bytes());
    data[8..10].copy_from_slice(&(if real_c64 { 0 } else { 0x1000_u16 }).to_be_bytes());
    data[10..12].copy_from_slice(&0x1000_u16.to_be_bytes());
    data[12..14].copy_from_slice(&(if real_c64 { 0 } else { 0x102a_u16 }).to_be_bytes());
    data[14..16].copy_from_slice(&2_u16.to_be_bytes());
    data[16..18].copy_from_slice(&1_u16.to_be_bytes());
    data[22..38].copy_from_slice(b"Kog SID fixture ");
    data[54..65].copy_from_slice(b"Kog tests  ");
    data[86..102].copy_from_slice(b"2026 synthetic  ");
    data[118..120].copy_from_slice(&(if real_c64 { 0 } else { 0x16_u16 }).to_be_bytes());
    if real_c64 {
        data.extend_from_slice(&0x1000_u16.to_le_bytes());
    }
    data.extend_from_slice(&code);
    data
}

#[cfg(test)]
fn test_stereo_psid_bytes() -> Vec<u8> {
    let mut data = test_psid_bytes(false);
    data[4..6].copy_from_slice(&3_u16.to_be_bytes());
    data[118..120].copy_from_slice(&0x56_u16.to_be_bytes());
    data[122] = 0x42;
    data
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_psid_opens_renders_seeks_and_stops_exactly() {
        let path = Path::new("generated-kog-fixture.sid");
        let data = test_psid_bytes(false);
        let mut decoder = Sid::from_bytes(
            &data,
            path,
            1,
            44_100,
            Duration::from_secs(1),
            Duration::from_millis(100),
        )
        .expect("open generated PSID");
        assert_eq!(Sid::version().as_deref(), Some("2.4.0a"));
        assert_eq!(decoder.sample_rate(), 44_100);
        assert_eq!(decoder.channels(), 1);
        assert_eq!(decoder.subsong_count(), 2);
        assert_eq!(decoder.selected_subsong(), 1);
        assert_eq!(decoder.duration(), Duration::from_millis(1_100));
        assert_eq!(decoder.codec(), "PlaySID one-file format (PSID)");
        assert_eq!(decoder.metadata().title.as_deref(), Some("Kog SID fixture"));
        assert_eq!(decoder.metadata().artist.as_deref(), Some("Kog tests"));

        let mut pcm = vec![0.0_f32; 4_096];
        assert_eq!(decoder.render(&mut pcm).expect("render PSID"), 4_096);
        assert!(pcm.iter().any(|sample| sample.abs() > 0.000_01));
        decoder
            .seek(Duration::from_millis(500))
            .expect("seek generated PSID");
        pcm.fill(0.0);
        assert_eq!(decoder.render(&mut pcm).expect("render after seek"), 4_096);
        assert!(pcm.iter().any(|sample| sample.abs() > 0.000_01));

        decoder
            .seek(Duration::from_millis(1_090))
            .expect("seek PSID tail");
        assert_eq!(decoder.render(&mut pcm).expect("render PSID tail"), 441);
        assert_eq!(decoder.render(&mut pcm).expect("render PSID end"), 0);
    }

    #[test]
    fn real_c64_tune_reports_the_missing_user_rom_policy() {
        let error = Sid::from_bytes(
            &test_psid_bytes(true),
            Path::new("generated-rsid.sid"),
            0,
            44_100,
            Duration::from_secs(1),
            Duration::ZERO,
        )
        .err()
        .expect("RSID must require user ROMs");
        assert!(
            error.contains("requires original C64 ROM images"),
            "{error}"
        );
    }

    #[test]
    fn multi_sid_header_selects_stereo_output() {
        let mut decoder = Sid::from_bytes(
            &test_stereo_psid_bytes(),
            Path::new("generated-stereo.sid"),
            0,
            44_100,
            Duration::from_secs(1),
            Duration::ZERO,
        )
        .expect("open generated stereo PSID");
        assert_eq!(decoder.channels(), 2);
        let mut pcm = vec![0.0_f32; 8_192];
        assert_eq!(decoder.render(&mut pcm).expect("render stereo PSID"), 4_096);
        assert!(pcm.iter().any(|sample| sample.abs() > 0.000_01));
    }
}
