use std::fs::File;
use std::io::Cursor;
use std::num::{NonZeroU16, NonZeroU32};
use std::path::Path;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, RwLock};
use std::time::Duration;
use std::time::SystemTime;

use rodio::source::SeekError;
use rodio::{ChannelCount, Decoder, Player, SampleRate, Source};
use rustysynth::{MidiFile, MidiFileSequencer, SoundFont, Synthesizer, SynthesizerSettings};

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
    fn probe(&self, path: &Path) -> Result<StreamProperties, String>;
    fn append(&self, path: &Path, player: &Player) -> Result<(), String>;

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
}

impl DecoderSettings {
    pub fn new(soundfont_path: Option<PathBuf>) -> Self {
        Self {
            soundfont_path: Arc::new(RwLock::new(soundfont_path)),
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
            backends: vec![Box::new(RodioBackend), Box::new(MidiBackend::new(settings))],
        }
    }

    pub fn probe(&self, path: &Path) -> Result<StreamProperties, String> {
        let backend = self.select(path).ok_or_else(|| unsupported_message(path))?;
        backend.probe(path)
    }

    pub fn append(&self, path: &Path, player: &Player) -> Result<SelectedBackend, String> {
        let backend = self.select(path).ok_or_else(|| unsupported_message(path))?;
        backend.append(path, player)?;
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

    fn probe(&self, path: &Path) -> Result<StreamProperties, String> {
        let decoder = Decoder::try_from(
            File::open(path).map_err(|error| format!("opening {}: {error}", path.display()))?,
        )
        .map_err(|error| format!("decoding {}: {error}", path.display()))?;

        Ok(StreamProperties {
            duration: decoder.total_duration(),
            sample_rate: Some(decoder.sample_rate().get()),
            channels: Some(decoder.channels().get()),
        })
    }

    fn append(&self, path: &Path, player: &Player) -> Result<(), String> {
        let decoder = Decoder::try_from(
            File::open(path).map_err(|error| format!("opening {}: {error}", path.display()))?,
        )
        .map_err(|error| format!("decoding {}: {error}", path.display()))?;
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
        "midi-rustysynth-sf2"
    }

    fn display_name(&self) -> &'static str {
        "RustySynth SoundFont"
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

    fn probe(&self, path: &Path) -> Result<StreamProperties, String> {
        let (_, duration) = load_midi_file(path)?;
        Ok(StreamProperties {
            duration: Some(duration),
            sample_rate: Some(MIDI_SAMPLE_RATE),
            channels: Some(MIDI_CHANNELS),
        })
    }

    fn append(&self, path: &Path, player: &Player) -> Result<(), String> {
        let soundfont = self.load_soundfont()?;
        let (midi_file, duration) = load_midi_file(path)?;
        player.append(MidiSource::new(soundfont, midi_file, duration)?);
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

        let properties = registry.probe(&path).expect("probe wave fixture");

        assert_eq!(registry.backend_id_for(&path), Some("rodio-symphonia"));
        assert_eq!(properties.sample_rate, Some(8_000));
        assert_eq!(properties.channels, Some(1));
        assert_eq!(properties.duration, Some(Duration::from_millis(100)));
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn registry_advertises_only_implemented_specialist_formats() {
        let registry = DecoderRegistry::default();
        assert_eq!(registry.backend_id_for(Path::new("song.sid")), None);
        assert_eq!(
            registry.backend_id_for(Path::new("song.mid")),
            Some("midi-rustysynth-sf2")
        );
        assert_eq!(
            registry.backend_id_for(Path::new("song.rmi")),
            Some("midi-rustysynth-sf2")
        );
        assert_eq!(registry.backend_id_for(Path::new("song.spc")), None);
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
            let properties = registry.probe(path).expect("probe MIDI fixture");
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
