use std::path::{Path, PathBuf};

use directories::ProjectDirs;

const SOUNDFONT_SETTING_FILE: &str = "soundfont-path";

#[derive(Clone, Debug, Default)]
pub struct AppSettings {
    pub soundfont_path: Option<PathBuf>,
}

impl AppSettings {
    pub fn load() -> Self {
        let soundfont_path = std::env::var_os("KOG_SOUNDFONT")
            .map(PathBuf::from)
            .or_else(load_persisted_soundfont)
            .or_else(discover_system_soundfont);
        Self { soundfont_path }
    }

    pub fn save_soundfont_path(path: Option<&Path>) -> Result<(), String> {
        let setting_path = soundfont_setting_path()
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
}

fn load_persisted_soundfont() -> Option<PathBuf> {
    let setting_path = soundfont_setting_path()?;
    let value = std::fs::read_to_string(setting_path).ok()?;
    let value = value.trim();
    (!value.is_empty()).then(|| PathBuf::from(value))
}

fn soundfont_setting_path() -> Option<PathBuf> {
    ProjectDirs::from("org", "Kog", "Kog")
        .map(|directories| directories.config_dir().join(SOUNDFONT_SETTING_FILE))
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
