//! Nuked SC-55 PCM stream and seek cache. Every frontend runs the pinned
//! emulator library in-process through the shared native renderer.

use std::collections::HashSet;
use std::ffi::{CString, c_char, c_int};
use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::thread::JoinHandle;
use std::time::Duration;

use midly::{Format, Fps, MetaMessage, MidiMessage, Smf, Timing, TrackEventKind};
use tempfile::NamedTempFile;

const SCHEDULE_MAGIC: [u8; 8] = *b"KOGSCM1\0";
const RESPONSE_MAGIC: [u8; 8] = *b"KOGSC551";
const PROTOCOL_VERSION: u32 = 1;
const CHANNELS: u16 = 2;
const NANOSECOND_RATE: u32 = 1_000_000_000;
const MAX_EVENTS: usize = 2_000_000;
const MAX_EVENT_BYTES: usize = 1024 * 1024;
const MAX_SCHEDULE_BYTES: usize = 256 * 1024 * 1024;
const MAX_DURATION: Duration = Duration::from_secs(24 * 60 * 60);
const MAX_MODEL_BYTES: usize = 256;
const BYTES_PER_FRAME: u64 = CHANNELS as u64 * 2;
const CACHE_COPY_BYTES: usize = 256 * 1024;

pub struct Sc55 {
    _schedule: NamedTempFile,
    _cache: NamedTempFile,
    cache_reader: File,
    cache_state: Arc<Sc55CacheState>,
    cache_worker: Option<JoinHandle<()>>,
    sample_rate: u32,
    total_frames: u64,
    rendered_frames: u64,
    #[cfg(any(test, feature = "test-util"))]
    model: String,
    native_bytes: Vec<u8>,
}

unsafe extern "C" {
    fn kog_sc55_version() -> *const c_char;
    fn kog_sc55_warm(roms: *const c_char, error: *mut c_char, error_capacity: usize) -> c_int;
    fn kog_sc55_shutdown();
    fn kog_sc55_render(
        schedule: *const c_char,
        roms: *const c_char,
        output_descriptor: isize,
        error: *mut c_char,
        error_capacity: usize,
    ) -> c_int;
}

struct Sc55CacheState {
    progress: Mutex<Sc55CacheProgress>,
    ready: Condvar,
    stopping: AtomicBool,
}

#[derive(Default)]
struct Sc55CacheProgress {
    available_bytes: u64,
    finished: bool,
    error: Option<String>,
}

#[derive(Debug, PartialEq, Eq)]
struct HelperHeader {
    sample_rate: u32,
    channels: u16,
    total_frames: u64,
    start_frame: u64,
    model: String,
}

#[derive(Debug)]
struct ScheduleInput {
    tick: u64,
    track: usize,
    order: usize,
    kind: ScheduleInputKind,
}

#[derive(Debug)]
enum ScheduleInputKind {
    Message(Vec<u8>),
    Tempo(u32),
}

#[derive(Debug, PartialEq, Eq)]
struct ScheduledEvent {
    nanoseconds: u64,
    bytes: Vec<u8>,
}

#[derive(Debug, PartialEq, Eq)]
struct Sc55Schedule {
    duration: Duration,
    events: Vec<ScheduledEvent>,
}

impl Sc55 {
    pub fn open(midi: &[u8], path: &Path, rom_directory: &Path) -> Result<Self, String> {
        let schedule = Sc55Schedule::parse(midi)?;
        // Android's default /data/local/tmp is not writable by an app. ROMs
        // imported by the mobile clients live under their private data area,
        // so keep both the schedule and seek cache alongside that directory.
        #[cfg(any(target_os = "ios", target_os = "android"))]
        let mut schedule_file = NamedTempFile::new_in(
            rom_directory
                .parent()
                .ok_or("SC-55 ROM directory has no parent")?,
        )
        .map_err(|error| format!("creating SC-55 schedule: {error}"))?;
        #[cfg(not(any(target_os = "ios", target_os = "android")))]
        let mut schedule_file =
            NamedTempFile::new().map_err(|error| format!("creating SC-55 schedule: {error}"))?;
        schedule.write(&mut schedule_file)?;
        schedule_file
            .flush()
            .map_err(|error| format!("flushing SC-55 schedule: {error}"))?;
        Self::open_embedded(schedule_file, rom_directory, path)
    }

