use std::path::{Path, PathBuf};

use directories::ProjectDirs;

const SOUNDFONT_SETTING_FILE: &str = "soundfont-path";
const MIDI_ENGINE_SETTING_FILE: &str = "midi-engine";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MidiEngine {
    #[default]
    RustySynth,
    Opl3Windows,
}

impl MidiEngine {
    pub const fn setting_value(self) -> &'static str {
        match self {
            Self::RustySynth => "rustysynth-sf2",
            Self::Opl3Windows => "opl3windows",
        }
    }

    pub fn from_setting(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "rustysynth-sf2" | "rustysynth" | "soundfont" => Some(Self::RustySynth),
            "opl3windows" | "opl3w" => Some(Self::Opl3Windows),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct AppSettings {
    pub soundfont_path: Option<PathBuf>,
    pub midi_engine: MidiEngine,
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
        Self {
            soundfont_path,
            midi_engine,
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
