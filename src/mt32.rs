//! In-process Munt / libmt32emu MIDI renderer.

use std::ffi::{CStr, CString, c_char, c_void};
use std::num::{NonZeroU16, NonZeroU32};
use std::path::{Path, PathBuf};
use std::ptr::NonNull;
use std::sync::Arc;
use std::time::Duration;

use midly::{Format, Fps, MetaMessage, MidiMessage, Smf, Timing, TrackEventKind};
use rodio::source::SeekError;
use rodio::{ChannelCount, SampleRate, Source};

const SAMPLE_RATE: u32 = 48_000;
const CHANNELS: u16 = 2;
const RENDER_FRAMES: usize = 512;
const MAX_EVENTS: usize = 2_000_000;
const MAX_EVENT_BYTES: usize = 32 * 1024;
const MAX_DURATION: Duration = Duration::from_secs(24 * 60 * 60);
const ERROR_BYTES: usize = 1024;

// Zero-based form of Munt's own GM-emulation program map from
// mt32emu_alsadrv/src/maps.h. This selects the closest stock MT-32 patch for
// each General MIDI program without changing native MT-32 scores when the
// compatibility option is disabled.
const GM_TO_MT32_PROGRAM: [u8; 128] = [
    5, 5, 60, 7, 3, 4, 16, 19, 22, 101, 100, 97, 104, 103, 102, 119, 12, 14, 9, 12, 14, 15, 87, 45,
    59, 60, 71, 59, 65, 62, 48, 63, 64, 64, 65, 70, 68, 69, 28, 30, 52, 53, 54, 56, 49, 51, 57,
    112, 52, 50, 49, 50, 34, 33, 33, 122, 88, 90, 94, 89, 92, 96, 26, 27, 78, 79, 80, 81, 84, 85,
    86, 82, 75, 73, 76, 77, 110, 107, 108, 75, 47, 47, 44, 34, 34, 42, 45, 31, 32, 38, 32, 34, 32,
    38, 33, 34, 41, 36, 35, 37, 37, 39, 43, 46, 63, 59, 59, 59, 59, 111, 53, 53, 100, 103, 103,
    117, 116, 112, 111, 118, 124, 110, 110, 124, 123, 110, 110, 117,
];

#[repr(C)]
struct KogMt32(c_void);

unsafe extern "C" {
    fn kog_mt32_open(
        rom_directory: *const c_char,
        sample_rate: u32,
        error: *mut c_char,
        error_size: usize,
    ) -> *mut KogMt32;
    fn kog_mt32_free(synth: *mut KogMt32);
    fn kog_mt32_model(synth: *const KogMt32) -> *const c_char;
    fn kog_mt32_sample_rate(synth: *const KogMt32) -> u32;
    fn kog_mt32_send(
        synth: *mut KogMt32,
        bytes: *const u8,
        length: usize,
        error: *mut c_char,
        error_size: usize,
    ) -> i32;
    fn kog_mt32_render(
        synth: *mut KogMt32,
        output: *mut f32,
        frames: usize,
        error: *mut c_char,
        error_size: usize,
    ) -> i32;
}

struct Mt32Synth {
    handle: NonNull<KogMt32>,
    sample_rate: u32,
    model: String,
}

// The context is owned by one Source and only moves to Rodio's playback
// thread. No Munt call is made concurrently through this handle.
unsafe impl Send for Mt32Synth {}

impl Mt32Synth {
    fn open(rom_directory: &Path) -> Result<Self, String> {
        let directory = CString::new(rom_directory.to_string_lossy().as_bytes())
            .map_err(|_| "MT-32 ROM directory contains a NUL byte".to_owned())?;
        let mut error = [0_i8; ERROR_BYTES];
        // SAFETY: `directory` and the writable error buffer remain valid for
        // the duration of the call. Ownership of a successful handle is
        // transferred to this RAII wrapper.
        let handle = unsafe {
            kog_mt32_open(
                directory.as_ptr(),
                SAMPLE_RATE,
                error.as_mut_ptr(),
                error.len(),
            )
        };
        let handle = NonNull::new(handle).ok_or_else(|| error_message(&error))?;
        // SAFETY: the successful handle remains alive and its model string is
        // owned by the native wrapper until `kog_mt32_free`.
        let model = unsafe {
            let value = kog_mt32_model(handle.as_ptr());
            if value.is_null() {
                String::new()
            } else {
                CStr::from_ptr(value).to_string_lossy().trim().to_owned()
            }
        };
        // SAFETY: the handle is valid for this query.
        let sample_rate = unsafe { kog_mt32_sample_rate(handle.as_ptr()) };
        if !(8_000..=192_000).contains(&sample_rate) || model.is_empty() {
            // SAFETY: this is the only owner of the handle.
            unsafe { kog_mt32_free(handle.as_ptr()) };
            return Err("Munt reported invalid MT-32 stream properties".to_owned());
        }
        Ok(Self {
            handle,
            sample_rate,
            model,
        })
    }