    fn open_embedded(
        schedule_file: NamedTempFile,
        rom_directory: &Path,
        path: &Path,
    ) -> Result<Self, String> {
        let (reader, header) = spawn_embedded(schedule_file.path(), rom_directory)?;
        validate_header(&header, 0, path)?;
        Self::from_stream(schedule_file, header, reader)
    }

    /// Shared tail: cache tempfiles, background PCM copy, and struct.
    fn from_stream(
        schedule_file: NamedTempFile,
        header: HelperHeader,
        pcm: impl Read + Send + 'static,
    ) -> Result<Self, String> {
        #[cfg(any(target_os = "ios", target_os = "android"))]
        let cache = NamedTempFile::new_in(
            schedule_file
                .path()
                .parent()
                .expect("SC-55 schedule has a parent"),
        )
        .map_err(|error| format!("creating SC-55 PCM seek cache: {error}"))?;
        #[cfg(not(any(target_os = "ios", target_os = "android")))]
        let cache = NamedTempFile::new()
            .map_err(|error| format!("creating SC-55 PCM seek cache: {error}"))?;
        let cache_reader = cache
            .reopen()
            .map_err(|error| format!("opening SC-55 PCM seek cache for playback: {error}"))?;
        let cache_writer = cache
            .reopen()
            .map_err(|error| format!("opening SC-55 PCM seek cache for rendering: {error}"))?;
        let expected_bytes = header
            .total_frames
            .checked_mul(BYTES_PER_FRAME)
            .ok_or_else(|| "Nuked SC-55 PCM cache size overflowed".to_owned())?;
        let cache_state = Arc::new(Sc55CacheState {
            progress: Mutex::new(Sc55CacheProgress::default()),
            ready: Condvar::new(),
            stopping: AtomicBool::new(false),
        });
        let worker_state = cache_state.clone();
        let cache_worker = match std::thread::Builder::new()
            .name("kog-sc55-cache".to_owned())
            .spawn(move || cache_helper_pcm(pcm, cache_writer, expected_bytes, worker_state))
        {
            Ok(worker) => worker,
            Err(error) => {
                return Err(format!("starting SC-55 PCM cache worker: {error}"));
            }
        };
        Ok(Self {
            _schedule: schedule_file,
            _cache: cache,
            cache_reader,
            cache_state,
            cache_worker: Some(cache_worker),
            sample_rate: header.sample_rate,
            total_frames: header.total_frames,
            rendered_frames: 0,
            #[cfg(any(test, feature = "test-util"))]
            model: header.model,
            native_bytes: Vec::new(),
        })
    }

    pub fn duration(&self) -> Duration {
        duration_from_frames(self.total_frames, self.sample_rate)
    }

    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    pub fn total_frames(&self) -> u64 {
        self.total_frames
    }

    pub fn rendered_frames(&self) -> u64 {
        self.rendered_frames
    }

    #[cfg(any(test, feature = "test-util"))]
    pub fn model(&self) -> &str {
        &self.model
    }

