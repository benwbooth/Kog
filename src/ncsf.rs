//! Safe ownership wrapper around psflib and SSEQPlayer for NCSF playback.

use std::ffi::{CStr, CString, c_char};
use std::path::Path;
use std::ptr::NonNull;
use std::time::Duration;

#[repr(C)]
struct NativeNcsf {
    _private: [u8; 0],
}

unsafe extern "C" {
    fn kog_ncsf_open(
        path: *const c_char,
        default_length_milliseconds: u32,
        default_fade_milliseconds: u32,
    ) -> *mut NativeNcsf;
    fn kog_ncsf_free(decoder: *mut NativeNcsf);
    fn kog_ncsf_sample_rate(decoder: *const NativeNcsf) -> u32;
    fn kog_ncsf_channels(decoder: *const NativeNcsf) -> u32;
    fn kog_ncsf_total_frames(decoder: *const NativeNcsf) -> u64;
    fn kog_ncsf_title(decoder: *const NativeNcsf) -> *const c_char;
    fn kog_ncsf_artist(decoder: *const NativeNcsf) -> *const c_char;
    fn kog_ncsf_album(decoder: *const NativeNcsf) -> *const c_char;
    fn kog_ncsf_genre(decoder: *const NativeNcsf) -> *const c_char;
    fn kog_ncsf_date(decoder: *const NativeNcsf) -> *const c_char;
    fn kog_ncsf_render(decoder: *mut NativeNcsf, output: *mut f32, frames: usize) -> i64;
    fn kog_ncsf_seek(decoder: *mut NativeNcsf, frame: u64) -> i64;
    fn kog_ncsf_last_error() -> *const c_char;
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct NcsfMetadata {
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub genre: Option<String>,
    pub date: Option<String>,
}

pub struct Ncsf {
    handle: NonNull<NativeNcsf>,
    sample_rate: u32,
    channels: u16,
    total_frames: u64,
    metadata: NcsfMetadata,
}

impl Ncsf {
    pub fn open(
        path: &Path,
        default_length: Duration,
        default_fade: Duration,
    ) -> Result<Self, String> {
        let path_text = path
            .to_str()
            .ok_or_else(|| format!("NCSF path is not valid Unicode: {}", path.display()))?;
        let path_c = CString::new(path_text)
            .map_err(|_| format!("NCSF path contains a NUL byte: {}", path.display()))?;
        let default_length_milliseconds = duration_milliseconds(default_length, "length")?;
        let default_fade_milliseconds = duration_milliseconds(default_fade, "fade")?;
        let handle = NonNull::new(unsafe {
            kog_ncsf_open(
                path_c.as_ptr(),
                default_length_milliseconds,
                default_fade_milliseconds,
            )
        })
        .ok_or_else(|| format!("opening {} as NCSF: {}", path.display(), last_error()))?;

        let sample_rate = unsafe { kog_ncsf_sample_rate(handle.as_ptr()) };
        let channels = u16::try_from(unsafe { kog_ncsf_channels(handle.as_ptr()) })
            .ok()
            .filter(|channels| *channels == 2)
            .ok_or_else(|| {
                unsafe { kog_ncsf_free(handle.as_ptr()) };
                format!(
                    "SSEQPlayer reported invalid channels for {}",
                    path.display()
                )
            })?;
        let total_frames = unsafe { kog_ncsf_total_frames(handle.as_ptr()) };
        if sample_rate == 0 || total_frames == 0 {
            unsafe { kog_ncsf_free(handle.as_ptr()) };
            return Err(format!(
                "SSEQPlayer reported invalid stream properties for {}",
                path.display()
            ));
        }
        let metadata = NcsfMetadata {
            title: native_text(unsafe { kog_ncsf_title(handle.as_ptr()) }),
            artist: native_text(unsafe { kog_ncsf_artist(handle.as_ptr()) }),
            album: native_text(unsafe { kog_ncsf_album(handle.as_ptr()) }),
            genre: native_text(unsafe { kog_ncsf_genre(handle.as_ptr()) }),
            date: native_text(unsafe { kog_ncsf_date(handle.as_ptr()) }),
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

    pub fn metadata(&self) -> &NcsfMetadata {
        &self.metadata
    }

    pub fn render(&mut self, output: &mut [f32]) -> Result<usize, String> {
        let channels = usize::from(self.channels);
        if output.is_empty() || !output.len().is_multiple_of(channels) {
            return Err(format!(
                "NCSF output must contain complete {}-channel frames",
                self.channels
            ));
        }
        let rendered = unsafe {
            kog_ncsf_render(
                self.handle.as_ptr(),
                output.as_mut_ptr(),
                output.len() / channels,
            )
        };
        usize::try_from(rendered).map_err(|_| format!("SSEQPlayer render failed: {}", last_error()))
    }

    pub fn seek(&mut self, position: Duration) -> Result<Duration, String> {
        let target = if position >= self.duration() {
            self.total_frames
        } else {
            frames_from_duration(position, self.sample_rate)?
        };
        let actual = unsafe { kog_ncsf_seek(self.handle.as_ptr(), target) };
        let actual = u64::try_from(actual)
            .map_err(|_| format!("SSEQPlayer seek failed: {}", last_error()))?;
        Ok(duration_from_frames(actual, self.sample_rate))
    }
}

unsafe impl Send for Ncsf {}

impl Drop for Ncsf {
    fn drop(&mut self) {
        unsafe { kog_ncsf_free(self.handle.as_ptr()) };
    }
}

fn duration_milliseconds(duration: Duration, label: &str) -> Result<u32, String> {
    u32::try_from(duration.as_millis())
        .map_err(|_| format!("default NCSF {label} exceeds the native API limit"))
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
    native_text(unsafe { kog_ncsf_last_error() }).unwrap_or_else(|| "unknown NCSF error".to_owned())
}

fn frames_from_duration(duration: Duration, sample_rate: u32) -> Result<u64, String> {
    let frames = duration.as_nanos().saturating_mul(u128::from(sample_rate)) / 1_000_000_000;
    u64::try_from(frames).map_err(|_| "NCSF duration exceeds Kog's limit".to_owned())
}

fn duration_from_frames(frames: u64, sample_rate: u32) -> Duration {
    let seconds = frames / u64::from(sample_rate);
    let remainder = frames % u64::from(sample_rate);
    let nanos = remainder * 1_000_000_000 / u64::from(sample_rate);
    Duration::new(
        seconds,
        u32::try_from(nanos).expect("subsecond NCSF duration fits u32"),
    )
}

#[cfg(test)]
pub fn test_ncsf_bytes(program: Option<&[u8]>, tags: &str) -> Vec<u8> {
    let executable = stored_zlib(program.unwrap_or_default());
    let mut output = Vec::new();
    output.extend_from_slice(b"PSF\x25");
    output.extend_from_slice(&4_u32.to_le_bytes());
    output.extend_from_slice(&u32::try_from(executable.len()).unwrap().to_le_bytes());
    output.extend_from_slice(&crc32(&executable).to_le_bytes());
    output.extend_from_slice(&0_u32.to_le_bytes());
    output.extend_from_slice(&executable);
    output.extend_from_slice(b"[TAG]");
    output.extend_from_slice(tags.as_bytes());
    output
}

#[cfg(test)]
pub fn test_sdat_bytes() -> Vec<u8> {
    let sseq = test_sseq();
    let sbnk = test_sbnk();
    let swar = test_swar();

    let mut info = Vec::new();
    info.extend_from_slice(b"INFO");
    info.extend_from_slice(&0_u32.to_le_bytes());
    for offset in [40_u32, 0, 48, 56, 0, 0, 0, 0] {
        info.extend_from_slice(&offset.to_le_bytes());
    }
    info.extend_from_slice(&1_u32.to_le_bytes());
    info.extend_from_slice(&64_u32.to_le_bytes());
    info.extend_from_slice(&1_u32.to_le_bytes());
    info.extend_from_slice(&76_u32.to_le_bytes());
    info.extend_from_slice(&1_u32.to_le_bytes());
    info.extend_from_slice(&88_u32.to_le_bytes());
    info.extend_from_slice(&0_u16.to_le_bytes());
    info.extend_from_slice(&0_u16.to_le_bytes());
    info.extend_from_slice(&0_u16.to_le_bytes());
    info.push(127);
    info.extend_from_slice(&[0, 0, 0, 0, 0]);
    info.extend_from_slice(&1_u16.to_le_bytes());
    info.extend_from_slice(&0_u16.to_le_bytes());
    for wave in [0_u16, u16::MAX, u16::MAX, u16::MAX] {
        info.extend_from_slice(&wave.to_le_bytes());
    }
    info.extend_from_slice(&2_u16.to_le_bytes());
    info.extend_from_slice(&0_u16.to_le_bytes());
    let info_size = u32::try_from(info.len()).unwrap();
    patch_u32(&mut info, 4, info_size);
    assert_eq!(info.len(), 92);

    let mut sdat = nds_header(*b"SDAT", 0, 3);
    sdat.resize(64, 0);
    let info_offset = u32::try_from(sdat.len()).unwrap();
    sdat.extend_from_slice(&info);
    let fat_offset = u32::try_from(sdat.len()).unwrap();
    let fat_size = 12 + 3 * 16;
    sdat.extend_from_slice(b"FAT ");
    sdat.extend_from_slice(&(fat_size as u32).to_le_bytes());
    sdat.extend_from_slice(&3_u32.to_le_bytes());
    sdat.resize(sdat.len() + 3 * 16, 0);

    let file_offset = u32::try_from(sdat.len()).unwrap();
    let files = [&sseq, &sbnk, &swar];
    for (index, file) in files.into_iter().enumerate() {
        let offset = u32::try_from(sdat.len()).unwrap();
        let size = u32::try_from(file.len()).unwrap();
        let record = usize::try_from(fat_offset).unwrap() + 12 + index * 16;
        patch_u32(&mut sdat, record, offset);
        patch_u32(&mut sdat, record + 4, size);
        sdat.extend_from_slice(file);
        while !sdat.len().is_multiple_of(4) {
            sdat.push(0);
        }
    }

    let file_size = u32::try_from(sdat.len()).unwrap();
    patch_u32(&mut sdat, 8, file_size);
    patch_u32(&mut sdat, 24, info_offset);
    patch_u32(&mut sdat, 28, u32::try_from(info.len()).unwrap());
    patch_u32(&mut sdat, 32, fat_offset);
    patch_u32(&mut sdat, 36, fat_size as u32);
    patch_u32(&mut sdat, 40, file_offset);
    patch_u32(&mut sdat, 44, file_size - file_offset);
    sdat
}

#[cfg(test)]
fn test_sseq() -> Vec<u8> {
    let sequence = [60, 127, 96, 0xff];
    let mut output = nds_header(*b"SSEQ", 0, 1);
    output.extend_from_slice(b"DATA");
    output.extend_from_slice(&(12_u32 + sequence.len() as u32).to_le_bytes());
    output.extend_from_slice(&28_u32.to_le_bytes());
    output.extend_from_slice(&sequence);
    let size = u32::try_from(output.len()).unwrap();
    patch_u32(&mut output, 8, size);
    output
}

#[cfg(test)]
fn test_sbnk() -> Vec<u8> {
    let mut output = nds_header(*b"SBNK", 0, 1);
    output.extend_from_slice(b"DATA");
    output.extend_from_slice(&58_u32.to_le_bytes());
    output.extend_from_slice(&[0; 32]);
    output.extend_from_slice(&1_u32.to_le_bytes());
    output.push(1);
    output.extend_from_slice(&64_u16.to_le_bytes());
    output.push(0);
    output.extend_from_slice(&0_u16.to_le_bytes());
    output.extend_from_slice(&0_u16.to_le_bytes());
    output.extend_from_slice(&[60, 127, 127, 127, 127, 64]);
    let size = u32::try_from(output.len()).unwrap();
    patch_u32(&mut output, 8, size);
    output
}

#[cfg(test)]
fn test_swar() -> Vec<u8> {
    let mut output = nds_header(*b"SWAR", 0, 1);
    output.extend_from_slice(b"DATA");
    output.extend_from_slice(&124_u32.to_le_bytes());
    output.extend_from_slice(&[0; 32]);
    output.extend_from_slice(&1_u32.to_le_bytes());
    output.extend_from_slice(&64_u32.to_le_bytes());
    output.push(1);
    output.push(1);
    output.extend_from_slice(&8_000_u16.to_le_bytes());
    output.extend_from_slice(&2_095_u16.to_le_bytes());
    output.extend_from_slice(&0_u16.to_le_bytes());
    output.extend_from_slice(&16_u32.to_le_bytes());
    for index in 0_i16..32 {
        let sample = if index < 16 {
            -12_000 + index * 1_500
        } else {
            12_000 - (index - 16) * 1_500
        };
        output.extend_from_slice(&sample.to_le_bytes());
    }
    let size = u32::try_from(output.len()).unwrap();
    patch_u32(&mut output, 8, size);
    output
}

#[cfg(test)]
fn nds_header(kind: [u8; 4], file_size: u32, blocks: u16) -> Vec<u8> {
    let mut output = Vec::new();
    output.extend_from_slice(&kind);
    output.extend_from_slice(&0x0100_feff_u32.to_le_bytes());
    output.extend_from_slice(&file_size.to_le_bytes());
    output.extend_from_slice(&16_u16.to_le_bytes());
    output.extend_from_slice(&blocks.to_le_bytes());
    output
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
