use std::path::{Path, PathBuf};

use directories::ProjectDirs;

use crate::equalizer::EqualizerSettings;

const SOUNDFONT_SETTING_FILE: &str = "soundfont-path";
const MIDI_ENGINE_SETTING_FILE: &str = "midi-engine";
const SC55_ROM_SETTING_FILE: &str = "sc55-rom-directory";
const MT32_ROM_SETTING_FILE: &str = "mt32-rom-directory";
const MUSIC_DIRECTORY_SETTING_FILE: &str = "music-directory";
const OPENING_BEHAVIOR_SETTING_FILE: &str = "opening-files-behavior";
const READ_CUE_SETTING_FILE: &str = "read-cue-sheets-in-folders";
const READ_PLAYLISTS_SETTING_FILE: &str = "read-playlists-in-folders";
const OUTPUT_VOLUME_SETTING_FILE: &str = "output-volume";
const OUTPUT_DEVICE_SETTING_FILE: &str = "output-device";
const PLAYLIST_COLUMN_LAYOUT_SETTING_FILE: &str = "playlist-column-layout";
const PLAYLIST_COLUMN_WIDTHS_SETTING_FILE: &str = "playlist-column-widths";
const EQUALIZER_SETTING_FILE: &str = "equalizer-settings";
const PLAYLIST_COLUMN_IDS: [&str; 19] = [
    "index",
    "status",
    "rating",
    "title",
    "albumartist",
    "artist",
    "composer",
    "album",
    "length",
    "date",
    "genre",
    "track",
    "playcount",
    "path",
    "filename",
    "codec",
    "samplerate",
    "bitspersample",
    "bitrate",
];

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum OpeningFilesBehavior {
    ClearAndPlay,
    Enqueue,
    #[default]
    EnqueueAndPlay,
}

