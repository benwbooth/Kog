#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
        include!("cxx-qt-lib/qurl.h");
        type QUrl = cxx_qt_lib::QUrl;
    }

    unsafe extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qproperty(i32, playlist_count)]
        #[qproperty(i32, playlist_revision)]
        #[qproperty(i32, current_index)]
        #[qproperty(QString, playback_state)]
        #[qproperty(QString, status)]
        #[qproperty(QString, now_title)]
        #[qproperty(QString, now_artist)]
        #[qproperty(QString, current_album)]
        #[qproperty(QString, current_genre)]
        #[qproperty(QString, current_lyrics)]
        #[qproperty(QString, current_file)]
        #[qproperty(QString, current_codec)]
        #[qproperty(QString, current_year)]
        #[qproperty(QString, current_track_number)]
        #[qproperty(QString, current_sample_rate)]
        #[qproperty(QString, current_channels)]
        #[qproperty(QString, current_bitrate)]
        #[qproperty(QString, current_bits_per_sample)]
        #[qproperty(f64, position_seconds)]
        #[qproperty(f64, duration_seconds)]
        #[qproperty(f64, volume)]
        #[qproperty(bool, shuffle_enabled)]
        #[qproperty(bool, repeat_enabled)]
        #[qproperty(QString, total_duration)]
        #[qproperty(QString, directory_path)]
        #[qproperty(QString, soundfont_path)]
        #[qproperty(QString, sc55_rom_path)]
        #[qproperty(QString, mt32_rom_path)]
        #[qproperty(QString, midi_engine)]
        #[qproperty(QString, midi_status)]
        #[qproperty(QString, opening_files_behavior)]
        #[qproperty(bool, read_cue_sheets_in_folders)]
        #[qproperty(bool, read_playlists_in_folders)]
        type AppController = super::AppControllerRust;

        #[qinvokable]
        fn add_file(self: Pin<&mut AppController>, url: QUrl);
        #[qinvokable]
        fn open_audio_files(self: Pin<&mut AppController>);
        #[qinvokable]
        fn choose_music_folder(self: Pin<&mut AppController>);
        #[qinvokable]
        fn remove_track(self: Pin<&mut AppController>, index: i32);
        #[qinvokable]
        fn clear_playlist(self: Pin<&mut AppController>);
        #[qinvokable]
        fn filter_playlist(self: Pin<&mut AppController>, query: QString);
        #[qinvokable]
        fn play_index(self: Pin<&mut AppController>, index: i32);
        #[qinvokable]
        fn play_pause(self: Pin<&mut AppController>);
        #[qinvokable]
        fn stop(self: Pin<&mut AppController>);
        #[qinvokable]
        fn previous(self: Pin<&mut AppController>);
        #[qinvokable]
        fn next(self: Pin<&mut AppController>);
        #[qinvokable]
        fn seek(self: Pin<&mut AppController>, seconds: f64);
        #[qinvokable]
        fn set_volume_level(self: Pin<&mut AppController>, volume: f64);
        #[qinvokable]
        fn set_shuffle_mode(self: Pin<&mut AppController>, enabled: bool);
        #[qinvokable]
        fn set_repeat_mode(self: Pin<&mut AppController>, enabled: bool);
        #[qinvokable]
        fn poll_playback(self: Pin<&mut AppController>);

        #[qinvokable]
        fn track_number_at(self: &AppController, index: i32) -> QString;
        #[qinvokable]
        fn track_metadata_number_at(self: &AppController, index: i32) -> QString;
        #[qinvokable]
        fn track_status_at(self: &AppController, index: i32) -> QString;
        #[qinvokable]
        fn track_rating_at(self: &AppController, index: i32) -> QString;
        #[qinvokable]
        fn track_title_at(self: &AppController, index: i32) -> QString;
        #[qinvokable]
        fn track_artist_at(self: &AppController, index: i32) -> QString;
        #[qinvokable]
        fn track_album_at(self: &AppController, index: i32) -> QString;
        #[qinvokable]
        fn track_length_at(self: &AppController, index: i32) -> QString;
        #[qinvokable]
        fn track_year_at(self: &AppController, index: i32) -> QString;
        #[qinvokable]
        fn track_genre_at(self: &AppController, index: i32) -> QString;

        #[qinvokable]
        fn parent_directory(self: Pin<&mut AppController>);
        #[qinvokable]
        fn choose_directory(self: Pin<&mut AppController>, url: QUrl);
        #[qinvokable]
        fn set_soundfont(self: Pin<&mut AppController>, url: QUrl);
        #[qinvokable]
        fn choose_soundfont_file(self: Pin<&mut AppController>);
        #[qinvokable]
        fn clear_soundfont(self: Pin<&mut AppController>);
        #[qinvokable]
        fn set_sc55_rom_directory(self: Pin<&mut AppController>, url: QUrl);
        #[qinvokable]
        fn choose_sc55_rom_folder(self: Pin<&mut AppController>);
        #[qinvokable]
        fn clear_sc55_rom_directory(self: Pin<&mut AppController>);
        #[qinvokable]
        fn set_mt32_rom_directory(self: Pin<&mut AppController>, url: QUrl);
        #[qinvokable]
        fn choose_mt32_rom_folder(self: Pin<&mut AppController>);
        #[qinvokable]
        fn clear_mt32_rom_directory(self: Pin<&mut AppController>);
        #[qinvokable]
        fn select_midi_engine(self: Pin<&mut AppController>, engine: QString);
        #[qinvokable]
        fn select_opening_files_behavior(self: Pin<&mut AppController>, behavior: QString);
        #[qinvokable]
        fn set_folder_cue_mode(self: Pin<&mut AppController>, enabled: bool);
        #[qinvokable]
        fn set_folder_playlist_mode(self: Pin<&mut AppController>, enabled: bool);
    }
}

use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::time::Duration;

use cxx_qt::CxxQtType;
use cxx_qt_lib::{QString, QUrl};

use crate::decoder::{DecoderRegistry, DecoderSettings, validate_soundfont};
use crate::playback::{PlaybackEngine, PlaybackState};
use crate::settings::{AppSettings, MidiEngine, OpeningFilesBehavior};
use crate::track::{Track, canonical_path};

#[derive(Debug, Default)]
struct AddPathResult {
    added: usize,
    warning: Option<String>,
}