    pub fn render(&mut self, output: &mut [f32]) -> Result<usize, String> {
        let channels = usize::from(CHANNELS);
        if output.is_empty() || !output.len().is_multiple_of(channels) {
            return Err("Nuked SC-55 output must contain complete stereo frames".to_owned());
        }
        let remaining = self.total_frames.saturating_sub(self.rendered_frames);
        let requested = usize::try_from(remaining.min((output.len() / channels) as u64))
            .expect("requested SC-55 frames fit the output buffer");
        if requested == 0 {
            return Ok(0);
        }
        let byte_count = requested
            .checked_mul(channels)
            .and_then(|samples| samples.checked_mul(2))
            .ok_or_else(|| "Nuked SC-55 render request exceeds Kog's buffer limit".to_owned())?;
        let byte_offset = self
            .rendered_frames
            .checked_mul(BYTES_PER_FRAME)
            .ok_or_else(|| "Nuked SC-55 PCM cache offset overflowed".to_owned())?;
        let byte_end = byte_offset
            .checked_add(byte_count as u64)
            .ok_or_else(|| "Nuked SC-55 PCM cache request overflowed".to_owned())?;
        let mut progress = lock_unpoisoned(&self.cache_state.progress);
        while progress.available_bytes < byte_end && !progress.finished && progress.error.is_none()
        {
            progress = self
                .cache_state
                .ready
                .wait(progress)
                .unwrap_or_else(|poisoned| poisoned.into_inner());
        }
        if progress.available_bytes < byte_end {
            return Err(progress.error.clone().unwrap_or_else(|| {
                format!(
                    "Nuked SC-55 renderer ended after {} of {byte_end} required PCM bytes",
                    progress.available_bytes
                )
            }));
        }
        drop(progress);
        self.cache_reader
            .seek(SeekFrom::Start(byte_offset))
            .and_then(|_| {
                self.native_bytes.resize(byte_count, 0);
                self.cache_reader.read_exact(&mut self.native_bytes)
            })
            .map_err(|error| format!("reading the Nuked SC-55 PCM seek cache: {error}"))?;
        for (destination, bytes) in output.iter_mut().zip(self.native_bytes.chunks_exact(2)) {
            *destination = f32::from(i16::from_le_bytes([bytes[0], bytes[1]])) / 32768.0;
        }
        self.rendered_frames += requested as u64;
        Ok(requested)
    }

    pub fn seek(&mut self, position: Duration) -> Result<Duration, String> {
        let target = if position >= self.duration() {
            self.total_frames
        } else {
            frames_from_duration(position, self.sample_rate)?
        };
        let byte_offset = target
            .checked_mul(BYTES_PER_FRAME)
            .ok_or_else(|| "Nuked SC-55 PCM cache seek offset overflowed".to_owned())?;
        self.cache_reader
            .seek(SeekFrom::Start(byte_offset))
            .map_err(|error| format!("seeking the Nuked SC-55 PCM cache: {error}"))?;
        self.rendered_frames = target;
        Ok(duration_from_frames(target, self.sample_rate))
    }
}

/// Read the duration needed for playlist metadata without starting the
/// expensive Nuked SC-55 emulator. Initialization belongs to playback,
/// not library scanning: starting the core per MIDI file makes archive and
/// folder imports appear to hang.
pub fn midi_duration(midi: &[u8]) -> Result<Duration, String> {
    Sc55Schedule::parse(midi).map(|schedule| schedule.duration)
}

impl Drop for Sc55 {
    fn drop(&mut self) {
        self.cache_state.stopping.store(true, Ordering::Release);
        self.cache_state.ready.notify_all();
        if let Some(worker) = self.cache_worker.take() {
            let _ = worker.join();
        }
    }
}

pub fn validate_rom_directory(path: &Path) -> Result<String, String> {
    let schedule = Sc55Schedule {
        duration: Duration::from_millis(1),
        events: Vec::new(),
    };
    let mut schedule_file = NamedTempFile::new()
        .map_err(|error| format!("creating SC-55 validation schedule: {error}"))?;
    schedule.write(&mut schedule_file)?;
    schedule_file
        .flush()
        .map_err(|error| format!("flushing SC-55 validation schedule: {error}"))?;
    let (mut renderer, header) = spawn_embedded(schedule_file.path(), path)?;
    renderer.cancel();
    Ok(header.model)
}

