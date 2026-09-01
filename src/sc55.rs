//! Process owner for the optional, separately licensed Nuked SC-55 helper.

use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdout, Command, Stdio};
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

pub struct Sc55 {
    schedule: NamedTempFile,
    rom_directory: PathBuf,
    process: Option<Sc55Process>,
    sample_rate: u32,
    total_frames: u64,
    rendered_frames: u64,
    model: String,
    native_bytes: Vec<u8>,
}

struct Sc55Process {
    child: Child,
    stdout: ChildStdout,
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
        let mut schedule_file =
            NamedTempFile::new().map_err(|error| format!("creating SC-55 schedule: {error}"))?;
        schedule.write(&mut schedule_file)?;
        schedule_file
            .flush()
            .map_err(|error| format!("flushing SC-55 schedule: {error}"))?;

        let rom_directory = rom_directory.to_path_buf();
        let (process, header) = spawn_helper(schedule_file.path(), &rom_directory, 0)?;
        if let Err(error) = validate_header(&header, 0, path) {
            let mut process = process;
            stop_process(&mut process);
            return Err(error);
        }
        Ok(Self {
            schedule: schedule_file,
            rom_directory,
            process: Some(process),
            sample_rate: header.sample_rate,
            total_frames: header.total_frames,
            rendered_frames: 0,
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

    pub fn channels(&self) -> u16 {
        CHANNELS
    }

    pub fn total_frames(&self) -> u64 {
        self.total_frames
    }

    pub fn rendered_frames(&self) -> u64 {
        self.rendered_frames
    }

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
        self.native_bytes.resize(byte_count, 0);
        if let Err(error) = self
            .process
            .as_mut()
            .ok_or_else(|| "Nuked SC-55 helper process is not running".to_owned())?
            .stdout
            .read_exact(&mut self.native_bytes)
        {
            return Err(
                self.process_error(format!("reading PCM from the Nuked SC-55 helper: {error}"))
            );
        }
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
        let (process, header) = spawn_helper(self.schedule.path(), &self.rom_directory, target)?;
        if header.sample_rate != self.sample_rate
            || header.channels != CHANNELS
            || header.total_frames != self.total_frames
            || header.start_frame != target
            || header.model != self.model
        {
            let mut process = process;
            stop_process(&mut process);
            return Err(
                "Nuked SC-55 helper reported different stream properties after seek".to_owned(),
            );
        }
        if let Some(mut old_process) = self.process.replace(process) {
            stop_process(&mut old_process);
        }
        self.rendered_frames = target;
        Ok(duration_from_frames(target, self.sample_rate))
    }

    fn process_error(&mut self, context: String) -> String {
        let Some(mut process) = self.process.take() else {
            return context;
        };
        drop(process.stdout);
        let status = process.child.wait();
        let stderr = read_stderr(&mut process.child);
        match (status, stderr.is_empty()) {
            (Ok(status), false) => format!("{context}; helper exited {status}: {stderr}"),
            (Ok(status), true) => format!("{context}; helper exited {status}"),
            (Err(error), false) => {
                format!("{context}; waiting for helper failed: {error}: {stderr}")
            }
            (Err(error), true) => format!("{context}; waiting for helper failed: {error}"),
        }
    }
}

impl Drop for Sc55 {
    fn drop(&mut self) {
        if let Some(mut process) = self.process.take() {
            stop_process(&mut process);
        }
    }
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

fn spawn_helper(
    schedule: &Path,
    rom_directory: &Path,
    start_frame: u64,
) -> Result<(Sc55Process, HelperHeader), String> {
    let helper = helper_path()?;
    let mut child = Command::new(&helper)
        .arg(schedule)
        .arg(rom_directory)
        .arg(start_frame.to_string())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("launching {}: {error}", helper.display()))?;
    let mut stdout = child
        .stdout
        .take()
        .ok_or_else(|| "Nuked SC-55 helper stdout was not captured".to_owned())?;
    let header = match HelperHeader::read(&mut stdout) {
        Ok(header) => header,
        Err(error) => {
            drop(stdout);
            let _ = child.kill();
            let status = child.wait();
            let stderr = read_stderr(&mut child);
            let detail = if stderr.is_empty() {
                String::new()
            } else {
                format!(": {stderr}")
            };
            return Err(match status {
                Ok(status) => {
                    format!("starting the Nuked SC-55 helper failed ({status}): {error}{detail}")
                }
                Err(wait_error) => format!(
                    "starting the Nuked SC-55 helper failed: {error}; waiting failed: {wait_error}{detail}"
                ),
            });
        }
    };
    Ok((Sc55Process { child, stdout }, header))
}

fn helper_path() -> Result<PathBuf, String> {
    if let Some(path) = std::env::var_os("KOG_SC55_HELPER") {
        let path = PathBuf::from(path);
        if path.is_file() {
            return Ok(path);
        }
        return Err(format!(
            "KOG_SC55_HELPER does not name a file: {}",
            path.display()
        ));
    }
    let executable_name = if cfg!(windows) {
        "kog-sc55-helper.exe"
    } else {
        "kog-sc55-helper"
    };
    if let Ok(executable) = std::env::current_exe() {
        let sibling = executable.with_file_name(executable_name);
        if sibling.is_file() {
            return Ok(sibling);
        }
    }
    let build_helper = PathBuf::from(env!("KOG_BUILD_SC55_HELPER"));
    if build_helper.is_file() {
        return Ok(build_helper);
    }
    Err(format!(
        "Nuked SC-55 helper is not installed beside Kog and the build copy is missing: {}",
        build_helper.display()
    ))
}

fn stop_process(process: &mut Sc55Process) {
    let _ = process.child.kill();
    let _ = process.child.wait();
}

fn read_stderr(child: &mut Child) -> String {
    let mut stderr = String::new();
    if let Some(mut stream) = child.stderr.take() {
        let _ = stream.read_to_string(&mut stderr);
    }
    stderr.trim().to_owned()
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

    #[test]
    fn built_helper_reports_its_pinned_protocol_version() {
        let output = Command::new(helper_path().expect("build helper path"))
            .arg("--version")
            .output()
            .expect("run SC-55 helper version");
        assert!(output.status.success());
        assert_eq!(
            String::from_utf8_lossy(&output.stdout).trim(),
            "kog-sc55-helper protocol 1; Nuked SC-55 0.6.1 (50dcdde)"
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
}