impl OpeningFilesBehavior {
    pub const fn setting_value(self) -> &'static str {
        match self {
            Self::ClearAndPlay => "clearAndPlay",
            Self::Enqueue => "enqueue",
            Self::EnqueueAndPlay => "enqueueAndPlay",
        }
    }

    pub const fn clears_playlist(self) -> bool {
        matches!(self, Self::ClearAndPlay)
    }

    pub const fn starts_playback(self) -> bool {
        matches!(self, Self::ClearAndPlay | Self::EnqueueAndPlay)
    }

    pub fn from_setting(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "clearandplay" | "clear-and-play" | "replace" | "clear-and-add" => {
                Some(Self::ClearAndPlay)
            }
            "add" | "enqueue" => Some(Self::Enqueue),
            "enqueueandplay" | "enqueue-and-play" | "add-and-play" => Some(Self::EnqueueAndPlay),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MidiEngine {
    #[default]
    RustySynth,
    Opl3Windows,
    Sc55,
    Mt32,
}

impl MidiEngine {
    pub const fn setting_value(self) -> &'static str {
        match self {
            Self::RustySynth => "rustysynth-sf2",
            Self::Opl3Windows => "opl3windows",
            Self::Sc55 => "nuked-sc55",
            Self::Mt32 => "munt-mt32",
        }
    }

    pub fn from_setting(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "rustysynth-sf2" | "rustysynth" | "soundfont" => Some(Self::RustySynth),
            "opl3windows" | "opl3w" => Some(Self::Opl3Windows),
            "nuked-sc55" | "sc55" | "sc-55" => Some(Self::Sc55),
            "munt-mt32" | "munt" | "mt32" | "mt-32" | "cm32l" | "cm-32l" => Some(Self::Mt32),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct AppSettings {
    pub soundfont_path: Option<PathBuf>,
    pub sc55_rom_path: Option<PathBuf>,
    pub mt32_rom_path: Option<PathBuf>,
    pub midi_engine: MidiEngine,
    pub music_directory: Option<PathBuf>,
    pub opening_files_behavior: OpeningFilesBehavior,
    pub read_cue_sheets_in_folders: bool,
    pub read_playlists_in_folders: bool,
    pub output_volume: f64,
    pub output_device: Option<OutputDevicePreference>,
    pub playlist_column_layout: Option<String>,
    pub equalizer: EqualizerSettings,
}

impl AppSettings {
    pub fn load() -> Self {
        let soundfont_path = std::env::var_os("KOG_SOUNDFONT")
            .map(PathBuf::from)
            .or_else(load_persisted_soundfont)
            .or_else(discover_system_soundfont);
        let midi_engine = std::env::var("KOG_MIDI_ENGINE")
            .ok()
            .and_then(|value| MidiEngine::from_setting(&value))
            .or_else(load_persisted_midi_engine)
            .unwrap_or_default();
        let sc55_rom_path = std::env::var_os("KOG_SC55_ROMS")
            .map(PathBuf::from)
            .or_else(load_persisted_sc55_rom_path);
        let mt32_rom_path = std::env::var_os("KOG_MT32_ROMS")
            .map(PathBuf::from)
            .or_else(|| load_path(MT32_ROM_SETTING_FILE));
        let music_directory = load_path(MUSIC_DIRECTORY_SETTING_FILE).filter(|path| path.is_dir());
        let opening_files_behavior = load_text(OPENING_BEHAVIOR_SETTING_FILE)
            .and_then(|value| OpeningFilesBehavior::from_setting(&value))
            .unwrap_or_default();
        let read_cue_sheets_in_folders = load_bool(READ_CUE_SETTING_FILE).unwrap_or(true);
        let read_playlists_in_folders = load_bool(READ_PLAYLISTS_SETTING_FILE).unwrap_or(true);
        let output_volume = load_text(OUTPUT_VOLUME_SETTING_FILE)
            .and_then(|value| value.parse::<f64>().ok())
            .filter(|value| value.is_finite())
            .unwrap_or(0.75)
            .clamp(0.0, 1.0);
        let output_device = load_text(OUTPUT_DEVICE_SETTING_FILE)
            .and_then(|value| OutputDevicePreference::parse(&value));
        let playlist_column_layout = load_text(PLAYLIST_COLUMN_LAYOUT_SETTING_FILE)
            .filter(|value| validate_playlist_column_layout(value))
            .or_else(|| {
                load_text(PLAYLIST_COLUMN_WIDTHS_SETTING_FILE)
                    .filter(|value| validate_legacy_playlist_column_widths(value))
            });
        let equalizer = load_text(EQUALIZER_SETTING_FILE)
            .and_then(|value| EqualizerSettings::parse(&value))
            .unwrap_or_default();
        Self {
            soundfont_path,
            sc55_rom_path,
            mt32_rom_path,
            midi_engine,
            music_directory,
            opening_files_behavior,
            read_cue_sheets_in_folders,
            read_playlists_in_folders,
            output_volume,
            output_device,
            playlist_column_layout,
            equalizer,
        }
    }

    pub fn save_soundfont_path(path: Option<&Path>) -> Result<(), String> {
        let setting_path = setting_path(SOUNDFONT_SETTING_FILE)
            .ok_or_else(|| "The platform configuration directory is unavailable".to_owned())?;
        let parent = setting_path
            .parent()
            .ok_or_else(|| "The Kog configuration directory is unavailable".to_owned())?;
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("creating {}: {error}", parent.display()))?;
        let value = path
            .map(|path| path.to_string_lossy().into_owned())
            .unwrap_or_default();
        std::fs::write(&setting_path, value)
            .map_err(|error| format!("writing {}: {error}", setting_path.display()))
    }

    pub fn save_midi_engine(engine: MidiEngine) -> Result<(), String> {
        let setting_path = setting_path(MIDI_ENGINE_SETTING_FILE)
            .ok_or_else(|| "The platform configuration directory is unavailable".to_owned())?;
        let parent = setting_path
            .parent()
            .ok_or_else(|| "The Kog configuration directory is unavailable".to_owned())?;
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("creating {}: {error}", parent.display()))?;
        std::fs::write(&setting_path, engine.setting_value())
            .map_err(|error| format!("writing {}: {error}", setting_path.display()))
    }

    pub fn save_sc55_rom_path(path: Option<&Path>) -> Result<(), String> {
        let setting_path = setting_path(SC55_ROM_SETTING_FILE)
            .ok_or_else(|| "The platform configuration directory is unavailable".to_owned())?;
        let parent = setting_path
            .parent()
            .ok_or_else(|| "The Kog configuration directory is unavailable".to_owned())?;
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("creating {}: {error}", parent.display()))?;
        let value = path
            .map(|path| path.to_string_lossy().into_owned())
            .unwrap_or_default();
        std::fs::write(&setting_path, value)
            .map_err(|error| format!("writing {}: {error}", setting_path.display()))
    }

    pub fn save_mt32_rom_path(path: Option<&Path>) -> Result<(), String> {
        save_text(
            MT32_ROM_SETTING_FILE,
            &path
                .map(|path| path.to_string_lossy().into_owned())
                .unwrap_or_default(),
        )
    }

    pub fn save_music_directory(path: &Path) -> Result<(), String> {
        save_text(MUSIC_DIRECTORY_SETTING_FILE, &path.to_string_lossy())
    }

    pub fn save_opening_files_behavior(behavior: OpeningFilesBehavior) -> Result<(), String> {
        save_text(OPENING_BEHAVIOR_SETTING_FILE, behavior.setting_value())
    }

    pub fn save_read_cue_sheets_in_folders(enabled: bool) -> Result<(), String> {
        save_text(
            READ_CUE_SETTING_FILE,
            if enabled { "true" } else { "false" },
        )
    }

    pub fn save_read_playlists_in_folders(enabled: bool) -> Result<(), String> {
        save_text(
            READ_PLAYLISTS_SETTING_FILE,
            if enabled { "true" } else { "false" },
        )
    }

    pub fn save_output_volume(volume: f64) -> Result<(), String> {
        save_text(
            OUTPUT_VOLUME_SETTING_FILE,
            &volume.clamp(0.0, 1.0).to_string(),
        )
    }

    pub fn save_output_device(device: Option<&OutputDevicePreference>) -> Result<(), String> {
        save_text(
            OUTPUT_DEVICE_SETTING_FILE,
            &device
                .map(OutputDevicePreference::serialize)
                .transpose()?
                .unwrap_or_default(),
        )
    }

    pub fn save_playlist_column_layout(layout: &str) -> Result<(), String> {
        if !validate_playlist_column_layout(layout) {
            return Err("Playlist column layout is invalid".to_owned());
        }
        save_text(PLAYLIST_COLUMN_LAYOUT_SETTING_FILE, layout)
    }

    pub fn save_equalizer(settings: &EqualizerSettings) -> Result<(), String> {
        save_text(EQUALIZER_SETTING_FILE, &settings.serialize()?)
    }
}