impl AddPathResult {
    fn push_warning(&mut self, warning: impl AsRef<str>) {
        let warning = warning.as_ref();
        match &mut self.warning {
            Some(existing) => {
                existing.push_str("; ");
                existing.push_str(warning);
            }
            None => self.warning = Some(warning.to_owned()),
        }
    }
}

fn add_path_status(result: &AddPathResult) -> String {
    let added = match result.added {
        0 => "No tracks added".to_owned(),
        1 => "Added to playlist".to_owned(),
        count => format!("Added {count} tracks to playlist"),
    };
    match result.warning.as_deref() {
        Some(warning) => format!("{added} — {warning}"),
        None => added,
    }
}

fn total_duration_label(duration: Duration) -> String {
    let total_seconds = duration.as_secs();
    let hours = total_seconds / 3_600;
    let minutes = (total_seconds % 3_600) / 60;
    let seconds = total_seconds % 60;
    let quantity =
        |value: u64, unit: &str| format!("{value} {unit}{}", if value == 1 { "" } else { "s" });

    let mut parts = Vec::with_capacity(3);
    if hours > 0 {
        parts.push(quantity(hours, "hour"));
    }
    if minutes > 0 || hours > 0 {
        parts.push(quantity(minutes, "minute"));
    }
    parts.push(quantity(seconds, "second"));
    format!("Total duration: {}", parts.join(" "))
}

fn shuffled_indices(length: usize, seed: &mut u64) -> Vec<usize> {
    let mut indices: Vec<_> = (0..length).collect();
    for upper in (1..length).rev() {
        // xorshift64* gives the UI shuffle mode a small, dependency-free PRNG.
        // A zero state is avoided because it is a fixed point for xorshift.
        if *seed == 0 {
            *seed = 0x9e37_79b9_7f4a_7c15;
        }
        *seed ^= *seed >> 12;
        *seed ^= *seed << 25;
        *seed ^= *seed >> 27;
        let random = seed.wrapping_mul(0x2545_f491_4f6c_dd1d);
        let selected = (random as usize) % (upper + 1);
        indices.swap(upper, selected);
    }
    indices
}

pub struct AppControllerRust {
    playlist_count: i32,
    playlist_revision: i32,
    current_index: i32,
    playback_state: QString,
    status: QString,
    now_title: QString,
    now_artist: QString,
    current_album: QString,
    current_genre: QString,
    current_lyrics: QString,
    current_file: QString,
    current_codec: QString,
    current_year: QString,
    current_track_number: QString,
    current_sample_rate: QString,
    current_channels: QString,
    current_bitrate: QString,
    current_bits_per_sample: QString,
    position_seconds: f64,
    duration_seconds: f64,
    volume: f64,
    shuffle_enabled: bool,
    repeat_enabled: bool,
    total_duration: QString,
    directory_path: QString,
    soundfont_path: QString,
    sc55_rom_path: QString,
    mt32_rom_path: QString,
    midi_engine: QString,
    midi_status: QString,
    opening_files_behavior: QString,
    read_cue_sheets_in_folders: bool,
    read_playlists_in_folders: bool,
    tracks: Vec<Track>,
    visible_indices: Vec<usize>,
    shuffle_order: Vec<usize>,
    shuffle_seed: u64,
    filter: String,
    directory: PathBuf,
    decoder_settings: DecoderSettings,
    decoders: DecoderRegistry,
    playback: PlaybackEngine,
}

impl Default for AppControllerRust {
    fn default() -> Self {
        let app_settings = AppSettings::load();
        let directory = app_settings
            .music_directory
            .clone()
            .unwrap_or_else(default_music_directory);
        let decoder_settings = DecoderSettings::new(
            app_settings.soundfont_path.clone(),
            app_settings.midi_engine,
        )
        .with_sc55_rom_path(app_settings.sc55_rom_path.clone())
        .with_mt32_rom_path(app_settings.mt32_rom_path.clone());
        let soundfont_path = app_settings
            .soundfont_path
            .as_deref()
            .map(|path| qstring(path.to_string_lossy()))
            .unwrap_or_default();
        let sc55_rom_path = app_settings
            .sc55_rom_path
            .as_deref()
            .map(|path| qstring(path.to_string_lossy()))
            .unwrap_or_default();
        let mt32_rom_path = app_settings
            .mt32_rom_path
            .as_deref()
            .map(|path| qstring(path.to_string_lossy()))
            .unwrap_or_default();
        let midi_engine = qstring(app_settings.midi_engine.setting_value());
        let midi_status = qstring(midi_status(
            app_settings.midi_engine,
            app_settings.soundfont_path.as_deref(),
            app_settings.sc55_rom_path.as_deref(),
            app_settings.mt32_rom_path.as_deref(),
        ));
        let decoders = DecoderRegistry::new(decoder_settings.clone());
        let mut playback = PlaybackEngine::new(DecoderRegistry::new(decoder_settings.clone()));
        playback.set_volume(app_settings.output_volume as f32);
        let mut controller = Self {
            playlist_count: 0,
            playlist_revision: 0,
            current_index: -1,
            playback_state: qstring(PlaybackState::Stopped.as_str()),
            status: qstring("Drop audio files here or use the Kog menu to add files"),
            now_title: qstring("Not Playing"),
            now_artist: QString::default(),
            current_album: QString::default(),
            current_genre: QString::default(),
            current_lyrics: QString::default(),
            current_file: QString::default(),
            current_codec: QString::default(),
            current_year: QString::default(),
            current_track_number: QString::default(),
            current_sample_rate: QString::default(),
            current_channels: QString::default(),
            current_bitrate: QString::default(),
            current_bits_per_sample: QString::default(),
            position_seconds: 0.0,
            duration_seconds: 0.0,
            volume: app_settings.output_volume,
            shuffle_enabled: false,
            repeat_enabled: false,
            total_duration: qstring("Total duration: 0 seconds"),
            directory_path: qstring(directory.to_string_lossy()),
            soundfont_path,
            sc55_rom_path,
            mt32_rom_path,
            midi_engine,
            midi_status,
            opening_files_behavior: qstring(app_settings.opening_files_behavior.setting_value()),
            read_cue_sheets_in_folders: app_settings.read_cue_sheets_in_folders,
            read_playlists_in_folders: app_settings.read_playlists_in_folders,
            tracks: Vec::new(),
            visible_indices: Vec::new(),
            shuffle_order: Vec::new(),
            shuffle_seed: 0x4b6f_672d_7368_7566 ^ u64::from(std::process::id()),
            filter: String::new(),
            directory,
            decoder_settings,
            decoders,
            playback,
        };

        if let Some(paths) = std::env::var_os("KOG_OPEN_FILES") {
            let mut open_result = AddPathResult::default();
            for path in std::env::split_paths(&paths) {
                match controller.add_path(path) {
                    Ok(result) => {
                        open_result.added += result.added;
                        if let Some(warning) = result.warning {
                            open_result.push_warning(warning);
                        }
                    }
                    Err(error) => open_result.push_warning(error),
                }
            }
            controller.rebuild_visible_indices();
            controller.refresh_total_duration_value();
            if open_result.added > 0 || open_result.warning.is_some() {
                controller.status = qstring(add_path_status(&open_result));
            }
        }
        controller
    }
}