fn spawn_embedded(
    schedule_path: &Path,
    rom_directory: &Path,
) -> Result<(crate::embedded_helper::EmbeddedHelper, HelperHeader), String> {
    let schedule = CString::new(schedule_path.to_string_lossy().as_bytes())
        .map_err(|_| "SC-55 schedule path contains a null byte".to_owned())?;
    let roms = CString::new(rom_directory.to_string_lossy().as_bytes())
        .map_err(|_| "SC-55 ROM path contains a null byte".to_owned())?;
    let mut stream =
        crate::embedded_helper::EmbeddedHelper::spawn(move |descriptor, error| unsafe {
            kog_sc55_render(
                schedule.as_ptr(),
                roms.as_ptr(),
                descriptor,
                error.as_mut_ptr(),
                error.len(),
            )
        })?;
    let header = HelperHeader::read(&mut stream).map_err(|read_error| {
        let detail = stream.failure().unwrap_or_default();
        if detail.is_empty() {
            format!("starting SC-55 renderer: {read_error}")
        } else {
            format!("starting SC-55 renderer: {detail}")
        }
    })?;
    Ok((stream, header))
}

impl Sc55Schedule {
    fn parse(bytes: &[u8]) -> Result<Self, String> {
        let smf = Smf::parse(bytes).map_err(|error| format!("parsing MIDI for SC-55: {error}"))?;
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
                    TrackEventKind::Midi { channel, message } => Some(ScheduleInputKind::Message(
                        midi_message_bytes(channel.as_int(), *message),
                    )),
                    TrackEventKind::SysEx(data) => {
                        let mut message = Vec::with_capacity(data.len() + 1);
                        message.push(0xf0);
                        message.extend_from_slice(data);
                        Some(ScheduleInputKind::Message(message))
                    }
                    TrackEventKind::Escape(data) if !data.is_empty() => {
                        Some(ScheduleInputKind::Message(data.to_vec()))
                    }
                    TrackEventKind::Meta(MetaMessage::Tempo(tempo)) => {
                        Some(ScheduleInputKind::Tempo(tempo.as_int()))
                    }
                    _ => None,
                };
                if let Some(kind) = kind {
                    if inputs.len() == MAX_EVENTS {
                        return Err("MIDI file has too many SC-55 events".to_owned());
                    }
                    inputs.push(ScheduleInput {
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

        let mut clock = NanosecondClock::new(smf.header.timing)?;
        let mut events = Vec::with_capacity(inputs.len());
        let mut current_tick = 0_u64;
        for input in inputs {
            clock.advance(input.tick - current_tick)?;
            current_tick = input.tick;
            match input.kind {
                ScheduleInputKind::Message(bytes) => {
                    if bytes.len() > MAX_EVENT_BYTES {
                        return Err("MIDI UART event exceeds Kog's 1 MiB limit".to_owned());
                    }
                    events.push(ScheduledEvent {
                        nanoseconds: clock.nanoseconds()?,
                        bytes,
                    });
                }
                ScheduleInputKind::Tempo(tempo) => clock.set_tempo(tempo),
            }
        }
        clock.advance(total_ticks - current_tick)?;
        let duration = Duration::from_nanos(clock.nanoseconds()?);
        if duration.is_zero() || duration > MAX_DURATION {
            return Err("MIDI duration is outside Kog's SC-55 limit".to_owned());
        }
        Ok(Self { duration, events })
    }

    fn write(&self, writer: &mut impl Write) -> Result<(), String> {
        let mut byte_count = 8_usize + 4 + 8 + 4;
        for event in &self.events {
            byte_count = byte_count
                .checked_add(8 + 4)
                .and_then(|size| size.checked_add(event.bytes.len()))
                .ok_or_else(|| "SC-55 schedule size overflowed".to_owned())?;
        }
        if byte_count > MAX_SCHEDULE_BYTES {
            return Err("SC-55 schedule exceeds Kog's 256 MiB limit".to_owned());
        }
        let duration_nanoseconds = duration_to_nanoseconds(self.duration)?;
        writer
            .write_all(&SCHEDULE_MAGIC)
            .map_err(|error| format!("writing SC-55 schedule magic: {error}"))?;
        writer
            .write_all(&PROTOCOL_VERSION.to_le_bytes())
            .map_err(|error| format!("writing SC-55 schedule version: {error}"))?;
        writer
            .write_all(&duration_nanoseconds.to_le_bytes())
            .map_err(|error| format!("writing SC-55 schedule duration: {error}"))?;
        writer
            .write_all(&(self.events.len() as u32).to_le_bytes())
            .map_err(|error| format!("writing SC-55 schedule event count: {error}"))?;
        for event in &self.events {
            writer
                .write_all(&event.nanoseconds.to_le_bytes())
                .map_err(|error| format!("writing SC-55 event timestamp: {error}"))?;
            writer
                .write_all(&(event.bytes.len() as u32).to_le_bytes())
                .map_err(|error| format!("writing SC-55 event length: {error}"))?;
            writer
                .write_all(&event.bytes)
                .map_err(|error| format!("writing SC-55 event bytes: {error}"))?;
        }
        Ok(())
    }
}

fn midi_message_bytes(channel: u8, message: MidiMessage) -> Vec<u8> {
    let (status, first, second) = match message {
        MidiMessage::NoteOff { key, vel } => (0x80, key.as_int(), Some(vel.as_int())),
        MidiMessage::NoteOn { key, vel } => (0x90, key.as_int(), Some(vel.as_int())),
        MidiMessage::Aftertouch { key, vel } => (0xa0, key.as_int(), Some(vel.as_int())),
        MidiMessage::Controller { controller, value } => {
            (0xb0, controller.as_int(), Some(value.as_int()))
        }
        MidiMessage::ProgramChange { program } => (0xc0, program.as_int(), None),
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

struct NanosecondClock {
    kind: ClockKind,
    nanoseconds: u128,
    remainder: u128,
}

impl NanosecondClock {
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
            nanoseconds: 0,
            remainder: 0,
        })
    }

    fn advance(&mut self, ticks: u64) -> Result<(), String> {
        let (numerator_per_tick, denominator) = match self.kind {
            ClockKind::Metrical {
                ticks_per_beat,
                tempo,
            } => (
                u128::from(NANOSECOND_RATE) * tempo,
                ticks_per_beat * 1_000_000,
            ),
            ClockKind::Timecode {
                fps_numerator,
                fps_denominator,
                subframe,
            } => (
                u128::from(NANOSECOND_RATE) * fps_denominator,
                fps_numerator * subframe,
            ),
        };
        let numerator = u128::from(ticks)
            .checked_mul(numerator_per_tick)
            .and_then(|value| value.checked_add(self.remainder))
            .ok_or_else(|| "MIDI nanosecond position overflowed".to_owned())?;
        self.nanoseconds = self
            .nanoseconds
            .checked_add(numerator / denominator)
            .ok_or_else(|| "MIDI nanosecond position overflowed".to_owned())?;
        self.remainder = numerator % denominator;
        Ok(())
    }

    fn set_tempo(&mut self, tempo: u32) {
        if let ClockKind::Metrical { tempo: current, .. } = &mut self.kind {
            *current = u128::from(tempo);
        }
    }

    fn nanoseconds(&self) -> Result<u64, String> {
        u64::try_from(self.nanoseconds).map_err(|_| "MIDI duration exceeds Kog's limit".to_owned())
    }
}

impl HelperHeader {
    fn read(reader: &mut impl Read) -> io::Result<Self> {
        let mut magic = [0_u8; 8];
        reader.read_exact(&mut magic)?;
        if magic != RESPONSE_MAGIC {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "invalid Nuked SC-55 helper protocol magic",
            ));
        }
        let version = read_u32_le(reader)?;
        if version != PROTOCOL_VERSION {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("unsupported Nuked SC-55 helper protocol {version}"),
            ));
        }
        let sample_rate = read_u32_le(reader)?;
        let channels = u16::try_from(read_u32_le(reader)?).map_err(|_| {
            io::Error::new(io::ErrorKind::InvalidData, "invalid SC-55 channel count")
        })?;
        let total_frames = read_u64_le(reader)?;
        let start_frame = read_u64_le(reader)?;
        let model_length = usize::try_from(read_u32_le(reader)?).map_err(|_| {
            io::Error::new(io::ErrorKind::InvalidData, "SC-55 model length overflow")
        })?;
        if model_length > MAX_MODEL_BYTES {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "SC-55 helper model name exceeds Kog's limit",
            ));
        }
        let mut model = vec![0_u8; model_length];
        reader.read_exact(&mut model)?;
        Ok(Self {
            sample_rate,
            channels,
            total_frames,
            start_frame,
            model: String::from_utf8_lossy(&model).trim().to_owned(),
        })
    }
}

