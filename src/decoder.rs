use std::fs::File;
use std::io::Cursor;
use std::num::{NonZeroU16, NonZeroU32};
use std::path::Path;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, RwLock};
use std::time::Duration;
use std::time::SystemTime;

use midly::{Format, Fps, MetaMessage, MidiMessage, Smf, Timing, TrackEventKind};
use rodio::source::SeekError;
use rodio::{ChannelCount, Decoder, Player, SampleRate, Source};
use rustysynth::{MidiFile, MidiFileSequencer, SoundFont, Synthesizer, SynthesizerSettings};

use crate::opl3::Opl3WindowsSynth;
use crate::settings::MidiEngine;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DecoderCapabilities {
    pub seek: bool,
    pub subsongs: bool,
    pub loop_metadata: bool,
    pub companion_files: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct StreamProperties {
    pub duration: Option<Duration>,
    pub sample_rate: Option<u32>,
    pub channels: Option<u16>,
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub genre: Option<String>,
    pub year: Option<u32>,
    pub track_number: Option<u32>,
    pub codec: Option<String>,
    pub bitrate: Option<u32>,
    pub bits_per_sample: Option<u8>,
    pub warning: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PlaybackSource {
    pub path: PathBuf,
    pub subsong: Option<u32>,
}

impl PlaybackSource {
    pub fn from_path(path: PathBuf) -> Self {
        Self {
            path,
            subsong: None,
        }
    }

    pub fn display_label(&self) -> String {
        match self.subsong {
            Some(subsong) => format!("{}#{}", self.path.display(), subsong + 1),
            None => self.path.display().to_string(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SelectedBackend {
    pub id: &'static str,
    pub display_name: &'static str,
    pub capabilities: DecoderCapabilities,
}

impl SelectedBackend {
    pub fn capability_summary(self) -> String {
        let mut capabilities = Vec::new();
        if self.capabilities.seek {
            capabilities.push("seek");
        }
        if self.capabilities.subsongs {
            capabilities.push("subsongs");
        }
        if self.capabilities.loop_metadata {
            capabilities.push("loops");
        }
        if self.capabilities.companion_files {
            capabilities.push("companion files");
        }
        capabilities.join(", ")
    }
}

/// One decoding family behind Kog's shared playback contract.
///
/// Specialist backends can render through C/C++ libraries and append a custom
/// `rodio::Source` without leaking their FFI details into the playlist or UI.
pub trait DecoderBackend: Send + Sync {
    fn id(&self) -> &'static str;
    fn display_name(&self) -> &'static str;
    fn extensions(&self) -> &'static [&'static str];
    fn capabilities(&self) -> DecoderCapabilities;
    fn probe(&self, source: &PlaybackSource) -> Result<StreamProperties, String>;
    fn append(&self, source: &PlaybackSource, player: &Player) -> Result<(), String>;

    fn subsong_count(&self, _path: &Path) -> Result<Option<u32>, String> {
        Ok(None)
    }

    fn accepts(&self, path: &Path) -> bool {
        let Some(extension) = path.extension().and_then(|value| value.to_str()) else {
            return false;
        };
        self.extensions()
            .iter()
            .any(|candidate| candidate.eq_ignore_ascii_case(extension))
    }
}

#[derive(Clone, Default)]
pub struct DecoderSettings {
    soundfont_path: Arc<RwLock<Option<PathBuf>>>,
    midi_engine: Arc<RwLock<MidiEngine>>,
}

impl DecoderSettings {
    pub fn new(soundfont_path: Option<PathBuf>, midi_engine: MidiEngine) -> Self {
        Self {
            soundfont_path: Arc::new(RwLock::new(soundfont_path)),
            midi_engine: Arc::new(RwLock::new(midi_engine)),
        }
    }

    pub fn soundfont_path(&self) -> Option<PathBuf> {
        self.soundfont_path
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }

    pub fn set_soundfont_path(&self, path: Option<PathBuf>) {
        *self
            .soundfont_path
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = path;
    }

    pub fn midi_engine(&self) -> MidiEngine {
        *self
            .midi_engine
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    pub fn set_midi_engine(&self, midi_engine: MidiEngine) {
        *self
            .midi_engine
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = midi_engine;
    }
}

pub struct DecoderRegistry {
    backends: Vec<Box<dyn DecoderBackend>>,
}

impl Default for DecoderRegistry {
    fn default() -> Self {
        Self::new(DecoderSettings::default())
    }
}

impl DecoderRegistry {
    pub fn new(settings: DecoderSettings) -> Self {
        Self {
            backends: vec![
                Box::new(RodioBackend),
                Box::new(MidiBackend::new(settings)),
                Box::new(crate::gme_decoder::GmeBackend),
                Box::new(crate::libvgm_decoder::LibVgmBackend),
                Box::new(crate::openmpt_decoder::OpenMptBackend),
                Box::new(crate::hively_decoder::HivelyBackend),
                Box::new(crate::organya_decoder::OrganyaBackend),
                Box::new(crate::sid_decoder::SidBackend),
                Box::new(crate::adplug_decoder::AdPlugBackend),
                Box::new(crate::vgmstream_decoder::VgmstreamBackend),
            ],
        }
    }

    pub fn expand(&self, path: PathBuf) -> Result<Vec<PlaybackSource>, String> {
        let Some(backend) = self.select(&path) else {
            return Ok(vec![PlaybackSource::from_path(path)]);
        };
        let Some(count) = backend.subsong_count(&path)? else {
            return Ok(vec![PlaybackSource::from_path(path)]);
        };
        if count == 0 {
            return Err(format!("{} contains no playable subsongs", path.display()));
        }
        Ok((0..count)
            .map(|subsong| PlaybackSource {
                path: path.clone(),
                subsong: Some(subsong),
            })
            .collect())
    }

    pub fn probe(&self, source: &PlaybackSource) -> Result<StreamProperties, String> {
        let backend = self
            .select(&source.path)
            .ok_or_else(|| unsupported_message(&source.path))?;
        backend.probe(source)
    }

    pub fn append(
        &self,
        source: &PlaybackSource,
        player: &Player,
    ) -> Result<SelectedBackend, String> {
        let backend = self
            .select(&source.path)
            .ok_or_else(|| unsupported_message(&source.path))?;
        backend.append(source, player)?;
        Ok(SelectedBackend {
            id: backend.id(),
            display_name: backend.display_name(),
            capabilities: backend.capabilities(),
        })
    }

    #[cfg(test)]
    pub fn backend_id_for(&self, path: &Path) -> Option<&'static str> {
        self.select(path).map(DecoderBackend::id)
    }

    fn select(&self, path: &Path) -> Option<&dyn DecoderBackend> {
        self.backends
            .iter()
            .map(Box::as_ref)
            .find(|backend| backend.accepts(path))
    }
}

fn unsupported_message(path: &Path) -> String {
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("unknown");
    format!("No installed decoder backend accepts .{extension}")
}

struct RodioBackend;

// These are the containers/codecs enabled by rodio's Symphonia-all feature.
// Selection is deliberately conservative: accepting an extension here is a
// promise that this backend will be asked to decode it, not a claim that every
// codec combination within a container is supported.
const RODIO_EXTENSIONS: &[&str] = &[
    "aac", "adts", "aif", "aifc", "aiff", "alac", "caf", "flac", "m4a", "m4b", "mka", "mkv", "mp1",
    "mp2", "mp3", "mp4", "oga", "ogg", "ogv", "opus", "wav", "wave", "webm",
];

impl DecoderBackend for RodioBackend {
    fn id(&self) -> &'static str {
        "rodio-symphonia"
    }

    fn display_name(&self) -> &'static str {
        "Symphonia"
    }

    fn extensions(&self) -> &'static [&'static str] {
        RODIO_EXTENSIONS
    }

    fn capabilities(&self) -> DecoderCapabilities {
        DecoderCapabilities {
            seek: true,
            ..DecoderCapabilities::default()
        }
    }

    fn probe(&self, source: &PlaybackSource) -> Result<StreamProperties, String> {
        let decoder = Decoder::try_from(
            File::open(&source.path)
                .map_err(|error| format!("opening {}: {error}", source.path.display()))?,
        )
        .map_err(|error| format!("decoding {}: {error}", source.path.display()))?;

        Ok(StreamProperties {
            duration: decoder.total_duration(),
            sample_rate: Some(decoder.sample_rate().get()),
            channels: Some(decoder.channels().get()),
            ..StreamProperties::default()
        })
    }

    fn append(&self, source: &PlaybackSource, player: &Player) -> Result<(), String> {
        let decoder = Decoder::try_from(
            File::open(&source.path)
                .map_err(|error| format!("opening {}: {error}", source.path.display()))?,
        )
        .map_err(|error| format!("decoding {}: {error}", source.path.display()))?;
        player.append(decoder);
        Ok(())
    }
}

const MIDI_EXTENSIONS: &[&str] = &["kar", "mid", "midi", "rmi"];
const MIDI_SAMPLE_RATE: u32 = 48_000;
const MIDI_CHANNELS: u16 = 2;
const MIDI_RENDER_FRAMES: usize = 512;

#[derive(Default)]
struct SoundFontCache {
    path: Option<PathBuf>,
    modified: Option<SystemTime>,
    soundfont: Option<Arc<SoundFont>>,
}

struct MidiBackend {
    settings: DecoderSettings,
    cache: Mutex<SoundFontCache>,
}

impl MidiBackend {
    fn new(settings: DecoderSettings) -> Self {
        Self {
            settings,
            cache: Mutex::new(SoundFontCache::default()),
        }
    }

    fn load_soundfont(&self) -> Result<Arc<SoundFont>, String> {
        let path = self.settings.soundfont_path().ok_or_else(|| {
            "MIDI playback requires an SF2 SoundFont. Choose one in Edit > Preferences > MIDI."
                .to_owned()
        })?;
        let metadata = path.metadata().map_err(|error| {
            format!("reading SoundFont metadata for {}: {error}", path.display())
        })?;
        let modified = metadata.modified().ok();
        let mut cache = self
            .cache
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if cache.path.as_ref() == Some(&path)
            && cache.modified == modified
            && let Some(soundfont) = cache.soundfont.as_ref()
        {
            return Ok(Arc::clone(soundfont));
        }

        let soundfont = Arc::new(load_soundfont_file(&path)?);
        cache.path = Some(path);
        cache.modified = modified;
        cache.soundfont = Some(Arc::clone(&soundfont));
        Ok(soundfont)
    }
}

impl DecoderBackend for MidiBackend {
    fn id(&self) -> &'static str {
        match self.settings.midi_engine() {
            MidiEngine::RustySynth => "midi-rustysynth-sf2",
            MidiEngine::Opl3Windows => "midi-opl3windows",
        }
    }

    fn display_name(&self) -> &'static str {
        match self.settings.midi_engine() {
            MidiEngine::RustySynth => "RustySynth SoundFont",
            MidiEngine::Opl3Windows => "OPL3Windows (Nuked OPL3)",
        }
    }

    fn extensions(&self) -> &'static [&'static str] {
        MIDI_EXTENSIONS
    }

    fn capabilities(&self) -> DecoderCapabilities {
        DecoderCapabilities {
            seek: true,
            ..DecoderCapabilities::default()
        }
    }

    fn probe(&self, source: &PlaybackSource) -> Result<StreamProperties, String> {
        let duration = match self.settings.midi_engine() {
            MidiEngine::RustySynth => load_midi_file(&source.path)?.1,
            MidiEngine::Opl3Windows => {
                OplMidiTimeline::parse(&read_standard_midi(&source.path)?)?.duration
            }
        };
        Ok(StreamProperties {
            duration: Some(duration),
            sample_rate: Some(MIDI_SAMPLE_RATE),
            channels: Some(MIDI_CHANNELS),
            ..StreamProperties::default()
        })
    }

    fn append(&self, source: &PlaybackSource, player: &Player) -> Result<(), String> {
        match self.settings.midi_engine() {
            MidiEngine::RustySynth => {
                let soundfont = self.load_soundfont()?;
                let (midi_file, duration) = load_midi_file(&source.path)?;
                player.append(MidiSource::new(soundfont, midi_file, duration)?);
            }
            MidiEngine::Opl3Windows => {
                let timeline =
                    Arc::new(OplMidiTimeline::parse(&read_standard_midi(&source.path)?)?);
                player.append(OplMidiSource::new(timeline)?);
            }
        }
        Ok(())
    }
}

