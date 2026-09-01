//! Safe owner for the pinned libADLMIDI sequencer and Nuked OPL3 renderer.

use std::ffi::{CStr, c_char, c_int, c_long, c_ulong, c_void};
use std::path::Path;
use std::ptr::NonNull;
use std::time::Duration;

const SAMPLE_RATE: u32 = 44_100;
const CHANNELS: u16 = 2;
const MAX_FILE_BYTES: u64 = 256 * 1024 * 1024;
const MAX_SUBSONGS: u32 = 4_096;
const MAX_DURATION_SECONDS: f64 = 24.0 * 60.0 * 60.0;
const ADLMIDI_EMU_NUKED: c_int = 0;

#[repr(C)]
struct NativeAdlMidi {
    _private: [u8; 0],
}

unsafe extern "C" {
    fn adl_init(sample_rate: c_long) -> *mut NativeAdlMidi;
    fn adl_close(device: *mut NativeAdlMidi);
    fn adl_errorString() -> *const c_char;
    fn adl_errorInfo(device: *mut NativeAdlMidi) -> *const c_char;
    fn adl_switchEmulator(device: *mut NativeAdlMidi, emulator: c_int) -> c_int;
    fn adl_setLoopEnabled(device: *mut NativeAdlMidi, enabled: c_int);
    fn adl_selectSongNum(device: *mut NativeAdlMidi, song_number: c_int);
    fn adl_openData(device: *mut NativeAdlMidi, data: *const c_void, size: c_ulong) -> c_int;
    fn adl_getSongsCount(device: *mut NativeAdlMidi) -> c_int;
    fn adl_totalTimeLength(device: *mut NativeAdlMidi) -> f64;
    fn adl_metaMusicTitle(device: *mut NativeAdlMidi) -> *const c_char;
    fn adl_play(device: *mut NativeAdlMidi, sample_count: c_int, output: *mut i16) -> c_int;
    fn adl_positionSeek(device: *mut NativeAdlMidi, seconds: f64);
    fn adl_atEnd(device: *mut NativeAdlMidi) -> c_int;
}

pub struct AdlMidi {
    handle: NonNull<NativeAdlMidi>,
    _file_bytes: Vec<u8>,
    duration: Duration,
    total_frames: u64,
    rendered_frames: u64,
    subsong_count: u32,
    selected_subsong: u32,
    title: String,
    native_pcm: Vec<i16>,
}

// libADLMIDI instances have no thread affinity. The API is not thread-safe for
// concurrent access to one instance, while `AdlMidi` provides only `&mut self`
// mutation and owns its handle exclusively when rodio moves it to the mixer.
unsafe impl Send for AdlMidi {}

impl AdlMidi {
    pub fn open(path: &Path, subsong: Option<u32>) -> Result<Self, String> {
        let metadata = path
            .metadata()
            .map_err(|error| format!("reading MIDI metadata for {}: {error}", path.display()))?;
        if metadata.len() == 0 || metadata.len() > MAX_FILE_BYTES {
            return Err(format!(
                "{} is empty or exceeds Kog's 256 MiB libADLMIDI limit",
                path.display()
            ));
        }
        let file_bytes = std::fs::read(path)
            .map_err(|error| format!("reading MIDI file {}: {error}", path.display()))?;
        let selected_subsong = subsong.unwrap_or(0);
        let selected_native = c_int::try_from(selected_subsong)
            .map_err(|_| "libADLMIDI subsong exceeds the native API limit".to_owned())?;

        let handle = NonNull::new(unsafe { adl_init(c_long::from(SAMPLE_RATE)) })
            .ok_or_else(global_error)?;
        let mut decoder = Self {
            handle,
            _file_bytes: file_bytes,
            duration: Duration::ZERO,
            total_frames: 0,
            rendered_frames: 0,
            subsong_count: 0,
            selected_subsong,
            title: String::new(),
            native_pcm: Vec::new(),
        };

        if unsafe { adl_switchEmulator(decoder.handle.as_ptr(), ADLMIDI_EMU_NUKED) } < 0 {
            return Err(decoder.error("selecting libADLMIDI's Nuked OPL3 emulator"));
        }
        unsafe {
            adl_setLoopEnabled(decoder.handle.as_ptr(), 0);
            adl_selectSongNum(decoder.handle.as_ptr(), selected_native);
        }
        let data_size = c_ulong::try_from(decoder._file_bytes.len())
            .map_err(|_| "MIDI input exceeds libADLMIDI's native size limit".to_owned())?;
        if unsafe {
            adl_openData(
                decoder.handle.as_ptr(),
                decoder._file_bytes.as_ptr().cast(),
                data_size,
            )
        } < 0
        {
            return Err(decoder.error(&format!("opening {} with libADLMIDI", path.display())));
        }

        let songs = unsafe { adl_getSongsCount(decoder.handle.as_ptr()) };
        let songs = u32::try_from(songs)
            .map_err(|_| decoder.error("reading MIDI subsongs"))?
            .max(1);
        if songs > MAX_SUBSONGS || selected_subsong >= songs {
            return Err(format!(
                "libADLMIDI reported an invalid subsong selection {selected_subsong}/{songs} for {}",
                path.display()
            ));
        }
        let seconds = unsafe { adl_totalTimeLength(decoder.handle.as_ptr()) };
        if !seconds.is_finite() || seconds <= 0.0 || seconds > MAX_DURATION_SECONDS {
            return Err(format!(
                "libADLMIDI reported an invalid duration for {}",
                path.display()
            ));
        }
        decoder.duration = Duration::from_secs_f64(seconds);
        decoder.total_frames = (seconds * f64::from(SAMPLE_RATE)).ceil() as u64;
        decoder.subsong_count = songs;
        decoder.title = decoder.read_title();
        Ok(decoder)
    }