fn validate_header(header: &HelperHeader, start_frame: u64, path: &Path) -> Result<(), String> {
    if header.sample_rate < 8_000
        || header.sample_rate > 192_000
        || header.channels != CHANNELS
        || header.total_frames == 0
        || header.start_frame != start_frame
        || start_frame > header.total_frames
        || header.model.is_empty()
        || duration_from_frames(header.total_frames, header.sample_rate) > MAX_DURATION
    {
        return Err(format!(
            "Nuked SC-55 helper reported invalid stream properties for {}",
            path.display()
        ));
    }
    Ok(())
}

fn cache_helper_pcm(
    mut input: impl Read + Send,
    mut cache: File,
    expected_bytes: u64,
    state: Arc<Sc55CacheState>,
) {
    let mut copied = 0_u64;
    let stream_result = (|| -> Result<(), String> {
        let mut buffer = vec![0_u8; CACHE_COPY_BYTES];
        loop {
            if state.stopping.load(Ordering::Acquire) {
                return Ok(());
            }
            let count = input
                .read(&mut buffer)
                .map_err(|error| format!("reading PCM from the Nuked SC-55 renderer: {error}"))?;
            if count == 0 {
                break;
            }
            let next = copied
                .checked_add(count as u64)
                .ok_or_else(|| "Nuked SC-55 PCM cache length overflowed".to_owned())?;
            if next > expected_bytes {
                return Err("Nuked SC-55 renderer produced more PCM than expected".to_owned());
            }
            cache
                .write_all(&buffer[..count])
                .map_err(|error| format!("writing the Nuked SC-55 PCM seek cache: {error}"))?;
            copied = next;
            lock_unpoisoned(&state.progress).available_bytes = copied;
            state.ready.notify_all();
        }
        if copied != expected_bytes {
            return Err(format!(
                "Nuked SC-55 renderer produced {copied} of {expected_bytes} expected PCM bytes"
            ));
        }
        Ok(())
    })();

    if state.stopping.load(Ordering::Acquire) {
        return;
    }

    let error = stream_result.err();
    let mut progress = lock_unpoisoned(&state.progress);
    progress.finished = true;
    progress.error = error;
    drop(progress);
    state.ready.notify_all();
}