    fn send(&mut self, bytes: &[u8]) -> Result<(), String> {
        if bytes.is_empty() || bytes.len() > MAX_EVENT_BYTES {
            return Err("MT-32 MIDI event is empty or exceeds Munt's 32 KiB limit".to_owned());
        }
        let mut error = [0_i8; ERROR_BYTES];
        // SAFETY: the handle is exclusively borrowed and the event/error
        // buffers remain valid for the call.
        let success = unsafe {
            kog_mt32_send(
                self.handle.as_ptr(),
                bytes.as_ptr(),
                bytes.len(),
                error.as_mut_ptr(),
                error.len(),
            )
        };
        if success == 0 {
            Err(error_message(&error))
        } else {
            Ok(())
        }
    }

    fn render(&mut self, output: &mut [f32]) -> Result<(), String> {
        if output.is_empty() || !output.len().is_multiple_of(usize::from(CHANNELS)) {
            return Err("Munt output must contain complete stereo frames".to_owned());
        }
        let frames = output.len() / usize::from(CHANNELS);
        let mut error = [0_i8; ERROR_BYTES];
        // SAFETY: the handle is exclusively borrowed and the output/error
        // buffers are writable for their advertised lengths.
        let success = unsafe {
            kog_mt32_render(
                self.handle.as_ptr(),
                output.as_mut_ptr(),
                frames,
                error.as_mut_ptr(),
                error.len(),
            )
        };
        if success == 0 {
            Err(error_message(&error))
        } else {
            Ok(())
        }
    }
}

impl Drop for Mt32Synth {
    fn drop(&mut self) {
        // SAFETY: this wrapper is the unique owner of the native handle.
        unsafe { kog_mt32_free(self.handle.as_ptr()) };
    }
}

pub fn validate_rom_directory(path: &Path) -> Result<String, String> {
    let synth = Mt32Synth::open(path)?;
    Ok(synth.model.clone())
}

fn error_message(error: &[c_char]) -> String {
    // The native bridge always NUL-terminates this fixed buffer.
    let message = unsafe { CStr::from_ptr(error.as_ptr()) }
        .to_string_lossy()
        .trim()
        .to_owned();
    if message.is_empty() {
        "Munt MT-32 operation failed".to_owned()
    } else {
        message
    }
}

#[derive(Debug)]
struct TimelineInput {
    tick: u64,
    track: usize,
    order: usize,
    kind: TimelineInputKind,
}

#[derive(Debug)]
enum TimelineInputKind {
    Message(Vec<u8>),
    Tempo(u32),
}

#[derive(Debug, PartialEq, Eq)]
struct Mt32Event {
    frame: u64,
    bytes: Vec<u8>,
}

#[derive(Debug, PartialEq, Eq)]
struct Mt32Timeline {
    duration: Duration,
    total_frames: u64,
    events: Vec<Mt32Event>,
}

