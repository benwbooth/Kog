//! Safe ownership wrapper around psflib and mGBA for GSF playback.

use std::ffi::{CStr, CString, c_char};
use std::path::Path;
use std::ptr::NonNull;
use std::time::Duration;

#[repr(C)]
struct NativeGsf {
    _private: [u8; 0],
}

unsafe extern "C" {
    fn kog_gsf_open(
        path: *const c_char,
        default_length_milliseconds: u32,
        default_fade_milliseconds: u32,
    ) -> *mut NativeGsf;
    fn kog_gsf_free(decoder: *mut NativeGsf);
    fn kog_gsf_sample_rate(decoder: *const NativeGsf) -> u32;
    fn kog_gsf_channels(decoder: *const NativeGsf) -> u32;
    fn kog_gsf_total_frames(decoder: *const NativeGsf) -> u64;
    fn kog_gsf_title(decoder: *const NativeGsf) -> *const c_char;
    fn kog_gsf_artist(decoder: *const NativeGsf) -> *const c_char;
    fn kog_gsf_album(decoder: *const NativeGsf) -> *const c_char;
    fn kog_gsf_genre(decoder: *const NativeGsf) -> *const c_char;
    fn kog_gsf_date(decoder: *const NativeGsf) -> *const c_char;
    fn kog_gsf_render(decoder: *mut NativeGsf, output: *mut f32, frames: usize) -> i64;
    fn kog_gsf_seek(decoder: *mut NativeGsf, frame: u64) -> i64;
    fn kog_gsf_last_error() -> *const c_char;
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GsfMetadata {
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub genre: Option<String>,
    pub date: Option<String>,
}

pub struct Gsf {
    handle: NonNull<NativeGsf>,
    sample_rate: u32,
    channels: u16,
    total_frames: u64,
    metadata: GsfMetadata,
}

impl Gsf {
    pub fn open(
        path: &Path,
        default_length: Duration,
        default_fade: Duration,
    ) -> Result<Self, String> {
        let path_text = path
            .to_str()
            .ok_or_else(|| format!("GSF path is not valid Unicode: {}", path.display()))?;
        let path_c = CString::new(path_text)
            .map_err(|_| format!("GSF path contains a NUL byte: {}", path.display()))?;
        let default_length_milliseconds = duration_milliseconds(default_length, "length")?;
        let default_fade_milliseconds = duration_milliseconds(default_fade, "fade")?;
        let handle = NonNull::new(unsafe {
            kog_gsf_open(
                path_c.as_ptr(),
                default_length_milliseconds,
                default_fade_milliseconds,
            )
        })
        .ok_or_else(|| format!("opening {} as GSF: {}", path.display(), last_error()))?;

        let sample_rate = unsafe { kog_gsf_sample_rate(handle.as_ptr()) };
        let channels = u16::try_from(unsafe { kog_gsf_channels(handle.as_ptr()) })
            .ok()
            .filter(|channels| *channels == 2)
            .ok_or_else(|| {
                unsafe { kog_gsf_free(handle.as_ptr()) };
                format!("mGBA reported invalid channels for {}", path.display())
            })?;
        let total_frames = unsafe { kog_gsf_total_frames(handle.as_ptr()) };
        if sample_rate == 0 || total_frames == 0 {
            unsafe { kog_gsf_free(handle.as_ptr()) };
            return Err(format!(
                "mGBA reported invalid stream properties for {}",
                path.display()
            ));
        }
        let metadata = GsfMetadata {
            title: native_text(unsafe { kog_gsf_title(handle.as_ptr()) }),
            artist: native_text(unsafe { kog_gsf_artist(handle.as_ptr()) }),
            album: native_text(unsafe { kog_gsf_album(handle.as_ptr()) }),
            genre: native_text(unsafe { kog_gsf_genre(handle.as_ptr()) }),
            date: native_text(unsafe { kog_gsf_date(handle.as_ptr()) }),
        };

        Ok(Self {
            handle,
            sample_rate,
            channels,
            total_frames,
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

    pub fn metadata(&self) -> &GsfMetadata {
        &self.metadata
    }

    pub fn render(&mut self, output: &mut [f32]) -> Result<usize, String> {
        let channels = usize::from(self.channels);
        if output.is_empty() || !output.len().is_multiple_of(channels) {
            return Err(format!(
                "GSF output must contain complete {}-channel frames",
                self.channels
            ));
        }
        let rendered = unsafe {
            kog_gsf_render(
                self.handle.as_ptr(),
                output.as_mut_ptr(),
                output.len() / channels,
            )
        };
        usize::try_from(rendered).map_err(|_| format!("mGBA render failed: {}", last_error()))
    }

    pub fn seek(&mut self, position: Duration) -> Result<Duration, String> {
        let target = if position >= self.duration() {
            self.total_frames
        } else {
            frames_from_duration(position, self.sample_rate)?
        };
        let actual = unsafe { kog_gsf_seek(self.handle.as_ptr(), target) };
        let actual =
            u64::try_from(actual).map_err(|_| format!("mGBA seek failed: {}", last_error()))?;
        Ok(duration_from_frames(actual, self.sample_rate))
    }
}

unsafe impl Send for Gsf {}

impl Drop for Gsf {
    fn drop(&mut self) {
        unsafe { kog_gsf_free(self.handle.as_ptr()) };
    }
}

fn duration_milliseconds(duration: Duration, label: &str) -> Result<u32, String> {
    u32::try_from(duration.as_millis())
        .map_err(|_| format!("default GSF {label} exceeds the native API limit"))
}

fn native_text(value: *const c_char) -> Option<String> {
    if value.is_null() {
        return None;
    }
    let value = crate::text_encoding::decode(unsafe { CStr::from_ptr(value) }.to_bytes())
        .trim_matches(|character: char| character.is_whitespace() || character == '\0')
        .to_owned();
    (!value.is_empty()).then_some(value)
}

fn last_error() -> String {
    native_text(unsafe { kog_gsf_last_error() }).unwrap_or_else(|| "unknown GSF error".to_owned())
}

fn frames_from_duration(duration: Duration, sample_rate: u32) -> Result<u64, String> {
    let frames = duration.as_nanos().saturating_mul(u128::from(sample_rate)) / 1_000_000_000;
    u64::try_from(frames).map_err(|_| "GSF duration exceeds Kog's limit".to_owned())
}

fn duration_from_frames(frames: u64, sample_rate: u32) -> Duration {
    let seconds = frames / u64::from(sample_rate);
    let remainder = frames % u64::from(sample_rate);
    let nanos = remainder * 1_000_000_000 / u64::from(sample_rate);
    Duration::new(
        seconds,
        u32::try_from(nanos).expect("subsecond GSF duration fits u32"),
    )
}

#[cfg(test)]
pub fn test_gsf_bytes(rom: Option<&[u8]>, tags: &str) -> Vec<u8> {
    let executable = rom.map_or_else(Vec::new, |rom| {
        let mut executable = Vec::with_capacity(12 + rom.len());
        executable.extend_from_slice(&0_u32.to_le_bytes());
        executable.extend_from_slice(&0_u32.to_le_bytes());
        executable.extend_from_slice(&u32::try_from(rom.len()).unwrap().to_le_bytes());
        executable.extend_from_slice(rom);
        executable
    });
    test_raw_gsf_bytes(&executable, tags)
}

#[cfg(test)]
pub fn test_raw_gsf_bytes(executable: &[u8], tags: &str) -> Vec<u8> {
    let compressed = stored_zlib(executable);
    let mut output = Vec::new();
    output.extend_from_slice(b"PSF\x22");
    output.extend_from_slice(&0_u32.to_le_bytes());
    output.extend_from_slice(&u32::try_from(compressed.len()).unwrap().to_le_bytes());
    output.extend_from_slice(&crc32(&compressed).to_le_bytes());
    output.extend_from_slice(&compressed);
    output.extend_from_slice(b"[TAG]");
    output.extend_from_slice(tags.as_bytes());
    output
}

#[cfg(test)]
pub fn test_gba_rom() -> Vec<u8> {
    let mut rom = vec![0_u8; 0x100];
    patch_u32(&mut rom, 0, 0xea00_002e);
    rom[0xa0..0xac].copy_from_slice(b"KOG GSF TEST");
    rom[0xac..0xb0].copy_from_slice(b"KGTE");
    rom[0xb0..0xb2].copy_from_slice(b"KO");
    rom[0xb2] = 0x96;

    let program = [
        0xe3a0_0301_u32, // mov r0, #0x04000000
        0xe3a0_1080,     // mov r1, #0x80
        0xe1c0_18b4,     // strh r1, [r0, #0x84]
        0xe59f_1014,     // ldr r1, [pc, #0x14]
        0xe580_1080,     // str r1, [r0, #0x80]
        0xe59f_1010,     // ldr r1, [pc, #0x10]
        0xe580_1060,     // str r1, [r0, #0x60]
        0xe59f_100c,     // ldr r1, [pc, #0x0c]
        0xe1c0_16b4,     // strh r1, [r0, #0x64]
        0xeaff_fffe,     // b .
        0x0000_ff77,     // full PSG volume, channel 1 to both outputs
        0xf080_0000,     // 50% duty, maximum initial envelope volume
        0x0000_86d6,     // restart channel 1 near A4
    ];
    for (index, instruction) in program.into_iter().enumerate() {
        patch_u32(&mut rom, 0xc0 + index * 4, instruction);
    }
    rom
}

#[cfg(test)]
fn patch_u32(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

#[cfg(test)]
fn stored_zlib(data: &[u8]) -> Vec<u8> {
    assert!(data.len() <= usize::from(u16::MAX));
    let length = u16::try_from(data.len()).unwrap();
    let mut output = vec![0x78, 0x01, 0x01];
    output.extend_from_slice(&length.to_le_bytes());
    output.extend_from_slice(&(!length).to_le_bytes());
    output.extend_from_slice(data);
    output.extend_from_slice(&adler32(data).to_be_bytes());
    output
}

#[cfg(test)]
fn adler32(bytes: &[u8]) -> u32 {
    let mut a = 1_u32;
    let mut b = 0_u32;
    for byte in bytes {
        a = (a + u32::from(*byte)) % 65_521;
        b = (b + a) % 65_521;
    }
    (b << 16) | a
}

#[cfg(test)]
fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = u32::MAX;
    for byte in bytes {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            let mask = 0_u32.wrapping_sub(crc & 1);
            crc = (crc >> 1) ^ (0xedb8_8320 & mask);
        }
    }
    !crc
}