fn lock_unpoisoned<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn warming_pool() -> &'static Mutex<HashSet<PathBuf>> {
    static POOL: std::sync::OnceLock<Mutex<HashSet<PathBuf>>> = std::sync::OnceLock::new();
    POOL.get_or_init(|| Mutex::new(HashSet::new()))
}

/// Preload the linked emulator on a background thread. The first render waits
/// for that same instance if boot is still in progress; later tracks reuse it.
pub fn warm_sc55_server(rom_dir: &Path) {
    let rom_dir = rom_dir.to_path_buf();
    if !lock_unpoisoned(warming_pool()).insert(rom_dir.clone()) {
        return;
    }
    let _ = std::thread::Builder::new()
        .name("kog-sc55-boot".to_owned())
        .spawn(move || {
            let result = CString::new(rom_dir.to_string_lossy().as_bytes())
                .map_err(|_| "SC-55 ROM path contains a null byte".to_owned())
                .and_then(|path| {
                    let mut error = [0 as c_char; 1024];
                    let status =
                        unsafe { kog_sc55_warm(path.as_ptr(), error.as_mut_ptr(), error.len()) };
                    if status == 0 {
                        return Ok(());
                    }
                    let detail = unsafe { std::ffi::CStr::from_ptr(error.as_ptr()) };
                    Err(detail.to_string_lossy().into_owned())
                });
            if let Err(error) = result {
                lock_unpoisoned(warming_pool()).remove(&rom_dir);
                eprintln!("kog: SC-55 preload failed: {error}");
            }
        });
}