impl Mt32Timeline {
    fn parse(bytes: &[u8], gm_program_mapping: bool) -> Result<Self, String> {
        let smf = Smf::parse(bytes).map_err(|error| format!("parsing MIDI for Munt: {error}"))?;
        if smf.header.format == Format::Sequential {
            return Err(
                "SMF format 2 contains separate songs; subsong selection is not implemented yet"
                    .to_owned(),
            );
        }

        let mut inputs = Vec::new();
        let mut total_ticks = 0_u64;
        for (track_index, track) in smf.tracks.iter().enumerate() {
            let mut tick = 0_u64;
            for (event_index, event) in track.iter().enumerate() {
                tick = tick
                    .checked_add(u64::from(event.delta.as_int()))
                    .ok_or_else(|| "MIDI tick position overflowed".to_owned())?;
                let kind = match &event.kind {
                    TrackEventKind::Midi { channel, message } => Some(TimelineInputKind::Message(
                        midi_message_bytes(channel.as_int(), *message, gm_program_mapping),
                    )),
                    TrackEventKind::SysEx(data) => {
                        let mut message = Vec::with_capacity(data.len() + 1);
                        message.push(0xf0);
                        message.extend_from_slice(data);
                        Some(TimelineInputKind::Message(message))
                    }
                    TrackEventKind::Escape(data) if !data.is_empty() => {
                        Some(TimelineInputKind::Message(data.to_vec()))
                    }
                    TrackEventKind::Meta(MetaMessage::Tempo(tempo)) => {
                        Some(TimelineInputKind::Tempo(tempo.as_int()))
                    }
                    _ => None,
                };
                if let Some(kind) = kind {
                    if inputs.len() == MAX_EVENTS {
                        return Err("MIDI file has too many MT-32 events".to_owned());
                    }
                    inputs.push(TimelineInput {
                        tick,
                        track: track_index,
                        order: event_index,
                        kind,
                    });
                }
            }
            total_ticks = total_ticks.max(tick);
        }
        inputs.sort_by_key(|event| (event.tick, event.track, event.order));

        let mut clock = FrameClock::new(smf.header.timing)?;
        let mut events = Vec::with_capacity(inputs.len());
        let mut current_tick = 0_u64;
        for input in inputs {
            clock.advance(input.tick - current_tick)?;
            current_tick = input.tick;
            match input.kind {
                TimelineInputKind::Message(bytes) => {
                    if bytes.len() > MAX_EVENT_BYTES {
                        return Err("MT-32 MIDI event exceeds Munt's 32 KiB limit".to_owned());
                    }
                    events.push(Mt32Event {
                        frame: clock.frame()?,
                        bytes,
                    });
                }
                TimelineInputKind::Tempo(tempo) => clock.set_tempo(tempo),
            }
        }
        clock.advance(total_ticks - current_tick)?;
        let total_frames = clock.frame()?;
        let duration = Duration::from_secs_f64(total_frames as f64 / f64::from(SAMPLE_RATE));
        if total_frames == 0 || duration > MAX_DURATION {
            return Err("MIDI duration is outside Kog's MT-32 limit".to_owned());
        }
        Ok(Self {
            duration,
            total_frames,
            events,
        })
    }
}

fn midi_message_bytes(channel: u8, message: MidiMessage, gm_program_mapping: bool) -> Vec<u8> {
    let (status, first, second) = match message {
        MidiMessage::NoteOff { key, vel } => (0x80, key.as_int(), Some(vel.as_int())),
        MidiMessage::NoteOn { key, vel } => (0x90, key.as_int(), Some(vel.as_int())),
        MidiMessage::Aftertouch { key, vel } => (0xa0, key.as_int(), Some(vel.as_int())),
        MidiMessage::Controller { controller, value } => {
            (0xb0, controller.as_int(), Some(value.as_int()))
        }
        MidiMessage::ProgramChange { program } => {
            let program = if gm_program_mapping && channel != 9 {
                GM_TO_MT32_PROGRAM[usize::from(program.as_int())]
            } else {
                program.as_int()
            };
            (0xc0, program, None)
        }
        MidiMessage::ChannelAftertouch { vel } => (0xd0, vel.as_int(), None),
        MidiMessage::PitchBend { bend } => {
            let raw = bend.0.as_int();
            (0xe0, (raw & 0x7f) as u8, Some((raw >> 7) as u8))
        }
    };
    let mut bytes = Vec::with_capacity(if second.is_some() { 3 } else { 2 });
    bytes.push(status | channel);
    bytes.push(first);
    if let Some(second) = second {
        bytes.push(second);
    }
    bytes
}

enum ClockKind {
    Metrical {
        ticks_per_beat: u128,
        tempo: u128,
    },
    Timecode {
        fps_numerator: u128,
        fps_denominator: u128,
        subframe: u128,
    },
}

struct FrameClock {
    kind: ClockKind,
    frame: u128,
    remainder: u128,
}