    pub fn duration(&self) -> Duration {
        self.duration
    }

    pub fn sample_rate(&self) -> u32 {
        SAMPLE_RATE
    }

    pub fn channels(&self) -> u16 {
        CHANNELS
    }

    pub fn subsong_count(&self) -> u32 {
        self.subsong_count
    }

    pub fn selected_subsong(&self) -> u32 {
        self.selected_subsong
    }

    pub fn title(&self) -> &str {
        &self.title
    }

    pub fn render(&mut self, output: &mut [f32]) -> Result<usize, String> {
        let channels = usize::from(CHANNELS);
        if output.is_empty() || !output.len().is_multiple_of(channels) {
            return Err("libADLMIDI output must contain complete stereo frames".to_owned());
        }
        let remaining = self.total_frames.saturating_sub(self.rendered_frames);
        let requested_frames = usize::try_from(
            remaining.min(u64::try_from(output.len() / channels).unwrap_or(u64::MAX)),
        )
        .expect("requested libADLMIDI frames fit the output buffer");
        if requested_frames == 0 || unsafe { adl_atEnd(self.handle.as_ptr()) } == 1 {
            return Ok(0);
        }
        let requested_samples = requested_frames
            .checked_mul(channels)
            .ok_or_else(|| "libADLMIDI render request exceeds Kog's buffer limit".to_owned())?;
        let requested_native = c_int::try_from(requested_samples)
            .map_err(|_| "libADLMIDI render request exceeds the native API limit".to_owned())?;
        self.native_pcm.resize(requested_samples, 0);
        let rendered = unsafe {
            adl_play(
                self.handle.as_ptr(),
                requested_native,
                self.native_pcm.as_mut_ptr(),
            )
        };
        if rendered < 0 {
            return Err(self.error("rendering MIDI through libADLMIDI"));
        }
        let rendered_samples = usize::try_from(rendered)
            .map_err(|_| "libADLMIDI returned an invalid sample count".to_owned())?;
        if rendered_samples > requested_samples || rendered_samples % channels != 0 {
            return Err("libADLMIDI returned an invalid stereo sample count".to_owned());
        }
        if rendered_samples == 0 && unsafe { adl_atEnd(self.handle.as_ptr()) } != 1 {
            return Err(self.error("libADLMIDI stopped before end of stream"));
        }
        for (destination, sample) in output
            .iter_mut()
            .zip(self.native_pcm.iter())
            .take(rendered_samples)
        {
            *destination = f32::from(*sample) / 32768.0;
        }
        let frames = rendered_samples / channels;
        self.rendered_frames += frames as u64;
        Ok(frames)
    }

