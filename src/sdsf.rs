//! Safe ownership wrapper around psflib and Highly Theoretical for SSF/DSF playback.

use std::ffi::{CStr, CString, c_char};
use std::path::Path;
use std::ptr::NonNull;
use std::time::Duration;

#[repr(C)]
struct NativeSdsf {
    _private: [u8; 0],
}

unsafe extern "C" {
    fn kog_sdsf_open(
        path: *const c_char,
        default_length_milliseconds: u32,
        default_fade_milliseconds: u32,
    ) -> *mut NativeSdsf;
    fn kog_sdsf_free(decoder: *mut NativeSdsf);
    fn kog_sdsf_sample_rate(decoder: *const NativeSdsf) -> u32;
    fn kog_sdsf_channels(decoder: *const NativeSdsf) -> u32;
    fn kog_sdsf_total_frames(decoder: *const NativeSdsf) -> u64;
    fn kog_sdsf_version(decoder: *const NativeSdsf) -> u8;
    fn kog_sdsf_title(decoder: *const NativeSdsf) -> *const c_char;
    fn kog_sdsf_artist(decoder: *const NativeSdsf) -> *const c_char;
    fn kog_sdsf_album(decoder: *const NativeSdsf) -> *const c_char;
    fn kog_sdsf_genre(decoder: *const NativeSdsf) -> *const c_char;
    fn kog_sdsf_date(decoder: *const NativeSdsf) -> *const c_char;
    fn kog_sdsf_render(decoder: *mut NativeSdsf, output: *mut f32, frames: usize) -> i64;
    fn kog_sdsf_seek(decoder: *mut NativeSdsf, frame: u64) -> i64;
    fn kog_sdsf_last_error() -> *const c_char;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SdsfKind {
    Ssf,
    Dsf,
}

impl SdsfKind {
    pub fn codec(self) -> &'static str {
        match self {
            Self::Ssf => "Sega Saturn Sound Format (SSF) / Highly Theoretical",
            Self::Dsf => "Dreamcast Sound Format (DSF) / Highly Theoretical",
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SdsfMetadata {
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub genre: Option<String>,
    pub date: Option<String>,
}

pub struct Sdsf {
    handle: NonNull<NativeSdsf>,
    kind: SdsfKind,
    sample_rate: u32,
    channels: u16,
    total_frames: u64,
    metadata: SdsfMetadata,
}

impl Sdsf {
    pub fn open(
        path: &Path,
        default_length: Duration,
        default_fade: Duration,
    ) -> Result<Self, String> {
        let path_text = path
            .to_str()
            .ok_or_else(|| format!("SSF/DSF path is not valid Unicode: {}", path.display()))?;
        let path_c = CString::new(path_text)
            .map_err(|_| format!("SSF/DSF path contains a NUL byte: {}", path.display()))?;
        let default_length_milliseconds = duration_milliseconds(default_length, "length")?;
        let default_fade_milliseconds = duration_milliseconds(default_fade, "fade")?;
        let handle = NonNull::new(unsafe {
            kog_sdsf_open(
                path_c.as_ptr(),
                default_length_milliseconds,
                default_fade_milliseconds,
            )
        })
        .ok_or_else(|| format!("opening {} as SSF/DSF: {}", path.display(), last_error()))?;

        let kind = match unsafe { kog_sdsf_version(handle.as_ptr()) } {
            0x11 => SdsfKind::Ssf,
            0x12 => SdsfKind::Dsf,
            version => {
                unsafe { kog_sdsf_free(handle.as_ptr()) };
                return Err(format!(
                    "Highly Theoretical reported unsupported PSF version {version:#04x} for {}",
                    path.display()
                ));
            }
        };
        let sample_rate = unsafe { kog_sdsf_sample_rate(handle.as_ptr()) };
        let channels = u16::try_from(unsafe { kog_sdsf_channels(handle.as_ptr()) })
            .ok()
            .filter(|channels| *channels == 2)
            .ok_or_else(|| {
                unsafe { kog_sdsf_free(handle.as_ptr()) };
                format!(
                    "Highly Theoretical reported invalid channels for {}",
                    path.display()
                )
            })?;
        let total_frames = unsafe { kog_sdsf_total_frames(handle.as_ptr()) };
        if sample_rate == 0 || total_frames == 0 {
            unsafe { kog_sdsf_free(handle.as_ptr()) };
            return Err(format!(
                "Highly Theoretical reported invalid stream properties for {}",
                path.display()
            ));
        }
        let metadata = SdsfMetadata {
            title: native_text(unsafe { kog_sdsf_title(handle.as_ptr()) }),
            artist: native_text(unsafe { kog_sdsf_artist(handle.as_ptr()) }),
            album: native_text(unsafe { kog_sdsf_album(handle.as_ptr()) }),
            genre: native_text(unsafe { kog_sdsf_genre(handle.as_ptr()) }),
            date: native_text(unsafe { kog_sdsf_date(handle.as_ptr()) }),
        };

        Ok(Self {
            handle,
            kind,
            sample_rate,
            channels,
            total_frames,
            metadata,
        })
    }

    pub fn kind(&self) -> SdsfKind {
        self.kind
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

    pub fn metadata(&self) -> &SdsfMetadata {
        &self.metadata
    }

    pub fn render(&mut self, output: &mut [f32]) -> Result<usize, String> {
        let channels = usize::from(self.channels);
        if output.is_empty() || !output.len().is_multiple_of(channels) {
            return Err(format!(
                "SSF/DSF output must contain complete {}-channel frames",
                self.channels
            ));
        }
        let rendered = unsafe {
            kog_sdsf_render(
                self.handle.as_ptr(),
                output.as_mut_ptr(),
                output.len() / channels,
            )
        };
        usize::try_from(rendered)
            .map_err(|_| format!("Highly Theoretical render failed: {}", last_error()))
    }

    pub fn seek(&mut self, position: Duration) -> Result<Duration, String> {
        let target = if position >= self.duration() {
            self.total_frames
        } else {
            frames_from_duration(position, self.sample_rate)?
        };
        let actual = unsafe { kog_sdsf_seek(self.handle.as_ptr(), target) };
        let actual = u64::try_from(actual)
            .map_err(|_| format!("Highly Theoretical seek failed: {}", last_error()))?;
        Ok(duration_from_frames(actual, self.sample_rate))
    }
}

unsafe impl Send for Sdsf {}

impl Drop for Sdsf {
    fn drop(&mut self) {
        unsafe { kog_sdsf_free(self.handle.as_ptr()) };
    }
}

fn duration_milliseconds(duration: Duration, label: &str) -> Result<u32, String> {
    u32::try_from(duration.as_millis())
        .map_err(|_| format!("default SSF/DSF {label} exceeds the native API limit"))
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
    native_text(unsafe { kog_sdsf_last_error() })
        .unwrap_or_else(|| "unknown SSF/DSF error".to_owned())
}

fn frames_from_duration(duration: Duration, sample_rate: u32) -> Result<u64, String> {
    let frames = duration.as_nanos().saturating_mul(u128::from(sample_rate)) / 1_000_000_000;
    u64::try_from(frames).map_err(|_| "SSF/DSF duration exceeds Kog's limit".to_owned())
}

fn duration_from_frames(frames: u64, sample_rate: u32) -> Duration {
    let seconds = frames / u64::from(sample_rate);
    let remainder = frames % u64::from(sample_rate);
    let nanos = remainder * 1_000_000_000 / u64::from(sample_rate);
    Duration::new(
        seconds,
        u32::try_from(nanos).expect("subsecond SSF/DSF duration fits u32"),
    )
}

#[cfg(test)]
pub fn test_sdsf_bytes(version: u8, executable: Option<&[u8]>, tags: &str) -> Vec<u8> {
    assert!(matches!(version, 0x11 | 0x12));
    let compressed = stored_zlib(executable.unwrap_or_default());
    let mut output = Vec::new();
    output.extend_from_slice(&[b'P', b'S', b'F', version]);
    output.extend_from_slice(&0_u32.to_le_bytes());
    output.extend_from_slice(&u32::try_from(compressed.len()).unwrap().to_le_bytes());
    output.extend_from_slice(&crc32(&compressed).to_le_bytes());
    output.extend_from_slice(&compressed);
    output.extend_from_slice(b"[TAG]");
    output.extend_from_slice(tags.as_bytes());
    output
}

#[cfg(test)]
pub fn test_ssf_program() -> Vec<u8> {
    let mut memory = vec![0_u8; 0x1200];
    put_big_u32(&mut memory, 0, 0x0007_f000);
    put_big_u32(&mut memory, 4, 0x0000_0100);

    let mut code = Vec::new();
    put_big_u16_push(&mut code, 0x207c); // MOVEA.L #0x00100000,A0
    put_big_u32_push(&mut code, 0x0010_0000);
    for (value, offset) in [
        (0x000f, 0x0400), // master volume
        (0x1000, 0x0002), // sample address
        (0x0000, 0x0004), // loop start
        (0x0100, 0x0006), // loop end
        (0x003f, 0x0008), // immediate attack + hold
        (0x0000, 0x000a),
        (0x0000, 0x000c),
        (0x0000, 0x0010), // native pitch
        (0xe000, 0x0016), // centered direct output
        (0x1820, 0x0000), // forward loop + key-on execute
    ] {
        put_big_u16_push(&mut code, 0x317c); // MOVE.W #value,offset(A0)
        put_big_u16_push(&mut code, value);
        put_big_u16_push(&mut code, offset);
    }
    put_big_u16_push(&mut code, 0x60fe); // BRA.S forever
    memory[0x100..0x100 + code.len()].copy_from_slice(&code);
    for index in 0..256_usize {
        let sample = (index as i32 * 256 - 32768) as i16;
        put_big_u16(&mut memory, 0x1000 + index * 2, sample as u16);
    }
    executable_at_zero(memory)
}

#[cfg(test)]
pub fn test_dsf_program() -> Vec<u8> {
    let mut memory = vec![0_u8; 0x1200];
    let instructions: [u32; 30] = [
        0xe59f_0068, // LDR r0,=0x00802800
        0xe3a0_100f, // MOV r1,#15
        0xe580_1000, // STR r1,[r0] (master volume)
        0xe3a0_0502, // MOV r0,#0x00800000
        0xe3a0_1a01, // MOV r1,#0x1000
        0xe580_1004, // sample address
        0xe3a0_1000,
        0xe580_1008, // loop start
        0xe3a0_1c01,
        0xe580_100c, // loop end
        0xe3a0_101f,
        0xe580_1010, // immediate attack
        0xe59f_103c,
        0xe580_1014, // stable envelope
        0xe3a0_1000,
        0xe580_1018, // native pitch
        0xe3a0_1c0f,
        0xe580_1024, // centered direct output
        0xe3a0_1020,
        0xe580_1028, // disable low-pass filter
        0xe3a0_1902,
        0xe580_1000, // key off before deterministic restart
        0xe3a0_1cc2,
        0xe580_1000, // forward loop + key-on execute
        0xe3a0_2901, // approximately 512 output frames between restarts
        0xe252_2001,
        0x1aff_fffd,
        0xeaff_fff7,
        0x0080_2800,
        0x0000_3fe0,
    ];
    for (index, instruction) in instructions.into_iter().enumerate() {
        memory[index * 4..index * 4 + 4].copy_from_slice(&instruction.to_le_bytes());
    }
    for offset in (0x1000..memory.len()).step_by(2) {
        let index = ((offset - 0x1000) / 2) & 0xff;
        let sample = (index as i32 * 256 - 32768) as i16;
        memory[offset..offset + 2].copy_from_slice(&sample.to_le_bytes());
    }
    executable_at_zero(memory)
}

#[cfg(test)]
pub fn test_sdsf_out_of_bounds_program(version: u8) -> Vec<u8> {
    let maximum = if version == 0x11 { 0x80000 } else { 0x800000 };
    let mut executable = u32::try_from(maximum - 1).unwrap().to_le_bytes().to_vec();
    executable.extend_from_slice(&[1, 2]);
    executable
}

#[cfg(test)]
fn executable_at_zero(memory: Vec<u8>) -> Vec<u8> {
    let mut executable = 0_u32.to_le_bytes().to_vec();
    executable.extend_from_slice(&memory);
    executable
}

#[cfg(test)]
fn put_big_u16(bytes: &mut [u8], offset: usize, value: u16) {
    bytes[offset..offset + 2].copy_from_slice(&value.to_be_bytes());
}

#[cfg(test)]
fn put_big_u32(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_be_bytes());
}

#[cfg(test)]
fn put_big_u16_push(bytes: &mut Vec<u8>, value: u16) {
    bytes.extend_from_slice(&value.to_be_bytes());
}

#[cfg(test)]
fn put_big_u32_push(bytes: &mut Vec<u8>, value: u32) {
    bytes.extend_from_slice(&value.to_be_bytes());
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