impl FrameClock {
    fn new(timing: Timing) -> Result<Self, String> {
        let kind = match timing {
            Timing::Metrical(ticks_per_beat) => {
                let ticks_per_beat = u128::from(ticks_per_beat.as_int());
                if ticks_per_beat == 0 {
                    return Err("MIDI metrical timing has zero ticks per beat".to_owned());
                }
                ClockKind::Metrical {
                    ticks_per_beat,
                    tempo: 500_000,
                }
            }
            Timing::Timecode(fps, subframe) => {
                if subframe == 0 {
                    return Err("MIDI timecode timing has zero subframes".to_owned());
                }
                let (fps_numerator, fps_denominator) = match fps {
                    Fps::Fps24 => (24, 1),
                    Fps::Fps25 => (25, 1),
                    Fps::Fps29 => (30_000, 1_001),
                    Fps::Fps30 => (30, 1),
                };
                ClockKind::Timecode {
                    fps_numerator,
                    fps_denominator,
                    subframe: u128::from(subframe),
                }
            }
        };
        Ok(Self {
            kind,
            frame: 0,
            remainder: 0,
        })
    }

    fn advance(&mut self, ticks: u64) -> Result<(), String> {
        let (numerator_per_tick, denominator) = match self.kind {
            ClockKind::Metrical {
                ticks_per_beat,
                tempo,
            } => (u128::from(SAMPLE_RATE) * tempo, ticks_per_beat * 1_000_000),
            ClockKind::Timecode {
                fps_numerator,
                fps_denominator,
                subframe,
            } => (
                u128::from(SAMPLE_RATE) * fps_denominator,
                fps_numerator * subframe,
            ),
        };
        let numerator = u128::from(ticks)
            .checked_mul(numerator_per_tick)
            .and_then(|value| value.checked_add(self.remainder))
            .ok_or_else(|| "MIDI frame position overflowed".to_owned())?;
        self.frame = self
            .frame
            .checked_add(numerator / denominator)
            .ok_or_else(|| "MIDI frame position overflowed".to_owned())?;
        self.remainder = numerator % denominator;
        Ok(())
    }

    fn set_tempo(&mut self, tempo: u32) {
        if let ClockKind::Metrical { tempo: current, .. } = &mut self.kind {
            *current = u128::from(tempo);
        }
    }

    fn frame(&self) -> Result<u64, String> {
        u64::try_from(self.frame).map_err(|_| "MIDI duration exceeds Kog's limit".to_owned())
    }
}

pub struct Mt32Source {
    timeline: Arc<Mt32Timeline>,
    rom_directory: PathBuf,
    synth: Mt32Synth,
    event_index: usize,
    frames_rendered: u64,
    samples_emitted: u64,
    interleaved: Vec<f32>,
    interleaved_index: usize,
    render_error: Option<String>,
}

impl Mt32Source {
    pub fn open(
        bytes: &[u8],
        rom_directory: &Path,
        gm_program_mapping: bool,
    ) -> Result<Self, String> {
        let timeline = Arc::new(Mt32Timeline::parse(bytes, gm_program_mapping)?);
        let synth = Mt32Synth::open(rom_directory)?;
        Ok(Self {
            timeline,
            rom_directory: rom_directory.to_path_buf(),
            synth,
            event_index: 0,
            frames_rendered: 0,
            samples_emitted: 0,
            interleaved: Vec::with_capacity(RENDER_FRAMES * usize::from(CHANNELS)),
            interleaved_index: 0,
            render_error: None,
        })
    }

    #[cfg(test)]
    pub fn model(&self) -> &str {
        &self.synth.model
    }

    fn render_frames(&mut self, frames: usize, emit: bool) -> Result<(), String> {
        let samples = frames
            .checked_mul(usize::from(CHANNELS))
            .ok_or_else(|| "MT-32 render request exceeds Kog's buffer limit".to_owned())?;
        self.interleaved.resize(samples, 0.0);
        let mut rendered = 0_usize;
        while rendered < frames {
            let absolute_frame = self.frames_rendered + rendered as u64;
            while let Some(event) = self.timeline.events.get(self.event_index) {
                if event.frame > absolute_frame {
                    break;
                }
                self.synth.send(&event.bytes)?;
                self.event_index += 1;
            }
            let remaining = frames - rendered;
            let until_event = self
                .timeline
                .events
                .get(self.event_index)
                .map(|event| event.frame.saturating_sub(absolute_frame))
                .unwrap_or(remaining as u64);
            let segment = usize::try_from(until_event)
                .unwrap_or(usize::MAX)
                .min(remaining);
            debug_assert!(segment > 0);
            let channels = usize::from(CHANNELS);
            self.synth.render(
                &mut self.interleaved[rendered * channels..(rendered + segment) * channels],
            )?;
            rendered += segment;
        }
        self.frames_rendered += frames as u64;
        if emit {
            self.interleaved_index = 0;
        } else {
            self.interleaved.clear();
            self.interleaved_index = 0;
        }
        Ok(())
    }