pub fn validate_soundfont(path: &Path) -> Result<(), String> {
    load_soundfont_file(path).map(|_| ())
}

fn load_soundfont_file(path: &Path) -> Result<SoundFont, String> {
    let mut file = File::open(path)
        .map_err(|error| format!("opening SoundFont {}: {error}", path.display()))?;
    SoundFont::new(&mut file)
        .map_err(|error| format!("loading SoundFont {}: {error}", path.display()))
}

fn load_midi_file(path: &Path) -> Result<(Arc<MidiFile>, Duration), String> {
    let bytes = read_standard_midi(path)?;
    let track_count = bytes
        .get(10..12)
        .and_then(|bytes| <[u8; 2]>::try_from(bytes).ok())
        .map(u16::from_be_bytes)
        .unwrap_or_default();
    if track_count == 0 {
        return Err(format!("MIDI file {} contains no tracks", path.display()));
    }
    let mut cursor = Cursor::new(bytes);
    let midi_file = Arc::new(
        MidiFile::new(&mut cursor)
            .map_err(|error| format!("parsing MIDI file {}: {error}", path.display()))?,
    );
    let length = midi_file.get_length();
    if !length.is_finite() || length.is_sign_negative() {
        return Err(format!(
            "MIDI file {} has an invalid duration",
            path.display()
        ));
    }
    Ok((midi_file, Duration::from_secs_f64(length)))
}

