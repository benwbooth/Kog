//! Safe ownership wrapper around psflib and Highly Quixotic for QSF playback.

use std::ffi::{CStr, CString, c_char};
use std::path::Path;
use std::ptr::NonNull;
use std::time::Duration;

#[repr(C)]
struct NativeQsf {
    _private: [u8; 0],
}

unsafe extern "C" {
    fn kog_qsf_open(
        path: *const c_char,
        default_length_milliseconds: u32,
        default_fade_milliseconds: u32,
    ) -> *mut NativeQsf;
    fn kog_qsf_free(decoder: *mut NativeQsf);
    fn kog_qsf_sample_rate(decoder: *const NativeQsf) -> u32;
    fn kog_qsf_channels(decoder: *const NativeQsf) -> u32;
    fn kog_qsf_total_frames(decoder: *const NativeQsf) -> u64;
    fn kog_qsf_title(decoder: *const NativeQsf) -> *const c_char;
    fn kog_qsf_artist(decoder: *const NativeQsf) -> *const c_char;
    fn kog_qsf_album(decoder: *const NativeQsf) -> *const c_char;
    fn kog_qsf_genre(decoder: *const NativeQsf) -> *const c_char;
    fn kog_qsf_date(decoder: *const NativeQsf) -> *const c_char;
    fn kog_qsf_render(decoder: *mut NativeQsf, output: *mut f32, frames: usize) -> i64;
    fn kog_qsf_seek(decoder: *mut NativeQsf, frame: u64) -> i64;
    fn kog_qsf_last_error() -> *const c_char;
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct QsfMetadata {
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub genre: Option<String>,
    pub date: Option<String>,
}

pub struct Qsf {
    handle: NonNull<NativeQsf>,
    sample_rate: u32,
    channels: u16,
    total_frames: u64,
    metadata: QsfMetadata,
}

impl Qsf {
    pub fn open(
        path: &Path,
        default_length: Duration,
        default_fade: Duration,
    ) -> Result<Self, String> {
        let path_text = path
            .to_str()
            .ok_or_else(|| format!("QSF path is not valid Unicode: {}", path.display()))?;
        let path_c = CString::new(path_text)
            .map_err(|_| format!("QSF path contains a NUL byte: {}", path.display()))?;
        let default_length_milliseconds = duration_milliseconds(default_length, "length")?;
        let default_fade_milliseconds = duration_milliseconds(default_fade, "fade")?;
        let handle = NonNull::new(unsafe {
            kog_qsf_open(
                path_c.as_ptr(),
                default_length_milliseconds,
                default_fade_milliseconds,
            )
        })
        .ok_or_else(|| format!("opening {} as QSF: {}", path.display(), last_error()))?;

        let sample_rate = unsafe { kog_qsf_sample_rate(handle.as_ptr()) };
        let channels = u16::try_from(unsafe { kog_qsf_channels(handle.as_ptr()) })
            .ok()
            .filter(|channels| *channels == 2)
            .ok_or_else(|| {
                unsafe { kog_qsf_free(handle.as_ptr()) };
                format!(
                    "Highly Quixotic reported invalid channels for {}",
                    path.display()
                )
            })?;
        let total_frames = unsafe { kog_qsf_total_frames(handle.as_ptr()) };
        if sample_rate == 0 || total_frames == 0 {
            unsafe { kog_qsf_free(handle.as_ptr()) };
            return Err(format!(
                "Highly Quixotic reported invalid stream properties for {}",
                path.display()
            ));
        }
        let metadata = QsfMetadata {
            title: native_text(unsafe { kog_qsf_title(handle.as_ptr()) }),
            artist: native_text(unsafe { kog_qsf_artist(handle.as_ptr()) }),
            album: native_text(unsafe { kog_qsf_album(handle.as_ptr()) }),
            genre: native_text(unsafe { kog_qsf_genre(handle.as_ptr()) }),
            date: native_text(unsafe { kog_qsf_date(handle.as_ptr()) }),
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

    pub fn metadata(&self) -> &QsfMetadata {
        &self.metadata
    }

    pub fn render(&mut self, output: &mut [f32]) -> Result<usize, String> {
        let channels = usize::from(self.channels);
        if output.is_empty() || !output.len().is_multiple_of(channels) {
            return Err(format!(
                "QSF output must contain complete {}-channel frames",
                self.channels
            ));
        }
        let rendered = unsafe {
            kog_qsf_render(
                self.handle.as_ptr(),
                output.as_mut_ptr(),
                output.len() / channels,
            )
        };
        usize::try_from(rendered)
            .map_err(|_| format!("Highly Quixotic render failed: {}", last_error()))
    }

    pub fn seek(&mut self, position: Duration) -> Result<Duration, String> {
        let target = if position >= self.duration() {
            self.total_frames
        } else {
            frames_from_duration(position, self.sample_rate)?
        };
        let actual = unsafe { kog_qsf_seek(self.handle.as_ptr(), target) };
        let actual = u64::try_from(actual)
            .map_err(|_| format!("Highly Quixotic seek failed: {}", last_error()))?;
        Ok(duration_from_frames(actual, self.sample_rate))
    }
}

unsafe impl Send for Qsf {}

impl Drop for Qsf {
    fn drop(&mut self) {
        unsafe { kog_qsf_free(self.handle.as_ptr()) };
    }
}

fn duration_milliseconds(duration: Duration, label: &str) -> Result<u32, String> {
    u32::try_from(duration.as_millis())
        .map_err(|_| format!("default QSF {label} exceeds the native API limit"))
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
    native_text(unsafe { kog_qsf_last_error() }).unwrap_or_else(|| "unknown QSF error".to_owned())
}

fn frames_from_duration(duration: Duration, sample_rate: u32) -> Result<u64, String> {
    let frames = duration.as_nanos().saturating_mul(u128::from(sample_rate)) / 1_000_000_000;
    u64::try_from(frames).map_err(|_| "QSF duration exceeds Kog's limit".to_owned())
}

fn duration_from_frames(frames: u64, sample_rate: u32) -> Duration {
    let seconds = frames / u64::from(sample_rate);
    let remainder = frames % u64::from(sample_rate);
    let nanos = remainder * 1_000_000_000 / u64::from(sample_rate);
    Duration::new(
        seconds,
        u32::try_from(nanos).expect("subsecond QSF duration fits u32"),
    )
}

#[cfg(test)]
pub fn test_qsf_bytes(executable: Option<&[u8]>, tags: &str) -> Vec<u8> {
    let compressed = stored_zlib(executable.unwrap_or_default());
    let mut output = Vec::new();
    output.extend_from_slice(b"PSF\x41");
    output.extend_from_slice(&0_u32.to_le_bytes());
    output.extend_from_slice(&u32::try_from(compressed.len()).unwrap().to_le_bytes());
    output.extend_from_slice(&crc32(&compressed).to_le_bytes());
    output.extend_from_slice(&compressed);
    output.extend_from_slice(b"[TAG]");
    output.extend_from_slice(tags.as_bytes());
    output
}

#[cfg(test)]
pub fn test_qsf_program() -> Vec<u8> {
    let z80 = test_z80_rom();
    let samples: Vec<u8> = (0_u16..=255)
        .map(|sample| u8::try_from(sample).unwrap().wrapping_sub(128))
        .collect();
    let mut program = Vec::new();
    append_section(&mut program, b"Z80", 0, &z80);
    append_section(&mut program, b"SMP", 0, &samples);
    program
}

#[cfg(test)]
pub fn test_qsf_malformed_section() -> Vec<u8> {
    let mut program = Vec::new();
    program.extend_from_slice(b"SMP");
    program.extend_from_slice(&0_u32.to_le_bytes());
    program.extend_from_slice(&u32::MAX.to_le_bytes());
    program.extend_from_slice(&[0; 4]);
    program
}

#[cfg(test)]
fn append_section(output: &mut Vec<u8>, name: &[u8; 3], offset: u32, data: &[u8]) {
    output.extend_from_slice(name);
    output.extend_from_slice(&offset.to_le_bytes());
    output.extend_from_slice(&u32::try_from(data.len()).unwrap().to_le_bytes());
    output.extend_from_slice(data);
}

#[cfg(test)]
fn test_z80_rom() -> Vec<u8> {
    let mut program = Vec::new();
    program.push(0xf3); // DI
    write_qsound_register(&mut program, 0xe3, 0x0288); // initialize DSP mode 1
    program.extend_from_slice(&[0x01, 0x00, 0x20]); // LD BC,0x2000
    let delay = program.len();
    program.extend_from_slice(&[0x0b, 0x78, 0xb1]); // DEC BC; LD A,B; OR C
    let displacement = i8::try_from(delay as isize - (program.len() as isize + 2)).unwrap();
    program.extend_from_slice(&[0x20, displacement as u8]); // JR NZ,delay

    write_qsound_register(&mut program, 0x78, 0x8000); // voice 0 bank
    write_qsound_register(&mut program, 0x01, 0x0000); // sample address
    write_qsound_register(&mut program, 0x02, 0x1000); // one ROM byte per frame
    write_qsound_register(&mut program, 0x03, 0x0000); // phase
    write_qsound_register(&mut program, 0x04, 0x0100); // loop length
    write_qsound_register(&mut program, 0x05, 0x0100); // loop end
    write_qsound_register(&mut program, 0x06, 0x3fff); // volume
    write_qsound_register(&mut program, 0x80, 0x0120); // centered pan

    let halt = u16::try_from(program.len()).unwrap();
    program.push(0xc3); // JP halt
    program.extend_from_slice(&halt.to_le_bytes());

    let mut rom = vec![0; 0x8000];
    rom[..program.len()].copy_from_slice(&program);
    rom
}

#[cfg(test)]
fn write_qsound_register(program: &mut Vec<u8>, command: u8, value: u16) {
    program.extend_from_slice(&[0x3e, (value >> 8) as u8, 0x32, 0x00, 0xd0]);
    program.extend_from_slice(&[0x3e, value as u8, 0x32, 0x01, 0xd0]);
    program.extend_from_slice(&[0x3e, command, 0x32, 0x02, 0xd0]);
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
