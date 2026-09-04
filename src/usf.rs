//! Safe ownership wrapper around psflib and LazyUSF2 for USF playback.

use std::ffi::{CStr, CString, c_char};
use std::path::Path;
use std::ptr::NonNull;
use std::time::Duration;

#[repr(C)]
struct NativeUsf {
    _private: [u8; 0],
}

unsafe extern "C" {
    fn kog_usf_open(
        path: *const c_char,
        default_length_milliseconds: u32,
        default_fade_milliseconds: u32,
    ) -> *mut NativeUsf;
    fn kog_usf_free(decoder: *mut NativeUsf);
    fn kog_usf_sample_rate(decoder: *const NativeUsf) -> u32;
    fn kog_usf_channels(decoder: *const NativeUsf) -> u32;
    fn kog_usf_total_frames(decoder: *const NativeUsf) -> u64;
    fn kog_usf_title(decoder: *const NativeUsf) -> *const c_char;
    fn kog_usf_artist(decoder: *const NativeUsf) -> *const c_char;
    fn kog_usf_album(decoder: *const NativeUsf) -> *const c_char;
    fn kog_usf_genre(decoder: *const NativeUsf) -> *const c_char;
    fn kog_usf_date(decoder: *const NativeUsf) -> *const c_char;
    fn kog_usf_render(decoder: *mut NativeUsf, output: *mut f32, frames: usize) -> i64;
    fn kog_usf_seek(decoder: *mut NativeUsf, frame: u64) -> i64;
    fn kog_usf_last_error() -> *const c_char;
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct UsfMetadata {
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub genre: Option<String>,
    pub date: Option<String>,
}

pub struct Usf {
    handle: NonNull<NativeUsf>,
    sample_rate: u32,
    channels: u16,
    total_frames: u64,
    metadata: UsfMetadata,
}

impl Usf {
    pub fn open(
        path: &Path,
        default_length: Duration,
        default_fade: Duration,
    ) -> Result<Self, String> {
        let path_text = path
            .to_str()
            .ok_or_else(|| format!("USF path is not valid Unicode: {}", path.display()))?;
        let path_c = CString::new(path_text)
            .map_err(|_| format!("USF path contains a NUL byte: {}", path.display()))?;
        let default_length_milliseconds = duration_milliseconds(default_length, "length")?;
        let default_fade_milliseconds = duration_milliseconds(default_fade, "fade")?;
        let handle = NonNull::new(unsafe {
            kog_usf_open(
                path_c.as_ptr(),
                default_length_milliseconds,
                default_fade_milliseconds,
            )
        })
        .ok_or_else(|| format!("opening {} as USF: {}", path.display(), last_error()))?;

        let sample_rate = unsafe { kog_usf_sample_rate(handle.as_ptr()) };
        let channels = u16::try_from(unsafe { kog_usf_channels(handle.as_ptr()) })
            .ok()
            .filter(|channels| *channels == 2)
            .ok_or_else(|| {
                unsafe { kog_usf_free(handle.as_ptr()) };
                format!("LazyUSF2 reported invalid channels for {}", path.display())
            })?;
        let total_frames = unsafe { kog_usf_total_frames(handle.as_ptr()) };
        if sample_rate == 0 || total_frames == 0 {
            unsafe { kog_usf_free(handle.as_ptr()) };
            return Err(format!(
                "LazyUSF2 reported invalid stream properties for {}",
                path.display()
            ));
        }
        let metadata = UsfMetadata {
            title: native_text(unsafe { kog_usf_title(handle.as_ptr()) }),
            artist: native_text(unsafe { kog_usf_artist(handle.as_ptr()) }),
            album: native_text(unsafe { kog_usf_album(handle.as_ptr()) }),
            genre: native_text(unsafe { kog_usf_genre(handle.as_ptr()) }),
            date: native_text(unsafe { kog_usf_date(handle.as_ptr()) }),
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

    pub fn metadata(&self) -> &UsfMetadata {
        &self.metadata
    }

    pub fn render(&mut self, output: &mut [f32]) -> Result<usize, String> {
        let channels = usize::from(self.channels);
        if output.is_empty() || !output.len().is_multiple_of(channels) {
            return Err(format!(
                "USF output must contain complete {}-channel frames",
                self.channels
            ));
        }
        let rendered = unsafe {
            kog_usf_render(
                self.handle.as_ptr(),
                output.as_mut_ptr(),
                output.len() / channels,
            )
        };
        usize::try_from(rendered).map_err(|_| format!("LazyUSF2 render failed: {}", last_error()))
    }

    pub fn seek(&mut self, position: Duration) -> Result<Duration, String> {
        let target = if position >= self.duration() {
            self.total_frames
        } else {
            frames_from_duration(position, self.sample_rate)?
        };
        let actual = unsafe { kog_usf_seek(self.handle.as_ptr(), target) };
        let actual =
            u64::try_from(actual).map_err(|_| format!("LazyUSF2 seek failed: {}", last_error()))?;
        Ok(duration_from_frames(actual, self.sample_rate))
    }
}

unsafe impl Send for Usf {}

impl Drop for Usf {
    fn drop(&mut self) {
        unsafe { kog_usf_free(self.handle.as_ptr()) };
    }
}

fn duration_milliseconds(duration: Duration, label: &str) -> Result<u32, String> {
    u32::try_from(duration.as_millis())
        .map_err(|_| format!("default USF {label} exceeds the native API limit"))
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
    native_text(unsafe { kog_usf_last_error() }).unwrap_or_else(|| "unknown USF error".to_owned())
}

fn frames_from_duration(duration: Duration, sample_rate: u32) -> Result<u64, String> {
    let frames = duration.as_nanos().saturating_mul(u128::from(sample_rate)) / 1_000_000_000;
    u64::try_from(frames).map_err(|_| "USF duration exceeds Kog's limit".to_owned())
}

fn duration_from_frames(frames: u64, sample_rate: u32) -> Duration {
    let seconds = frames / u64::from(sample_rate);
    let remainder = frames % u64::from(sample_rate);
    let nanos = remainder * 1_000_000_000 / u64::from(sample_rate);
    Duration::new(
        seconds,
        u32::try_from(nanos).expect("subsecond USF duration fits u32"),
    )
}

#[cfg(test)]
pub fn test_usf_bytes(reserved: Option<&[u8]>, tags: &str) -> Vec<u8> {
    let reserved = reserved.unwrap_or_default();
    let compressed = stored_zlib(&[]);
    let mut output = Vec::new();
    output.extend_from_slice(b"PSF\x21");
    output.extend_from_slice(&u32::try_from(reserved.len()).unwrap().to_le_bytes());
    output.extend_from_slice(&u32::try_from(compressed.len()).unwrap().to_le_bytes());
    output.extend_from_slice(&crc32(&compressed).to_le_bytes());
    output.extend_from_slice(reserved);
    output.extend_from_slice(&compressed);
    output.extend_from_slice(b"[TAG]");
    output.extend_from_slice(tags.as_bytes());
    output
}

#[cfg(test)]
pub fn test_usf_reserved() -> Vec<u8> {
    const RDRAM_OFFSET: u32 = 0x75c;
    let mut reserved = Vec::new();
    reserved.extend_from_slice(&0_u32.to_le_bytes()); // no ROM blocks
    reserved.extend_from_slice(&0x3436_5253_u32.to_le_bytes()); // SR64 save-state blocks

    let mut header = vec![0_u8; 8];
    header[0..4].copy_from_slice(&[0xc8, 0xa6, 0xd8, 0x23]);
    header[4..8].copy_from_slice(&0x0040_0000_u32.to_le_bytes());
    push_block(&mut reserved, 0, &header);

    let mut timing = Vec::new();
    timing.extend_from_slice(&1_000_000_u32.to_le_bytes());
    timing.extend_from_slice(&0x8000_0000_u32.to_le_bytes());
    push_block(&mut reserved, 8 + 0x40, &timing);

    push_block(
        &mut reserved,
        8 + 0x40 + 8 + 32 * 8 + 32 * 8 + 11 * 4,
        &0x7fff_ffff_u32.to_le_bytes(),
    );

    let program = [
        0x3c08_a450_u32, // LUI t0,0xa450
        0x3409_1000,     // ORI t1,zero,0x1000
        0xad09_0000,     // SW t1,AI_DRAM_ADDR(t0)
        0x3409_03f5,     // ORI t1,zero,1013: about 48 kHz NTSC
        0xad09_0010,     // SW t1,AI_DACRATE(t0)
        0x3409_000f,     // ORI t1,zero,15
        0xad09_0014,     // SW t1,AI_BITRATE(t0)
        0x3409_0001,     // ORI t1,zero,1
        0xad09_0008,     // SW t1,AI_CONTROL(t0)
        0x3409_1000,     // ORI t1,zero,0x1000
        0xad09_0004,     // loop: SW t1,AI_LEN(t0)
        0x1000_fffe,     // BEQ zero,zero,loop
        0x0000_0000,     // NOP delay slot
    ];
    let program_bytes: Vec<u8> = program.into_iter().flat_map(u32::to_le_bytes).collect();
    push_block(&mut reserved, RDRAM_OFFSET, &program_bytes);

    let mut waveform = Vec::with_capacity(1024 * 4);
    for frame in 0..1024_i32 {
        let sample = ((frame & 255) * 256 - 32768) as i16;
        waveform.extend_from_slice(&sample.to_le_bytes());
        waveform.extend_from_slice(&sample.wrapping_neg().to_le_bytes());
    }
    push_block(&mut reserved, RDRAM_OFFSET + 0x1000, &waveform);
    reserved.extend_from_slice(&0_u32.to_le_bytes());
    reserved
}

#[cfg(test)]
pub fn test_usf_out_of_bounds_reserved() -> Vec<u8> {
    let mut reserved = Vec::new();
    reserved.extend_from_slice(&0_u32.to_le_bytes());
    reserved.extend_from_slice(&0x3436_5253_u32.to_le_bytes());
    push_block(&mut reserved, 0x0080_275b, &[1, 2]);
    reserved.extend_from_slice(&0_u32.to_le_bytes());
    reserved
}

#[cfg(test)]
fn push_block(reserved: &mut Vec<u8>, start: u32, bytes: &[u8]) {
    reserved.extend_from_slice(&u32::try_from(bytes.len()).unwrap().to_le_bytes());
    reserved.extend_from_slice(&start.to_le_bytes());
    reserved.extend_from_slice(bytes);
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
