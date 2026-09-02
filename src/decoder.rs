use std::borrow::Cow;
use std::fs::File;
use std::hash::{Hash, Hasher};
use std::io::Cursor;
use std::num::{NonZeroU16, NonZeroU32};
use std::path::Path;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, RwLock};
use std::time::Duration;
use std::time::SystemTime;

use encoding_rs::WINDOWS_1252;
use midly::{Format, Fps, Header, MetaMessage, MidiMessage, Smf, Timing, TrackEventKind};
use rodio::source::SeekError;
use rodio::{ChannelCount, Decoder, Player, SampleRate, Source};
use rustysynth::{MidiFile, MidiFileSequencer, SoundFont, Synthesizer, SynthesizerSettings};
use url::Url;

use crate::mt32::Mt32Source;
use crate::opl3::Opl3WindowsSynth;
use crate::sc55::Sc55;
use crate::settings::MidiEngine;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DecoderCapabilities {
    pub seek: bool,
    pub subsongs: bool,
    pub loop_metadata: bool,
    pub companion_files: bool,
}

impl DecoderCapabilities {
    pub fn summary(self) -> String {
        let mut capabilities = Vec::new();
        if self.seek {
            capabilities.push("seek");
        }
        if self.subsongs {
            capabilities.push("subsongs");
        }
        if self.loop_metadata {
            capabilities.push("loops");
        }
        if self.companion_files {
            capabilities.push("companion files");
        }
        capabilities.join(", ")
    }
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
    pub lyrics: Option<String>,
    pub year: Option<u32>,
    pub track_number: Option<u32>,
    pub codec: Option<String>,
    pub bitrate: Option<u32>,
    pub bits_per_sample: Option<u8>,
    pub warning: Option<String>,
}

#[derive(Clone, Debug, Default)]
pub struct PlaybackSource {
    pub path: PathBuf,
    pub remote_url: Option<String>,
    pub subsong: Option<u32>,
    pub archive_origin: Option<ArchiveOrigin>,
}

impl PartialEq for PlaybackSource {
    fn eq(&self, other: &Self) -> bool {
        self.subsong == other.subsong
            && match (&self.archive_origin, &other.archive_origin) {
                (Some(left), Some(right)) => left == right,
                (None, None) => match (&self.remote_url, &other.remote_url) {
                    (Some(left), Some(right)) => left == right,
                    (None, None) => self.path == other.path,
                    _ => false,
                },
                _ => false,
            }
    }
}

impl Eq for PlaybackSource {}

impl Hash for PlaybackSource {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.subsong.hash(state);
        if let Some(origin) = &self.archive_origin {
            0_u8.hash(state);
            origin.hash(state);
        } else if let Some(url) = &self.remote_url {
            1_u8.hash(state);
            url.hash(state);
        } else {
            2_u8.hash(state);
            self.path.hash(state);
        }
    }
}

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct ArchiveOrigin {
    pub archive_path: PathBuf,
    pub entry_name: String,
}

impl PlaybackSource {
    pub fn from_path(path: PathBuf) -> Self {
        Self {
            path,
            remote_url: None,
            subsong: None,
            archive_origin: None,
        }
    }

    pub fn from_remote_url(mut url: Url) -> Self {
        url.set_fragment(None);
        let path = PathBuf::from(url.path());
        Self {
            path,
            remote_url: Some(url.into()),
            subsong: None,
            archive_origin: None,
        }
    }

    pub fn is_remote(&self) -> bool {
        self.remote_url.is_some()
    }