fn qstring(value: impl AsRef<str>) -> QString {
    QString::from(value.as_ref())
}

fn saturating_i32(value: usize) -> i32 {
    i32::try_from(value).unwrap_or(i32::MAX)
}

fn default_music_directory() -> PathBuf {
    let home = std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    let music = home.join("Music");
    if music.is_dir() { music } else { home }
}

fn midi_status(
    engine: MidiEngine,
    soundfont_path: Option<&Path>,
    sc55_rom_path: Option<&Path>,
    mt32_rom_path: Option<&Path>,
) -> String {
    match engine {
        MidiEngine::Opl3Windows => {
            "Ready to render MIDI with Cog's OPL3Windows / Nuked OPL3 engine".to_owned()
        }
        MidiEngine::RustySynth => match soundfont_path {
            Some(path) if path.is_file() => {
                format!("Ready to render MIDI with {}", path.display())
            }
            Some(path) => format!("Selected SoundFont is unavailable: {}", path.display()),
            None => "Choose an SF2 SoundFont to enable MIDI playback".to_owned(),
        },
        MidiEngine::Sc55 => match sc55_rom_path {
            Some(path) if path.is_dir() => format!(
                "Ready to detect a supported Roland ROM set in {}",
                path.display()
            ),
            Some(path) => format!(
                "Selected SC-55 ROM directory is unavailable: {}",
                path.display()
            ),
            None => "Choose a directory containing your own supported Roland SC-55 ROMs".to_owned(),
        },
        MidiEngine::Mt32 => match mt32_rom_path {
            Some(path) if path.is_dir() => format!(
                "Ready to detect a supported MT-32 or CM-32L ROM pair in {}",
                path.display()
            ),
            Some(path) => format!(
                "Selected MT-32 ROM directory is unavailable: {}",
                path.display()
            ),
            None => "Choose a directory containing your own MT-32 or CM-32L control and PCM ROMs"
                .to_owned(),
        },
    }
}

impl AppControllerRust {
    fn rebuild_shuffle_order(&mut self) {
        self.shuffle_order = shuffled_indices(self.tracks.len(), &mut self.shuffle_seed);
        let Ok(current) = usize::try_from(self.current_index) else {
            return;
        };
        if let Some(position) = self
            .shuffle_order
            .iter()
            .position(|index| *index == current)
        {
            self.shuffle_order.swap(0, position);
        }
    }

    fn ensure_shuffle_order(&mut self) {
        let valid = self.shuffle_order.len() == self.tracks.len()
            && self
                .shuffle_order
                .iter()
                .all(|index| *index < self.tracks.len());
        if !valid {
            self.rebuild_shuffle_order();
        }
    }

    fn next_navigation_index(&mut self) -> Option<usize> {
        let count = self.tracks.len();
        if count == 0 {
            return None;
        }
        if !self.shuffle_enabled {
            let current = self.current_index.max(-1);
            let next = usize::try_from(current + 1).unwrap_or_default();
            return (next < count)
                .then_some(next)
                .or_else(|| self.repeat_enabled.then_some(0));
        }

        self.ensure_shuffle_order();
        let current = usize::try_from(self.current_index).ok();
        let position = current.and_then(|current| {
            self.shuffle_order
                .iter()
                .position(|index| *index == current)
        });
        if let Some(next) = position.and_then(|position| self.shuffle_order.get(position + 1)) {
            return Some(*next);
        }
        if position.is_none() {
            return self.shuffle_order.first().copied();
        }
        if !self.repeat_enabled {
            return None;
        }

        let previous = current;
        self.rebuild_shuffle_order();
        if count > 1 && self.shuffle_order.first().copied() == previous {
            self.shuffle_order.swap(0, 1);
        }
        self.shuffle_order.first().copied()
    }

    fn previous_navigation_index(&mut self) -> Option<usize> {
        let count = self.tracks.len();
        if count == 0 {
            return None;
        }
        if !self.shuffle_enabled {
            let current = usize::try_from(self.current_index).unwrap_or_default();
            if current > 0 {
                return Some(current - 1);
            }
            return Some(if self.repeat_enabled { count - 1 } else { 0 });
        }

        self.ensure_shuffle_order();
        let current = usize::try_from(self.current_index).ok();
        let position = current.and_then(|current| {
            self.shuffle_order
                .iter()
                .position(|index| *index == current)
        });
        match position {
            Some(position) if position > 0 => self.shuffle_order.get(position - 1).copied(),
            Some(_) if self.repeat_enabled => self.shuffle_order.last().copied(),
            Some(_) => current,
            None => self.shuffle_order.first().copied(),
        }
    }

    fn add_path(&mut self, path: PathBuf) -> Result<AddPathResult, String> {
        let path = canonical_path(&path)?;
        if !path.is_file() {
            return Err(format!("{} is not a playable file", path.display()));
        }
        let expansion = self.decoders.expand_detailed(path)?;
        let mut result = AddPathResult::default();
        for warning in expansion.warnings {
            result.push_warning(warning);
        }
        for source in expansion.sources {
            if self.tracks.iter().any(|track| track.source == source) {
                continue;
            }
            let track = Track::from_source(source, &self.decoders);
            if let Some(warning) = &track.decoder_warning {
                result.push_warning(warning.clone());
            }
            self.tracks.push(track);
            result.added += 1;
        }
        Ok(result)
    }