fn read_standard_midi(path: &Path) -> Result<Vec<u8>, String> {
    let bytes = std::fs::read(path)
        .map_err(|error| format!("reading MIDI file {}: {error}", path.display()))?;
    if bytes.starts_with(b"MThd") {
        return Ok(bytes);
    }
    if bytes.len() >= 12 && bytes.starts_with(b"RIFF") && &bytes[8..12] == b"RMID" {
        let mut offset = 12_usize;
        while offset.checked_add(8).is_some_and(|end| end <= bytes.len()) {
            let chunk_id = &bytes[offset..offset + 4];
            let size = u32::from_le_bytes(
                bytes[offset + 4..offset + 8]
                    .try_into()
                    .expect("bounded RIFF chunk header"),
            ) as usize;
            let start = offset + 8;
            let Some(end) = start.checked_add(size).filter(|end| *end <= bytes.len()) else {
                break;
            };
            if chunk_id == b"data" && bytes[start..end].starts_with(b"MThd") {
                return Ok(bytes[start..end].to_vec());
            }
            let Some(next) = end.checked_add(size & 1) else {
                break;
            };
            offset = next;
        }
        return Err(format!(
            "RIFF MIDI file {} has no valid data chunk",
            path.display()
        ));
    }
    Err(format!(
        "MIDI file {} has neither an MThd nor RIFF RMID header",
        path.display()
    ))
}

#[derive(Clone, Copy, Debug)]
enum OplTimelineInputKind {
    Midi(u32),
    Tempo(u32),
}