    pub fn input_location(&self) -> Cow<'_, str> {
        self.remote_url
            .as_deref()
            .map(Cow::Borrowed)
            .unwrap_or_else(|| self.path.to_string_lossy())
    }

    pub fn set_archive_origin(&mut self, archive_path: PathBuf, entry_name: String) {
        self.archive_origin = Some(ArchiveOrigin {
            archive_path,
            entry_name,
        });
    }

    pub fn display_label(&self) -> String {
        let path = self.remote_url.clone().unwrap_or_else(|| {
            self.archive_origin.as_ref().map_or_else(
                || self.path.display().to_string(),
                |origin| format!("{} :: {}", origin.archive_path.display(), origin.entry_name),
            )
        });
        match self.subsong {
            Some(subsong) => format!("{path}#{}", subsong + 1),
            None => path,
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
        self.capabilities.summary()
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

    /// File extensions this backend can advertise in user-facing format lists.
    ///
    /// Most backends use a static allow-list. Native libraries whose accepted
    /// formats are discovered at runtime override this so the UI cannot drift
    /// from the library actually bundled into the current build.
    fn advertised_extensions(&self) -> Vec<String> {
        self.extensions()
            .iter()
            .map(|extension| (*extension).to_owned())
            .collect()
    }

    fn subsong_count(&self, _path: &Path) -> Result<Option<u32>, String> {
        Ok(None)
    }

    fn source_for_fragment(&self, path: PathBuf, fragment: &str) -> Result<PlaybackSource, String> {
        let subsong = fragment.parse::<u32>().map_err(|error| {
            format!(
                "{} has unsupported non-numeric fragment #{fragment}: {error}",
                path.display()
            )
        })?;
        Ok(PlaybackSource {
            path,
            remote_url: None,
            subsong: Some(subsong),
            archive_origin: None,
        })
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
    sc55_rom_path: Arc<RwLock<Option<PathBuf>>>,
    mt32_rom_path: Arc<RwLock<Option<PathBuf>>>,
    midi_engine: Arc<RwLock<MidiEngine>>,
}

impl DecoderSettings {
    pub fn new(soundfont_path: Option<PathBuf>, midi_engine: MidiEngine) -> Self {
        Self {
            soundfont_path: Arc::new(RwLock::new(soundfont_path)),
            sc55_rom_path: Arc::new(RwLock::new(None)),
            mt32_rom_path: Arc::new(RwLock::new(None)),
            midi_engine: Arc::new(RwLock::new(midi_engine)),
        }
    }

    pub fn with_sc55_rom_path(self, path: Option<PathBuf>) -> Self {
        self.set_sc55_rom_path(path);
        self
    }

    pub fn with_mt32_rom_path(self, path: Option<PathBuf>) -> Self {
        self.set_mt32_rom_path(path);
        self
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

    pub fn sc55_rom_path(&self) -> Option<PathBuf> {
        self.sc55_rom_path
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }

    pub fn set_sc55_rom_path(&self, path: Option<PathBuf>) {
        *self
            .sc55_rom_path
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = path;
    }

    pub fn mt32_rom_path(&self) -> Option<PathBuf> {
        self.mt32_rom_path
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }

    pub fn set_mt32_rom_path(&self, path: Option<PathBuf>) {
        *self
            .mt32_rom_path
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
    archive_workspaces: Arc<Mutex<Vec<tempfile::TempDir>>>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ExpansionResult {
    pub sources: Vec<PlaybackSource>,
    pub warnings: Vec<String>,
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
                Box::new(crate::apl_decoder::AplBackend),
                Box::new(crate::cuesheet_decoder::CueSheetBackend),
                Box::new(RodioBackend),
                Box::new(MidiBackend::new(settings)),
                Box::new(crate::psf_decoder::PsfBackend),
                Box::new(crate::sdsf_decoder::SdsfBackend),
                Box::new(crate::usf_decoder::UsfBackend),
                Box::new(crate::ffmpeg_decoder::FfmpegBackend),
                Box::new(crate::sfm_decoder::SfmBackend),
                Box::new(crate::gme_decoder::GmeBackend),
                Box::new(crate::libvgm_decoder::LibVgmBackend),
                Box::new(crate::adlmidi_decoder::AdlMidiBackend),
                Box::new(crate::openmpt_decoder::OpenMptBackend),
                Box::new(crate::hively_decoder::HivelyBackend),
                Box::new(crate::syntrax_decoder::SyntraxBackend),
                Box::new(crate::organya_decoder::OrganyaBackend),
                Box::new(crate::gsf_decoder::GsfBackend),
                Box::new(crate::ncsf_decoder::NcsfBackend),
                Box::new(crate::qsf_decoder::QsfBackend),
                Box::new(crate::sid_decoder::SidBackend),
                Box::new(crate::adplug_decoder::AdPlugBackend),
                Box::new(crate::vgmstream_decoder::VgmstreamBackend),
            ],
            archive_workspaces: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Build an independent decoder set for background metadata work while
    /// sharing ownership of extracted archive workspaces with this registry.
    pub fn background_worker(&self, settings: DecoderSettings) -> Self {
        let mut worker = Self::new(settings);
        worker.archive_workspaces = Arc::clone(&self.archive_workspaces);
        worker
    }

    #[cfg(test)]
    pub fn expand(&self, path: PathBuf) -> Result<Vec<PlaybackSource>, String> {
        Ok(self.expand_detailed(path)?.sources)
    }

    pub fn expand_detailed(&self, path: PathBuf) -> Result<ExpansionResult, String> {
        self.expand_local(path, None, &mut Vec::new(), 0)
    }

    pub fn expand_remote_url(&self, value: &str) -> Result<ExpansionResult, String> {
        if value.len() > 8_192 {
            return Err("Remote URL exceeds Kog's 8192-character safety limit".to_owned());
        }
        let url = Url::parse(value.trim()).map_err(|error| format!("Invalid URL: {error}"))?;
        if !matches!(url.scheme(), "http" | "https") {
            return Err("Kog supports remote http:// and https:// audio URLs".to_owned());
        }
        if !url.has_host() {
            return Err("Remote URL must include a host".to_owned());
        }
        Ok(ExpansionResult {
            sources: vec![PlaybackSource::from_remote_url(url)],
            warnings: Vec::new(),
        })
    }

    pub fn accepts_path(&self, path: &Path) -> bool {
        crate::playlist::Playlist::is_path(path)
            || crate::archive::is_path(path)
            || self.select(path).is_some()
    }

    pub fn supported_formats_json(&self) -> String {
        let mut groups = Vec::new();
        let mut unique_extensions = std::collections::BTreeSet::new();

        let mut add_group = |name: &str, detail: &str, extensions: Vec<String>| {
            let mut extensions = extensions
                .into_iter()
                .map(|extension| extension.trim_start_matches('.').to_ascii_lowercase())
                .filter(|extension| !extension.is_empty())
                .collect::<Vec<_>>();
            extensions.sort_unstable();
            extensions.dedup();
            unique_extensions.extend(extensions.iter().cloned());
            groups.push(serde_json::json!({
                "name": name,
                "detail": detail,
                "extensions": extensions,
            }));
        };

        add_group(
            "Playlists",
            "M3U, M3U8, and PLS playlists; HTTP(S) streams are also supported",
            crate::playlist::Playlist::supported_extensions()
                .iter()
                .map(|extension| (*extension).to_owned())
                .collect(),
        );
        add_group(
            "Archives",
            "Archives are expanded in process and scanned for playable entries",
            crate::archive::supported_extensions(),
        );
        for backend in &self.backends {
            add_group(
                backend.display_name(),
                &backend.capabilities().summary(),
                backend.advertised_extensions(),
            );
        }

        serde_json::json!({
            "uniqueExtensionCount": unique_extensions.len(),
            "groups": groups,
        })
        .to_string()
    }

    fn expand_local(
        &self,
        path: PathBuf,
        fragment: Option<&str>,
        playlist_stack: &mut Vec<PathBuf>,
        depth: usize,
    ) -> Result<ExpansionResult, String> {
        if depth > 32 {
            return Err("playlist nesting exceeds Kog's 32-level safety limit".to_owned());
        }
        if let Some(fragment) = fragment {
            let backend = self
                .select(&path)
                .ok_or_else(|| unsupported_message(&path))?;
            return backend
                .source_for_fragment(path, fragment)
                .map(|source| ExpansionResult {
                    sources: vec![source],
                    warnings: Vec::new(),
                });
        }

        if crate::playlist::Playlist::is_path(&path) && crate::playlist::Playlist::is_hls(&path)? {
            return Ok(ExpansionResult {
                sources: vec![PlaybackSource::from_path(path)],
                warnings: Vec::new(),
            });
        }
        if crate::playlist::Playlist::is_path(&path) {
            return self.expand_playlist(path, playlist_stack, depth);
        }
        if crate::archive::is_path(&path) {
            return self.expand_archive(path, playlist_stack, depth);
        }

        let Some(backend) = self.select(&path) else {
            return Ok(ExpansionResult {
                sources: vec![PlaybackSource::from_path(path)],
                warnings: Vec::new(),
            });
        };
        let Some(count) = backend.subsong_count(&path)? else {
            return Ok(ExpansionResult {
                sources: vec![PlaybackSource::from_path(path)],
                warnings: Vec::new(),
            });
        };
        if count == 0 {
            return Err(format!("{} contains no playable subsongs", path.display()));
        }
        Ok(ExpansionResult {
            sources: (0..count)
                .map(|subsong| PlaybackSource {
                    path: path.clone(),
                    remote_url: None,
                    subsong: Some(subsong),
                    archive_origin: None,
                })
                .collect(),
            warnings: Vec::new(),
        })
    }

    fn expand_playlist(
        &self,
        path: PathBuf,
        playlist_stack: &mut Vec<PathBuf>,
        depth: usize,
    ) -> Result<ExpansionResult, String> {
        let path = path
            .canonicalize()
            .map_err(|error| format!("resolving playlist {}: {error}", path.display()))?;
        if let Some(cycle_start) = playlist_stack.iter().position(|ancestor| ancestor == &path) {
            let mut cycle = playlist_stack[cycle_start..]
                .iter()
                .map(|entry| entry.display().to_string())
                .collect::<Vec<_>>();
            cycle.push(path.display().to_string());
            return Err(format!("playlist cycle: {}", cycle.join(" -> ")));
        }

        let playlist = crate::playlist::Playlist::open(&path)?;
        playlist_stack.push(path.clone());
        let mut result = ExpansionResult::default();
        for entry in playlist.entries() {
            match &entry.location {
                crate::playlist::PlaylistLocation::Remote(url) => {
                    match self.expand_remote_url(url) {
                        Ok(expansion) => result.sources.extend(expansion.sources),
                        Err(error) => result.warnings.push(format!(
                            "Remote playlist entry {url} was not added: {error}"
                        )),
                    }
                }
                crate::playlist::PlaylistLocation::Archive {
                    archive_path,
                    entry_name,
                } => match self.expand_archive_entry(
                    archive_path.clone(),
                    entry_name,
                    entry.fragment.as_deref(),
                    playlist_stack,
                    depth,
                ) {
                    Ok(expansion) => {
                        result.sources.extend(expansion.sources);
                        result.warnings.extend(expansion.warnings);
                    }
                    Err(error) => result.warnings.push(format!(
                        "Archive playlist entry {} :: {} was not added: {error}",
                        archive_path.display(),
                        entry_name
                    )),
                },
                crate::playlist::PlaylistLocation::Local(entry_path) => {
                    let resolved = match entry_path.canonicalize() {
                        Ok(path) => path,
                        Err(error) => {
                            result.warnings.push(format!(
                                "Could not resolve playlist entry {}: {error}",
                                entry_path.display()
                            ));
                            continue;
                        }
                    };
                    if !crate::playlist::Playlist::is_path(&resolved)
                        && self.select(&resolved).is_none()
                    {
                        result.warnings.push(format!(
                            "Playlist entry {} was not added: {}",
                            resolved.display(),
                            unsupported_message(&resolved)
                        ));
                        continue;
                    }
                    match self.expand_local(
                        resolved,
                        entry.fragment.as_deref(),
                        playlist_stack,
                        depth + 1,
                    ) {
                        Ok(expansion) => {
                            result.sources.extend(expansion.sources);
                            result.warnings.extend(expansion.warnings);
                        }
                        Err(error) => result.warnings.push(error),
                    }
                }
            }
        }
        playlist_stack.pop();

        if result.sources.is_empty() && result.warnings.is_empty() {
            return Err(format!("{} contains no playlist entries", path.display()));
        }
        Ok(result)
    }

    fn expand_archive_entry(
        &self,
        path: PathBuf,
        entry_name: &str,
        fragment: Option<&str>,
        playlist_stack: &mut Vec<PathBuf>,
        depth: usize,
    ) -> Result<ExpansionResult, String> {
        let path = path
            .canonicalize()
            .map_err(|error| format!("resolving archive {}: {error}", path.display()))?;
        let extracted = crate::archive::ExtractedArchive::open(&path)?;
        let (workspace, entries, warnings) = extracted.into_parts();
        let workspace_path = workspace.path().to_path_buf();
        let entry = entries
            .into_iter()
            .find(|entry| entry.name == entry_name)
            .ok_or_else(|| {
                format!(
                    "{} has no archive entry named {entry_name:?}",
                    path.display()
                )
            })?;
        if crate::archive::is_path(&entry.path) {
            return Err(format!(
                "nested archive entry {entry_name:?} is not supported"
            ));
        }
        if !crate::playlist::Playlist::is_path(&entry.path) && self.select(&entry.path).is_none() {
            return Err(unsupported_message(&entry.path));
        }

        let mut result = self.expand_local(entry.path, fragment, playlist_stack, depth + 1)?;
        for source in &mut result.sources {
            let logical_entry = source
                .path
                .strip_prefix(&workspace_path)
                .ok()
                .map(crate::archive::portable_name)
                .unwrap_or_else(|| entry_name.to_owned());
            source.set_archive_origin(path.clone(), logical_entry);
        }
        result.warnings.splice(0..0, warnings);
        self.archive_workspaces
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .push(workspace);
        Ok(result)
    }

    fn expand_archive(
        &self,
        path: PathBuf,
        playlist_stack: &mut Vec<PathBuf>,
        depth: usize,
    ) -> Result<ExpansionResult, String> {
        let path = path
            .canonicalize()
            .map_err(|error| format!("resolving archive {}: {error}", path.display()))?;
        let extracted = crate::archive::ExtractedArchive::open(&path)?;
        let (workspace, entries, warnings) = extracted.into_parts();
        let workspace_path = workspace.path().to_path_buf();
        let mut result = ExpansionResult {
            sources: Vec::new(),
            warnings,
        };

        for entry in entries {
            if crate::archive::is_path(&entry.path) || self.select(&entry.path).is_none() {
                continue;
            }
            match self.expand_local(entry.path, None, playlist_stack, depth + 1) {
                Ok(mut expansion) => {
                    for source in &mut expansion.sources {
                        let entry_name = source
                            .path
                            .strip_prefix(&workspace_path)
                            .ok()
                            .map(crate::archive::portable_name)
                            .unwrap_or_else(|| entry.name.clone());
                        source.set_archive_origin(path.clone(), entry_name);
                    }
                    result.sources.extend(expansion.sources);
                    result.warnings.extend(expansion.warnings);
                }
                Err(error) => result.warnings.push(format!(
                    "Archive entry {} was not added: {error}",
                    entry.name
                )),
            }
        }

        if result.sources.is_empty() {
            let details = if result.warnings.is_empty() {
                String::new()
            } else {
                format!(": {}", result.warnings.join("; "))
            };
            return Err(format!(
                "{} contains no supported audio entries{details}",
                path.display()
            ));
        }
        self.archive_workspaces
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .push(workspace);
        Ok(result)
    }

    #[cfg(test)]
    pub fn probe(&self, source: &PlaybackSource) -> Result<StreamProperties, String> {
        self.probe_with_backend(source)
            .map(|(_, properties)| properties)
    }

    pub fn probe_with_backend(
        &self,
        source: &PlaybackSource,
    ) -> Result<(&'static str, StreamProperties), String> {
        let backend = self
            .select_source(source)
            .ok_or_else(|| unsupported_message(&source.path))?;
        backend
            .probe(source)
            .map(|properties| (backend.id(), properties))
    }

    pub fn selected_backend_id(&self, source: &PlaybackSource) -> Option<&'static str> {
        self.select_source(source).map(DecoderBackend::id)
    }

    pub fn append(
        &self,
        source: &PlaybackSource,
        player: &Player,
    ) -> Result<SelectedBackend, String> {
        let backend = self
            .select_source(source)
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

    fn select_source(&self, source: &PlaybackSource) -> Option<&dyn DecoderBackend> {
        if source.is_remote() {
            return self
                .backends
                .iter()
                .map(Box::as_ref)
                .find(|backend| backend.id() == "ffmpeg");
        }
        self.select(&source.path)
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

const MIDI_EXTENSIONS: &[&str] = &[
    "kar", "mid", "midi", "rmi", "mids", "mds", "lds", "xmf", "mxmf",
];
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

    fn open_sc55(&self, path: &Path, midi: &[u8]) -> Result<Sc55, String> {
        let rom_directory = self.settings.sc55_rom_path().ok_or_else(|| {
            "SC-55 playback requires user-supplied Roland ROMs. Choose their directory in Edit > Preferences > MIDI."
                .to_owned()
        })?;
        if !rom_directory.is_dir() {
            return Err(format!(
                "Selected SC-55 ROM directory is unavailable: {}",
                rom_directory.display()
            ));
        }
        Sc55::open(midi, path, &rom_directory)
    }

    fn open_mt32(&self, midi: &[u8]) -> Result<Mt32Source, String> {
        let rom_directory = self.settings.mt32_rom_path().ok_or_else(|| {
            "MT-32 playback requires user-supplied Roland ROMs. Choose their directory in Preferences > Synthesis."
                .to_owned()
        })?;
        if !rom_directory.is_dir() {
            return Err(format!(
                "Selected MT-32 ROM directory is unavailable: {}",
                rom_directory.display()
            ));
        }
        Mt32Source::open(midi, &rom_directory)
    }
}

impl DecoderBackend for MidiBackend {
    fn id(&self) -> &'static str {
        match self.settings.midi_engine() {
            MidiEngine::RustySynth => "midi-rustysynth-sf2",
            MidiEngine::Opl3Windows => "midi-opl3windows",
            MidiEngine::Sc55 => "midi-nuked-sc55",
            MidiEngine::Mt32 => "midi-munt-mt32",
        }
    }

    fn display_name(&self) -> &'static str {
        match self.settings.midi_engine() {
            MidiEngine::RustySynth => "RustySynth SoundFont",
            MidiEngine::Opl3Windows => "OPL3Windows (Nuked OPL3)",
            MidiEngine::Sc55 => "Nuked SC-55 0.6.1",
            MidiEngine::Mt32 => "Munt MT-32/CM-32L",
        }
    }

    fn extensions(&self) -> &'static [&'static str] {
        MIDI_EXTENSIONS
    }

    fn accepts(&self, path: &Path) -> bool {
        let Some(extension) = path.extension().and_then(|value| value.to_str()) else {
            return false;
        };
        if extension.eq_ignore_ascii_case("mds") {
            return file_has_mids_header(path);
        }
        if extension.eq_ignore_ascii_case("xmf") {
            return file_starts_with(path, b"XMF_");
        }
        MIDI_EXTENSIONS
            .iter()
            .any(|candidate| candidate.eq_ignore_ascii_case(extension))
    }

    fn capabilities(&self) -> DecoderCapabilities {
        DecoderCapabilities {
            seek: true,
            subsongs: true,
            ..DecoderCapabilities::default()
        }
    }

    fn subsong_count(&self, path: &Path) -> Result<Option<u32>, String> {
        Ok(read_standard_midi_subsong(path, None)?.subsong_count)
    }

    fn probe(&self, source: &PlaybackSource) -> Result<StreamProperties, String> {
        let midi = read_standard_midi_subsong(&source.path, source.subsong)?;
        let title = midi.title.clone();
        let track_number = source.subsong.map(|subsong| subsong + 1);
        let properties = match self.settings.midi_engine() {
            MidiEngine::RustySynth => StreamProperties {
                duration: Some(load_midi_file(&source.path, &midi.bytes)?.1),
                sample_rate: Some(MIDI_SAMPLE_RATE),
                channels: Some(MIDI_CHANNELS),
                title,
                track_number,
                codec: Some("SoundFont 2".to_owned()),
                ..StreamProperties::default()
            },
            MidiEngine::Opl3Windows => {
                let duration = OplMidiTimeline::parse(&midi.bytes)?.duration;
                StreamProperties {
                    duration: Some(duration),
                    sample_rate: Some(MIDI_SAMPLE_RATE),
                    channels: Some(MIDI_CHANNELS),
                    title,
                    track_number,
                    codec: Some("OPL3Windows / Nuked OPL3".to_owned()),
                    ..StreamProperties::default()
                }
            }
            MidiEngine::Sc55 => {
                let sc55 = self.open_sc55(&source.path, &midi.bytes)?;
                StreamProperties {
                    duration: Some(sc55.duration()),
                    sample_rate: Some(sc55.sample_rate()),
                    channels: Some(sc55.channels()),
                    title,
                    track_number,
                    codec: Some(format!("Nuked SC-55 ({})", sc55.model())),
                    ..StreamProperties::default()
                }
            }
            MidiEngine::Mt32 => {
                let mt32 = self.open_mt32(&midi.bytes)?;
                StreamProperties {
                    duration: Some(mt32.duration()),
                    sample_rate: Some(mt32.sample_rate_value()),
                    channels: Some(MIDI_CHANNELS),
                    title,
                    track_number,
                    codec: Some(format!("Munt MT-32 ({})", mt32.model())),
                    ..StreamProperties::default()
                }
            }
        };
        Ok(properties)
    }

    fn append(&self, source: &PlaybackSource, player: &Player) -> Result<(), String> {
        let midi = read_standard_midi_subsong(&source.path, source.subsong)?;
        match self.settings.midi_engine() {
            MidiEngine::RustySynth => {
                let soundfont = self.load_soundfont()?;
                let (midi_file, duration) = load_midi_file(&source.path, &midi.bytes)?;
                player.append(MidiSource::new(soundfont, midi_file, duration)?);
            }
            MidiEngine::Opl3Windows => {
                let timeline = Arc::new(OplMidiTimeline::parse(&midi.bytes)?);
                player.append(OplMidiSource::new(timeline)?);
            }
            MidiEngine::Sc55 => {
                player.append(Sc55MidiSource::new(
                    self.open_sc55(&source.path, &midi.bytes)?,
                ));
            }
            MidiEngine::Mt32 => {
                player.append(self.open_mt32(&midi.bytes)?);
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

fn load_midi_file(path: &Path, bytes: &[u8]) -> Result<(Arc<MidiFile>, Duration), String> {
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

struct MidiDocument {
    bytes: Vec<u8>,
    title: Option<Vec<u8>>,
}

fn read_midi_document(path: &Path) -> Result<MidiDocument, String> {
    const MAX_MIDI_BYTES: u64 = 256 * 1024 * 1024;
    let metadata = std::fs::metadata(path)
        .map_err(|error| format!("reading MIDI metadata for {}: {error}", path.display()))?;
    if metadata.len() > MAX_MIDI_BYTES {
        return Err(format!(
            "MIDI file {} exceeds Kog's 256 MiB limit",
            path.display()
        ));
    }
    let bytes = std::fs::read(path)
        .map_err(|error| format!("reading MIDI file {}: {error}", path.display()))?;
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > MAX_MIDI_BYTES {
        return Err(format!(
            "MIDI file {} grew beyond Kog's 256 MiB limit while it was read",
            path.display()
        ));
    }
    if bytes.starts_with(b"MThd") {
        return Ok(MidiDocument { bytes, title: None });
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
                return Ok(MidiDocument {
                    bytes: bytes[start..end].to_vec(),
                    title: None,
                });
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
    if is_spessasynth_container_path(path) {
        return crate::spessasynth_midi::convert(&bytes, path)
            .map(|converted| MidiDocument {
                bytes: converted.bytes,
                title: converted.title,
            })
            .map_err(|error| format!("converting MIDI container {}: {error}", path.display()));
    }
    Err(format!(
        "MIDI file {} has no supported MIDI container header",
        path.display()
    ))
}

#[derive(Debug)]
pub(crate) struct StandardMidiSubsong {
    pub bytes: Vec<u8>,
    pub title: Option<String>,
    pub subsong_count: Option<u32>,
}

pub(crate) fn read_standard_midi_subsong(
    path: &Path,
    subsong: Option<u32>,
) -> Result<StandardMidiSubsong, String> {
    let document = read_midi_document(path)?;
    let mut selected = select_standard_midi_subsong(&document.bytes, subsong)
        .map_err(|error| format!("preparing MIDI file {}: {error}", path.display()))?;
    if selected.title.is_none() {
        selected.title = document.title.as_deref().and_then(decode_midi_text);
    }
    Ok(selected)
}

fn is_spessasynth_container_path(path: &Path) -> bool {
    path.extension()
        .and_then(|value| value.to_str())
        .is_some_and(|extension| {
            ["mids", "mds", "lds", "xmf", "mxmf"]
                .iter()
                .any(|candidate| candidate.eq_ignore_ascii_case(extension))
        })
}

fn file_has_mids_header(path: &Path) -> bool {
    let mut header = [0_u8; 16];
    std::fs::File::open(path)
        .and_then(|mut file| std::io::Read::read_exact(&mut file, &mut header))
        .is_ok_and(|_| &header[0..4] == b"RIFF" && &header[8..16] == b"MIDSfmt ")
}

fn file_starts_with(path: &Path, expected: &[u8]) -> bool {
    let mut header = vec![0_u8; expected.len()];
    std::fs::File::open(path)
        .and_then(|mut file| std::io::Read::read_exact(&mut file, &mut header))
        .is_ok_and(|_| header == expected)
}

pub(crate) fn select_standard_midi_subsong(
    bytes: &[u8],
    subsong: Option<u32>,
) -> Result<StandardMidiSubsong, String> {
    let smf = Smf::parse(bytes).map_err(|error| format!("parsing SMF: {error}"))?;
    if smf.tracks.is_empty() {
        return Err("SMF contains no tracks".to_owned());
    }

    let sequential = smf.header.format == Format::Sequential;
    let subsong_count = if sequential && smf.tracks.len() > 1 {
        Some(
            u32::try_from(smf.tracks.len())
                .map_err(|_| "SMF format 2 track count exceeds Kog's limit".to_owned())?,
        )
    } else {
        None
    };
    let selected_index = usize::try_from(subsong.unwrap_or(0))
        .map_err(|_| "MIDI subsong index exceeds this platform's limit".to_owned())?;
    if sequential {
        let track = smf.tracks.get(selected_index).ok_or_else(|| {
            format!(
                "SMF format 2 subsong {} is outside the {}-track file",
                selected_index,
                smf.tracks.len()
            )
        })?;
        let title = midi_track_title(track);
        let selected = Smf {
            header: Header::new(Format::SingleTrack, smf.header.timing),
            tracks: vec![track.clone()],
        };
        let mut selected_bytes = Vec::new();
        selected
            .write_std(&mut selected_bytes)
            .map_err(|error| format!("encoding selected SMF format 2 track: {error}"))?;
        return Ok(StandardMidiSubsong {
            bytes: selected_bytes,
            title,
            subsong_count,
        });
    }

    if selected_index != 0 {
        return Err(format!(
            "MIDI subsong {selected_index} was requested from a format {} file with one song",
            match smf.header.format {
                Format::SingleTrack => 0,
                Format::Parallel => 1,
                Format::Sequential => unreachable!("handled above"),
            }
        ));
    }
    Ok(StandardMidiSubsong {
        title: smf.tracks.first().and_then(|track| midi_track_title(track)),
        bytes: bytes.to_vec(),
        subsong_count,
    })
}

fn midi_track_title(track: &[midly::TrackEvent<'_>]) -> Option<String> {
    track.iter().find_map(|event| match event.kind {
        TrackEventKind::Meta(MetaMessage::TrackName(bytes)) => decode_midi_text(bytes),
        _ => None,
    })
}

fn decode_midi_text(bytes: &[u8]) -> Option<String> {
    let decoded = std::str::from_utf8(bytes).map_or_else(
        |_| WINDOWS_1252.decode_without_bom_handling(bytes).0,
        Cow::Borrowed,
    );
    let mut text = String::with_capacity(decoded.len().min(512));
    for character in decoded
        .trim_matches(['\0', ' ', '\t', '\r', '\n'])
        .chars()
        .take(512)
    {
        if matches!(character, '\r' | '\n' | '\t') {
            if !text.ends_with(' ') {
                text.push(' ');
            }
        } else if !character.is_control() {
            text.push(character);
        }
    }
    let text = text.trim().to_owned();
    (!text.is_empty()).then_some(text)
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

struct Sc55MidiSource {
    decoder: Sc55,
    duration: Duration,
    sample_rate: u32,
    interleaved: Vec<f32>,
    interleaved_index: usize,
    render_error: Option<String>,
}

impl Sc55MidiSource {
    fn new(decoder: Sc55) -> Self {
        let duration = decoder.duration();
        let sample_rate = decoder.sample_rate();
        Self {
            decoder,
            duration,
            sample_rate,
            interleaved: Vec::with_capacity(MIDI_RENDER_FRAMES * usize::from(MIDI_CHANNELS)),
            interleaved_index: 0,
            render_error: None,
        }
    }

    fn fill_interleaved(&mut self) {
        self.interleaved
            .resize(MIDI_RENDER_FRAMES * usize::from(MIDI_CHANNELS), 0.0);
        match self.decoder.render(&mut self.interleaved) {
            Ok(frames) => self
                .interleaved
                .truncate(frames * usize::from(MIDI_CHANNELS)),
            Err(error) => {
                eprintln!("Kog SC-55 playback stopped: {error}");
                self.render_error = Some(error);
                self.interleaved.clear();
            }
        }
        self.interleaved_index = 0;
    }

    fn remaining_samples(&self) -> u64 {
        if self.render_error.is_some() {
            return 0;
        }
        self.decoder
            .total_frames()
            .saturating_sub(self.decoder.rendered_frames())
            .saturating_mul(u64::from(MIDI_CHANNELS))
            .saturating_add(
                u64::try_from(
                    self.interleaved
                        .len()
                        .saturating_sub(self.interleaved_index),
                )
                .unwrap_or(u64::MAX),
            )
    }
}

impl Iterator for Sc55MidiSource {
    type Item = f32;

    fn next(&mut self) -> Option<Self::Item> {
        if self.interleaved_index == self.interleaved.len() {
            if self.render_error.is_some()
                || self.decoder.rendered_frames() >= self.decoder.total_frames()
            {
                return None;
            }
            self.fill_interleaved();
        }
        let sample = *self.interleaved.get(self.interleaved_index)?;
        self.interleaved_index += 1;
        Some(sample)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = usize::try_from(self.remaining_samples()).unwrap_or(usize::MAX);
        (remaining, Some(remaining))
    }
}

impl Source for Sc55MidiSource {
    fn current_span_len(&self) -> Option<usize> {
        self.size_hint().1
    }

    fn channels(&self) -> ChannelCount {
        NonZeroU16::new(MIDI_CHANNELS).expect("SC-55 output is stereo")
    }

    fn sample_rate(&self) -> SampleRate {
        NonZeroU32::new(self.sample_rate).expect("validated SC-55 sample rate is nonzero")
    }

    fn total_duration(&self) -> Option<Duration> {
        Some(self.duration)
    }

    fn try_seek(&mut self, position: Duration) -> Result<(), SeekError> {
        self.decoder
            .seek(position)
            .map_err(|error| SeekError::Other(Arc::new(std::io::Error::other(error))))?;
        self.interleaved.clear();
        self.interleaved_index = 0;
        self.render_error = None;
        Ok(())
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

    #[test]
    fn supported_format_catalog_is_registry_driven_and_complete() {
        let registry = DecoderRegistry::default();
        let catalog: serde_json::Value = serde_json::from_str(&registry.supported_formats_json())
            .expect("supported-format catalog JSON");
        let groups = catalog["groups"].as_array().expect("format groups");

        let extensions_for = |name: &str| {
            groups
                .iter()
                .find(|group| group["name"] == name)
                .and_then(|group| group["extensions"].as_array())
                .expect("named format group")
        };
        assert!(
            extensions_for("Playlists")
                .iter()
                .any(|value| value == "m3u8")
        );
        assert!(
            extensions_for("Archives")
                .iter()
                .any(|value| value == "vgm7z")
        );
        assert!(
            extensions_for("AdPlug (Cog pin) + Nuked OPL3")
                .iter()
                .any(|value| value == "cmf")
        );
        assert!(
            extensions_for("vgmstream r2117 (built-in codecs)")
                .iter()
                .any(|value| value == "vag")
        );
        assert!(catalog["uniqueExtensionCount"].as_u64().unwrap_or_default() > 700);
    }

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

    fn mids_test_bytes(eight_byte_records: bool) -> Vec<u8> {
        let mut records = Vec::new();
        let mut push_record = |delta: u32, event: u32| {
            records.extend_from_slice(&delta.to_le_bytes());
            if !eight_byte_records {
                records.extend_from_slice(&0_u32.to_le_bytes());
            }
            records.extend_from_slice(&event.to_le_bytes());
        };
        push_record(0, 0x0107_a120); // 500,000 microseconds per quarter note.
        push_record(0, 0x0000_00c0); // Program 0 on channel 0.
        push_record(0, 0x0064_3c90); // Note-on: middle C, velocity 100.
        push_record(480, 0x0040_3c80); // Note-off after one quarter note.

        let mut body = Vec::new();
        body.extend_from_slice(&1_u32.to_le_bytes()); // One segment.
        body.extend_from_slice(&0_u32.to_le_bytes()); // Reserved segment word.
        body.extend_from_slice(&(records.len() as u32).to_le_bytes());
        body.extend_from_slice(&records);

        let mut mids = Vec::new();
        mids.extend_from_slice(b"RIFF");
        mids.extend_from_slice(&0_u32.to_le_bytes());
        mids.extend_from_slice(b"MIDSfmt ");
        mids.extend_from_slice(&12_u32.to_le_bytes());
        mids.extend_from_slice(&480_u32.to_le_bytes());
        mids.extend_from_slice(&0_u32.to_le_bytes());
        mids.extend_from_slice(&u32::from(eight_byte_records).to_le_bytes());
        mids.extend_from_slice(b"data");
        mids.extend_from_slice(&(body.len() as u32).to_le_bytes());
        mids.extend_from_slice(&body);
        let riff_size = (mids.len() - 8) as u32;
        mids[4..8].copy_from_slice(&riff_size.to_le_bytes());
        mids
    }

    fn stored_zlib_test_bytes(data: &[u8]) -> Vec<u8> {
        assert!(data.len() <= usize::from(u16::MAX));
        let length = u16::try_from(data.len()).expect("bounded stored zlib length");
        let mut output = vec![0x78, 0x01, 0x01];
        output.extend_from_slice(&length.to_le_bytes());
        output.extend_from_slice(&(!length).to_le_bytes());
        output.extend_from_slice(data);
        let mut a = 1_u32;
        let mut b = 0_u32;
        for byte in data {
            a = (a + u32::from(*byte)) % 65_521;
            b = (b + a) % 65_521;
        }
        output.extend_from_slice(&((b << 16) | a).to_be_bytes());
        output
    }

    fn xmf_test_bytes(title: &str, compressed: bool) -> Vec<u8> {
        let midi = minimal_test_midi();
        let decoded_size = midi.len();
        let mut metadata = vec![
            0, 3, // Numeric field: ResourceFormat.
            0, // No international variants.
            3, // Format VLQ plus the two-byte payload.
            4, // Binary metadata.
            0, 0, // Standard resource, SMF type 0.
        ];
        metadata.extend_from_slice(&[
            0,
            8, // Numeric field: Title.
            0, // No international variants.
            (title.len() + 1) as u8,
            0, // Visible ASCII metadata.
        ]);
        metadata.extend_from_slice(title.as_bytes());

        let payload = if compressed {
            stored_zlib_test_bytes(&midi)
        } else {
            midi
        };
        let unpackers = if compressed {
            vec![
                4, // Block length, including this byte.
                0,
                0, // Standard unpacker and its ID.
                decoded_size as u8,
            ]
        } else {
            vec![0]
        };
        let header_size = 4 + metadata.len() + unpackers.len();
        let node_length = header_size + 1 + payload.len();
        let file_length = 11 + node_length;
        assert!(file_length < 128, "fixture must use one-byte XMF VLQs");

        let mut xmf = Vec::with_capacity(file_length);
        xmf.extend_from_slice(b"XMF_1.00");
        xmf.push(file_length as u8);
        xmf.push(0); // Empty file-level metadata table.
        xmf.push(11); // Root node absolute offset.
        xmf.push(node_length as u8);
        xmf.push(0); // A FileNode has no children.
        xmf.push(header_size as u8);
        xmf.push(metadata.len() as u8);
        xmf.extend_from_slice(&metadata);
        xmf.extend_from_slice(&unpackers);
        xmf.push(1); // Inline resource reference.
        xmf.extend_from_slice(&payload);
        assert_eq!(xmf.len(), file_length);
        xmf
    }

    fn oversized_compressed_xmf_test_bytes() -> Vec<u8> {
        let metadata = [0, 3, 0, 3, 4, 0, 0];
        let mut xmf = Vec::new();
        xmf.extend_from_slice(b"XMF_1.00");
        xmf.extend_from_slice(&[32, 0, 11]); // File length, metadata size, tree offset.
        xmf.extend_from_slice(&[21, 0, 19, 7]); // Node, child count, header, metadata.
        xmf.extend_from_slice(&metadata);
        xmf.extend_from_slice(&[
            8, // Eight-byte unpacker block including this length.
            0, 0, // Standard unpacker and its ID.
            0x81, 0x80, 0x80, 0x80, 0x01, // 256 MiB plus one byte decoded.
            1,    // Inline resource reference.
            0,    // Compressed payload placeholder.
        ]);
        assert_eq!(xmf.len(), 32);
        xmf
    }

    fn lds_test_bytes() -> Vec<u8> {
        let mut lds = vec![
            0, // Mode.
            0, 0,  // Legacy speed value.
            10, // Ten simulation ticks per pattern step.
            2,  // Two pattern steps.
        ];
        lds.extend_from_slice(&[0; 9]); // Per-channel note delays.
        lds.push(0); // BD register.
        lds.extend_from_slice(&1_u16.to_le_bytes()); // One patch.

        let mut patch = [0_u8; 46];
        patch[40] = 0; // General MIDI acoustic grand piano.
        patch[41] = 100; // MIDI velocity.
        lds.extend_from_slice(&patch);
        lds.extend_from_slice(&1_u16.to_le_bytes()); // One position.

        for channel in 0_u16..9 {
            let pattern_word_offset = channel * 2;
            lds.extend_from_slice(&(pattern_word_offset * 2).to_le_bytes());
            lds.push(0); // No transpose.
        }
        lds.extend_from_slice(&[0, 0]); // Legacy digital-instrument fields.

        for channel in 0..9 {
            if channel == 0 {
                lds.extend_from_slice(&0x3c00_u16.to_le_bytes());
                lds.extend_from_slice(&0xfc00_u16.to_le_bytes());
            } else {
                lds.extend_from_slice(&0_u16.to_le_bytes());
                lds.extend_from_slice(&0_u16.to_le_bytes());
            }
        }
        lds
    }

    fn format_two_test_midi() -> Vec<u8> {
        let mut midi = vec![b'M', b'T', b'h', b'd', 0, 0, 0, 6, 0, 2, 0, 2, 1, 0xe0];
        for (name, note, duration) in [
            ("Opening Theme", 60_u8, [0x83, 0x60]),
            ("Finale", 67_u8, [0x87, 0x40]),
        ] {
            let mut track = vec![0, 0xff, 0x03, name.len() as u8];
            track.extend_from_slice(name.as_bytes());
            track.extend_from_slice(&[0, 0xc0, 0, 0, 0x90, note, 100]);
            track.extend_from_slice(&duration);
            track.extend_from_slice(&[0x80, note, 64, 0, 0xff, 0x2f, 0]);
            midi.extend_from_slice(b"MTrk");
            midi.extend_from_slice(&(track.len() as u32).to_be_bytes());
            midi.extend_from_slice(&track);
        }
        midi
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
    fn every_registered_decoder_promises_seek_support() {
        let registry = DecoderRegistry::default();
        let missing = registry
            .backends
            .iter()
            .filter(|backend| !backend.capabilities().seek)
            .map(|backend| backend.id())
            .collect::<Vec<_>>();
        assert!(
            missing.is_empty(),
            "decoder backends without seek support: {missing:?}"
        );
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
    fn remote_http_sources_are_normalized_and_forced_through_ffmpeg() {
        let registry = DecoderRegistry::default();
        let expansion = registry
            .expand_remote_url(" https://example.invalid/live?token=test#ignored ")
            .expect("expand remote stream");
        assert_eq!(expansion.sources.len(), 1);
        let source = &expansion.sources[0];
        assert_eq!(
            source.remote_url.as_deref(),
            Some("https://example.invalid/live?token=test")
        );
        assert_eq!(source.display_label(), source.input_location().as_ref());
        assert_eq!(
            registry.select_source(source).map(DecoderBackend::id),
            Some("ffmpeg")
        );
        assert!(registry.expand_remote_url("file:///tmp/song.flac").is_err());
        assert!(
            registry
                .expand_remote_url("ftp://example.invalid/song.mp3")
                .is_err()
        );
        assert!(registry.expand_remote_url("https://").is_err());
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

        settings.set_midi_engine(MidiEngine::Sc55);
        assert_eq!(
            registry.backend_id_for(Path::new("song.mid")),
            Some("midi-nuked-sc55")
        );

        settings.set_midi_engine(MidiEngine::Mt32);
        assert_eq!(
            registry.backend_id_for(Path::new("song.mid")),
            Some("midi-munt-mt32")
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
    fn mids_and_mds_containers_convert_to_audible_seekable_midi() {
        let fixture = tempfile::tempdir().expect("create MIDS fixture directory");
        let settings = DecoderSettings::new(None, MidiEngine::Opl3Windows);
        let registry = DecoderRegistry::new(settings);

        for (name, eight_byte_records) in [("directmusic.mids", true), ("legacy.mds", false)] {
            let path = fixture.path().join(name);
            std::fs::write(&path, mids_test_bytes(eight_byte_records)).expect("write MIDS fixture");
            assert_eq!(registry.backend_id_for(&path), Some("midi-opl3windows"));
            let sources = registry.expand(path.clone()).expect("expand MIDS fixture");
            assert_eq!(sources.len(), 1);
            let properties = registry.probe(&sources[0]).expect("probe MIDS fixture");
            assert_eq!(properties.duration, Some(Duration::from_millis(500)));

            let converted = read_standard_midi_subsong(&path, None)
                .expect("convert MIDS fixture to Standard MIDI");
            let smf = Smf::parse(&converted.bytes).expect("parse converted MIDS stream");
            assert_eq!(smf.header.format, Format::SingleTrack);
            assert!(smf.tracks[0].iter().any(|event| matches!(
                event.kind,
                TrackEventKind::Midi {
                    message: MidiMessage::NoteOn { key, .. },
                    ..
                } if key.as_int() == 60
            )));

            let timeline = Arc::new(
                OplMidiTimeline::parse(&converted.bytes).expect("converted MIDS OPL timeline"),
            );
            let mut source = OplMidiSource::new(timeline).expect("MIDS OPL source");
            assert!(
                source
                    .by_ref()
                    .take(4_800 * 2)
                    .any(|sample| sample.abs() > 0.000_01),
                "converted {name} stream was silent"
            );
            source
                .try_seek(Duration::from_millis(250))
                .expect("seek converted MIDS stream");
            assert_eq!(source.frames_rendered, 12_000);

            if eight_byte_records {
                let mut soundfont_bytes = Cursor::new(minimal_test_soundfont());
                let soundfont = Arc::new(
                    SoundFont::new(&mut soundfont_bytes).expect("load generated minimal SoundFont"),
                );
                let (midi_file, duration) =
                    load_midi_file(&path, &converted.bytes).expect("load converted MIDS SMF");
                let mut source =
                    MidiSource::new(soundfont, midi_file, duration).expect("MIDS SoundFont source");
                assert!(
                    source
                        .by_ref()
                        .take(4_800 * 2)
                        .any(|sample| sample.abs() > 0.000_01),
                    "converted MIDS SoundFont stream was silent"
                );
                source
                    .try_seek(Duration::from_millis(250))
                    .expect("seek converted MIDS SoundFont stream");
                assert_eq!(source.frames_rendered, 12_000);
            }
        }
    }

    #[test]
    fn xmf_and_mxmf_containers_preserve_title_and_render_pcm() {
        let fixture = tempfile::tempdir().expect("create XMF fixture directory");
        let registry = DecoderRegistry::new(DecoderSettings::new(None, MidiEngine::Opl3Windows));

        for (name, compressed) in [("collection.xmf", false), ("mobile.mxmf", true)] {
            let path = fixture.path().join(name);
            std::fs::write(&path, xmf_test_bytes("XMF Fixture", compressed))
                .expect("write XMF fixture");
            assert_eq!(registry.backend_id_for(&path), Some("midi-opl3windows"));
            let source = PlaybackSource::from_path(path.clone());
            let properties = registry.probe(&source).expect("probe XMF fixture");
            assert_eq!(properties.title.as_deref(), Some("XMF Fixture"));
            assert_eq!(properties.duration, Some(Duration::from_millis(500)));

            let converted = read_standard_midi_subsong(&path, None)
                .expect("convert XMF fixture to Standard MIDI");
            let timeline = Arc::new(
                OplMidiTimeline::parse(&converted.bytes).expect("converted XMF OPL timeline"),
            );
            let mut source = OplMidiSource::new(timeline).expect("XMF OPL source");
            assert!(
                source
                    .by_ref()
                    .take(4_800 * 2)
                    .any(|sample| sample.abs() > 0.000_01),
                "converted {name} stream was silent"
            );
        }
    }

    #[test]
    fn lds_tracker_converts_to_audible_seekable_midi() {
        let fixture = tempfile::tempdir().expect("create LDS fixture directory");
        let path = fixture.path().join("tracker.lds");
        std::fs::write(&path, lds_test_bytes()).expect("write LDS fixture");
        let registry = DecoderRegistry::new(DecoderSettings::new(None, MidiEngine::Opl3Windows));
        assert_eq!(registry.backend_id_for(&path), Some("midi-opl3windows"));

        let converted =
            read_standard_midi_subsong(&path, None).expect("convert LDS fixture to Standard MIDI");
        let smf = Smf::parse(&converted.bytes).expect("parse converted LDS stream");
        assert_eq!(smf.header.format, Format::Parallel);
        assert!(smf.tracks.iter().flatten().any(|event| matches!(
            event.kind,
            TrackEventKind::Midi {
                message: MidiMessage::NoteOn { key, .. },
                ..
            } if key.as_int() == 60
        )));

        let timeline =
            Arc::new(OplMidiTimeline::parse(&converted.bytes).expect("converted LDS OPL timeline"));
        let duration = timeline.duration;
        assert!(duration > Duration::ZERO);
        let mut source = OplMidiSource::new(timeline).expect("LDS OPL source");
        assert!(
            source
                .by_ref()
                .take(4_800 * 2)
                .any(|sample| sample.abs() > 0.000_01),
            "converted LDS stream was silent"
        );
        let seek_position = duration / 2;
        source
            .try_seek(seek_position)
            .expect("seek converted LDS stream");
        assert_eq!(
            source.frames_rendered,
            seek_position.as_secs_f64().mul_add(48_000.0, 0.0) as u64
        );
    }

    #[test]
    fn midi_container_routing_rejects_extension_collisions_and_malformed_data() {
        let fixture = tempfile::tempdir().expect("create MIDI collision fixture directory");
        let registry = DecoderRegistry::new(DecoderSettings::new(None, MidiEngine::Opl3Windows));
        let mds_path = fixture.path().join("disc-image.mds");
        std::fs::write(&mds_path, b"MEDIA DESCRIPTOR").expect("write colliding MDS fixture");
        assert_eq!(registry.backend_id_for(&mds_path), Some("vgmstream"));

        let xmf_path = fixture.path().join("tracker.xmf");
        std::fs::write(&xmf_path, b"Extended Module: unrelated tracker")
            .expect("write colliding XMF fixture");
        assert_eq!(registry.backend_id_for(&xmf_path), Some("libopenmpt"));

        let malformed_path = fixture.path().join("broken.mids");
        let mut zero_division = mids_test_bytes(true);
        zero_division[20..24].copy_from_slice(&0_u32.to_le_bytes());
        std::fs::write(&malformed_path, zero_division).expect("write malformed MIDS fixture");
        let error = read_standard_midi_subsong(&malformed_path, None)
            .expect_err("reject zero time division");
        assert!(error.contains("rejected the MIDI container"));

        let mut truncated = mids_test_bytes(false);
        truncated.truncate(truncated.len() - 2);
        std::fs::write(&malformed_path, truncated).expect("write truncated MIDS fixture");
        assert!(read_standard_midi_subsong(&malformed_path, None).is_err());

        let oversized_xmf = fixture.path().join("oversized.mxmf");
        std::fs::write(&oversized_xmf, oversized_compressed_xmf_test_bytes())
            .expect("write oversized XMF fixture");
        let error = read_standard_midi_subsong(&oversized_xmf, None)
            .expect_err("reject oversized compressed XMF payload");
        assert!(error.contains("decoded payloads exceed Kog's 256 MiB limit"));
    }

    #[test]
    fn midi_backend_requires_a_configured_soundfont() {
        let backend = MidiBackend::new(DecoderSettings::default());
        let error = backend.load_soundfont().expect_err("missing SoundFont");
        assert!(error.contains("requires an SF2 SoundFont"));
    }

    #[test]
    fn sc55_backend_requires_a_configured_rom_directory() {
        let path = std::env::temp_dir().join(format!("kog-sc55-midi-{}.mid", std::process::id()));
        write_test_midi(&path);
        let backend = MidiBackend::new(DecoderSettings::new(None, MidiEngine::Sc55));
        let error = backend
            .probe(&PlaybackSource::from_path(path.clone()))
            .expect_err("missing SC-55 ROM directory");
        assert!(error.contains("user-supplied Roland ROMs"));
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn mt32_backend_requires_a_configured_rom_directory() {
        let path = std::env::temp_dir().join(format!("kog-mt32-midi-{}.mid", std::process::id()));
        write_test_midi(&path);
        let backend = MidiBackend::new(DecoderSettings::new(None, MidiEngine::Mt32));
        let error = backend
            .probe(&PlaybackSource::from_path(path.clone()))
            .expect_err("missing MT-32 ROM directory");
        assert!(error.contains("user-supplied Roland ROMs"));
        std::fs::remove_file(path).ok();
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
    fn opl3_backend_needs_no_soundfont() {
        let path = std::env::temp_dir().join(format!("kog-opl3-midi-{}.mid", std::process::id()));
        write_test_midi(&path);
        let backend = MidiBackend::new(DecoderSettings::new(None, MidiEngine::Opl3Windows));
        let source = PlaybackSource::from_path(path.clone());
        let properties = backend.probe(&source).expect("probe OPL3 MIDI without SF2");
        assert_eq!(properties.duration, Some(Duration::from_millis(500)));
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn format_two_midi_expands_named_independent_subsongs_for_opl3() {
        let fixture = tempfile::tempdir().expect("create format 2 fixture directory");
        let path = fixture.path().join("two-songs.mid");
        std::fs::write(&path, format_two_test_midi()).expect("write format 2 fixture");
        let registry = DecoderRegistry::new(DecoderSettings::new(None, MidiEngine::Opl3Windows));

        let sources = registry.expand(path.clone()).expect("expand format 2 MIDI");
        assert_eq!(sources.len(), 2);
        assert_eq!(sources[0].subsong, Some(0));
        assert_eq!(sources[1].subsong, Some(1));

        let first = registry.probe(&sources[0]).expect("probe first song");
        assert_eq!(first.title.as_deref(), Some("Opening Theme"));
        assert_eq!(first.track_number, Some(1));
        assert_eq!(first.duration, Some(Duration::from_millis(500)));
        let second = registry.probe(&sources[1]).expect("probe second song");
        assert_eq!(second.title.as_deref(), Some("Finale"));
        assert_eq!(second.track_number, Some(2));
        assert_eq!(second.duration, Some(Duration::from_secs(1)));

        let selected = read_standard_midi_subsong(&path, Some(1)).expect("select second song");
        let smf = Smf::parse(&selected.bytes).expect("parse selected format 0 stream");
        assert_eq!(smf.header.format, Format::SingleTrack);
        assert_eq!(smf.tracks.len(), 1);
        assert!(smf.tracks[0].iter().any(|event| matches!(
            event.kind,
            TrackEventKind::Midi {
                message: MidiMessage::NoteOn { key, .. },
                ..
            } if key.as_int() == 67
        )));

        let timeline = Arc::new(OplMidiTimeline::parse(&selected.bytes).expect("OPL3 timeline"));
        let mut opl_source = OplMidiSource::new(timeline).expect("OPL3 format 2 source");
        assert!(
            opl_source
                .by_ref()
                .take(4_800 * 2)
                .any(|sample| sample.abs() > 0.000_01),
            "selected OPL3 subsong was silent"
        );
        opl_source
            .try_seek(Duration::from_millis(750))
            .expect("seek selected OPL3 subsong");
        assert_eq!(opl_source.frames_rendered, 36_000);

        let mut soundfont_bytes = Cursor::new(minimal_test_soundfont());
        let soundfont = Arc::new(
            SoundFont::new(&mut soundfont_bytes).expect("load generated minimal SoundFont"),
        );
        let (midi_file, duration) =
            load_midi_file(&path, &selected.bytes).expect("load selected SoundFont song");
        assert_eq!(duration, Duration::from_secs(1));
        let mut soundfont_source =
            MidiSource::new(soundfont, midi_file, duration).expect("SoundFont format 2 source");
        assert!(
            soundfont_source
                .by_ref()
                .take(4_800 * 2)
                .any(|sample| sample.abs() > 0.000_01),
            "selected SoundFont subsong was silent"
        );
        soundfont_source
            .try_seek(Duration::from_millis(750))
            .expect("seek selected SoundFont subsong");
        assert_eq!(soundfont_source.frames_rendered, 36_000);

        let error = registry
            .probe(&PlaybackSource {
                path,
                subsong: Some(2),
                ..PlaybackSource::default()
            })
            .expect_err("reject out-of-range format 2 subsong");
        assert!(error.contains("outside the 2-track file"));
    }

    #[test]
    fn midi_subsong_selector_bounds_fragments_and_decodes_legacy_track_names() {
        let format_zero = minimal_test_midi();
        let error = select_standard_midi_subsong(&format_zero, Some(1))
            .expect_err("reject a fragment on a one-song MIDI file");
        assert!(error.contains("format 0 file with one song"));

        let mut one_track_format_two = format_two_test_midi();
        one_track_format_two[11] = 1;
        let first_track_size = u32::from_be_bytes(
            one_track_format_two[18..22]
                .try_into()
                .expect("first track length"),
        ) as usize;
        one_track_format_two.truncate(22 + first_track_size);
        let selected = select_standard_midi_subsong(&one_track_format_two, None)
            .expect("select the only format 2 track");
        assert_eq!(selected.subsong_count, None);
        assert_eq!(selected.title.as_deref(), Some("Opening Theme"));

        assert_eq!(
            decode_midi_text(b" Caf\xe9\r\nTheme ").as_deref(),
            Some("Caf\u{e9} Theme")
        );
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