/// Release pooled emulator instances when the application exits.
pub fn shutdown_sc55_servers() {
    lock_unpoisoned(warming_pool()).clear();
    unsafe {
        kog_sc55_shutdown();
    }
}

fn read_u32_le(reader: &mut impl Read) -> io::Result<u32> {
    let mut bytes = [0_u8; 4];
    reader.read_exact(&mut bytes)?;
    Ok(u32::from_le_bytes(bytes))
}

fn read_u64_le(reader: &mut impl Read) -> io::Result<u64> {
    let mut bytes = [0_u8; 8];
    reader.read_exact(&mut bytes)?;
    Ok(u64::from_le_bytes(bytes))
}

fn duration_to_nanoseconds(duration: Duration) -> Result<u64, String> {
    u64::try_from(duration.as_nanos()).map_err(|_| "SC-55 duration exceeds Kog's limit".to_owned())
}

fn duration_from_frames(frames: u64, sample_rate: u32) -> Duration {
    Duration::from_secs_f64(frames as f64 / f64::from(sample_rate))
}

fn frames_from_duration(duration: Duration, sample_rate: u32) -> Result<u64, String> {
    let frames = duration
        .as_nanos()
        .checked_mul(u128::from(sample_rate))
        .ok_or_else(|| "SC-55 seek position overflowed".to_owned())?
        / u128::from(NANOSECOND_RATE);
    u64::try_from(frames).map_err(|_| "SC-55 seek position exceeds Kog's limit".to_owned())
}