    pub fn seek(&mut self, position: Duration) -> Duration {
        let target = position.min(self.duration);
        unsafe { adl_positionSeek(self.handle.as_ptr(), target.as_secs_f64()) };
        self.rendered_frames =
            ((target.as_secs_f64() * f64::from(SAMPLE_RATE)).floor() as u64).min(self.total_frames);
        target
    }

    fn read_title(&self) -> String {
        let title = unsafe { adl_metaMusicTitle(self.handle.as_ptr()) };
        if title.is_null() {
            return String::new();
        }
        unsafe { CStr::from_ptr(title) }
            .to_string_lossy()
            .trim()
            .to_owned()
    }

    fn error(&self, context: &str) -> String {
        let message = unsafe { adl_errorInfo(self.handle.as_ptr()) };
        let detail = c_string(message);
        if detail.is_empty() {
            context.to_owned()
        } else {
            format!("{context}: {detail}")
        }
    }
}

impl Drop for AdlMidi {
    fn drop(&mut self) {
        unsafe { adl_close(self.handle.as_ptr()) };
    }
}

fn global_error() -> String {
    let message = unsafe { adl_errorString() };
    let detail = c_string(message);
    if detail.is_empty() {
        "initializing libADLMIDI failed".to_owned()
    } else {
        format!("initializing libADLMIDI failed: {detail}")
    }
}

fn c_string(value: *const c_char) -> String {
    if value.is_null() {
        String::new()
    } else {
        unsafe { CStr::from_ptr(value) }
            .to_string_lossy()
            .trim()
            .to_owned()
    }
}

#[cfg(test)]
pub fn test_mus_bytes() -> Vec<u8> {
    // Original, minimal DMX MUS score: program 0, middle-C on/off, end.
    let score = [
        0x40, 0x00, 0x00, // program change
        0x90, 0xBC, 100, 70, // note on with velocity and delay
        0x80, 60, 35,   // note off with delay
        0x60, // end of score
    ];
    let mut bytes = Vec::with_capacity(16 + score.len());
    bytes.extend_from_slice(b"MUS\x1A");
    bytes.extend_from_slice(&(score.len() as u16).to_le_bytes());
    bytes.extend_from_slice(&16_u16.to_le_bytes());
    bytes.extend_from_slice(&1_u16.to_le_bytes());
    bytes.extend_from_slice(&0_u16.to_le_bytes());
    bytes.extend_from_slice(&0_u16.to_le_bytes());
    bytes.extend_from_slice(&0_u16.to_le_bytes());
    bytes.extend_from_slice(&score);
    bytes
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_path(name: &str) -> std::path::PathBuf {
        let path =
            std::env::temp_dir().join(format!("kog-adlmidi-{}-{name}.mus", std::process::id()));
        std::fs::write(&path, test_mus_bytes()).expect("write generated MUS fixture");
        path
    }

    #[test]
    fn generated_mus_renders_non_silent_pcm_and_seeks() {
        let path = fixture_path("render");
        let mut decoder = AdlMidi::open(&path, Some(0)).expect("open generated MUS");
        assert_eq!(decoder.subsong_count(), 1);
        assert_eq!(decoder.selected_subsong(), 0);
        assert_eq!(decoder.sample_rate(), 44_100);
        assert_eq!(decoder.channels(), 2);
        assert!(decoder.duration() > Duration::from_millis(100));
        let mut pcm = vec![0.0_f32; 4_410 * 2];
        let frames = decoder.render(&mut pcm).expect("render generated MUS");
        assert!(frames > 0);
        assert!(
            pcm[..frames * 2]
                .iter()
                .any(|sample| sample.abs() > 0.000_01)
        );
        decoder.seek(Duration::from_millis(50));
        pcm.fill(0.0);
        let frames = decoder.render(&mut pcm).expect("render sought MUS");
        assert!(frames > 0);
        assert!(
            pcm[..frames * 2]
                .iter()
                .any(|sample| sample.abs() > 0.000_01)
        );
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn malformed_mus_and_missing_subsongs_are_rejected() {
        let path = fixture_path("malformed");
        std::fs::write(&path, b"MUS\x1A").expect("write malformed MUS");
        assert!(AdlMidi::open(&path, Some(0)).is_err());
        std::fs::write(&path, test_mus_bytes()).expect("restore generated MUS");
        assert!(AdlMidi::open(&path, Some(1)).is_err());
        std::fs::remove_file(path).ok();
    }
}