    fn fill_interleaved(&mut self) {
        let frames = usize::try_from(
            self.timeline
                .total_frames
                .saturating_sub(self.frames_rendered),
        )
        .unwrap_or(usize::MAX)
        .min(RENDER_FRAMES);
        if frames == 0 {
            return;
        }
        if let Err(error) = self.render_frames(frames, true) {
            eprintln!("Kog Munt playback stopped: {error}");
            self.render_error = Some(error);
            self.interleaved.clear();
            self.interleaved_index = 0;
        }
    }

    fn seek_to(&mut self, position: Duration) -> Result<(), String> {
        self.synth = Mt32Synth::open(&self.rom_directory)?;
        self.event_index = 0;
        self.frames_rendered = 0;
        self.samples_emitted = 0;
        self.interleaved.clear();
        self.interleaved_index = 0;
        self.render_error = None;
        let target = position.min(self.timeline.duration);
        let target_frames = (target.as_secs_f64() * f64::from(SAMPLE_RATE)).floor() as u64;
        while self.frames_rendered < target_frames {
            let frames = usize::try_from(target_frames - self.frames_rendered)
                .unwrap_or(usize::MAX)
                .min(RENDER_FRAMES);
            self.render_frames(frames, false)?;
        }
        self.samples_emitted = target_frames * u64::from(CHANNELS);
        Ok(())
    }
}

/// Read MIDI timing for playlist metadata without constructing a Munt synth or
/// loading ROMs. The synth is opened only when playback actually starts.
pub fn midi_duration(bytes: &[u8]) -> Result<Duration, String> {
    Mt32Timeline::parse(bytes, false).map(|timeline| timeline.duration)
}

pub const fn output_sample_rate() -> u32 {
    SAMPLE_RATE
}

impl Iterator for Mt32Source {
    type Item = f32;

    fn next(&mut self) -> Option<Self::Item> {
        let total_samples = self.timeline.total_frames * u64::from(CHANNELS);
        if self.render_error.is_some() || self.samples_emitted >= total_samples {
            return None;
        }
        if self.interleaved_index == self.interleaved.len() {
            self.fill_interleaved();
        }
        let sample = *self.interleaved.get(self.interleaved_index)?;
        self.interleaved_index += 1;
        self.samples_emitted += 1;
        Some(sample)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = if self.render_error.is_some() {
            0
        } else {
            self.timeline
                .total_frames
                .saturating_mul(u64::from(CHANNELS))
                .saturating_sub(self.samples_emitted)
        };
        let remaining = usize::try_from(remaining).unwrap_or(usize::MAX);
        (remaining, Some(remaining))
    }
}

impl Source for Mt32Source {
    fn current_span_len(&self) -> Option<usize> {
        self.size_hint().1
    }

    fn channels(&self) -> ChannelCount {
        NonZeroU16::new(CHANNELS).expect("MT-32 output is stereo")
    }

    fn sample_rate(&self) -> SampleRate {
        NonZeroU32::new(self.synth.sample_rate).expect("validated Munt sample rate is nonzero")
    }

    fn total_duration(&self) -> Option<Duration> {
        Some(self.timeline.duration)
    }