#[cfg(any(test, feature = "test-util"))]
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
    fn schedule_preserves_uart_messages_and_duration() {
        let schedule = Sc55Schedule::parse(&minimal_midi()).expect("parse generated MIDI");
        assert_eq!(schedule.duration, Duration::from_millis(500));
        assert_eq!(schedule.events.len(), 3);
        assert_eq!(schedule.events[0].bytes, [0xc0, 0]);
        assert_eq!(schedule.events[1].bytes, [0x90, 60, 100]);
        assert_eq!(schedule.events[2].bytes, [0x80, 60, 0]);
        assert_eq!(schedule.events[2].nanoseconds, 500_000_000);
    }

    #[test]
    fn schedule_renders_only_the_selected_format_two_subsong() {
        let selected = crate::decoder::select_standard_midi_subsong(&format_two_midi(), Some(1))
            .expect("select second format 2 song");
        let schedule = Sc55Schedule::parse(&selected.bytes).expect("parse selected SC-55 song");

        assert_eq!(selected.title.as_deref(), Some("Second"));
        assert_eq!(selected.subsong_count, Some(2));
        assert_eq!(schedule.duration, Duration::from_secs(1));
        assert_eq!(schedule.events.len(), 2);
        assert_eq!(schedule.events[0].bytes, [0x90, 67, 100]);
        assert_eq!(schedule.events[1].bytes, [0x80, 67, 0]);
    }

    /// Linked-renderer timing probe: needs user ROMs and a real MIDI file.
    ///   KOG_SC55_ROMS=<rom dir> KOG_SC55_PROBE_MIDI=<file.mid>
    ///   cargo test -p kog-audio -- --ignored sc55_reuse_probe --nocapture
    /// Proves the pooled renderer serves later tracks without re-booting.
    #[test]
    #[ignore = "needs KOG_SC55_ROMS and KOG_SC55_PROBE_MIDI"]
    fn sc55_reuse_probe() {
        use std::time::Instant;
        let (Ok(roms), Ok(midi_path)) = (
            std::env::var("KOG_SC55_ROMS"),
            std::env::var("KOG_SC55_PROBE_MIDI"),
        ) else {
            panic!("set KOG_SC55_ROMS and KOG_SC55_PROBE_MIDI");
        };
        let roms = PathBuf::from(roms);
        let bytes = std::fs::read(&midi_path).expect("read probe MIDI");
        let cold = Instant::now();
        warm_sc55_server(&roms);
        // Render real audio from consecutive tracks: this is what playback
        // does, and it catches a server that opens fast but yields no PCM.
        let mut probe = |label: &str| {
            let started = Instant::now();
            let mut source =
                Sc55::open(&bytes, Path::new("probe.mid"), &roms).expect("linked render");
            let opened = started.elapsed();
            let mut pcm = vec![0.0_f32; 4_096];
            let mut frames = 0_usize;
            let mut loud = 0_usize;
            for _ in 0..24 {
                let produced = source.render(&mut pcm).expect("render");
                if produced == 0 {
                    break;
                }
                frames += produced;
                loud += pcm[..produced * 2]
                    .iter()
                    .filter(|sample| sample.abs() > 0.000_1)
                    .count();
            }
            println!("{label}: open {opened:?}, {frames} frames, {loud} loud samples");
        };
        probe("job1");
        println!("first render ready after {:?}", cold.elapsed());
        probe("job2");
        probe("job3");
        shutdown_sc55_servers();
    }

    #[test]
    fn linked_core_reports_its_pinned_version() {
        let version = unsafe { std::ffi::CStr::from_ptr(kog_sc55_version()) };
        assert_eq!(
            version.to_str().expect("version is UTF-8"),
            "Nuked SC-55 0.7.0 (e8a6bdc)"
        );
    }

    #[test]
    fn empty_rom_directory_is_rejected_without_entering_kog() {
        let midi = NamedTempFile::new().expect("create MIDI fixture");
        std::fs::write(midi.path(), minimal_midi()).expect("write MIDI fixture");
        let roms = tempfile::tempdir().expect("create empty ROM directory");
        let error = Sc55::open(&minimal_midi(), midi.path(), roms.path())
            .err()
            .expect("missing ROM rejection");
        assert!(
            error.contains("No complete romsets") || error.contains("loading SC-55 ROM set failed"),
            "unexpected error: {error}"
        );
    }

    #[test]
    #[ignore = "requires user-supplied Roland ROMs through KOG_SC55_ROMS"]
    fn user_rom_gate_recognizes_and_renders_sc55() {
        let path = std::env::var_os("KOG_SC55_ROMS")
            .map(PathBuf::from)
            .expect("set KOG_SC55_ROMS to a user-owned ROM directory");
        let midi = NamedTempFile::new().expect("create MIDI fixture");
        std::fs::write(midi.path(), minimal_midi()).expect("write MIDI fixture");
        let mut source =
            Sc55::open(&minimal_midi(), midi.path(), &path).expect("open real Nuked SC-55 ROM set");
        assert_eq!(source.model(), "SC-55mk1");
        let mut samples = vec![0.0; 8_192];
        let frames = source
            .render(&mut samples)
            .expect("render real SC-55 ROM set");
        assert_eq!(frames, samples.len() / 2);
        assert!(samples.iter().any(|sample| sample.abs() > 0.000_01));

        let started = std::time::Instant::now();
        source
            .seek(Duration::from_millis(250))
            .expect("seek real Nuked SC-55 ROM set");
        assert!(
            started.elapsed() < Duration::from_millis(100),
            "SC-55 seek blocked for {:?}",
            started.elapsed()
        );
    }
}