#[derive(Clone, Copy, Debug)]
struct OplTimelineInput {
    tick: u64,
    track: usize,
    order: usize,
    kind: OplTimelineInputKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct OplMidiEvent {
    frame: u64,
    packed: u32,
}

#[derive(Clone, Debug)]
struct OplMidiTimeline {
    events: Vec<OplMidiEvent>,
    total_frames: u64,
    duration: Duration,
}

impl OplMidiTimeline {
    fn parse(bytes: &[u8]) -> Result<Self, String> {
        let smf = Smf::parse(bytes).map_err(|error| format!("parsing MIDI for OPL3: {error}"))?;
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
                    TrackEventKind::Midi { channel, message } => {
                        pack_opl_midi(channel.as_int(), *message).map(OplTimelineInputKind::Midi)
                    }
                    TrackEventKind::Meta(MetaMessage::Tempo(tempo)) => {
                        Some(OplTimelineInputKind::Tempo(tempo.as_int()))
                    }
                    _ => None,
                };
                if let Some(kind) = kind {
                    inputs.push(OplTimelineInput {
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

        let mut clock = OplFrameClock::new(smf.header.timing)?;
        let mut events = Vec::new();
        let mut current_tick = 0_u64;
        for input in inputs {
            clock.advance(input.tick - current_tick)?;
            current_tick = input.tick;
            match input.kind {
                OplTimelineInputKind::Midi(packed) => events.push(OplMidiEvent {
                    frame: clock.frame()?,
                    packed,
                }),
                OplTimelineInputKind::Tempo(tempo) => clock.set_tempo(tempo),
            }
        }
        clock.advance(total_ticks - current_tick)?;
        let total_frames = clock.frame()?;
        let duration = Duration::from_secs_f64(total_frames as f64 / f64::from(MIDI_SAMPLE_RATE));
        Ok(Self {
            events,
            total_frames,
            duration,
        })
    }
}

fn pack_opl_midi(channel: u8, message: MidiMessage) -> Option<u32> {
    let channel = u32::from(channel);
    let packed = match message {
        MidiMessage::NoteOff { key, vel } => {
            0x80 | channel | (u32::from(key.as_int()) << 8) | (u32::from(vel.as_int()) << 16)
        }
        MidiMessage::NoteOn { key, vel } => {
            0x90 | channel | (u32::from(key.as_int()) << 8) | (u32::from(vel.as_int()) << 16)
        }
        MidiMessage::Controller { controller, value } => {
            0xb0 | channel
                | (u32::from(controller.as_int()) << 8)
                | (u32::from(value.as_int()) << 16)
        }
        MidiMessage::ProgramChange { program } => {
            0xc0 | channel | (u32::from(program.as_int()) << 8)
        }
        MidiMessage::PitchBend { bend } => {
            let raw = bend.0.as_int();
            0xe0 | channel | (u32::from(raw & 0x7f) << 8) | (u32::from(raw >> 7) << 16)
        }
        MidiMessage::Aftertouch { .. } | MidiMessage::ChannelAftertouch { .. } => return None,
    };
    Some(packed)
}

enum OplFrameClockKind {
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

struct OplFrameClock {
    kind: OplFrameClockKind,
    frame: u128,
    remainder: u128,
}

impl OplFrameClock {
    fn new(timing: Timing) -> Result<Self, String> {
        let kind = match timing {
            Timing::Metrical(ticks_per_beat) => {
                let ticks_per_beat = u128::from(ticks_per_beat.as_int());
                if ticks_per_beat == 0 {
                    return Err("MIDI metrical timing has zero ticks per beat".to_owned());
                }
                OplFrameClockKind::Metrical {
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
                OplFrameClockKind::Timecode {
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
            OplFrameClockKind::Metrical {
                ticks_per_beat,
                tempo,
            } => (
                u128::from(MIDI_SAMPLE_RATE) * tempo,
                ticks_per_beat * 1_000_000,
            ),
            OplFrameClockKind::Timecode {
                fps_numerator,
                fps_denominator,
                subframe,
            } => (
                u128::from(MIDI_SAMPLE_RATE) * fps_denominator,
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
        if let OplFrameClockKind::Metrical { tempo: current, .. } = &mut self.kind {
            *current = u128::from(tempo);
        }
    }

    fn frame(&self) -> Result<u64, String> {
        u64::try_from(self.frame).map_err(|_| "MIDI duration exceeds Kog's limit".to_owned())
    }
}

struct OplMidiSource {
    timeline: Arc<OplMidiTimeline>,
    synth: Opl3WindowsSynth,
    event_index: usize,
    frames_rendered: u64,
    samples_emitted: u64,
    pcm: Vec<i16>,
    interleaved: Vec<f32>,
    interleaved_index: usize,
}

impl OplMidiSource {
    fn new(timeline: Arc<OplMidiTimeline>) -> Result<Self, String> {
        Ok(Self {
            timeline,
            synth: Opl3WindowsSynth::new(MIDI_SAMPLE_RATE)?,
            event_index: 0,
            frames_rendered: 0,
            samples_emitted: 0,
            pcm: vec![0; MIDI_RENDER_FRAMES * usize::from(MIDI_CHANNELS)],
            interleaved: Vec::with_capacity(MIDI_RENDER_FRAMES * usize::from(MIDI_CHANNELS)),
            interleaved_index: 0,
        })
    }

    fn render_frames(&mut self, frames: usize, emit: bool) {
        self.pcm[..frames * 2].fill(0);
        let mut rendered = 0_usize;
        while rendered < frames {
            let absolute_frame = self.frames_rendered + rendered as u64;
            while let Some(event) = self.timeline.events.get(self.event_index) {
                if event.frame > absolute_frame {
                    break;
                }
                self.synth.write_packed(event.packed);
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
            self.synth
                .generate(&mut self.pcm[rendered * 2..(rendered + segment) * 2])
                .expect("fixed OPL3 render buffer is valid");
            rendered += segment;
        }
        self.frames_rendered += frames as u64;
        if emit {
            self.interleaved.clear();
            self.interleaved.extend(
                self.pcm[..frames * 2]
                    .iter()
                    .map(|sample| f32::from(*sample) * (1.0 / 8192.0)),
            );
            self.interleaved_index = 0;
        }
    }

    fn fill_interleaved(&mut self) {
        let frames = usize::try_from(
            self.timeline
                .total_frames
                .saturating_sub(self.frames_rendered),
        )
        .unwrap_or(usize::MAX)
        .min(MIDI_RENDER_FRAMES);
        if frames > 0 {
            self.render_frames(frames, true);
        }
    }

    fn seek_to(&mut self, position: Duration) -> Result<(), String> {
        self.synth = Opl3WindowsSynth::new(MIDI_SAMPLE_RATE)?;
        self.event_index = 0;
        self.frames_rendered = 0;
        self.samples_emitted = 0;
        self.interleaved.clear();
        self.interleaved_index = 0;
        let target = position.min(self.timeline.duration);
        let target_frames = (target.as_secs_f64() * f64::from(MIDI_SAMPLE_RATE)).floor() as u64;
        while self.frames_rendered < target_frames {
            let frames = usize::try_from(target_frames - self.frames_rendered)
                .unwrap_or(usize::MAX)
                .min(MIDI_RENDER_FRAMES);
            self.render_frames(frames, false);
        }
        self.samples_emitted = target_frames * u64::from(MIDI_CHANNELS);
        Ok(())
    }
}

impl Iterator for OplMidiSource {
    type Item = f32;

    fn next(&mut self) -> Option<Self::Item> {
        let total_samples = self.timeline.total_frames * u64::from(MIDI_CHANNELS);
        if self.samples_emitted >= total_samples {
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
        let remaining = self
            .timeline
            .total_frames
            .saturating_mul(u64::from(MIDI_CHANNELS))
            .saturating_sub(self.samples_emitted);
        let remaining = usize::try_from(remaining).unwrap_or(usize::MAX);
        (remaining, Some(remaining))
    }
}

impl Source for OplMidiSource {
    fn current_span_len(&self) -> Option<usize> {
        self.size_hint().1
    }

    fn channels(&self) -> ChannelCount {
        NonZeroU16::new(MIDI_CHANNELS).expect("MIDI output is stereo")
    }

    fn sample_rate(&self) -> SampleRate {
        NonZeroU32::new(MIDI_SAMPLE_RATE).expect("MIDI sample rate is nonzero")
    }

    fn total_duration(&self) -> Option<Duration> {
        Some(self.timeline.duration)
    }

    fn try_seek(&mut self, position: Duration) -> Result<(), SeekError> {
        self.seek_to(position)
            .map_err(|error| SeekError::Other(Arc::new(std::io::Error::other(error))))
    }
}

struct MidiSource {
    soundfont: Arc<SoundFont>,
    midi_file: Arc<MidiFile>,
    sequencer: MidiFileSequencer,
    duration: Duration,
    total_frames: u64,
    frames_rendered: u64,
    samples_emitted: u64,
    left: Vec<f32>,
    right: Vec<f32>,
    interleaved: Vec<f32>,
    interleaved_index: usize,
}

impl MidiSource {
    fn new(
        soundfont: Arc<SoundFont>,
        midi_file: Arc<MidiFile>,
        duration: Duration,
    ) -> Result<Self, String> {
        let sequencer = create_midi_sequencer(&soundfont, &midi_file)?;
        let total_frames = (duration.as_secs_f64() * f64::from(MIDI_SAMPLE_RATE)).ceil() as u64;
        Ok(Self {
            soundfont,
            midi_file,
            sequencer,
            duration,
            total_frames,
            frames_rendered: 0,
            samples_emitted: 0,
            left: vec![0.0; MIDI_RENDER_FRAMES],
            right: vec![0.0; MIDI_RENDER_FRAMES],
            interleaved: Vec::with_capacity(MIDI_RENDER_FRAMES * usize::from(MIDI_CHANNELS)),
            interleaved_index: 0,
        })
    }

    fn fill_interleaved(&mut self) {
        let frames = usize::try_from(self.total_frames.saturating_sub(self.frames_rendered))
            .unwrap_or(usize::MAX)
            .min(MIDI_RENDER_FRAMES);
        if frames == 0 {
            return;
        }
        self.sequencer
            .render(&mut self.left[..frames], &mut self.right[..frames]);
        self.interleaved.clear();
        for index in 0..frames {
            self.interleaved.push(self.left[index]);
            self.interleaved.push(self.right[index]);
        }
        self.interleaved_index = 0;
        self.frames_rendered += frames as u64;
    }

    fn seek_to(&mut self, position: Duration) -> Result<(), String> {
        self.sequencer = create_midi_sequencer(&self.soundfont, &self.midi_file)?;
        let target = position.min(self.duration);
        let target_frames = (target.as_secs_f64() * f64::from(MIDI_SAMPLE_RATE)).floor() as u64;
        let mut remaining = target_frames;
        while remaining > 0 {
            let frames = usize::try_from(remaining)
                .unwrap_or(usize::MAX)
                .min(MIDI_RENDER_FRAMES);
            self.sequencer
                .render(&mut self.left[..frames], &mut self.right[..frames]);
            remaining -= frames as u64;
        }
        self.total_frames =
            (self.duration.as_secs_f64() * f64::from(MIDI_SAMPLE_RATE)).ceil() as u64;
        self.frames_rendered = target_frames;
        self.samples_emitted = target_frames * u64::from(MIDI_CHANNELS);
        self.interleaved.clear();
        self.interleaved_index = 0;
        Ok(())
    }
}

impl Iterator for MidiSource {
    type Item = f32;

    fn next(&mut self) -> Option<Self::Item> {
        let total_samples = self.total_frames * u64::from(MIDI_CHANNELS);
        if self.samples_emitted >= total_samples {
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
        let remaining = self
            .total_frames
            .saturating_mul(u64::from(MIDI_CHANNELS))
            .saturating_sub(self.samples_emitted);
        let remaining = usize::try_from(remaining).unwrap_or(usize::MAX);
        (remaining, Some(remaining))
    }
}

impl Source for MidiSource {
    fn current_span_len(&self) -> Option<usize> {
        let remaining = self
            .total_frames
            .saturating_mul(u64::from(MIDI_CHANNELS))
            .saturating_sub(self.samples_emitted);
        Some(
            usize::try_from(remaining)
                .unwrap_or(usize::MAX - usize::MAX % usize::from(MIDI_CHANNELS)),
        )
    }

    fn channels(&self) -> ChannelCount {
        NonZeroU16::new(MIDI_CHANNELS).expect("MIDI output is stereo")
    }

    fn sample_rate(&self) -> SampleRate {
        NonZeroU32::new(MIDI_SAMPLE_RATE).expect("MIDI sample rate is nonzero")
    }

    fn total_duration(&self) -> Option<Duration> {
        Some(self.duration)
    }

    fn try_seek(&mut self, position: Duration) -> Result<(), SeekError> {
        self.seek_to(position)
            .map_err(|error| SeekError::Other(Arc::new(std::io::Error::other(error))))
    }
}

fn create_midi_sequencer(
    soundfont: &Arc<SoundFont>,
    midi_file: &Arc<MidiFile>,
) -> Result<MidiFileSequencer, String> {
    let settings = SynthesizerSettings::new(MIDI_SAMPLE_RATE as i32);
    let synthesizer = Synthesizer::new(soundfont, &settings)
        .map_err(|error| format!("creating SoundFont synthesizer: {error}"))?;
    let mut sequencer = MidiFileSequencer::new(synthesizer);
    sequencer.play(midi_file, false);
    Ok(sequencer)
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use super::*;

    // This 484-byte saw-wave SoundFont is the MinimalSoundFont fixture from
    // TinySoundFont's examples/example1.c. See THIRD_PARTY_NOTICES.md.
    const MINIMAL_SF2_HEX: &str = "52494646dc0100007366626b4c4953545801000070647461706864724c00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000ff00ff00010000000000000000000000000070626167080000000000000001000000706d6f640a000000000000000000000000007067656e080000002900000000000000696e73742c000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000010069626167080000000000000002000000696d6f640a000000000000000000000000006967656e0c000000360001003500000000000000736864725c000000000000000000000000000000000000000000000000000000320000000000000031000000225600003c0000000100000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000004c4953547000000073647461736d706c64000000560077031f07930a2b0ea9113a15bd18491ccc1f4923f9262e2a472efa309635f2377e3c973f6c427e48cf46565364484a64a327f1a33baf3bb309b386bb06ba02c205c20fc806ca60ce9fd123d5d5d82ddcdddf4ce3dde65beaf2ed69f108f576f820fc";

    fn decode_hex(value: &str) -> Vec<u8> {
        value
            .as_bytes()
            .chunks_exact(2)
            .map(|pair| {
                u8::from_str_radix(std::str::from_utf8(pair).expect("ASCII fixture"), 16)
                    .expect("hex fixture")
            })
            .collect()
    }

    fn minimal_test_soundfont() -> Vec<u8> {
        let source = decode_hex(MINIMAL_SF2_HEX);
        assert_eq!(source.len(), 484);
        let parameter_list_size =
            u32::from_le_bytes(source[16..20].try_into().expect("pdta length")) as usize + 8;
        let parameter_start = 12;
        let sample_start = parameter_start + parameter_list_size;
        let mut sample_list = source[sample_start..].to_vec();
        let sample_list_size =
            u32::from_le_bytes(sample_list[4..8].try_into().expect("sdta length")) + 2;
        sample_list[4..8].copy_from_slice(&sample_list_size.to_le_bytes());
        let sample_chunk_size =
            u32::from_le_bytes(sample_list[16..20].try_into().expect("smpl length")) + 2;
        sample_list[16..20].copy_from_slice(&sample_chunk_size.to_le_bytes());
        sample_list.extend_from_slice(&[0, 0]);

        // TinySoundFont's compact fixture places pdta before sdta and omits
        // INFO. It also ends its sample exactly at the sample buffer boundary.
        // Reorder the lists, insert an empty INFO list, and add one guard
        // sample so the stricter RustySynth parser sees a valid layout.
        let total_size = 12 + 12 + sample_list.len() + parameter_list_size;
        let mut soundfont = Vec::with_capacity(total_size);
        soundfont.extend_from_slice(b"RIFF");
        soundfont.extend_from_slice(&((total_size - 8) as u32).to_le_bytes());
        soundfont.extend_from_slice(b"sfbkLIST\x04\0\0\0INFO");
        soundfont.extend_from_slice(&sample_list);
        soundfont.extend_from_slice(&source[parameter_start..sample_start]);
        soundfont
    }

    fn minimal_test_midi() -> Vec<u8> {
        vec![
            b'M', b'T', b'h', b'd', 0, 0, 0, 6, 0, 0, 0, 1, 1, 0xe0, b'M', b'T', b'r', b'k', 0, 0,
            0, 16, 0, 0xc0, 0, 0, 0x90, 60, 100, 0x83, 0x60, 0x80, 60, 64, 0, 0xff, 0x2f, 0,
        ]
    }

    fn tempo_change_test_midi() -> Vec<u8> {
        vec![
            b'M', b'T', b'h', b'd', 0, 0, 0, 6, 0, 1, 0, 2, 1, 0xe0, b'M', b'T', b'r', b'k', 0, 0,
            0, 20, 0, 0xff, 0x51, 3, 0x07, 0xa1, 0x20, 0x83, 0x60, 0xff, 0x51, 3, 0x0f, 0x42, 0x40,
            0x83, 0x60, 0xff, 0x2f, 0, b'M', b'T', b'r', b'k', 0, 0, 0, 16, 0, 0xc0, 0, 0, 0x90,
            60, 100, 0x87, 0x40, 0x80, 60, 64, 0, 0xff, 0x2f, 0,
        ]
    }

    fn timecode_test_midi() -> Vec<u8> {
        vec![
            b'M', b'T', b'h', b'd', 0, 0, 0, 6, 0, 0, 0, 1, 0xe7, 40, b'M', b'T', b'r', b'k', 0, 0,
            0, 13, 0, 0x90, 60, 100, 0x87, 0x68, 0x80, 60, 64, 0, 0xff, 0x2f, 0,
        ]
    }

    fn write_test_midi(path: &Path) {
        std::fs::write(path, minimal_test_midi()).expect("write MIDI fixture");
    }

    fn write_test_rmid(path: &Path) {
        let midi = minimal_test_midi();
        let padded_size = midi.len() + (midi.len() & 1);
        let riff_size = 4 + 8 + padded_size;
        let mut bytes = Vec::with_capacity(riff_size + 8);
        bytes.extend_from_slice(b"RIFF");
        bytes.extend_from_slice(&(riff_size as u32).to_le_bytes());
        bytes.extend_from_slice(b"RMIDdata");
        bytes.extend_from_slice(&(midi.len() as u32).to_le_bytes());
        bytes.extend_from_slice(&midi);
        if midi.len() & 1 != 0 {
            bytes.push(0);
        }
        std::fs::write(path, bytes).expect("write RMID fixture");
    }

    fn write_test_wav(path: &Path) {
        let sample_rate = 8_000_u32;
        let sample_count = sample_rate / 10;
        let data_size = sample_count * 2;
        let mut file = File::create(path).expect("create wave fixture");
        file.write_all(b"RIFF").unwrap();
        file.write_all(&(36 + data_size).to_le_bytes()).unwrap();
        file.write_all(b"WAVEfmt ").unwrap();
        file.write_all(&16_u32.to_le_bytes()).unwrap();
        file.write_all(&1_u16.to_le_bytes()).unwrap();
        file.write_all(&1_u16.to_le_bytes()).unwrap();
        file.write_all(&sample_rate.to_le_bytes()).unwrap();
        file.write_all(&(sample_rate * 2).to_le_bytes()).unwrap();
        file.write_all(&2_u16.to_le_bytes()).unwrap();
        file.write_all(&16_u16.to_le_bytes()).unwrap();
        file.write_all(b"data").unwrap();
        file.write_all(&data_size.to_le_bytes()).unwrap();
        for _ in 0..sample_count {
            file.write_all(&0_i16.to_le_bytes()).unwrap();
        }
    }

    #[test]
    fn registry_probes_a_real_wave_stream() {
        let path = std::env::temp_dir().join(format!("kog-decoder-{}.wav", std::process::id()));
        write_test_wav(&path);
        let registry = DecoderRegistry::default();

        let source = PlaybackSource::from_path(path.clone());
        let properties = registry.probe(&source).expect("probe wave fixture");

        assert_eq!(registry.backend_id_for(&path), Some("rodio-symphonia"));
        assert_eq!(properties.sample_rate, Some(8_000));
        assert_eq!(properties.channels, Some(1));
        assert_eq!(properties.duration, Some(Duration::from_millis(100)));
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn registry_advertises_only_implemented_specialist_formats() {
        let registry = DecoderRegistry::default();
        assert_eq!(
            registry.backend_id_for(Path::new("song.sid")),
            Some("libsidplayfp-residfp")
        );
        assert_eq!(
            registry.backend_id_for(Path::new("song.mid")),
            Some("midi-rustysynth-sf2")
        );
        assert_eq!(
            registry.backend_id_for(Path::new("song.rmi")),
            Some("midi-rustysynth-sf2")
        );
        assert_eq!(
            registry.backend_id_for(Path::new("song.spc")),
            Some("game-music-emu")
        );
    }

    #[test]
    fn registry_reports_the_selected_midi_engine() {
        let settings = DecoderSettings::new(None, MidiEngine::Opl3Windows);
        let registry = DecoderRegistry::new(settings.clone());
        assert_eq!(
            registry.backend_id_for(Path::new("song.mid")),
            Some("midi-opl3windows")
        );

        settings.set_midi_engine(MidiEngine::RustySynth);
        assert_eq!(
            registry.backend_id_for(Path::new("song.mid")),
            Some("midi-rustysynth-sf2")
        );
    }

    #[test]
    fn midi_probe_reports_smf_and_rmid_stream_properties() {
        let directory = std::env::temp_dir();
        let midi_path = directory.join(format!("kog-midi-{}.mid", std::process::id()));
        let rmid_path = directory.join(format!("kog-midi-{}.rmi", std::process::id()));
        write_test_midi(&midi_path);
        write_test_rmid(&rmid_path);
        let registry = DecoderRegistry::default();

        for path in [&midi_path, &rmid_path] {
            let source = PlaybackSource::from_path(path.clone());
            let properties = registry.probe(&source).expect("probe MIDI fixture");
            assert_eq!(properties.sample_rate, Some(MIDI_SAMPLE_RATE));
            assert_eq!(properties.channels, Some(MIDI_CHANNELS));
            let duration = properties.duration.expect("MIDI duration").as_secs_f64();
            assert!((duration - 0.5).abs() < 0.001, "duration was {duration}");
        }

        std::fs::remove_file(midi_path).ok();
        std::fs::remove_file(rmid_path).ok();
    }

    #[test]
    fn midi_backend_requires_a_configured_soundfont() {
        let backend = MidiBackend::new(DecoderSettings::default());
        let error = backend.load_soundfont().expect_err("missing SoundFont");
        assert!(error.contains("requires an SF2 SoundFont"));
    }

    #[test]
    fn opl3_timeline_handles_tempo_changes_and_smpte_timing() {
        let timeline = OplMidiTimeline::parse(&tempo_change_test_midi()).expect("tempo timeline");
        assert_eq!(timeline.total_frames, 72_000);
        assert_eq!(
            timeline.events.last().map(|event| event.frame),
            Some(72_000)
        );

        let timeline = OplMidiTimeline::parse(&timecode_test_midi()).expect("timecode timeline");
        assert_eq!(timeline.total_frames, 48_000);
        assert_eq!(
            timeline.events.last().map(|event| event.frame),
            Some(48_000)
        );
    }

    #[test]
    fn opl3_backend_needs_no_soundfont_and_rejects_format_two() {
        let path = std::env::temp_dir().join(format!("kog-opl3-midi-{}.mid", std::process::id()));
        write_test_midi(&path);
        let backend = MidiBackend::new(DecoderSettings::new(None, MidiEngine::Opl3Windows));
        let source = PlaybackSource::from_path(path.clone());
        let properties = backend.probe(&source).expect("probe OPL3 MIDI without SF2");
        assert_eq!(properties.duration, Some(Duration::from_millis(500)));

        let mut format_two = minimal_test_midi();
        format_two[8..10].copy_from_slice(&2_u16.to_be_bytes());
        let error = OplMidiTimeline::parse(&format_two).expect_err("format 2 rejection");
        assert!(error.contains("subsong selection is not implemented"));
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn opl3_midi_source_renders_non_silent_pcm_and_seeks() {
        let timeline = Arc::new(
            OplMidiTimeline::parse(&minimal_test_midi()).expect("minimal OPL3 MIDI timeline"),
        );
        let mut source = OplMidiSource::new(timeline).expect("OPL3 MIDI source");

        let initial_pcm = source.by_ref().take(4_800 * 2).collect::<Vec<_>>();
        assert!(
            initial_pcm.iter().any(|sample| sample.abs() > 0.000_01),
            "rendered OPL3 PCM was silent"
        );

        source
            .try_seek(Duration::from_millis(250))
            .expect("seek OPL3 MIDI source");
        assert_eq!(source.frames_rendered, 12_000);
        assert_eq!(source.samples_emitted, 24_000);
        assert!(
            source
                .by_ref()
                .take(2_400 * 2)
                .any(|sample| sample.abs() > 0.000_01),
            "rendered OPL3 PCM was silent after seeking"
        );
    }

    #[test]
    fn midi_source_renders_non_silent_pcm_and_seeks() {
        let mut soundfont_bytes = Cursor::new(minimal_test_soundfont());
        let soundfont = Arc::new(SoundFont::new(&mut soundfont_bytes).expect("minimal SoundFont"));
        let mut midi_bytes = Cursor::new(minimal_test_midi());
        let midi_file = Arc::new(MidiFile::new(&mut midi_bytes).expect("minimal MIDI"));
        let duration = Duration::from_secs_f64(midi_file.get_length());
        let mut source = MidiSource::new(soundfont, midi_file, duration).expect("MIDI source");

        let initial_pcm = source.by_ref().take(4_800 * 2).collect::<Vec<_>>();
        assert!(
            initial_pcm.iter().any(|sample| sample.abs() > 0.000_01),
            "rendered MIDI PCM was silent"
        );

        source
            .try_seek(Duration::from_millis(250))
            .expect("seek MIDI source");
        assert_eq!(source.frames_rendered, 12_000);
        assert_eq!(source.samples_emitted, 24_000);
        assert!(
            source
                .by_ref()
                .take(2_400 * 2)
                .any(|sample| sample.abs() > 0.000_01),
            "rendered MIDI PCM was silent after seeking"
        );
    }
}