fn validate_legacy_playlist_column_widths(value: &str) -> bool {
    if value.len() > 256 {
        return false;
    }
    let widths = value.split(',').collect::<Vec<_>>();
    widths.len() == 9
        && widths.iter().all(|width| {
            width
                .trim()
                .parse::<f64>()
                .is_ok_and(|width| width.is_finite() && (24.0..=4096.0).contains(&width))
        })
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OutputDevicePreference {
    pub id: String,
    pub name: String,
}

impl OutputDevicePreference {
    fn is_valid(&self) -> bool {
        valid_output_device_text(&self.id, 1_024) && valid_output_device_text(&self.name, 512)
    }

    fn serialize(&self) -> Result<String, String> {
        if !self.is_valid() {
            return Err("The output device selection is invalid".to_owned());
        }
        Ok(serde_json::json!({
            "version": 1,
            "id": self.id,
            "name": self.name,
        })
        .to_string())
    }

    fn parse(value: &str) -> Option<Self> {
        if value.len() > 2_048 {
            return None;
        }
        let value: serde_json::Value = serde_json::from_str(value).ok()?;
        if value.as_object()?.len() != 3 || value.get("version")?.as_u64()? != 1 {
            return None;
        }
        let selection = Self {
            id: value.get("id")?.as_str()?.to_owned(),
            name: value.get("name")?.as_str()?.to_owned(),
        };
        selection.is_valid().then_some(selection)
    }
}

fn valid_output_device_text(value: &str, maximum_length: usize) -> bool {
    !value.is_empty() && value.len() <= maximum_length && !value.contains(['\0', '\r', '\n'])
}

fn validate_playlist_column_layout(value: &str) -> bool {
    if value.len() > 2_048 {
        return false;
    }
    let entries = value.split(';').collect::<Vec<_>>();
    if entries.len() != PLAYLIST_COLUMN_IDS.len() {
        return false;
    }

    let mut seen = Vec::with_capacity(entries.len());
    let mut visible = 0;
    for entry in entries {
        let fields = entry.split(',').collect::<Vec<_>>();
        if fields.len() != 3 {
            return false;
        }
        let identifier = fields[0].trim();
        if !PLAYLIST_COLUMN_IDS.contains(&identifier) || seen.contains(&identifier) {
            return false;
        }
        let valid_width = fields[1]
            .trim()
            .parse::<f64>()
            .is_ok_and(|width| width.is_finite() && (20.0..=4096.0).contains(&width));
        if !valid_width || !matches!(fields[2].trim(), "0" | "1") {
            return false;
        }
        visible += usize::from(fields[2].trim() == "1");
        seen.push(identifier);
    }
    visible > 0
}

fn save_text(file_name: &str, value: &str) -> Result<(), String> {
    let path = setting_path(file_name)
        .ok_or_else(|| "The platform configuration directory is unavailable".to_owned())?;
    let parent = path
        .parent()
        .ok_or_else(|| "The Kog configuration directory is unavailable".to_owned())?;
    std::fs::create_dir_all(parent)
        .map_err(|error| format!("creating {}: {error}", parent.display()))?;
    std::fs::write(&path, value).map_err(|error| format!("writing {}: {error}", path.display()))
}

fn load_text(file_name: &str) -> Option<String> {
    let value = std::fs::read_to_string(setting_path(file_name)?).ok()?;
    let value = value.trim();
    (!value.is_empty()).then(|| value.to_owned())
}

fn load_path(file_name: &str) -> Option<PathBuf> {
    load_text(file_name).map(PathBuf::from)
}

fn load_bool(file_name: &str) -> Option<bool> {
    match load_text(file_name)?.to_ascii_lowercase().as_str() {
        "true" | "1" | "yes" | "on" => Some(true),
        "false" | "0" | "no" | "off" => Some(false),
        _ => None,
    }
}

fn load_persisted_soundfont() -> Option<PathBuf> {
    let setting_path = setting_path(SOUNDFONT_SETTING_FILE)?;
    let value = std::fs::read_to_string(setting_path).ok()?;
    let value = value.trim();
    (!value.is_empty()).then(|| PathBuf::from(value))
}

fn load_persisted_midi_engine() -> Option<MidiEngine> {
    let setting_path = setting_path(MIDI_ENGINE_SETTING_FILE)?;
    let value = std::fs::read_to_string(setting_path).ok()?;
    MidiEngine::from_setting(&value)
}

fn load_persisted_sc55_rom_path() -> Option<PathBuf> {
    let setting_path = setting_path(SC55_ROM_SETTING_FILE)?;
    let value = std::fs::read_to_string(setting_path).ok()?;
    let value = value.trim();
    (!value.is_empty()).then(|| PathBuf::from(value))
}

fn setting_path(file_name: &str) -> Option<PathBuf> {
    ProjectDirs::from("org", "Kog", "Kog")
        .map(|directories| directories.config_dir().join(file_name))
}

fn discover_system_soundfont() -> Option<PathBuf> {
    let mut candidates = vec![
        PathBuf::from("/usr/share/sounds/sf2/default-GM.sf2"),
        PathBuf::from("/usr/share/sounds/sf2/TimGM6mb.sf2"),
        PathBuf::from("/usr/share/soundfonts/default.sf2"),
        PathBuf::from("/run/current-system/sw/share/soundfonts/default.sf2"),
        PathBuf::from("/Library/Audio/Sounds/Banks/FluidR3_GM_GS.sf2"),
    ];
    if let Some(home) = std::env::var_os("HOME").map(PathBuf::from) {
        candidates.push(
            home.join("Library")
                .join("Audio")
                .join("Sounds")
                .join("Banks")
                .join("FluidR3_GM_GS.sf2"),
        );
    }
    candidates.into_iter().find(|path| path.is_file())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn midi_engine_settings_accept_all_supported_aliases() {
        assert_eq!(
            MidiEngine::from_setting("rustysynth"),
            Some(MidiEngine::RustySynth)
        );
        assert_eq!(
            MidiEngine::from_setting("opl3w"),
            Some(MidiEngine::Opl3Windows)
        );
        assert_eq!(MidiEngine::from_setting("SC-55"), Some(MidiEngine::Sc55));
        assert_eq!(MidiEngine::from_setting("MT-32"), Some(MidiEngine::Mt32));
        assert_eq!(MidiEngine::from_setting("cm32l"), Some(MidiEngine::Mt32));
        assert_eq!(MidiEngine::Mt32.setting_value(), "munt-mt32");
    }

    #[test]
    fn opening_behavior_matches_cog_and_migrates_kog_aliases() {
        assert_eq!(
            OpeningFilesBehavior::default(),
            OpeningFilesBehavior::EnqueueAndPlay
        );
        assert_eq!(
            OpeningFilesBehavior::from_setting("clearAndPlay"),
            Some(OpeningFilesBehavior::ClearAndPlay)
        );
        assert_eq!(
            OpeningFilesBehavior::from_setting("add"),
            Some(OpeningFilesBehavior::Enqueue)
        );
        assert_eq!(
            OpeningFilesBehavior::from_setting("replace"),
            Some(OpeningFilesBehavior::ClearAndPlay)
        );
        assert!(OpeningFilesBehavior::EnqueueAndPlay.starts_playback());
        assert!(!OpeningFilesBehavior::Enqueue.clears_playlist());
    }

    #[test]
    fn output_device_selection_roundtrips_and_rejects_malformed_state() {
        let selection = OutputDevicePreference {
            id: "Alsa:default".to_owned(),
            name: "Built-in Audio Analog Stereo".to_owned(),
        };
        assert_eq!(
            OutputDevicePreference::parse(&selection.serialize().unwrap()),
            Some(selection)
        );
        assert!(
            OutputDevicePreference::parse(r#"{"version":1,"id":"","name":"Output"}"#).is_none()
        );
        assert!(
            OutputDevicePreference::parse(r#"{"version":2,"id":"id","name":"Output"}"#).is_none()
        );
        assert!(!valid_output_device_text("Line Out\nInjected", 512));
    }

    #[test]
    fn legacy_playlist_column_widths_require_nine_bounded_finite_values() {
        assert!(validate_legacy_playlist_column_widths(
            "54,78,189.5,212,210.5,70,58,121,54"
        ));
        assert!(!validate_legacy_playlist_column_widths("54,78,189"));
        assert!(!validate_legacy_playlist_column_widths(
            "54,78,NaN,212,210.5,70,58,121,54"
        ));
        assert!(!validate_legacy_playlist_column_widths(
            "12,78,189.5,212,210.5,70,58,121,54"
        ));
    }

    #[test]
    fn playlist_column_layout_requires_every_unique_column_and_one_visible_column() {
        let valid = "index,54,1;status,20,1;rating,78,1;title,220,1;albumartist,150,0;artist,190,1;composer,151,0;album,220,1;length,70,1;date,58,1;genre,120,1;track,54,1;playcount,71,0;path,64,0;filename,64,0;codec,64,0;samplerate,64,0;bitspersample,64,0;bitrate,64,0";
        assert!(validate_playlist_column_layout(valid));
        assert!(!validate_playlist_column_layout(
            &valid.replace("bitrate,64,0", "title,64,0")
        ));
        assert!(!validate_playlist_column_layout(&valid.replace(",1", ",0")));
        assert!(!validate_playlist_column_layout(
            &valid.replace("genre,120,1", "genre,NaN,1")
        ));
    }

    #[test]
    fn equalizer_setting_parser_rejects_partial_or_out_of_range_state() {
        let mut gains_db = [0.0; 31];
        gains_db[17] = 7.5;
        let settings = EqualizerSettings {
            enabled: true,
            gains_db,
            ..EqualizerSettings::default()
        };
        assert_eq!(
            EqualizerSettings::parse(&settings.serialize().unwrap()),
            Some(settings)
        );
        assert!(EqualizerSettings::parse("version=1\nenabled=true").is_none());
        let invalid = EqualizerSettings {
            preamp_db: 21.0,
            ..EqualizerSettings::default()
        };
        assert!(invalid.serialize().is_err());
    }
}