    fn add_directory(&mut self, directory: &Path) -> Result<AddPathResult, String> {
        let directory = canonical_path(directory)?;
        if !directory.is_dir() {
            return Err(format!("{} is not a folder", directory.display()));
        }

        let mut result = AddPathResult::default();
        let mut pending = vec![directory];
        while let Some(folder) = pending.pop() {
            let mut entries = std::fs::read_dir(&folder)
                .map_err(|error| format!("reading {}: {error}", folder.display()))?
                .filter_map(Result::ok)
                .collect::<Vec<_>>();
            entries.sort_by_key(std::fs::DirEntry::path);
            for entry in entries.into_iter().rev() {
                let path = entry.path();
                let Ok(file_type) = entry.file_type() else {
                    continue;
                };
                if file_type.is_dir() {
                    pending.push(path);
                    continue;
                }
                if !file_type.is_file() || !self.decoders.accepts_path(&path) {
                    continue;
                }
                let extension = path
                    .extension()
                    .and_then(|value| value.to_str())
                    .unwrap_or_default();
                if extension.eq_ignore_ascii_case("cue") && !self.read_cue_sheets_in_folders {
                    continue;
                }
                if matches!(
                    extension.to_ascii_lowercase().as_str(),
                    "m3u" | "m3u8" | "pls"
                ) && !self.read_playlists_in_folders
                {
                    continue;
                }
                match self.add_path(path) {
                    Ok(added) => {
                        result.added += added.added;
                        if let Some(warning) = added.warning {
                            result.push_warning(warning);
                        }
                    }
                    Err(error) => result.push_warning(error),
                }
            }
        }
        Ok(result)
    }

    fn rebuild_visible_indices(&mut self) {
        self.visible_indices = self
            .tracks
            .iter()
            .enumerate()
            .filter(|(_, track)| track.matches(&self.filter))
            .map(|(index, _)| index)
            .collect();
        self.playlist_count = saturating_i32(self.visible_indices.len());
        self.playlist_revision = self.playlist_revision.wrapping_add(1);
    }

    fn refresh_total_duration_value(&mut self) {
        let duration = self
            .tracks
            .iter()
            .filter_map(|track| track.duration)
            .fold(Duration::ZERO, |total, duration| total + duration);
        self.total_duration = qstring(total_duration_label(duration));
    }
}

fn visible_track(model: &qobject::AppController, index: i32) -> Option<&Track> {
    let visible_index = usize::try_from(index).ok()?;
    let source_index = *model.rust().visible_indices.get(visible_index)?;
    model.rust().tracks.get(source_index)
}

fn visible_source_index(model: &qobject::AppController, index: i32) -> Option<usize> {
    usize::try_from(index)
        .ok()
        .and_then(|index| model.rust().visible_indices.get(index))
        .copied()
}

impl qobject::AppController {
    pub fn open_audio_files(mut self: Pin<&mut Self>) {
        let directory = self.as_ref().rust().directory.clone();
        let Some(paths) = rfd::FileDialog::new()
            .set_title("Add Audio Files")
            .set_directory(directory)
            .pick_files()
        else {
            return;
        };
        let opening_behavior = OpeningFilesBehavior::from_setting(
            &self.as_ref().rust().opening_files_behavior.to_string(),
        )
        .unwrap_or_default();
        if opening_behavior == OpeningFilesBehavior::Replace {
            self.as_mut().clear_playlist();
        }
        for path in paths {
            let url = QUrl::from_local_file(&qstring(path.to_string_lossy()));
            self.as_mut().add_file(url);
        }
    }

    pub fn choose_music_folder(mut self: Pin<&mut Self>) {
        let directory = self.as_ref().rust().directory.clone();
        let Some(path) = rfd::FileDialog::new()
            .set_title("Choose Music Folder")
            .set_directory(directory)
            .pick_folder()
        else {
            return;
        };
        self.as_mut().set_directory(path);
    }

    pub fn add_file(mut self: Pin<&mut Self>, url: QUrl) {
        let Some(local_file) = url.to_local_file() else {
            self.as_mut()
                .set_status(qstring("Only local files can be added"));
            return;
        };
        let path = PathBuf::from(local_file.to_string());
        let result = match if path.is_dir() {
            self.as_mut().rust_mut().add_directory(&path)
        } else {
            self.as_mut().rust_mut().add_path(path)
        } {
            Ok(result) if result.added == 0 => {
                self.as_mut()
                    .set_status(qstring("The file is already in the playlist"));
                return;
            }
            Ok(result) => result,
            Err(error) => {
                self.as_mut().set_status(qstring(error));
                return;
            }
        };
        self.as_mut().rebuild_playlist();
        self.as_mut().set_status(qstring(add_path_status(&result)));
    }

    pub fn remove_track(mut self: Pin<&mut Self>, index: i32) {
        let Some(source_index) = visible_source_index(self.as_ref().get_ref(), index) else {
            return;
        };
        let removed_current = self.as_ref().rust().current_index == saturating_i32(source_index);
        if removed_current {
            self.as_mut().stop();
            self.as_mut().set_current_index(-1);
            self.as_mut().reset_now_playing();
        }
        self.as_mut().rust_mut().tracks.remove(source_index);
        let current_index = self.as_ref().rust().current_index;
        if !removed_current && current_index > saturating_i32(source_index) {
            self.as_mut().set_current_index(current_index - 1);
        }
        self.as_mut().rebuild_playlist();
    }

    pub fn clear_playlist(mut self: Pin<&mut Self>) {
        self.as_mut().stop();
        self.as_mut().rust_mut().tracks.clear();
        self.as_mut().set_current_index(-1);
        self.as_mut().reset_now_playing();
        self.as_mut().rebuild_playlist();
        self.as_mut().set_status(qstring("Playlist cleared"));
    }

    pub fn filter_playlist(mut self: Pin<&mut Self>, query: QString) {
        self.as_mut().rust_mut().filter = query.to_string().trim().to_lowercase();
        self.as_mut().rebuild_playlist();
    }

    pub fn play_index(mut self: Pin<&mut Self>, index: i32) {
        let Some(source_index) = visible_source_index(self.as_ref().get_ref(), index) else {
            return;
        };
        self.as_mut().play_source_index(source_index);
    }