    fn try_seek(&mut self, position: Duration) -> Result<(), SeekError> {
        self.seek_to(position)
            .map_err(|error| SeekError::Other(Arc::new(std::io::Error::other(error))))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn minimal_midi() -> Vec<u8> {
        vec![
            b'M', b'T', b'h', b'd', 0, 0, 0, 6, 0, 0, 0, 1, 1, 0xe0, b'M', b'T', b'r', b'k', 0, 0,
            0, 16, 0, 0xc0, 0, 0, 0x90, 60, 100, 0x83, 0x60, 0x80, 60, 0, 0, 0xff, 0x2f, 0,
        ]
    }

    fn format_two_midi() -> Vec<u8> {
        let mut midi = vec![b'M', b'T', b'h', b'd', 0, 0, 0, 6, 0, 2, 0, 2, 1, 0xe0];
        for (name, note, duration) in [
            ("First", 60_u8, [0x83, 0x60]),
            ("Second", 67_u8, [0x87, 0x40]),
        ] {
            let mut track = vec![0, 0xff, 0x03, name.len() as u8];
            track.extend_from_slice(name.as_bytes());
            track.extend_from_slice(&[0, 0x90, note, 100]);
            track.extend_from_slice(&duration);
            track.extend_from_slice(&[0x80, note, 0, 0, 0xff, 0x2f, 0]);
            midi.extend_from_slice(b"MTrk");
            midi.extend_from_slice(&(track.len() as u32).to_be_bytes());
            midi.extend_from_slice(&track);
        }
        midi
    }

    #[test]
    fn timeline_preserves_messages_and_duration() {
        let timeline = Mt32Timeline::parse(&minimal_midi(), false).expect("parse generated MIDI");
        assert_eq!(timeline.duration, Duration::from_millis(500));
        assert_eq!(timeline.total_frames, 24_000);
        assert_eq!(timeline.events.len(), 3);
        assert_eq!(timeline.events[0].bytes, [0xc0, 0]);
        assert_eq!(timeline.events[1].bytes, [0x90, 60, 100]);
        assert_eq!(timeline.events[2].bytes, [0x80, 60, 0]);
        assert_eq!(timeline.events[2].frame, 24_000);
    }

    #[test]
    fn general_midi_program_mapping_is_optional_and_skips_rhythm_channel() {
        let mut midi = minimal_midi();
        let program_event = midi
            .windows(2)
            .position(|bytes| bytes == [0xc0, 0])
            .expect("generated program change");
        midi[program_event + 1] = 2; // GM Electric Grand -> MT-32 E.Piano 3.

        let native = Mt32Timeline::parse(&midi, false).expect("parse native programs");
        let mapped = Mt32Timeline::parse(&midi, true).expect("parse mapped programs");
        assert_eq!(native.events[0].bytes, [0xc0, 2]);
        assert_eq!(mapped.events[0].bytes, [0xc0, 60]);

        midi[program_event] = 0xc9;
        let rhythm = Mt32Timeline::parse(&midi, true).expect("parse rhythm program");
        assert_eq!(rhythm.events[0].bytes, [0xc9, 2]);
    }

    #[test]
    fn timeline_renders_only_the_selected_format_two_subsong() {
        let selected = crate::decoder::select_standard_midi_subsong(&format_two_midi(), Some(1))
            .expect("select second format 2 song");
        let timeline =
            Mt32Timeline::parse(&selected.bytes, false).expect("parse selected Munt song");

        assert_eq!(selected.title.as_deref(), Some("Second"));
        assert_eq!(selected.subsong_count, Some(2));
        assert_eq!(timeline.duration, Duration::from_secs(1));
        assert_eq!(timeline.total_frames, 48_000);
        assert_eq!(timeline.events.len(), 2);
        assert_eq!(timeline.events[0].bytes, [0x90, 67, 100]);
        assert_eq!(timeline.events[1].bytes, [0x80, 67, 0]);
    }

    #[test]
    fn empty_rom_directory_is_rejected() {
        let roms = tempfile::tempdir().expect("create empty ROM directory");
        let error = Mt32Synth::open(roms.path())
            .err()
            .expect("missing ROM rejection");
        assert!(
            error.contains("no complete compatible MT-32/CM-32L ROM pair"),
            "unexpected error: {error}"
        );
    }

    #[test]
    #[ignore = "requires user-supplied Roland ROMs through KOG_MT32_ROMS"]
    fn user_rom_gate_recognizes_and_renders_mt32() {
        let path = std::env::var_os("KOG_MT32_ROMS")
            .map(PathBuf::from)
            .expect("set KOG_MT32_ROMS to a user-owned ROM directory");
        let mut midi = minimal_midi();
        for byte in &mut midi {
            *byte = match *byte {
                0xc0 => 0xc2,
                0x90 => 0x92,
                0x80 => 0x82,
                value => value,
            };
        }
        let mut source = Mt32Source::open(&midi, &path, true).expect("open real Munt ROM set");
        assert!(
            source.model().contains("MT-32"),
            "model: {}",
            source.model()
        );
        let samples = source.by_ref().take(48_000).collect::<Vec<_>>();
        assert_eq!(samples.len(), 48_000);
        assert!(samples.iter().any(|sample| sample.abs() > 0.000_01));
    }
}
