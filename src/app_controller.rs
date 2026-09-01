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
        #[qproperty(QString, total_duration)]
        #[qproperty(QString, directory_path)]
        #[qproperty(QString, soundfont_path)]
        #[qproperty(QString, sc55_rom_path)]
        #[qproperty(QString, midi_engine)]
        #[qproperty(QString, midi_status)]
        type AppController = super::AppControllerRust;

        #[qinvokable]
        fn add_file(self: Pin<&mut AppController>, url: QUrl);
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
        fn poll_playback(self: Pin<&mut AppController>);

        #[qinvokable]
        fn track_number_at(self: &AppController, index: i32) -> QString;
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
        fn clear_soundfont(self: Pin<&mut AppController>);
        #[qinvokable]
        fn set_sc55_rom_directory(self: Pin<&mut AppController>, url: QUrl);
        #[qinvokable]
        fn clear_sc55_rom_directory(self: Pin<&mut AppController>);
        #[qinvokable]
        fn select_midi_engine(self: Pin<&mut AppController>, engine: QString);
    }
}

use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::time::Duration;

use cxx_qt::CxxQtType;
use cxx_qt_lib::{QString, QUrl};

use crate::decoder::{DecoderRegistry, DecoderSettings, validate_soundfont};
use crate::playback::{PlaybackEngine, PlaybackState};
use crate::settings::{AppSettings, MidiEngine};
use crate::track::{Track, canonical_path, duration_label};

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
    total_duration: QString,
    directory_path: QString,
    soundfont_path: QString,
    sc55_rom_path: QString,
    midi_engine: QString,
    midi_status: QString,
    tracks: Vec<Track>,
    visible_indices: Vec<usize>,
    filter: String,
    directory: PathBuf,
    decoder_settings: DecoderSettings,
    decoders: DecoderRegistry,
    playback: PlaybackEngine,
}

impl Default for AppControllerRust {
    fn default() -> Self {
        let directory = default_music_directory();
        let app_settings = AppSettings::load();
        let decoder_settings = DecoderSettings::new(
            app_settings.soundfont_path.clone(),
            app_settings.midi_engine,
        )
        .with_sc55_rom_path(app_settings.sc55_rom_path.clone());
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
        let midi_engine = qstring(app_settings.midi_engine.setting_value());
        let midi_status = qstring(midi_status(
            app_settings.midi_engine,
            app_settings.soundfont_path.as_deref(),
            app_settings.sc55_rom_path.as_deref(),
        ));
        let decoders = DecoderRegistry::new(decoder_settings.clone());
        let playback = PlaybackEngine::new(DecoderRegistry::new(decoder_settings.clone()));
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
            volume: 0.75,
            total_duration: qstring("Total Duration: 0:00"),
            directory_path: qstring(directory.to_string_lossy()),
            soundfont_path,
            sc55_rom_path,
            midi_engine,
            midi_status,
            tracks: Vec::new(),
            visible_indices: Vec::new(),
            filter: String::new(),
            directory,
            decoder_settings,
            decoders,
            playback,
        };

        if let Some(paths) = std::env::var_os("KOG_OPEN_FILES") {
            let mut open_issue = None;
            for path in std::env::split_paths(&paths) {
                match controller.add_path(path) {
                    Ok(result) if result.warning.is_some() => open_issue = result.warning,
                    Ok(_) => {}
                    Err(error) => open_issue = Some(error),
                }
            }
            controller.rebuild_visible_indices();
            controller.refresh_total_duration_value();
            if let Some(issue) = open_issue {
                controller.status = qstring(issue);
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
    }
}

impl AppControllerRust {
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
        self.total_duration = qstring(format!("Total Duration: {}", duration_label(duration)));
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
    pub fn add_file(mut self: Pin<&mut Self>, url: QUrl) {
        let Some(local_file) = url.to_local_file() else {
            self.as_mut()
                .set_status(qstring("Only local files can be added"));
            return;
        };
        let path = PathBuf::from(local_file.to_string());
        if path.is_dir() {
            self.as_mut().set_directory(path);
            return;
        }
        let result = match self.as_mut().rust_mut().add_path(path) {
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
        if self.as_ref().rust().tracks.is_empty() {
            return;
        }
        let current = self.as_ref().rust().current_index.max(0) as usize;
        self.as_mut().play_source_index(current.saturating_sub(1));
    }

    pub fn next(mut self: Pin<&mut Self>) {
        let count = self.as_ref().rust().tracks.len();
        if count == 0 {
            return;
        }
        let current = self.as_ref().rust().current_index.max(-1);
        let next = usize::try_from(current + 1).unwrap_or_default();
        if next < count {
            self.as_mut().play_source_index(next);
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
        self.as_mut().rust_mut().playback.set_volume(volume as f32);
        self.as_mut().set_volume(volume);
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

    pub fn track_rating_at(&self, index: i32) -> QString {
        if visible_track(self, index).is_some() {
            qstring("☆☆☆☆☆")
        } else {
            QString::default()
        }
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
        self.as_mut().set_midi_status(qstring(midi_status(
            engine,
            Some(&path),
            sc55_rom_path.as_deref(),
        )));
        self.as_mut().set_status(qstring("MIDI SoundFont updated"));
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
        self.as_mut()
            .set_midi_status(qstring(midi_status(engine, None, sc55_rom_path.as_deref())));
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
        self.as_mut().set_midi_status(qstring(midi_status(
            engine,
            soundfont_path.as_deref(),
            Some(&path),
        )));
        self.as_mut()
            .set_status(qstring("SC-55 ROM directory updated"));
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
        self.as_mut().set_midi_status(qstring(midi_status(
            engine,
            soundfont_path.as_deref(),
            None,
        )));
        self.as_mut()
            .set_status(qstring("SC-55 ROM directory cleared"));
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
        self.as_mut()
            .set_midi_engine(qstring(engine.setting_value()));
        self.as_mut().set_midi_status(qstring(midi_status(
            engine,
            soundfont_path.as_deref(),
            sc55_rom_path.as_deref(),
        )));
        self.as_mut().set_status(qstring(match engine {
            MidiEngine::RustySynth => "MIDI engine changed to RustySynth SoundFont",
            MidiEngine::Opl3Windows => "MIDI engine changed to OPL3Windows",
            MidiEngine::Sc55 => "MIDI engine changed to Nuked SC-55",
        }));
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