    pub fn play_pause(mut self: Pin<&mut Self>) {
        if self.as_ref().rust().tracks.is_empty() {
            return;
        }
        if self.as_ref().rust().playback.state() == PlaybackState::Stopped {
            let source_index = usize::try_from(self.as_ref().rust().current_index)
                .unwrap_or_default()
                .min(self.as_ref().rust().tracks.len() - 1);
            self.as_mut().play_source_index(source_index);
            return;
        }
        self.as_mut().rust_mut().playback.play_pause();
        self.as_mut().sync_playback_state();
    }

    pub fn stop(mut self: Pin<&mut Self>) {
        self.as_mut().rust_mut().playback.stop();
        self.as_mut().set_position_seconds(0.0);
        self.as_mut().set_status(qstring("Stopped"));
        self.as_mut().sync_playback_state();
    }

    pub fn previous(mut self: Pin<&mut Self>) {
        let target = self.as_mut().rust_mut().previous_navigation_index();
        if let Some(target) = target {
            self.as_mut().play_source_index(target);
        }
    }

    pub fn next(mut self: Pin<&mut Self>) {
        let target = self.as_mut().rust_mut().next_navigation_index();
        if let Some(target) = target {
            self.as_mut().play_source_index(target);
        } else {
            self.as_mut().stop();
        }
    }

    pub fn seek(mut self: Pin<&mut Self>, seconds: f64) {
        let seconds = seconds.clamp(0.0, self.as_ref().rust().duration_seconds.max(0.0));
        match self
            .as_ref()
            .rust()
            .playback
            .seek(Duration::from_secs_f64(seconds))
        {
            Ok(()) => self.as_mut().set_position_seconds(seconds),
            Err(error) => self.as_mut().set_status(qstring(error)),
        }
    }

    pub fn set_volume_level(mut self: Pin<&mut Self>, volume: f64) {
        let volume = volume.clamp(0.0, 1.0);
        if let Err(error) = AppSettings::save_output_volume(volume) {
            self.as_mut().set_status(qstring(error));
            return;
        }
        self.as_mut().rust_mut().playback.set_volume(volume as f32);
        self.as_mut().set_volume(volume);
    }

    pub fn set_shuffle_mode(mut self: Pin<&mut Self>, enabled: bool) {
        if self.as_ref().rust().shuffle_enabled == enabled {
            return;
        }
        self.as_mut().rust_mut().shuffle_enabled = enabled;
        if enabled {
            self.as_mut().rust_mut().rebuild_shuffle_order();
        } else {
            self.as_mut().rust_mut().shuffle_order.clear();
        }
        self.as_mut().set_shuffle_enabled(enabled);
        self.as_mut().set_status(qstring(if enabled {
            "Shuffle enabled"
        } else {
            "Shuffle disabled"
        }));
    }

    pub fn set_repeat_mode(mut self: Pin<&mut Self>, enabled: bool) {
        if self.as_ref().rust().repeat_enabled == enabled {
            return;
        }
        self.as_mut().rust_mut().repeat_enabled = enabled;
        self.as_mut().set_repeat_enabled(enabled);
        self.as_mut().set_status(qstring(if enabled {
            "Repeat playlist enabled"
        } else {
            "Repeat disabled"
        }));
    }

    pub fn poll_playback(mut self: Pin<&mut Self>) {
        if self.as_ref().rust().playback.finished() {
            self.as_mut().next();
            return;
        }
        if self.as_ref().rust().playback.state() == PlaybackState::Stopped {
            return;
        }
        let position = self.as_ref().rust().playback.position().as_secs_f64();
        self.as_mut().set_position_seconds(position);
    }

    pub fn track_number_at(&self, index: i32) -> QString {
        visible_source_index(self, index)
            .map(|index| qstring((index + 1).to_string()))
            .unwrap_or_default()
    }

    pub fn track_metadata_number_at(&self, index: i32) -> QString {
        visible_track(self, index)
            .and_then(|track| track.track_number)
            .map(|number| qstring(number.to_string()))
            .unwrap_or_default()
    }

    pub fn track_status_at(&self, index: i32) -> QString {
        let Some(source_index) = visible_source_index(self, index) else {
            return QString::default();
        };
        if self.rust().current_index == saturating_i32(source_index) {
            match self.rust().playback.state() {
                PlaybackState::Playing => qstring("▶"),
                PlaybackState::Paused => qstring("Ⅱ"),
                PlaybackState::Stopped => QString::default(),
            }
        } else {
            QString::default()
        }
    }

    pub fn track_rating_at(&self, _index: i32) -> QString {
        // Keep CogX's Rating column in the visual model without inventing
        // ratings that are not present in Kog's track metadata yet.
        QString::default()
    }

    pub fn track_title_at(&self, index: i32) -> QString {
        visible_track(self, index)
            .map(|track| qstring(&track.title))
            .unwrap_or_default()
    }

    pub fn track_artist_at(&self, index: i32) -> QString {
        visible_track(self, index)
            .map(|track| qstring(&track.artist))
            .unwrap_or_default()
    }

    pub fn track_album_at(&self, index: i32) -> QString {
        visible_track(self, index)
            .map(|track| qstring(&track.album))
            .unwrap_or_default()
    }

    pub fn track_length_at(&self, index: i32) -> QString {
        visible_track(self, index)
            .map(|track| qstring(track.duration_label()))
            .unwrap_or_default()
    }

    pub fn track_year_at(&self, index: i32) -> QString {
        visible_track(self, index)
            .and_then(|track| track.year)
            .map(|year| qstring(year.to_string()))
            .unwrap_or_default()
    }

    pub fn track_genre_at(&self, index: i32) -> QString {
        visible_track(self, index)
            .map(|track| qstring(&track.genre))
            .unwrap_or_default()
    }

    pub fn parent_directory(mut self: Pin<&mut Self>) {
        let parent = self
            .as_ref()
            .rust()
            .directory
            .parent()
            .map(Path::to_path_buf);
        if let Some(parent) = parent {
            self.as_mut().set_directory(parent);
        }
    }

    pub fn choose_directory(mut self: Pin<&mut Self>, url: QUrl) {
        let Some(path) = url.to_local_file() else {
            return;
        };
        self.as_mut().set_directory(PathBuf::from(path.to_string()));
    }

    pub fn set_soundfont(mut self: Pin<&mut Self>, url: QUrl) {
        let Some(local_file) = url.to_local_file() else {
            self.as_mut()
                .set_midi_status(qstring("Only local SF2 SoundFonts can be selected"));
            return;
        };
        let path = PathBuf::from(local_file.to_string());
        let is_sf2 = path
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| extension.eq_ignore_ascii_case("sf2"));
        if !is_sf2 {
            self.as_mut()
                .set_midi_status(qstring("Kog's current MIDI backend accepts SF2 files"));
            return;
        }
        let path = match std::fs::canonicalize(&path) {
            Ok(path) => path,
            Err(error) => {
                self.as_mut().set_midi_status(qstring(format!(
                    "Opening SoundFont {}: {error}",
                    path.display()
                )));
                return;
            }
        };
        if let Err(error) = validate_soundfont(&path) {
            self.as_mut().set_midi_status(qstring(error));
            return;
        }
        if let Err(error) = AppSettings::save_soundfont_path(Some(&path)) {
            self.as_mut().set_midi_status(qstring(error));
            return;
        }
        self.as_ref()
            .rust()
            .decoder_settings
            .set_soundfont_path(Some(path.clone()));
        self.as_mut()
            .set_soundfont_path(qstring(path.to_string_lossy()));
        let engine = self.as_ref().rust().decoder_settings.midi_engine();
        let sc55_rom_path = self.as_ref().rust().decoder_settings.sc55_rom_path();
        let mt32_rom_path = self.as_ref().rust().decoder_settings.mt32_rom_path();
        self.as_mut().set_midi_status(qstring(midi_status(
            engine,
            Some(&path),
            sc55_rom_path.as_deref(),
            mt32_rom_path.as_deref(),
        )));
        self.as_mut().set_status(qstring("MIDI SoundFont updated"));
    }

    pub fn choose_soundfont_file(mut self: Pin<&mut Self>) {
        let initial_directory = self
            .as_ref()
            .rust()
            .decoder_settings
            .soundfont_path()
            .and_then(|path| path.parent().map(Path::to_path_buf))
            .unwrap_or_else(|| self.as_ref().rust().directory.clone());
        let Some(path) = rfd::FileDialog::new()
            .set_title("Choose an SF2 SoundFont")
            .set_directory(initial_directory)
            .add_filter("SoundFont 2 banks", &["sf2"])
            .pick_file()
        else {
            return;
        };
        let url = QUrl::from_local_file(&qstring(path.to_string_lossy()));
        self.as_mut().set_soundfont(url);
    }

    pub fn clear_soundfont(mut self: Pin<&mut Self>) {
        if let Err(error) = AppSettings::save_soundfont_path(None) {
            self.as_mut().set_midi_status(qstring(error));
            return;
        }
        self.as_ref()
            .rust()
            .decoder_settings
            .set_soundfont_path(None);
        self.as_mut().set_soundfont_path(QString::default());
        let engine = self.as_ref().rust().decoder_settings.midi_engine();
        let sc55_rom_path = self.as_ref().rust().decoder_settings.sc55_rom_path();
        let mt32_rom_path = self.as_ref().rust().decoder_settings.mt32_rom_path();
        self.as_mut().set_midi_status(qstring(midi_status(
            engine,
            None,
            sc55_rom_path.as_deref(),
            mt32_rom_path.as_deref(),
        )));
        self.as_mut().set_status(qstring("MIDI SoundFont cleared"));
    }

    pub fn set_sc55_rom_directory(mut self: Pin<&mut Self>, url: QUrl) {
        let Some(local_directory) = url.to_local_file() else {
            self.as_mut()
                .set_midi_status(qstring("Only a local SC-55 ROM directory can be selected"));
            return;
        };
        let path = PathBuf::from(local_directory.to_string());
        let path = match std::fs::canonicalize(&path) {
            Ok(path) if path.is_dir() => path,
            Ok(path) => {
                self.as_mut().set_midi_status(qstring(format!(
                    "SC-55 ROM path is not a directory: {}",
                    path.display()
                )));
                return;
            }
            Err(error) => {
                self.as_mut().set_midi_status(qstring(format!(
                    "Opening SC-55 ROM directory {}: {error}",
                    path.display()
                )));
                return;
            }
        };
        if let Err(error) = AppSettings::save_sc55_rom_path(Some(&path)) {
            self.as_mut().set_midi_status(qstring(error));
            return;
        }
        self.as_ref()
            .rust()
            .decoder_settings
            .set_sc55_rom_path(Some(path.clone()));
        self.as_mut()
            .set_sc55_rom_path(qstring(path.to_string_lossy()));
        let engine = self.as_ref().rust().decoder_settings.midi_engine();
        let soundfont_path = self.as_ref().rust().decoder_settings.soundfont_path();
        let mt32_rom_path = self.as_ref().rust().decoder_settings.mt32_rom_path();
        self.as_mut().set_midi_status(qstring(midi_status(
            engine,
            soundfont_path.as_deref(),
            Some(&path),
            mt32_rom_path.as_deref(),
        )));
        self.as_mut()
            .set_status(qstring("SC-55 ROM directory updated"));
    }

    pub fn choose_sc55_rom_folder(mut self: Pin<&mut Self>) {
        let initial_directory = self
            .as_ref()
            .rust()
            .decoder_settings
            .sc55_rom_path()
            .unwrap_or_else(|| self.as_ref().rust().directory.clone());
        let Some(path) = rfd::FileDialog::new()
            .set_title("Choose the folder containing your Roland ROMs")
            .set_directory(initial_directory)
            .pick_folder()
        else {
            return;
        };
        let url = QUrl::from_local_file(&qstring(path.to_string_lossy()));
        self.as_mut().set_sc55_rom_directory(url);
    }

    pub fn clear_sc55_rom_directory(mut self: Pin<&mut Self>) {
        if let Err(error) = AppSettings::save_sc55_rom_path(None) {
            self.as_mut().set_midi_status(qstring(error));
            return;
        }
        self.as_ref()
            .rust()
            .decoder_settings
            .set_sc55_rom_path(None);
        self.as_mut().set_sc55_rom_path(QString::default());
        let engine = self.as_ref().rust().decoder_settings.midi_engine();
        let soundfont_path = self.as_ref().rust().decoder_settings.soundfont_path();
        let mt32_rom_path = self.as_ref().rust().decoder_settings.mt32_rom_path();
        self.as_mut().set_midi_status(qstring(midi_status(
            engine,
            soundfont_path.as_deref(),
            None,
            mt32_rom_path.as_deref(),
        )));
        self.as_mut()
            .set_status(qstring("SC-55 ROM directory cleared"));
    }

    pub fn set_mt32_rom_directory(mut self: Pin<&mut Self>, url: QUrl) {
        let Some(local_directory) = url.to_local_file() else {
            self.as_mut()
                .set_midi_status(qstring("Only a local MT-32 ROM directory can be selected"));
            return;
        };
        let path = PathBuf::from(local_directory.to_string());
        let path = match std::fs::canonicalize(&path) {
            Ok(path) if path.is_dir() => path,
            Ok(path) => {
                self.as_mut().set_midi_status(qstring(format!(
                    "MT-32 ROM path is not a directory: {}",
                    path.display()
                )));
                return;
            }
            Err(error) => {
                self.as_mut().set_midi_status(qstring(format!(
                    "Opening MT-32 ROM directory {}: {error}",
                    path.display()
                )));
                return;
            }
        };
        if let Err(error) = AppSettings::save_mt32_rom_path(Some(&path)) {
            self.as_mut().set_midi_status(qstring(error));
            return;
        }
        self.as_ref()
            .rust()
            .decoder_settings
            .set_mt32_rom_path(Some(path.clone()));
        self.as_mut()
            .set_mt32_rom_path(qstring(path.to_string_lossy()));
        let engine = self.as_ref().rust().decoder_settings.midi_engine();
        let soundfont_path = self.as_ref().rust().decoder_settings.soundfont_path();
        let sc55_rom_path = self.as_ref().rust().decoder_settings.sc55_rom_path();
        self.as_mut().set_midi_status(qstring(midi_status(
            engine,
            soundfont_path.as_deref(),
            sc55_rom_path.as_deref(),
            Some(&path),
        )));
        self.as_mut()
            .set_status(qstring("MT-32 ROM directory updated"));
    }

    pub fn choose_mt32_rom_folder(mut self: Pin<&mut Self>) {
        let initial_directory = self
            .as_ref()
            .rust()
            .decoder_settings
            .mt32_rom_path()
            .unwrap_or_else(|| self.as_ref().rust().directory.clone());
        let Some(path) = rfd::FileDialog::new()
            .set_title("Choose the folder containing your MT-32 or CM-32L ROMs")
            .set_directory(initial_directory)
            .pick_folder()
        else {
            return;
        };
        let url = QUrl::from_local_file(&qstring(path.to_string_lossy()));
        self.as_mut().set_mt32_rom_directory(url);
    }

    pub fn clear_mt32_rom_directory(mut self: Pin<&mut Self>) {
        if let Err(error) = AppSettings::save_mt32_rom_path(None) {
            self.as_mut().set_midi_status(qstring(error));
            return;
        }
        self.as_ref()
            .rust()
            .decoder_settings
            .set_mt32_rom_path(None);
        self.as_mut().set_mt32_rom_path(QString::default());
        let engine = self.as_ref().rust().decoder_settings.midi_engine();
        let soundfont_path = self.as_ref().rust().decoder_settings.soundfont_path();
        let sc55_rom_path = self.as_ref().rust().decoder_settings.sc55_rom_path();
        self.as_mut().set_midi_status(qstring(midi_status(
            engine,
            soundfont_path.as_deref(),
            sc55_rom_path.as_deref(),
            None,
        )));
        self.as_mut()
            .set_status(qstring("MT-32 ROM directory cleared"));
    }

    pub fn select_midi_engine(mut self: Pin<&mut Self>, engine: QString) {
        let value = engine.to_string();
        let Some(engine) = MidiEngine::from_setting(&value) else {
            self.as_mut()
                .set_midi_status(qstring(format!("Unknown MIDI engine: {value}")));
            return;
        };
        if let Err(error) = AppSettings::save_midi_engine(engine) {
            self.as_mut().set_midi_status(qstring(error));
            return;
        }
        self.as_ref()
            .rust()
            .decoder_settings
            .set_midi_engine(engine);
        let soundfont_path = self.as_ref().rust().decoder_settings.soundfont_path();
        let sc55_rom_path = self.as_ref().rust().decoder_settings.sc55_rom_path();
        let mt32_rom_path = self.as_ref().rust().decoder_settings.mt32_rom_path();
        self.as_mut()
            .set_midi_engine(qstring(engine.setting_value()));
        self.as_mut().set_midi_status(qstring(midi_status(
            engine,
            soundfont_path.as_deref(),
            sc55_rom_path.as_deref(),
            mt32_rom_path.as_deref(),
        )));
        self.as_mut().set_status(qstring(match engine {
            MidiEngine::RustySynth => "MIDI engine changed to RustySynth SoundFont",
            MidiEngine::Opl3Windows => "MIDI engine changed to OPL3Windows",
            MidiEngine::Sc55 => "MIDI engine changed to Nuked SC-55",
            MidiEngine::Mt32 => "MIDI engine changed to Munt MT-32/CM-32L",
        }));
    }

    pub fn select_opening_files_behavior(mut self: Pin<&mut Self>, behavior: QString) {
        let value = behavior.to_string();
        let Some(behavior) = OpeningFilesBehavior::from_setting(&value) else {
            self.as_mut()
                .set_status(qstring(format!("Unknown file opening behavior: {value}")));
            return;
        };
        if let Err(error) = AppSettings::save_opening_files_behavior(behavior) {
            self.as_mut().set_status(qstring(error));
            return;
        }
        self.as_mut()
            .set_opening_files_behavior(qstring(behavior.setting_value()));
        self.as_mut().set_status(qstring(match behavior {
            OpeningFilesBehavior::Add => "New files will be added to the playlist",
            OpeningFilesBehavior::Replace => "New files will replace the playlist",
        }));
    }

    pub fn set_folder_cue_mode(mut self: Pin<&mut Self>, enabled: bool) {
        if let Err(error) = AppSettings::save_read_cue_sheets_in_folders(enabled) {
            self.as_mut().set_status(qstring(error));
            return;
        }
        self.as_mut().set_read_cue_sheets_in_folders(enabled);
    }

    pub fn set_folder_playlist_mode(mut self: Pin<&mut Self>, enabled: bool) {
        if let Err(error) = AppSettings::save_read_playlists_in_folders(enabled) {
            self.as_mut().set_status(qstring(error));
            return;
        }
        self.as_mut().set_read_playlists_in_folders(enabled);
    }

    fn rebuild_playlist(mut self: Pin<&mut Self>) {
        self.as_mut().rust_mut().rebuild_visible_indices();
        self.as_mut().rust_mut().refresh_total_duration_value();
        let count = self.as_ref().rust().playlist_count;
        let revision = self.as_ref().rust().playlist_revision;
        let duration = self.as_ref().rust().total_duration.clone();
        self.as_mut().set_playlist_count(count);
        self.as_mut().set_playlist_revision(revision);
        self.as_mut().set_total_duration(duration);
    }

    fn play_source_index(mut self: Pin<&mut Self>, source_index: usize) {
        let Some(source) = self
            .as_ref()
            .get_ref()
            .rust()
            .tracks
            .get(source_index)
            .map(|track| track.source.clone())
        else {
            return;
        };
        match self.as_mut().rust_mut().playback.play_source(&source) {
            Ok(backend) => {
                self.as_mut()
                    .set_current_index(saturating_i32(source_index));
                self.as_mut().populate_now_playing(source_index);
                let capability_summary = backend.capability_summary();
                let status = if capability_summary.is_empty() {
                    format!("Playing with {} ({})", backend.display_name, backend.id)
                } else {
                    format!(
                        "Playing with {} ({}) — {capability_summary}",
                        backend.display_name, backend.id
                    )
                };
                self.as_mut().set_status(qstring(status));
                self.as_mut().sync_playback_state();
                self.as_mut().bump_playlist_revision();
            }
            Err(error) => {
                self.as_mut().set_status(qstring(error));
                self.as_mut().rust_mut().playback.stop();
                self.as_mut().sync_playback_state();
            }
        }
    }

    fn populate_now_playing(mut self: Pin<&mut Self>, source_index: usize) {
        let Some(track) = self
            .as_ref()
            .get_ref()
            .rust()
            .tracks
            .get(source_index)
            .cloned()
        else {
            return;
        };
        let title = qstring(&track.title);
        let artist = qstring(&track.artist);
        let album = qstring(&track.album);
        let genre = qstring(&track.genre);
        let lyrics = qstring(&track.lyrics);
        let file = qstring(track.source.display_label());
        let codec = qstring(&track.codec);
        let year = track
            .year
            .map(|value| value.to_string())
            .unwrap_or_default();
        let track_number = track
            .track_number
            .map(|value| value.to_string())
            .unwrap_or_default();
        let sample_rate = track
            .sample_rate
            .map(|value| format!("{value} Hz"))
            .unwrap_or_default();
        let channels = track
            .channels
            .map(|value| value.to_string())
            .unwrap_or_default();
        let bitrate = track
            .bitrate
            .map(|value| format!("{value} kbps"))
            .unwrap_or_default();
        let bits_per_sample = track
            .bits_per_sample
            .map(|value| value.to_string())
            .unwrap_or_default();
        let duration = track.duration.unwrap_or_default().as_secs_f64();

        self.as_mut().set_now_title(title);
        self.as_mut().set_now_artist(artist);
        self.as_mut().set_current_album(album);
        self.as_mut().set_current_genre(genre);
        self.as_mut().set_current_lyrics(lyrics);
        self.as_mut().set_current_file(file);
        self.as_mut().set_current_codec(codec);
        self.as_mut().set_current_year(qstring(year));
        self.as_mut()
            .set_current_track_number(qstring(track_number));
        self.as_mut().set_current_sample_rate(qstring(sample_rate));
        self.as_mut().set_current_channels(qstring(channels));
        self.as_mut().set_current_bitrate(qstring(bitrate));
        self.as_mut()
            .set_current_bits_per_sample(qstring(bits_per_sample));
        self.as_mut().set_duration_seconds(duration);
        self.as_mut().set_position_seconds(0.0);
    }

    fn reset_now_playing(mut self: Pin<&mut Self>) {
        self.as_mut().set_now_title(qstring("Not Playing"));
        self.as_mut().set_now_artist(QString::default());
        self.as_mut().set_current_album(QString::default());
        self.as_mut().set_current_genre(QString::default());
        self.as_mut().set_current_lyrics(QString::default());
        self.as_mut().set_current_file(QString::default());
        self.as_mut().set_current_codec(QString::default());
        self.as_mut().set_current_year(QString::default());
        self.as_mut().set_current_track_number(QString::default());
        self.as_mut().set_current_sample_rate(QString::default());
        self.as_mut().set_current_channels(QString::default());
        self.as_mut().set_current_bitrate(QString::default());
        self.as_mut()
            .set_current_bits_per_sample(QString::default());
        self.as_mut().set_duration_seconds(0.0);
        self.as_mut().set_position_seconds(0.0);
    }

    fn sync_playback_state(mut self: Pin<&mut Self>) {
        let state = self.as_ref().rust().playback.state();
        self.as_mut().set_playback_state(qstring(state.as_str()));
        self.as_mut().bump_playlist_revision();
    }

    fn bump_playlist_revision(mut self: Pin<&mut Self>) {
        let revision = self.as_ref().rust().playlist_revision.wrapping_add(1);
        self.as_mut().rust_mut().playlist_revision = revision;
        self.as_mut().set_playlist_revision(revision);
    }

    fn set_directory(mut self: Pin<&mut Self>, path: PathBuf) {
        let Ok(path) = canonical_path(&path) else {
            self.as_mut()
                .set_status(qstring("Directory is unavailable"));
            return;
        };
        if !path.is_dir() {
            return;
        }
        if let Err(error) = AppSettings::save_music_directory(&path) {
            self.as_mut().set_status(qstring(error));
            return;
        }
        self.as_mut().rust_mut().directory = path.clone();
        self.as_mut()
            .set_directory_path(qstring(path.to_string_lossy()));
    }
}

#[cfg(test)]
mod tests {
    use super::{AddPathResult, add_path_status};

    #[test]
    fn add_path_status_keeps_every_warning() {
        let mut result = AddPathResult {
            added: 0,
            warning: None,
        };
        result.push_warning("remote entry skipped");
        result.push_warning("decoder metadata unavailable");

        assert_eq!(
            add_path_status(&result),
            "No tracks added — remote entry skipped; decoder metadata unavailable"
        );
    }
}
