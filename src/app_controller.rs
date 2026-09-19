#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
        include!("cxx-qt-lib/qurl.h");
        type QUrl = cxx_qt_lib::QUrl;
        include!("cxx-qt-lib/qbytearray.h");
        type QByteArray = cxx_qt_lib::QByteArray;
        include!("kog/kog_cover_art_network.h");
        #[cxx_name = "kogFetchCoverArtUrl"]
        fn fetch_cover_art_url(url: &QString, max_bytes: u32) -> Result<QByteArray>;
    }

    unsafe extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qproperty(i32, playlist_count)]
        #[qproperty(i32, playlist_revision)]
        #[qproperty(i32, playlists_revision)]
        #[qproperty(QString, playlist_sort_column)]
        #[qproperty(bool, playlist_sort_ascending)]
        #[qproperty(QString, playlist_column_layout)]
        #[qproperty(i32, current_index)]
        #[qproperty(QString, playback_state)]
        #[qproperty(i32, mpris_raise_serial)]
        #[qproperty(i32, notification_serial)]
        #[qproperty(f64, audio_level_low)]
        #[qproperty(f64, audio_level_low_mid)]
        #[qproperty(f64, audio_level_mid)]
        #[qproperty(f64, audio_level_high_mid)]
        #[qproperty(f64, audio_level_high)]
        #[qproperty(QString, status)]
        #[qproperty(QString, now_title)]
        #[qproperty(QString, now_artist)]
        #[qproperty(QString, current_album)]
        #[qproperty(QString, current_artwork_path)]
        #[qproperty(bool, download_cover_art)]
        #[qproperty(bool, radio_active)]
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
        #[qproperty(QString, output_device_id)]
        #[qproperty(QString, output_devices_json)]
        #[qproperty(QString, output_device_status)]
        #[qproperty(QString, supported_formats_json)]
        #[qproperty(QString, shuffle_mode)]
        #[qproperty(QString, repeat_mode)]
        #[qproperty(i32, queue_count)]
        #[qproperty(QString, total_duration)]
        #[qproperty(QString, directory_path)]
        #[qproperty(QString, soundfont_path)]
        #[qproperty(QString, sc55_rom_path)]
        #[qproperty(QString, mt32_rom_path)]
        #[qproperty(bool, mt32_gm_program_mapping)]
        #[qproperty(QString, midi_engine)]
        #[qproperty(QString, midi_status)]
        #[qproperty(QString, opening_files_behavior)]
        #[qproperty(bool, read_cue_sheets_in_folders)]
        #[qproperty(bool, read_playlists_in_folders)]
        #[qproperty(bool, show_tray_icon)]
        #[qproperty(bool, close_to_tray)]
        #[qproperty(bool, minimize_to_tray)]
        #[qproperty(bool, track_notifications)]
        #[qproperty(bool, directory_scan_active)]
        #[qproperty(i32, directory_scan_files_scanned)]
        #[qproperty(i32, directory_scan_tracks_added)]
        #[qproperty(QString, directory_scan_current_path)]
        #[qproperty(bool, tree_delete_active)]
        #[qproperty(i32, tree_delete_done)]
        #[qproperty(i32, tree_delete_total)]
        #[qproperty(QString, tree_delete_current_path)]
        #[qproperty(QString, tree_delete_error)]
        #[qproperty(bool, equalizer_enabled)]
        #[qproperty(bool, equalizer_track_genre)]
        #[qproperty(f64, equalizer_preamp_db)]
        #[qproperty(QString, equalizer_preset)]
        #[qproperty(QString, equalizer_preset_names)]
        #[qproperty(i32, equalizer_revision)]
        type AppController = super::AppControllerRust;

        #[qinvokable]
        fn add_file(self: Pin<&mut AppController>, url: QUrl);
        #[qinvokable]
        fn activate_file(self: Pin<&mut AppController>, url: QUrl);
        #[qinvokable]
        fn add_local_path(self: Pin<&mut AppController>, path: QString);
        #[qinvokable]
        fn activate_local_path(self: Pin<&mut AppController>, path: QString);
        #[qinvokable]
        fn add_local_paths_json(self: Pin<&mut AppController>, paths: QString);
        #[qinvokable]
        fn activate_local_paths_json(self: Pin<&mut AppController>, paths: QString);
        #[qinvokable]
        fn poll_directory_scan(self: Pin<&mut AppController>);
        #[qinvokable]
        fn cancel_directory_scan(self: Pin<&mut AppController>);
        #[qinvokable]
        fn start_tree_delete(
            self: Pin<&mut AppController>,
            paths: QString,
            permanent: bool,
        ) -> bool;
        #[qinvokable]
        fn poll_tree_delete(self: Pin<&mut AppController>);
        #[qinvokable]
        fn cancel_tree_delete(self: Pin<&mut AppController>);
        #[qinvokable]
        fn set_radio_enabled(self: Pin<&mut AppController>, enabled: bool);
        #[qinvokable]
        fn reshuffle_radio(self: Pin<&mut AppController>);
        #[qinvokable]
        fn poll_radio(self: Pin<&mut AppController>);
        #[qinvokable]
        fn add_url(self: Pin<&mut AppController>, url: QString);
        #[qinvokable]
        fn enqueue_url(self: Pin<&mut AppController>, url: QString);
        #[qinvokable]
        fn enqueue_urls_json(self: Pin<&mut AppController>, urls: QString);
        #[qinvokable]
        fn open_audio_files(self: Pin<&mut AppController>);
        #[qinvokable]
        fn choose_music_folder(self: Pin<&mut AppController>);
        #[qinvokable]
        fn save_playlist(self: Pin<&mut AppController>);
        #[qinvokable]
        fn save_playlist_selection(self: Pin<&mut AppController>, indices: QString);
        #[qinvokable]
        fn remove_track(self: Pin<&mut AppController>, index: i32);
        #[qinvokable]
        fn remove_tracks(self: Pin<&mut AppController>, indices: QString) -> i32;
        #[qinvokable]
        fn move_tracks(
            self: Pin<&mut AppController>,
            indices: QString,
            target_index: i32,
        ) -> QString;
        #[qinvokable]
        fn clear_playlist(self: Pin<&mut AppController>);
        #[qinvokable]
        fn filter_playlist(self: Pin<&mut AppController>, query: QString);
        #[qinvokable]
        fn sort_playlist(
            self: Pin<&mut AppController>,
            column: QString,
            selected_indices: QString,
        ) -> QString;
        #[qinvokable]
        fn toggle_stars(self: Pin<&mut AppController>, indices: QString);
        #[qinvokable]
        fn playlists_json(self: &AppController) -> QString;
        #[qinvokable]
        fn create_playlist(self: Pin<&mut AppController>, name: QString) -> QString;
        #[qinvokable]
        fn rename_playlist(self: Pin<&mut AppController>, id: i32, name: QString) -> QString;
        #[qinvokable]
        fn duplicate_playlist(self: Pin<&mut AppController>, id: i32, name: QString) -> QString;
        #[qinvokable]
        fn delete_playlist(self: Pin<&mut AppController>, id: i32);
        #[qinvokable]
        fn move_playlist(self: Pin<&mut AppController>, id: i32, to_position: i32);
        #[qinvokable]
        fn enqueue_playlist(self: Pin<&mut AppController>, id: i32, start_playback: bool);
        #[qinvokable]
        fn load_playlist_into_pane(self: Pin<&mut AppController>, id: i32);
        #[qinvokable]
        fn prune_missing_playlist_entries(self: Pin<&mut AppController>, id: i32) -> QString;
        #[qinvokable]
        fn blacklist_tree_paths(self: Pin<&mut AppController>, paths: QString, folders: bool)
        -> QString;
        #[qinvokable]
        fn blacklist_pane_selection(
            self: Pin<&mut AppController>,
            indices: QString,
            folders: bool,
        ) -> QString;
        #[qinvokable]
        fn blacklist_json(self: &AppController) -> QString;
        #[qinvokable]
        fn remove_blacklist_entry(self: Pin<&mut AppController>, id: i32) -> QString;
        #[qinvokable]
        fn save_pane_as_playlist(self: Pin<&mut AppController>, name: QString) -> QString;
        #[qinvokable]
        fn save_selection_as_playlist(
            self: Pin<&mut AppController>,
            indices: QString,
            name: QString,
        ) -> QString;
        #[qinvokable]
        fn export_playlist(self: Pin<&mut AppController>, id: i32);
        #[qinvokable]
        fn save_playlist_column_layout(self: Pin<&mut AppController>, layout: QString);
        #[qinvokable]
        fn play_index(self: Pin<&mut AppController>, index: i32);
        #[qinvokable]
        fn activate_playlist_index(self: Pin<&mut AppController>, index: i32);
        #[qinvokable]
        fn play_pause(self: Pin<&mut AppController>);
        #[qinvokable]
        fn stop(self: Pin<&mut AppController>);
        #[qinvokable]
        fn shutdown_synth_helpers(self: &AppController);
        #[qinvokable]
        fn server_settings_json(self: &AppController) -> QString;
        #[qinvokable]
        fn save_server_settings(self: Pin<&mut AppController>, json: QString) -> QString;
        #[qinvokable]
        fn generate_api_token(self: &AppController) -> QString;
        #[qinvokable]
        fn import_server_certificate(
            self: Pin<&mut AppController>,
            certificate: QUrl,
            private_key: QUrl,
        ) -> QString;
        #[qinvokable]
        fn start_api_server(self: Pin<&mut AppController>) -> QString;
        #[qinvokable]
        fn stop_api_server(self: Pin<&mut AppController>) -> QString;
        #[qinvokable]
        fn server_addresses_json(self: &AppController) -> QString;
        #[qinvokable]
        fn build_revision(self: &AppController) -> QString;
        #[qinvokable]
        fn build_timestamp(self: &AppController) -> f64;
        #[qinvokable]
        fn previous(self: Pin<&mut AppController>);
        #[qinvokable]
        fn next(self: Pin<&mut AppController>);
        #[qinvokable]
        fn seek(self: Pin<&mut AppController>, seconds: f64);
        #[qinvokable]
        fn set_volume_level(self: Pin<&mut AppController>, volume: f64);
        #[qinvokable]
        fn refresh_output_devices(self: Pin<&mut AppController>);
        #[qinvokable]
        fn select_output_device(self: Pin<&mut AppController>, id: QString);
        #[qinvokable]
        fn cycle_shuffle_mode(self: Pin<&mut AppController>);
        #[qinvokable]
        fn select_shuffle_mode(self: Pin<&mut AppController>, mode: QString);
        #[qinvokable]
        fn cycle_repeat_mode(self: Pin<&mut AppController>);
        #[qinvokable]
        fn select_repeat_mode(self: Pin<&mut AppController>, mode: QString);
        #[qinvokable]
        fn toggle_queue(self: Pin<&mut AppController>, indices: QString);
        #[qinvokable]
        fn clear_queue(self: Pin<&mut AppController>);
        #[qinvokable]
        fn queue_selection_state(self: &AppController, indices: QString) -> QString;
        #[qinvokable]
        fn toggle_stop_after(self: Pin<&mut AppController>, indices: QString);
        #[qinvokable]
        fn stop_after_selection_state(self: &AppController, indices: QString) -> QString;
        #[qinvokable]
        fn poll_playback(self: Pin<&mut AppController>);
        #[qinvokable]
        fn poll_audio_levels(self: Pin<&mut AppController>);

        #[qinvokable]
        fn visualizer_frame(self: &AppController) -> QString;
        #[qinvokable]
        fn skin_state(self: &AppController, include_tracks: bool) -> QString;
        #[qinvokable]
        fn update_skin_equalizer_band(self: Pin<&mut AppController>, index: i32, gain_db: f64);
        #[qinvokable]
        fn show_now_playing_notification(self: Pin<&mut AppController>);
        #[qinvokable]
        fn equalizer_band_gain(self: &AppController, index: i32) -> f64;
        #[qinvokable]
        fn update_equalizer_enabled(self: Pin<&mut AppController>, enabled: bool);
        #[qinvokable]
        fn update_equalizer_tracking(self: Pin<&mut AppController>, enabled: bool);
        #[qinvokable]
        fn update_equalizer_preamp(self: Pin<&mut AppController>, gain_db: f64);
        #[qinvokable]
        fn update_equalizer_band(self: Pin<&mut AppController>, index: i32, gain_db: f64);
        #[qinvokable]
        fn select_equalizer_preset(self: Pin<&mut AppController>, name: QString);
        #[qinvokable]
        fn flatten_equalizer(self: Pin<&mut AppController>);
        #[qinvokable]
        fn level_equalizer_preamp(self: Pin<&mut AppController>);

        #[qinvokable]
        fn track_number_at(self: &AppController, index: i32) -> QString;
        #[qinvokable]
        fn track_metadata_number_at(self: &AppController, index: i32) -> QString;
        #[qinvokable]
        fn track_status_at(self: &AppController, index: i32) -> QString;
        #[qinvokable]
        fn track_status_message_at(self: &AppController, index: i32) -> QString;
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
        fn track_value_at(self: &AppController, index: i32, column: QString) -> QString;
        #[qinvokable]
        fn track_missing_at(self: &AppController, index: i32) -> bool;
        #[qinvokable]
        fn tag_editor_data(self: &AppController, indices: QString) -> QString;
        #[qinvokable]
        fn choose_tag_artwork(self: &AppController) -> QString;
        #[qinvokable]
        fn save_tags(self: Pin<&mut AppController>, indices: QString, edits: QString) -> QString;

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
        fn import_sc55_rom_archive(self: Pin<&mut AppController>, url: QUrl);
        #[qinvokable]
        fn choose_sc55_rom_archive(self: Pin<&mut AppController>);
        #[qinvokable]
        fn clear_sc55_rom_directory(self: Pin<&mut AppController>);
        #[qinvokable]
        fn set_mt32_rom_directory(self: Pin<&mut AppController>, url: QUrl);
        #[qinvokable]
        fn choose_mt32_rom_folder(self: Pin<&mut AppController>);
        #[qinvokable]
        fn import_mt32_rom_archive(self: Pin<&mut AppController>, url: QUrl);
        #[qinvokable]
        fn choose_mt32_rom_archive(self: Pin<&mut AppController>);
        #[qinvokable]
        fn clear_mt32_rom_directory(self: Pin<&mut AppController>);
        #[qinvokable]
        fn update_mt32_gm_program_mapping(self: Pin<&mut AppController>, enabled: bool);
        #[qinvokable]
        fn select_midi_engine(self: Pin<&mut AppController>, engine: QString);
        #[qinvokable]
        fn select_opening_files_behavior(self: Pin<&mut AppController>, behavior: QString);
        #[qinvokable]
        fn set_folder_cue_mode(self: Pin<&mut AppController>, enabled: bool);
        #[qinvokable]
        fn set_folder_playlist_mode(self: Pin<&mut AppController>, enabled: bool);
        #[qinvokable]
        fn update_show_tray_icon(self: Pin<&mut AppController>, enabled: bool);
        #[qinvokable]
        fn update_close_to_tray(self: Pin<&mut AppController>, enabled: bool);
        #[qinvokable]
        fn update_minimize_to_tray(self: Pin<&mut AppController>, enabled: bool);
        #[qinvokable]
        fn update_track_notifications(self: Pin<&mut AppController>, enabled: bool);
        #[qinvokable]
        fn prewarm_synths(self: &AppController);
        #[qinvokable]
        fn update_download_cover_art(self: Pin<&mut AppController>, enabled: bool);
        #[qinvokable]
        fn poll_cover_art(self: Pin<&mut AppController>);
    }
}

use std::cmp::Ordering;
use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering as AtomicOrdering};
use std::sync::mpsc::{Receiver, SyncSender, TryRecvError, TrySendError};
use std::time::{Duration, Instant};

use cxx_qt::CxxQtType;
use cxx_qt_lib::{QString, QUrl};

use kog_audio::decoder::{
    DecoderRegistry, DecoderSettings, ExpansionResult, PlaybackSource, validate_soundfont,
};
use kog_core::equalizer::{
    EqualizerSettings, apply_preset, preset_for_genre, preset_named, preset_names,
};
use kog_core::mpris::{
    MprisCommand, MprisLoopStatus, MprisPlaybackStatus, MprisService, MprisSnapshot,
};
use kog_audio::playback::{OutputDevice, PlaybackEngine, PlaybackState, available_output_devices};
use kog_audio::playback_order::{PlaybackOrder, SelectionState};
use kog_audio::playlist::{Playlist, PlaylistEntry, PlaylistLocation};
use crate::rom_import::{ImportedRomSet, RomKind, import_rom_archive};
use kog_audio::settings::{
    AppSettings, MidiEngine, OpeningFilesBehavior, OutputDevicePreference, RepeatMode, ShuffleMode,
};
use crate::tag_editor::{artwork_file_json, parse_edits, snapshot_json, write_tags};
use kog_audio::track::{Track, canonical_path};

#[derive(Debug, Default)]
struct AddPathResult {
    added: usize,
    warning: Option<String>,
}

enum DirectoryScanEvent {
    Prepared(PreparedScanFile),
    Warning(String),
    Complete { cancelled: bool },
}

struct PreparedScanFile {
    path: PathBuf,
    tracks: Vec<Track>,
    warnings: Vec<String>,
}

struct DirectoryScanState {
    receiver: Receiver<DirectoryScanEvent>,
    cancel: Arc<AtomicBool>,
    cancel_requested: bool,
    behavior: OpeningFilesBehavior,
    first_new_source_index: usize,
    combined: AddPathResult,
    known_sources: HashSet<PlaybackSource>,
    playlist_dirty: bool,
    last_playlist_refresh: Instant,
}

enum TreeDeleteEvent {
    Total { items: usize },
    Progress { path: PathBuf, done: usize },
    Deleted { path: PathBuf },
    Failed { path: PathBuf, error: String },
    Complete { cancelled: bool, files: usize },
}

struct TreeDeleteState {
    receiver: Receiver<TreeDeleteEvent>,
    cancel: Arc<AtomicBool>,
    cancel_requested: bool,
    permanent: bool,
    deleted: Vec<PathBuf>,
    failures: Vec<String>,
}

enum RadioEvent {
    TracksReady {
        locator: PathBuf,
        tracks: Vec<Track>,
        warnings: Vec<String>,
    },
}

/// One in-flight radio expansion: up to MAX_EXPANDERS run at once so a
/// single slow archive cannot wedge staging. The locator identifies timed
/// out picks for the session skip-list.
struct RadioJob {
    receiver: Receiver<RadioEvent>,
    cancel: Arc<AtomicBool>,
    locator: PathBuf,
    started: Instant,
}

/// Live radio state: staged tracks waiting for their playlist turn, the
/// in-flight expansions, the background staging thread's pick channel,
/// locators that timed out this session, and whether an explicit play press
/// is owed an instant start. Picking never touches the UI thread: the
/// staging thread owns the round, its decoders, and its cursors, paced by
/// the bounded channel.
struct RadioState {
    ready: VecDeque<Track>,
    /// Surplus subsongs from multi-song single files (NSF and friends),
    /// already shuffled, served spaced out instead of back-to-back.
    deferred: VecDeque<Track>,
    /// Stagings committed (fresh expansions plus deferred servings): drives
    /// the interleave below.
    staged_count: u64,
    expand_jobs: Vec<RadioJob>,
    dead: Arc<Mutex<HashSet<String>>>,
    blacklist: Arc<Mutex<kog_audio::radio::Blacklist>>,
    staging: Option<StagingWorker>,
    kickstart_armed: bool,
    consecutive_dead: u32,
}

/// Background staging thread handle: picks arrive on the bounded channel
/// (which paces the thread), and the cancel flag stops it after teardown.
struct StagingWorker {
    picks: Receiver<kog_audio::radio::StagingResponse>,
    cancel: Arc<AtomicBool>,
}

/// Spawn the staging thread. Only cheap clones happen here, so toggling
/// never waits: decoder construction and the first descent run over there.
/// Returns the channel handle plus the shared dead set, which the UI feeds
/// with proven-unplayable locators and the thread consults per pick.
fn spawn_staging_worker(
    root: PathBuf,
    settings: DecoderSettings,
    read_cue: bool,
    initial: kog_audio::radio::RoundInitial,
    blacklist: kog_audio::radio::Blacklist,
) -> Result<
    (
        StagingWorker,
        Arc<Mutex<HashSet<String>>>,
        Arc<Mutex<kog_audio::radio::Blacklist>>,
    ),
    String,
> {
    let (sender, receiver) = std::sync::mpsc::sync_channel(4);
    let cancel = Arc::new(AtomicBool::new(false));
    let worker_cancel = Arc::clone(&cancel);
    let nested_cache = kog_audio::archive::nested_cache_dir();
    let save_path = kog_audio::settings::setting_path("radio-round.json");
    let dead = Arc::new(Mutex::new(HashSet::new()));
    let worker_dead = Arc::clone(&dead);
    let blacklist = Arc::new(Mutex::new(blacklist));
    let worker_blacklist = Arc::clone(&blacklist);
    std::thread::Builder::new()
        .name("kog-radio-stage".to_owned())
        .spawn(move || {
            kog_audio::radio::run_staging(
                root,
                settings,
                read_cue,
                nested_cache,
                initial,
                save_path,
                worker_dead,
                worker_blacklist,
                sender,
                worker_cancel,
            );
        })
        .map_err(|_| "Random Radio could not start its worker".to_owned())?;
    Ok((
        StagingWorker {
            picks: receiver,
            cancel,
        },
        dead,
        blacklist,
    ))
}

/// Blacklist snapshot from the library store for radio staging.
fn blacklist_snapshot(db: &kog_core::db::LibraryDb) -> kog_audio::radio::Blacklist {
    let rows: Vec<(String, String, String)> = db
        .list_blacklist()
        .unwrap_or_default()
        .into_iter()
        .map(|entry| (entry.kind, entry.path, entry.entry))
        .collect();
    kog_audio::radio::Blacklist::from_rows(&rows)
}

/// One blacklist row, with the path resolved best-effort so keys match
/// radio locators derived from the same tree.
fn blacklist_row(kind: &str, path: &str, entry: &str) -> kog_core::db::BlacklistEntry {
    kog_core::db::BlacklistEntry {
        id: 0,
        kind: kind.to_owned(),
        path: path.to_owned(),
        entry: entry.to_owned(),
    }
}

/// Best-effort canonical path for blacklist keys; falls back to the raw
/// string for missing files so the row still records intent.
fn blacklist_path(path: &str) -> String {
    let candidate = PathBuf::from(path);
    kog_audio::track::canonical_path(&candidate)
        .map(|canonical| canonical.to_string_lossy().into_owned())
        .unwrap_or_else(|_| path.to_owned())
}

/// Persisted round for `root`, or None when nothing valid waits. A wrong
/// music folder never resumes another folder's positions.
fn load_radio_round(root: &Path) -> Option<kog_audio::radio::RoundInitial> {
    let path = kog_audio::settings::setting_path("radio-round.json")?;
    kog_audio::radio::RadioRound::load(&path, root)
}

struct CoverArtRequest {
    generation: u64,
    artist: String,
    album: String,
    cache_dir: PathBuf,
}

struct CoverArtResult {
    generation: u64,
    path: Option<PathBuf>,
}

struct CoverArtState {
    receiver: Receiver<CoverArtResult>,
    cancel: Arc<AtomicBool>,
    generation: u64,
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

fn rom_import_status(
    engine: &str,
    recognized_model: &str,
    archive: &Path,
    imported: &ImportedRomSet,
) -> String {
    let mut status = format!(
        "Imported {} {} ROM files from {} — recognized {}",
        imported.file_count,
        engine,
        archive.display(),
        recognized_model
    );
    if !imported.warnings.is_empty() {
        status.push_str(" — ");
        status.push_str(&imported.warnings.join("; "));
    }
    status
}

fn ordered_directory_files(directory: &Path) -> Result<Vec<PathBuf>, String> {
    let mut files = Vec::new();
    let mut pending = vec![(directory.to_owned(), true)];

    while let Some((path, is_directory)) = pending.pop() {
        if kog_core::media_path::is_metadata(&path) {
            continue;
        }
        if !is_directory {
            files.push(path);
            continue;
        }

        let mut entries = std::fs::read_dir(&path)
            .map_err(|error| format!("reading {}: {error}", path.display()))?
            .filter_map(Result::ok)
            .collect::<Vec<_>>();
        entries.sort_by_key(std::fs::DirEntry::path);

        // The stack is LIFO, so queue entries backward to visit them in their
        // sorted order. Keep the file/directory distinction with each queued
        // path so regular files are appended only when they are visited.
        for entry in entries.into_iter().rev() {
            let Ok(file_type) = entry.file_type() else {
                continue;
            };
            if file_type.is_dir() {
                pending.push((entry.path(), true));
            } else if file_type.is_file() {
                pending.push((entry.path(), false));
            }
        }
    }

    Ok(files)
}

fn send_directory_scan_event(
    sender: &SyncSender<DirectoryScanEvent>,
    cancel: &AtomicBool,
    mut event: DirectoryScanEvent,
) -> bool {
    loop {
        match sender.try_send(event) {
            Ok(()) => return true,
            Err(TrySendError::Disconnected(_)) => return false,
            Err(TrySendError::Full(returned)) => {
                event = returned;
                if cancel.load(AtomicOrdering::Relaxed)
                    && !matches!(event, DirectoryScanEvent::Complete { .. })
                {
                    return false;
                }
                std::thread::sleep(Duration::from_millis(5));
            }
        }
    }
}

/// Parse the JSON path list handed over from the file-tree context menu.
fn parse_delete_paths_json(value: &str) -> Vec<PathBuf> {
    serde_json::from_str::<Vec<String>>(value)
        .unwrap_or_default()
        .into_iter()
        .map(PathBuf::from)
        .collect()
}

/// Drop empty selections, relative paths, filesystem roots, duplicates, and
/// paths already covered by another selected ancestor so each on-disk entry
/// is deleted exactly once. Symlinks are kept as-is: trashing a link removes
/// the link, never its target.
fn sanitize_delete_paths(paths: Vec<PathBuf>) -> Vec<PathBuf> {
    let mut kept: Vec<PathBuf> = Vec::new();
    let mut sorted = paths;
    sorted.sort();
    sorted.dedup();
    for path in sorted {
        if path.as_os_str().is_empty() || !path.is_absolute() || path.parent().is_none() {
            continue;
        }
        if kept
            .iter()
            .any(|kept| path == *kept || path.starts_with(kept))
        {
            continue;
        }
        kept.push(path);
    }
    kept
}

/// Count the entries under `path` for the delete summary without following
/// symlinked directories. The path itself counts as one entry; missing paths
/// count as zero so a concurrent external change cannot fail the job.
fn count_delete_entries(path: &Path, cancel: &AtomicBool) -> (usize, usize) {
    let mut files = 0_usize;
    let mut directories = 0_usize;
    let mut pending = vec![path.to_owned()];
    while let Some(current) = pending.pop() {
        if cancel.load(AtomicOrdering::Relaxed) {
            break;
        }
        let Ok(metadata) = std::fs::symlink_metadata(&current) else {
            continue;
        };
        if metadata.file_type().is_dir() {
            directories = directories.saturating_add(1);
            let Ok(entries) = std::fs::read_dir(&current) else {
                continue;
            };
            for entry in entries.filter_map(Result::ok) {
                pending.push(entry.path());
            }
        } else {
            files = files.saturating_add(1);
        }
    }
    (files, directories)
}

/// Permanently remove a file, link, or directory tree without following the
/// final link itself.
fn remove_path_permanent(path: &Path) -> Result<(), String> {
    let metadata = std::fs::symlink_metadata(path)
        .map_err(|error| format!("reading {}: {error}", path.display()))?;
    if metadata.file_type().is_dir() {
        std::fs::remove_dir_all(path)
    } else {
        std::fs::remove_file(path)
    }
    .map_err(|error| format!("deleting {}: {error}", path.display()))
}

/// Local filesystem paths a playlist track depends on: its own file plus the
/// archive it was expanded from, if any. Remote tracks depend on nothing
/// the file tree can delete.
fn track_local_paths(track: &Track) -> Vec<PathBuf> {
    if track.source.is_remote() {
        return Vec::new();
    }
    let mut paths = vec![track.source.path.clone()];
    if let Some(origin) = &track.source.archive_origin {
        paths.push(origin.archive_path.clone());
    }
    paths
}

/// Source indices of tracks with a local path at or under any deleted path.
/// `Path::starts_with` compares whole components, so `/music/rock` never
/// matches `/music/rock2`. Nested tracks name their real outer archive, so
/// deleting it purges them like any other entry.
fn purged_track_indices(tracks: &[Track], deleted: &[PathBuf]) -> Vec<usize> {
    tracks
        .iter()
        .enumerate()
        .filter(|(_, track)| {
            track_local_paths(track).iter().any(|path| {
                deleted
                    .iter()
                    .any(|root| path == root || path.starts_with(root))
            })
        })
        .map(|(index, _)| index)
        .collect()
}

fn run_tree_delete_job(
    paths: Vec<PathBuf>,
    permanent: bool,
    sender: SyncSender<TreeDeleteEvent>,
    cancel: Arc<AtomicBool>,
) {
    // Deleted/Failed/Complete use blocking sends: the summary must not lose
    // them if the worker outruns the 100 ms UI poll. Progress and Total may
    // drop under pressure; the next poll catches up. Blocking is
    // deadlock-free because the UI keeps draining until Complete arrives.
    let mut files = 0_usize;
    for path in &paths {
        if cancel.load(AtomicOrdering::Relaxed) {
            break;
        }
        let (path_files, _) = count_delete_entries(path, &cancel);
        files = files.saturating_add(path_files);
    }
    let mut completed = 0_usize;
    let total = paths.len();
    let _ = sender.try_send(TreeDeleteEvent::Total { items: total });
    for path in &paths {
        if cancel.load(AtomicOrdering::Relaxed) {
            break;
        }
        let outcome = if permanent {
            remove_path_permanent(path)
        } else {
            trash::delete(path).map_err(|error| error.to_string())
        };
        match outcome {
            Ok(()) => {
                let _ = sender.send(TreeDeleteEvent::Deleted { path: path.clone() });
            }
            Err(error) => {
                let _ = sender.send(TreeDeleteEvent::Failed {
                    path: path.clone(),
                    error,
                });
            }
        }
        completed += 1;
        let _ = sender.try_send(TreeDeleteEvent::Progress {
            path: path.clone(),
            done: completed,
        });
    }
    let _ = sender.send(TreeDeleteEvent::Complete {
        cancelled: cancel.load(AtomicOrdering::Relaxed),
        files,
    });
}

fn cover_art_cache_dir() -> PathBuf {
    directories::ProjectDirs::from("org", "Kog", "Kog")
        .map(|directories| kog_audio::cover_art::cache_directory(directories.cache_dir()))
        .unwrap_or_else(|| std::env::temp_dir().join("kog-covers"))
}

/// Stable cache directory for materialized playlist files (one `.m3u` per
/// custom playlist id, `favorites.m3u` for stars). Enqueueing a playlist
/// rewrites its file and feeds it through the normal background folder
/// scanner, so progress, cancel, dedup, and play behavior all match
/// adding files from the tree.
fn playlist_cache_dir() -> PathBuf {
    directories::ProjectDirs::from("org", "Kog", "Kog")
        .map(|directories| directories.cache_dir().join("kog-playlists"))
        .unwrap_or_else(|| std::env::temp_dir().join("kog-playlists"))
}

fn fetch_cover(url: &str, max_bytes: u32) -> Result<Vec<u8>, String> {
    qobject::fetch_cover_art_url(&QString::from(url), max_bytes)
        .map(|bytes| bytes.as_slice().to_vec())
        .map_err(|error| error.to_string())
}

fn fetch_validated_cover(url: &str) -> Option<Vec<u8>> {
    let bytes = fetch_cover(url, kog_audio::cover_art::MAX_COVER_BYTES).ok()?;
    if kog_audio::cover_art::sniff_image_kind(&bytes).is_some() {
        Some(bytes)
    } else {
        None
    }
}

fn musicbrainz_rate_limit() {
    use std::sync::OnceLock;
    static LAST_REQUEST: OnceLock<std::sync::Mutex<std::time::Instant>> = OnceLock::new();
    let guard = LAST_REQUEST.get_or_init(|| {
        std::sync::Mutex::new(std::time::Instant::now() - Duration::from_secs(2))
    });
    if let Ok(mut last) = guard.lock() {
        let elapsed = last.elapsed();
        if elapsed < Duration::from_millis(1100) {
            std::thread::sleep(Duration::from_millis(1100) - elapsed);
        }
        *last = std::time::Instant::now();
    }
}

fn resolve_cover_art(request: &CoverArtRequest, cancel: &AtomicBool) -> Option<PathBuf> {
    let cancelled = || cancel.load(AtomicOrdering::Relaxed);
    let key = kog_audio::cover_art::cache_key(&request.artist, &request.album);
    let store = |bytes: Vec<u8>| kog_audio::cover_art::store_cache(&request.cache_dir, &key, &bytes);

    if !cancelled() {
        let url = kog_audio::cover_art::deezer_search_url(&request.artist, &request.album);
        if let Ok(json) = fetch_cover(&url, kog_audio::cover_art::MAX_SEARCH_BYTES)
            && let Ok(text) = String::from_utf8(json)
            && let Some((title, artist, cover)) = kog_audio::cover_art::parse_deezer_cover(&text)
            && !cover.is_empty()
            && kog_audio::cover_art::titles_match(&request.artist, &request.album, &title, &artist)
            && let Some(bytes) = fetch_validated_cover(&cover)
            && let Some(path) = store(bytes)
        {
            return Some(path);
        }
    }
    if !cancelled() {
        let url = kog_audio::cover_art::itunes_search_url(&request.artist, &request.album);
        if let Ok(json) = fetch_cover(&url, kog_audio::cover_art::MAX_SEARCH_BYTES)
            && let Ok(text) = String::from_utf8(json)
            && let Some((title, artist, artwork)) = kog_audio::cover_art::parse_itunes_cover(&text)
            && !artwork.is_empty()
            && kog_audio::cover_art::titles_match(&request.artist, &request.album, &title, &artist)
            && let Some(bytes) = fetch_validated_cover(&artwork)
            && let Some(path) = store(bytes)
        {
            return Some(path);
        }
    }
    if !cancelled() {
        musicbrainz_rate_limit();
        if !cancelled() {
            let url = kog_audio::cover_art::mb_release_group_url(&request.artist, &request.album);
            if let Ok(json) = fetch_cover(&url, kog_audio::cover_art::MAX_SEARCH_BYTES)
                && let Ok(text) = String::from_utf8(json)
                && let Some((title, artist, mbid)) =
                    kog_audio::cover_art::parse_mb_release_group(&text)
                && !mbid.is_empty()
                && kog_audio::cover_art::titles_match(&request.artist, &request.album, &title, &artist)
                && let Some(bytes) =
                    fetch_validated_cover(&kog_audio::cover_art::caa_front_url(&mbid))
                && let Some(path) = store(bytes)
            {
                return Some(path);
            }
        }
    }
    if !cancelled() {
        let query = format!("{} {} cover art", request.artist, request.album)
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        if let Ok(page) = fetch_cover(
            &kog_audio::cover_art::ddg_page_url(&query),
            kog_audio::cover_art::MAX_SEARCH_BYTES,
        )
        && let Ok(html) = String::from_utf8(page)
        && let Some(token) = kog_audio::cover_art::ddg_token(&html)
        && let Ok(results) = fetch_cover(
            &kog_audio::cover_art::ddg_image_url(&query, &token),
            kog_audio::cover_art::MAX_SEARCH_BYTES,
        )
        && let Ok(text) = String::from_utf8(results)
        && let Some((title, thumbnail)) = kog_audio::cover_art::parse_ddg_thumbnail(&text)
        && kog_audio::cover_art::titles_match(&request.artist, &request.album, &title, "")
        && let Some(bytes) = fetch_validated_cover(&thumbnail)
        && let Some(path) = store(bytes)
        {
            return Some(path);
        }
    }
    None
}

fn run_cover_art_job(
    request: CoverArtRequest,
    sender: SyncSender<CoverArtResult>,
    cancel: Arc<AtomicBool>,
) {
    let path = if cancel.load(AtomicOrdering::Relaxed) {
        None
    } else {
        resolve_cover_art(&request, &cancel)
    };
    let _ = sender.send(CoverArtResult {
        generation: request.generation,
        path,
    });
}

fn prepare_scan_file(
    path: PathBuf,
    decoders: &DecoderRegistry,
    read_cue_sheets: bool,
    read_playlists: bool,
    explicit_root: bool,
) -> PreparedScanFile {
    let mut prepared = PreparedScanFile {
        path: path.clone(),
        tracks: Vec::new(),
        warnings: Vec::new(),
    };
    if !decoders.accepts_path(&path) {
        return prepared;
    }
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default();
    if extension.eq_ignore_ascii_case("cue") && !read_cue_sheets && !explicit_root {
        return prepared;
    }
    if matches!(
        extension.to_ascii_lowercase().as_str(),
        "m3u" | "m3u8" | "pls"
    ) && !read_playlists
        && !explicit_root
    {
        return prepared;
    }

    let path = match if kog_audio::archive::is_tree_location(&path) {
        Ok(path)
    } else {
        canonical_path(&path)
    } {
        Ok(path) => path,
        Err(error) => {
            prepared.warnings.push(error);
            return prepared;
        }
    };
    let expansion = match decoders.expand_detailed(path) {
        Ok(expansion) => expansion,
        Err(error) => {
            prepared.warnings.push(error);
            return prepared;
        }
    };
    prepared.warnings.extend(expansion.warnings);
    for source in expansion.sources {
        let track = Track::from_source(source, decoders);
        if let Some(warning) = &track.decoder_warning {
            prepared.warnings.push(warning.clone());
        }
        prepared.tracks.push(track);
    }
    prepared
}

fn scan_directory_paths(
    paths: Vec<PathBuf>,
    sender: SyncSender<DirectoryScanEvent>,
    cancel: Arc<AtomicBool>,
    decoders: DecoderRegistry,
    decoder_settings: DecoderSettings,
    read_cue_sheets: bool,
    read_playlists: bool,
) {
    let mut files = Vec::new();
    // Explicitly passed playlist/cue files are always parsed: the folder
    // preference only governs playlists discovered during folder walks.
    // Without this, staging a playlist for import (e.g. loading a stored
    // playlist into the pane) silently yields zero tracks.
    let mut explicit_roots = HashSet::new();
    'roots: for root in paths {
        if kog_core::media_path::is_metadata(&root) {
            continue;
        }
        if cancel.load(AtomicOrdering::Relaxed) {
            break;
        }
        if kog_audio::archive::is_tree_location(&root) {
            files.push(root);
            continue;
        }
        let root = match std::fs::canonicalize(&root) {
            Ok(root) => root,
            Err(error) => {
                if !send_directory_scan_event(
                    &sender,
                    &cancel,
                    DirectoryScanEvent::Warning(format!("reading {}: {error}", root.display())),
                ) {
                    return;
                }
                continue;
            }
        };

        if root.is_file() {
            explicit_roots.insert(root.clone());
            files.push(root);
            continue;
        }
        if !root.is_dir() {
            let _ = send_directory_scan_event(
                &sender,
                &cancel,
                DirectoryScanEvent::Warning(format!("{} is not a file or folder", root.display())),
            );
            continue;
        }

        let mut pending = vec![(root, true)];
        while let Some((path, is_directory)) = pending.pop() {
            if kog_core::media_path::is_metadata(&path) {
                continue;
            }
            if cancel.load(AtomicOrdering::Relaxed) {
                break 'roots;
            }
            if !is_directory {
                files.push(path);
                continue;
            }

            let mut entries = match std::fs::read_dir(&path) {
                Ok(entries) => entries.filter_map(Result::ok).collect::<Vec<_>>(),
                Err(error) => {
                    if !send_directory_scan_event(
                        &sender,
                        &cancel,
                        DirectoryScanEvent::Warning(format!("reading {}: {error}", path.display())),
                    ) {
                        return;
                    }
                    continue;
                }
            };
            entries.sort_by_key(std::fs::DirEntry::path);
            for entry in entries.into_iter().rev() {
                if cancel.load(AtomicOrdering::Relaxed) {
                    break 'roots;
                }
                let path = entry.path();
                let Ok(file_type) = entry.file_type() else {
                    continue;
                };
                if file_type.is_dir() {
                    pending.push((path, true));
                } else if file_type.is_file() {
                    pending.push((path, false));
                }
            }
        }
    }

    if !files.is_empty() && !cancel.load(AtomicOrdering::Relaxed) {
        let files = Arc::new(files);
        let explicit_roots = Arc::new(explicit_roots);
        let next_index = Arc::new(AtomicUsize::new(0));
        let worker_count = std::thread::available_parallelism()
            .map_or(2, usize::from)
            .min(8)
            .min(files.len());
        let (prepared_sender, prepared_receiver) =
            std::sync::mpsc::sync_channel::<(usize, PreparedScanFile)>(worker_count * 4);

        std::thread::scope(|scope| {
            for _ in 0..worker_count {
                let files = Arc::clone(&files);
                let explicit_roots = Arc::clone(&explicit_roots);
                let next_index = Arc::clone(&next_index);
                let prepared_sender = prepared_sender.clone();
                let cancel = Arc::clone(&cancel);
                let worker_decoders = decoders.background_worker(decoder_settings.clone());
                scope.spawn(move || {
                    loop {
                        if cancel.load(AtomicOrdering::Relaxed) {
                            break;
                        }
                        let index = next_index.fetch_add(1, AtomicOrdering::Relaxed);
                        let Some(path) = files.get(index).cloned() else {
                            break;
                        };
                        let prepared = prepare_scan_file(
                            path.clone(),
                            &worker_decoders,
                            read_cue_sheets,
                            read_playlists,
                            explicit_roots.contains(&path),
                        );
                        if prepared_sender.send((index, prepared)).is_err() {
                            break;
                        }
                    }
                });
            }
            drop(prepared_sender);

            let mut next_to_send = 0_usize;
            let mut pending = BTreeMap::new();
            let mut forwarding = true;
            while let Ok((index, prepared)) = prepared_receiver.recv() {
                if !forwarding || cancel.load(AtomicOrdering::Relaxed) {
                    forwarding = false;
                    continue;
                }
                pending.insert(index, prepared);
                while let Some(prepared) = pending.remove(&next_to_send) {
                    if !send_directory_scan_event(
                        &sender,
                        &cancel,
                        DirectoryScanEvent::Prepared(prepared),
                    ) {
                        forwarding = false;
                        break;
                    }
                    next_to_send += 1;
                }
            }
        });
    }

    let cancelled = cancel.load(AtomicOrdering::Relaxed);
    let _ = send_directory_scan_event(&sender, &cancel, DirectoryScanEvent::Complete { cancelled });
}

fn local_paths_from_json(value: &str) -> Result<Vec<PathBuf>, String> {
    if value.len() > 1_048_576 {
        return Err("The file-tree selection is too large".to_owned());
    }
    let paths = serde_json::from_str::<Vec<String>>(value)
        .map_err(|error| format!("Reading the file-tree selection: {error}"))?;
    if paths.len() > 4_096 {
        return Err("The file-tree selection contains too many items".to_owned());
    }
    Ok(paths
        .into_iter()
        .filter(|path| !path.is_empty())
        .map(PathBuf::from)
        .collect())
}

fn dropped_urls_from_json(value: &str) -> Result<(Vec<PathBuf>, Vec<String>), String> {
    if value.len() > 1_048_576 {
        return Err("The dropped URL list is too large".to_owned());
    }
    let values = serde_json::from_str::<Vec<String>>(value)
        .map_err(|error| format!("Reading the dropped URL list: {error}"))?;
    if values.len() > 4_096 {
        return Err("No more than 4096 items can be dropped at once".to_owned());
    }

    let mut paths = Vec::new();
    let mut remote_urls = Vec::new();
    for value in values {
        let value = value.trim();
        if value.is_empty() || value.starts_with('#') {
            continue;
        }
        let parsed = url::Url::parse(value)
            .map_err(|error| format!("Invalid dropped URL {value:?}: {error}"))?;
        match parsed.scheme() {
            "file" => paths.push(parsed.to_file_path().map_err(|()| {
                format!("Dropped file URL cannot be represented on this system: {value}")
            })?),
            "http" | "https" => remote_urls.push(value.to_owned()),
            scheme => return Err(format!("Unsupported dropped URL scheme: {scheme}")),
        }
    }
    Ok((paths, remote_urls))
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

fn parse_row_indices(value: &str, row_count: usize) -> Vec<usize> {
    let mut indices = value
        .split(',')
        .filter_map(|value| value.trim().parse::<usize>().ok())
        .filter(|index| *index < row_count)
        .collect::<Vec<_>>();
    indices.sort_unstable();
    indices.dedup();
    indices
}

/// Cache key and download policy for a track's artwork. Fully untagged
/// files (no artist, album only from the parent-folder fallback) get a
/// per-file key and never download: the shared folder-name key is what
/// glued one irrelevant download onto every MIDI in a folder like "new",
/// and embedded art must not leak across files via a shared key either.
fn cover_art_key(artist: &str, tagged_album: &str, album: &str, file: &Path) -> (String, bool) {
    let untagged = artist.trim().is_empty() && tagged_album.trim().is_empty();
    if untagged {
        let scoped = format!("{} \0 {}", album, file.display());
        (kog_audio::cover_art::cache_key("", &scoped), false)
    } else {
        (kog_audio::cover_art::cache_key(artist, album), true)
    }
}

fn playlist_entry_for_track(track: &Track) -> Result<PlaylistEntry, String> {
    let location = if let Some(origin) = &track.source.archive_origin {
        PlaylistLocation::Archive {
            archive_path: origin.archive_path.clone(),
            entry_name: origin.entry_name.clone(),
        }
    } else if let Some(url) = &track.source.remote_url {
        PlaylistLocation::Remote(url.clone())
    } else {
        PlaylistLocation::Local(track.source.path.clone())
    };
    let fragment = match track.source.subsong {
        None => None,
        Some(_) if track.backend_id == "cuesheet" => Some(
            track
                .track_number
                .ok_or_else(|| {
                    format!(
                        "CueSheet track {} has no declared track number",
                        track.source.display_label()
                    )
                })?
                .to_string(),
        ),
        Some(subsong) => Some(subsong.to_string()),
    };
    Ok(PlaylistEntry { location, fragment })
}

fn normalize_playlist_save_path(mut path: PathBuf) -> Result<PathBuf, String> {
    let extension = path.extension().and_then(|value| value.to_str());
    match extension {
        None => {
            path.set_extension("m3u");
            Ok(path)
        }
        Some(extension)
            if ["m3u", "m3u8", "pls"]
                .iter()
                .any(|candidate| candidate.eq_ignore_ascii_case(extension)) =>
        {
            Ok(path)
        }
        Some(extension) => Err(format!(
            "Playlist filename must end in .m3u, .m3u8, or .pls, not .{extension}"
        )),
    }
}

fn move_selected_items<T>(
    items: &mut Vec<T>,
    selected_indices: &[usize],
    target_slot: usize,
) -> Vec<usize> {
    if selected_indices.is_empty() || items.is_empty() {
        return Vec::new();
    }

    let item_count = items.len();
    let target_slot = target_slot.min(item_count);
    let mut selected = vec![false; item_count];
    for &index in selected_indices {
        if let Some(value) = selected.get_mut(index) {
            *value = true;
        }
    }

    let mut moving = Vec::with_capacity(selected_indices.len());
    let mut remaining = Vec::with_capacity(item_count - selected_indices.len());
    for (index, item) in std::mem::take(items).into_iter().enumerate() {
        if selected[index] {
            moving.push(item);
        } else {
            remaining.push(item);
        }
    }

    let selected_before_target = selected_indices
        .iter()
        .filter(|&&index| index < target_slot)
        .count();
    let insertion_index = target_slot
        .saturating_sub(selected_before_target)
        .min(remaining.len());
    let moved_count = moving.len();
    remaining.splice(insertion_index..insertion_index, moving);
    *items = remaining;

    (insertion_index..insertion_index + moved_count).collect()
}

fn encode_row_indices(indices: &[usize]) -> QString {
    qstring(
        indices
            .iter()
            .map(usize::to_string)
            .collect::<Vec<_>>()
            .join(","),
    )
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum PlaylistSortColumn {
    #[default]
    Index,
    Star,
    Rating,
    Title,
    AlbumArtist,
    Artist,
    Composer,
    Album,
    Length,
    Date,
    Genre,
    Track,
    PlayCount,
    Path,
    Filename,
    Codec,
    SampleRate,
    BitsPerSample,
    Bitrate,
    Status,
}

impl PlaylistSortColumn {
    fn from_identifier(identifier: &str) -> Option<Self> {
        match identifier {
            "index" => Some(Self::Index),
            "star" => Some(Self::Star),
            "rating" => Some(Self::Rating),
            "title" => Some(Self::Title),
            "albumartist" => Some(Self::AlbumArtist),
            "artist" => Some(Self::Artist),
            "composer" => Some(Self::Composer),
            "album" => Some(Self::Album),
            "length" => Some(Self::Length),
            "year" | "date" => Some(Self::Date),
            "genre" => Some(Self::Genre),
            "track" => Some(Self::Track),
            "playcount" => Some(Self::PlayCount),
            "path" => Some(Self::Path),
            "filename" => Some(Self::Filename),
            "codec" => Some(Self::Codec),
            "samplerate" => Some(Self::SampleRate),
            "bitspersample" => Some(Self::BitsPerSample),
            "bitrate" => Some(Self::Bitrate),
            "status" => Some(Self::Status),
            _ => None,
        }
    }

    const fn identifier(self) -> &'static str {
        match self {
            Self::Index => "index",
            Self::Star => "star",
            Self::Rating => "rating",
            Self::Title => "title",
            Self::AlbumArtist => "albumartist",
            Self::Artist => "artist",
            Self::Composer => "composer",
            Self::Album => "album",
            Self::Length => "length",
            Self::Date => "date",
            Self::Genre => "genre",
            Self::Track => "track",
            Self::PlayCount => "playcount",
            Self::Path => "path",
            Self::Filename => "filename",
            Self::Codec => "codec",
            Self::SampleRate => "samplerate",
            Self::BitsPerSample => "bitspersample",
            Self::Bitrate => "bitrate",
            Self::Status => "status",
        }
    }

    const fn display_name(self) -> &'static str {
        match self {
            Self::Index => "playlist order",
            Self::Star => "Star",
            Self::Rating => "Rating",
            Self::Title => "Title",
            Self::AlbumArtist => "Album Artist",
            Self::Artist => "Artist",
            Self::Composer => "Composer",
            Self::Album => "Album",
            Self::Length => "Length",
            Self::Date => "Date",
            Self::Genre => "Genre",
            Self::Track => "Track",
            Self::PlayCount => "Play Count",
            Self::Path => "Path",
            Self::Filename => "Filename",
            Self::Codec => "Codec",
            Self::SampleRate => "Sample Rate",
            Self::BitsPerSample => "Bits Per Sample",
            Self::Bitrate => "Bitrate",
            Self::Status => "Status",
        }
    }
}

fn natural_compare(left: &str, right: &str) -> Ordering {
    let left = left.to_lowercase().chars().collect::<Vec<_>>();
    let right = right.to_lowercase().chars().collect::<Vec<_>>();
    let mut left_index = 0;
    let mut right_index = 0;

    while left_index < left.len() && right_index < right.len() {
        if left[left_index].is_ascii_digit() && right[right_index].is_ascii_digit() {
            let left_end = left[left_index..]
                .iter()
                .position(|character| !character.is_ascii_digit())
                .map_or(left.len(), |offset| left_index + offset);
            let right_end = right[right_index..]
                .iter()
                .position(|character| !character.is_ascii_digit())
                .map_or(right.len(), |offset| right_index + offset);
            let left_significant = left_index
                + left[left_index..left_end]
                    .iter()
                    .take_while(|character| **character == '0')
                    .count();
            let right_significant = right_index
                + right[right_index..right_end]
                    .iter()
                    .take_while(|character| **character == '0')
                    .count();
            let left_digits = &left[left_significant..left_end];
            let right_digits = &right[right_significant..right_end];

            match left_digits.len().cmp(&right_digits.len()) {
                Ordering::Equal => match left_digits.cmp(right_digits) {
                    Ordering::Equal => {}
                    ordering => return ordering,
                },
                ordering => return ordering,
            }
            left_index = left_end;
            right_index = right_end;
            continue;
        }

        match left[left_index].cmp(&right[right_index]) {
            Ordering::Equal => {
                left_index += 1;
                right_index += 1;
            }
            ordering => return ordering,
        }
    }

    (left.len() - left_index).cmp(&(right.len() - right_index))
}

/// Full path of a track for the path column: the file itself, or the outer
/// archive with nested members appended as folders. The outer join uses the
/// OS separator; member separators inside archives are always `/`.
fn track_path(track: &Track) -> String {
    if let Some(url) = &track.source.remote_url {
        return url.clone();
    }
    if let Some(origin) = &track.source.archive_origin {
        return format!(
            "{}{}{}",
            origin.archive_path.display(),
            std::path::MAIN_SEPARATOR,
            origin.entry_name
        );
    }
    track.source.path.display().to_string()
}

fn track_filename(track: &Track) -> String {
    let path = track
        .source
        .archive_origin
        .as_ref()
        .map(|origin| Path::new(&origin.entry_name))
        .unwrap_or(&track.source.path);
    path.file_name()
        .map(|filename| filename.to_string_lossy().into_owned())
        .unwrap_or_default()
}

fn sample_rate_label(sample_rate: Option<u32>) -> String {
    let Some(sample_rate) = sample_rate else {
        return String::new();
    };
    if sample_rate >= 1_000 {
        let kilohertz = f64::from(sample_rate) / 1_000.0;
        if sample_rate.is_multiple_of(1_000) {
            format!("{kilohertz:.0} kHz")
        } else {
            format!("{kilohertz:.1} kHz")
        }
    } else {
        format!("{sample_rate} Hz")
    }
}

fn compare_tracks(
    left: &Track,
    right: &Track,
    column: PlaylistSortColumn,
    starred: &HashSet<String>,
) -> Ordering {
    match column {
        PlaylistSortColumn::Index
        | PlaylistSortColumn::Rating
        | PlaylistSortColumn::PlayCount
        | PlaylistSortColumn::Status => Ordering::Equal,
        // Starred first when ascending: reversed boolean order so a click
        // on the star header groups favorites on top.
        PlaylistSortColumn::Star => starred
            .contains(&star_key_for_track(right))
            .cmp(&starred.contains(&star_key_for_track(left))),
        PlaylistSortColumn::Title => natural_compare(&left.title, &right.title),
        PlaylistSortColumn::AlbumArtist => natural_compare(&left.album_artist, &right.album_artist),
        PlaylistSortColumn::Artist => natural_compare(&left.artist, &right.artist),
        PlaylistSortColumn::Composer => natural_compare(&left.composer, &right.composer),
        PlaylistSortColumn::Album => natural_compare(&left.album, &right.album),
        PlaylistSortColumn::Length => left.duration.cmp(&right.duration),
        PlaylistSortColumn::Date => left.year.cmp(&right.year),
        PlaylistSortColumn::Genre => natural_compare(&left.genre, &right.genre),
        PlaylistSortColumn::Path => natural_compare(&track_path(left), &track_path(right)),
        PlaylistSortColumn::Filename => {
            natural_compare(&track_filename(left), &track_filename(right))
        }
        PlaylistSortColumn::Codec => natural_compare(&left.codec, &right.codec),
        PlaylistSortColumn::SampleRate => left.sample_rate.cmp(&right.sample_rate),
        PlaylistSortColumn::BitsPerSample => left.bits_per_sample.cmp(&right.bits_per_sample),
        PlaylistSortColumn::Bitrate => left.bitrate.cmp(&right.bitrate),
        PlaylistSortColumn::Track => {
            let left_album_artist = if left.album_artist.is_empty() {
                &left.artist
            } else {
                &left.album_artist
            };
            let right_album_artist = if right.album_artist.is_empty() {
                &right.artist
            } else {
                &right.album_artist
            };
            natural_compare(left_album_artist, right_album_artist)
                .then_with(|| natural_compare(&left.album, &right.album))
                .then_with(|| {
                    left.disc_number
                        .unwrap_or_default()
                        .cmp(&right.disc_number.unwrap_or_default())
                })
                .then_with(|| {
                    left.track_number
                        .unwrap_or_default()
                        .cmp(&right.track_number.unwrap_or_default())
                })
        }
    }
}

/// Song-level identity for stars: the radio locator plus the subsong
/// fragment when one exists, so two cue tracks from one file star
/// independently. Mirrors `playlist_entry_for_track` addressing.
fn star_key_for_track(track: &Track) -> String {
    let base = kog_audio::radio::radio_track_key(track);
    match playlist_entry_for_track(track) {
        Ok(entry) => match entry.fragment {
            Some(fragment) => format!("{base}#{fragment}"),
            None => base,
        },
        Err(_) => base,
    }
}

/// Full-fidelity database rows for tracks, skipping the unaddressable
/// ones and counting them for status lines.
fn collect_stored_entries(tracks: &[Track]) -> (Vec<kog_core::db::StoredEntry>, usize) {
    let mut entries = Vec::new();
    let mut skipped = 0_usize;
    for track in tracks {
        match stored_entry_for_track(track) {
            Some(entry) => entries.push(entry),
            None => skipped += 1,
        }
    }
    (entries, skipped)
}

/// Outcome of attempting playback at one pane index: Started plays or
/// keeps the existing stop-with-error behavior, while Skipped means the
/// file was verifiably gone and the caller should move on.
#[derive(Clone, Copy, PartialEq, Eq)]
enum PlayOutcome {
    Started,
    Skipped,
}

/// True when a playback source points at a file that is gone. Archives
/// only need their outer file, and remotes are never checked: transient
/// network failures must not read as missing files.
fn playback_source_is_missing(source: &PlaybackSource) -> bool {
    if source.remote_url.is_some() {
        return false;
    }
    if let Some(origin) = &source.archive_origin {
        return !origin.archive_path.exists();
    }
    !source.path.exists()
}

/// True when a stored entry points at a file that is gone. Archives
/// only need their outer file, and remotes are never checked: an
/// unmounted drive must not read as missing files.
fn stored_entry_is_missing(entry: &kog_core::db::StoredEntry) -> bool {
    use kog_core::db::{KIND_ARCHIVE, KIND_LOCAL};
    match entry.kind.as_str() {
        KIND_LOCAL | KIND_ARCHIVE => !std::path::Path::new(&entry.path).exists(),
        _ => false,
    }
}

/// Full-fidelity database row for a track, or None when the track cannot
/// be addressed on its own (e.g. a cue sheet entry without a track number).
/// Shared by starring and by saving panes and selections as playlists.
fn stored_entry_for_track(track: &Track) -> Option<kog_core::db::StoredEntry> {
    let entry = playlist_entry_for_track(track).ok()?;
    let (kind, path, name) = match entry.location {
        kog_audio::playlist::PlaylistLocation::Local(path) => (
            kog_core::db::KIND_LOCAL.to_owned(),
            path.to_string_lossy().into_owned(),
            String::new(),
        ),
        kog_audio::playlist::PlaylistLocation::Archive {
            archive_path,
            entry_name,
        } => (
            kog_core::db::KIND_ARCHIVE.to_owned(),
            archive_path.to_string_lossy().into_owned(),
            entry_name,
        ),
        kog_audio::playlist::PlaylistLocation::Remote(url) => {
            (kog_core::db::KIND_REMOTE.to_owned(), url, String::new())
        }
    };
    Some(kog_core::db::StoredEntry {
        kind,
        path,
        entry: name,
        fragment: entry.fragment,
    })
}

/// Playlist entries back out of database rows. Remote entries keep their
/// URL; anything malformed is left for the caller to skip and count.
fn playlist_entry_from_stored(entry: &kog_core::db::StoredEntry) -> Option<kog_audio::playlist::PlaylistEntry> {
    use kog_core::db::{KIND_ARCHIVE, KIND_LOCAL, KIND_REMOTE};
    use kog_audio::playlist::{PlaylistEntry, PlaylistLocation};
    let location = match entry.kind.as_str() {
        KIND_LOCAL => {
            if entry.path.is_empty() {
                return None;
            }
            PlaylistLocation::Local(PathBuf::from(&entry.path))
        }
        KIND_ARCHIVE => {
            if entry.path.is_empty() || entry.entry.is_empty() {
                return None;
            }
            PlaylistLocation::Archive {
                archive_path: PathBuf::from(&entry.path),
                entry_name: entry.entry.clone(),
            }
        }
        KIND_REMOTE => {
            if entry.path.is_empty() {
                return None;
            }
            PlaylistLocation::Remote(entry.path.clone())
        }
        _ => return None,
    };
    Some(PlaylistEntry {
        location,
        fragment: entry.fragment.clone(),
    })
}

fn sort_visible_indices(
    tracks: &[Track],
    visible_indices: &mut [usize],
    column: PlaylistSortColumn,
    ascending: bool,
    starred: &HashSet<String>,
) {
    if column == PlaylistSortColumn::Index {
        return;
    }
    visible_indices.sort_by(|left, right| {
        let ordering = compare_tracks(&tracks[*left], &tracks[*right], column, starred);
        if ascending {
            ordering
        } else {
            ordering.reverse()
        }
    });
}

/// A running API server: the shutdown channel plus what to show the user.
struct ApiServerHandle {
    shutdown: Option<tokio::sync::oneshot::Sender<()>>,
    address: std::net::SocketAddr,
    scheme: &'static str,
    /// Certificate clients should trust, when TLS is on.
    certificate_path: Option<std::path::PathBuf>,
}

pub struct AppControllerRust {
    playlist_count: i32,
    playlist_revision: i32,
    playlists_revision: i32,
    playlist_sort_column: QString,
    playlist_sort_ascending: bool,
    playlist_column_layout: QString,
    current_index: i32,
    playback_state: QString,
    mpris_raise_serial: i32,
    notification_serial: i32,
    audio_level_low: f64,
    audio_level_low_mid: f64,
    audio_level_mid: f64,
    audio_level_high_mid: f64,
    audio_level_high: f64,
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
    output_device_id: QString,
    output_devices_json: QString,
    output_device_status: QString,
    supported_formats_json: QString,
    shuffle_mode: QString,
    repeat_mode: QString,
    queue_count: i32,
    total_duration: QString,
    directory_path: QString,
    soundfont_path: QString,
    sc55_rom_path: QString,
    mt32_rom_path: QString,
    mt32_gm_program_mapping: bool,
    midi_engine: QString,
    midi_status: QString,
    opening_files_behavior: QString,
    read_cue_sheets_in_folders: bool,
    read_playlists_in_folders: bool,
    show_tray_icon: bool,
    close_to_tray: bool,
    minimize_to_tray: bool,
    track_notifications: bool,
    download_cover_art: bool,
    current_artwork_path: QString,
    radio_active: bool,
    radio: Option<RadioState>,
    cover_art: Option<CoverArtState>,
    cover_art_generation: u64,
    directory_scan_active: bool,
    directory_scan_files_scanned: i32,
    directory_scan_tracks_added: i32,
    directory_scan_current_path: QString,
    tree_delete_active: bool,
    tree_delete_done: i32,
    tree_delete_total: i32,
    tree_delete_current_path: QString,
    tree_delete_error: QString,
    equalizer_enabled: bool,
    equalizer_track_genre: bool,
    equalizer_preamp_db: f64,
    equalizer_preset: QString,
    equalizer_preset_names: QString,
    equalizer_revision: i32,
    tracks: Vec<Track>,
    visible_indices: Vec<usize>,
    sort_column: PlaylistSortColumn,
    playback_order: PlaybackOrder,
    filter: String,
    library_db: kog_core::db::LibraryDb,
    starred: HashSet<String>,
    directory: PathBuf,
    decoder_settings: DecoderSettings,
    decoders: DecoderRegistry,
    playback: PlaybackEngine,
    equalizer_settings: EqualizerSettings,
    directory_scan: Option<DirectoryScanState>,
    tree_delete: Option<TreeDeleteState>,
    mpris: MprisService,
    api_server: Option<ApiServerHandle>,
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
        .with_mt32_rom_path(app_settings.mt32_rom_path.clone())
        .with_mt32_gm_program_mapping(app_settings.mt32_gm_program_mapping);
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
        let playlist_column_layout = app_settings
            .playlist_column_layout
            .as_deref()
            .map(qstring)
            .unwrap_or_default();
        let midi_status = qstring(midi_status(
            app_settings.midi_engine,
            app_settings.soundfont_path.as_deref(),
            app_settings.sc55_rom_path.as_deref(),
            app_settings.mt32_rom_path.as_deref(),
        ));
        let equalizer_settings = app_settings.equalizer.clone();
        let shuffle_mode = app_settings.shuffle_mode;
        let repeat_mode = app_settings.repeat_mode;
        let available_output_devices = available_output_devices();
        let output_devices = available_output_devices.clone().unwrap_or_default();
        let requested_output_device = app_settings.output_device.clone();
        let selected_output_device = requested_output_device
            .as_ref()
            .and_then(|requested| resolve_output_device(&output_devices, requested));
        let remapped_output_device = requested_output_device
            .as_ref()
            .zip(selected_output_device.as_ref())
            .filter(|(requested, selected)| requested.id != selected.id);
        let mut output_device_status = match (&available_output_devices, &requested_output_device) {
            (Err(error), _) => error.clone(),
            (Ok(_), Some(requested)) if selected_output_device.is_none() => {
                format!(
                    "Saved audio output is unavailable; using the system default: {}",
                    requested.name
                )
            }
            (Ok(_), Some(_)) if remapped_output_device.is_some() => format!(
                "Using audio output: {} (matched the saved device name and refreshed its device ID)",
                selected_output_device
                    .as_ref()
                    .map(|device| device.label.as_str())
                    .unwrap_or("System Default Device")
            ),
            (Ok(_), Some(_)) => format!(
                "Using audio output: {}",
                selected_output_device
                    .as_ref()
                    .map(|device| device.label.as_str())
                    .unwrap_or("System Default Device")
            ),
            (Ok(_), None) => "Following the system default audio output".to_owned(),
        };
        if let Some((_, selected)) = remapped_output_device
            && let Err(error) = AppSettings::save_output_device(Some(&OutputDevicePreference {
                id: selected.id.clone(),
                name: selected.name.clone(),
            }))
        {
            output_device_status.push_str(&format!(
                "; the refreshed device ID could not be saved: {error}"
            ));
        }
        let decoders = DecoderRegistry::new(decoder_settings.clone());
        let supported_formats_json = qstring(decoders.supported_formats_json());
        let mut playback = PlaybackEngine::with_equalizer_and_output(
            DecoderRegistry::new(decoder_settings.clone()),
            equalizer_settings.clone(),
            selected_output_device
                .as_ref()
                .map(|device| device.id.clone()),
        );
        playback.set_volume(app_settings.output_volume as f32);
        let mut controller = Self {
            playlist_count: 0,
            playlist_revision: 0,
            playlists_revision: 0,
            playlist_sort_column: qstring(PlaylistSortColumn::Index.identifier()),
            playlist_sort_ascending: true,
            playlist_column_layout,
            current_index: -1,
            playback_state: qstring(PlaybackState::Stopped.as_str()),
            mpris_raise_serial: 0,
            notification_serial: 0,
            audio_level_low: 0.0,
            audio_level_low_mid: 0.0,
            audio_level_mid: 0.0,
            audio_level_high_mid: 0.0,
            audio_level_high: 0.0,
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
            output_device_id: selected_output_device
                .as_ref()
                .map(|device| qstring(&device.id))
                .unwrap_or_default(),
            output_devices_json: qstring(output_devices_json(&output_devices)),
            output_device_status: qstring(output_device_status),
            supported_formats_json,
            shuffle_mode: qstring(shuffle_mode.setting_value()),
            repeat_mode: qstring(repeat_mode.setting_value()),
            queue_count: 0,
            total_duration: qstring("Total duration: 0 seconds"),
            directory_path: qstring(directory.to_string_lossy()),
            soundfont_path,
            sc55_rom_path,
            mt32_rom_path,
            mt32_gm_program_mapping: app_settings.mt32_gm_program_mapping,
            midi_engine,
            midi_status,
            opening_files_behavior: qstring(app_settings.opening_files_behavior.setting_value()),
            read_cue_sheets_in_folders: app_settings.read_cue_sheets_in_folders,
            read_playlists_in_folders: app_settings.read_playlists_in_folders,
            show_tray_icon: app_settings.show_tray_icon,
            close_to_tray: app_settings.close_to_tray,
            minimize_to_tray: app_settings.minimize_to_tray,
            track_notifications: app_settings.track_notifications,
            download_cover_art: app_settings.download_cover_art,
            current_artwork_path: QString::default(),
            radio_active: false,
            radio: None,
            cover_art: None,
            cover_art_generation: 0,
            directory_scan_active: false,
            directory_scan_files_scanned: 0,
            directory_scan_tracks_added: 0,
            directory_scan_current_path: QString::default(),
            tree_delete_active: false,
            tree_delete_done: 0,
            tree_delete_total: 0,
            tree_delete_current_path: QString::default(),
            tree_delete_error: QString::default(),
            equalizer_enabled: equalizer_settings.enabled,
            equalizer_track_genre: equalizer_settings.track_genre,
            equalizer_preamp_db: f64::from(equalizer_settings.preamp_db),
            equalizer_preset: qstring(&equalizer_settings.preset_name),
            equalizer_preset_names: qstring(preset_names().join("\n")),
            equalizer_revision: 0,
            tracks: Vec::new(),
            visible_indices: Vec::new(),
            sort_column: PlaylistSortColumn::Index,
            playback_order: PlaybackOrder::new(
                shuffle_mode,
                repeat_mode,
                0x4b6f_672d_7368_7566 ^ u64::from(std::process::id()),
            ),
            filter: String::new(),
            library_db: kog_core::db::LibraryDb::open().unwrap_or_else(|_| {
                kog_core::db::LibraryDb::open_in_memory()
                    .expect("in-memory library database always opens")
            }),
            starred: HashSet::new(),
            directory,
            decoder_settings,
            decoders,
            playback,
            equalizer_settings,
            directory_scan: None,
            tree_delete: None,
            mpris: MprisService::default(),
            api_server: None,
        };

        // Startup restore for radio users: resume the persisted round so
        // picks continue instead of replaying openers, and the first pick
        // only needs a short descent. Skipped when repeat is on (radio and
        // repeat are mutually exclusive) or the folder is unavailable.
        if app_settings.radio_enabled {
            if app_settings.repeat_mode != RepeatMode::Off {
                let _ = AppSettings::save_radio_enabled(false);
            } else if controller.directory.is_dir() {
                let restored = load_radio_round(&controller.directory);
                let resumed = restored.is_some();
                let initial = restored.unwrap_or_else(|| {
                    kog_audio::radio::RoundInitial::fresh(kog_audio::radio::random_seed())
                });
                let staged = spawn_staging_worker(
                    controller.directory.clone(),
                    controller.decoder_settings.clone(),
                    controller.read_cue_sheets_in_folders,
                    initial,
                    blacklist_snapshot(&controller.library_db),
                )
                .ok();
                match staged {
                    None => {
                        let _ = AppSettings::save_radio_enabled(false);
                    }
                    Some((staging, dead, blacklist)) => {
                        controller.radio = Some(RadioState {
                            ready: VecDeque::new(),
                            deferred: VecDeque::new(),
                            staged_count: 0,
                            expand_jobs: Vec::new(),
                            dead,
                            blacklist,
                            staging: Some(staging),
                            kickstart_armed: false,
                            consecutive_dead: 0,
                        });
                        controller.radio_active = true;
                        controller.status = qstring(if resumed {
                            "Random Radio on — resumed"
                        } else {
                            "Random Radio on"
                        });
                    }
                }
            }
        }

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
            controller.playlist_count = saturating_i32(controller.visible_indices.len());
            controller.playlist_revision = 1;
            controller.total_duration = controller.total_duration_value();
            if open_result.added > 0 || open_result.warning.is_some() {
                controller.status = qstring(add_path_status(&open_result));
            }
        }
        {
            let (playback_order, tracks) = (&mut controller.playback_order, &controller.tracks);
            playback_order.tracks_changed(tracks, None);
        }
        // In-memory star cache for column display and star sorting; the
        // database stays the source of truth on every toggle.
        if let Ok(locators) = controller.library_db.starred_locators() {
            controller.starred.extend(locators);
        }
        controller.mpris.publish(mpris_snapshot(&controller));
        controller
    }
}

fn qstring(value: impl AsRef<str>) -> QString {
    QString::from(value.as_ref())
}

fn mpris_track_id(source_index: usize) -> String {
    format!("/org/kog/player/track/{source_index}")
}

fn mpris_snapshot(controller: &AppControllerRust) -> MprisSnapshot {
    let current_index = usize::try_from(controller.current_index)
        .ok()
        .filter(|index| *index < controller.tracks.len());
    let track = current_index.and_then(|index| controller.tracks.get(index));
    let has_tracks = !controller.tracks.is_empty();
    let playback_status = match controller.playback.state() {
        PlaybackState::Playing => MprisPlaybackStatus::Playing,
        PlaybackState::Paused => MprisPlaybackStatus::Paused,
        PlaybackState::Stopped => MprisPlaybackStatus::Stopped,
    };
    let loop_status = match controller.playback_order.repeat_mode() {
        RepeatMode::Off => MprisLoopStatus::None,
        RepeatMode::One => MprisLoopStatus::Track,
        RepeatMode::Album | RepeatMode::All => MprisLoopStatus::Playlist,
    };
    let url = track.and_then(|track| {
        track.source.remote_url.clone().or_else(|| {
            if track.source.archive_origin.is_some() {
                None
            } else {
                url::Url::from_file_path(&track.source.path)
                    .ok()
                    .map(Into::into)
            }
        })
    });

    MprisSnapshot {
        playback_status,
        loop_status,
        shuffle: controller.playback_order.shuffle_mode() != ShuffleMode::Off,
        volume: controller.volume,
        position_seconds: controller.position_seconds,
        duration_seconds: controller.duration_seconds,
        track_id: current_index.map(mpris_track_id),
        title: track.map(|track| track.title.clone()).unwrap_or_default(),
        artist: track.map(|track| track.artist.clone()).unwrap_or_default(),
        album_artist: track
            .map(|track| track.album_artist.clone())
            .unwrap_or_default(),
        album: track.map(|track| track.album.clone()).unwrap_or_default(),
        genre: track.map(|track| track.genre.clone()).unwrap_or_default(),
        composer: track
            .map(|track| track.composer.clone())
            .unwrap_or_default(),
        year: track.and_then(|track| track.year),
        disc_number: track.and_then(|track| track.disc_number),
        track_number: track.and_then(|track| track.track_number),
        url,
        can_go_next: has_tracks,
        can_go_previous: has_tracks,
        can_play: has_tracks,
        can_pause: track.is_some(),
        can_seek: track
            .is_some_and(|track| track.duration.is_some_and(|duration| !duration.is_zero())),
    }
}

fn output_devices_json(devices: &[OutputDevice]) -> String {
    serde_json::Value::Array(
        devices
            .iter()
            .map(|device| {
                serde_json::json!({
                    "id": device.id,
                    "label": device.label,
                    "isDefault": device.is_default,
                })
            })
            .collect(),
    )
    .to_string()
}

fn resolve_output_device(
    devices: &[OutputDevice],
    requested: &OutputDevicePreference,
) -> Option<OutputDevice> {
    devices
        .iter()
        .find(|device| device.id == requested.id)
        .or_else(|| devices.iter().find(|device| device.name == requested.name))
        .cloned()
}

fn saturating_i32(value: usize) -> i32 {
    i32::try_from(value).unwrap_or(i32::MAX)
}

fn valid_equalizer_gain(value: f64) -> Option<f32> {
    (value.is_finite() && (-20.0..=20.0).contains(&value)).then_some(value as f32)
}

const SKIN_EQ_FREQUENCIES: [f32; 10] = [
    60.0, 170.0, 310.0, 600.0, 1000.0, 3000.0, 6000.0, 12000.0, 14000.0, 16000.0,
];

#[test]
fn skin_equalizer_interpolates_in_log_frequency() {
    assert_eq!(interpolate_eq(&[1000.0, 2000.0], &[0.0, 10.0], 1000.0), 0.0);
    assert_eq!(
        interpolate_eq(&[1000.0, 2000.0], &[0.0, 10.0], 2000.0),
        10.0
    );
    assert!(
        (interpolate_eq(&[1000.0, 2000.0], &[0.0, 10.0], 1000.0 * 2.0_f32.sqrt()) - 5.0).abs()
            < 0.0001
    );
    assert_eq!(
        interpolate_eq(&[1000.0, 2000.0], &[0.0, 10.0], 2500.0),
        10.0
    );
    let mut gains = [0.0; 31];
    set_interpolated_eq_gain(
        &kog_core::equalizer::EQUALIZER_FREQUENCIES,
        &mut gains,
        14000.0,
        6.0,
    );
    assert!(
        (interpolate_eq(&kog_core::equalizer::EQUALIZER_FREQUENCIES, &gains, 14000.0) - 6.0).abs()
            < 0.0001
    );
    assert!(gains[28] > 0.0 && gains[29] > 0.0);
    assert_eq!(gains[27], 0.0);
}

// Read the native curve on a logarithmic frequency axis.
fn interpolate_eq(frequencies: &[f32], gains: &[f32], hz: f32) -> f32 {
    if hz <= frequencies[0] {
        return gains[0];
    }
    for i in 1..frequencies.len() {
        if hz <= frequencies[i] {
            let fraction =
                (hz / frequencies[i - 1]).ln() / (frequencies[i] / frequencies[i - 1]).ln();
            return gains[i - 1] + fraction * (gains[i] - gains[i - 1]);
        }
    }
    gains[gains.len() - 1]
}

// Project a skin control's requested gain onto its two neighboring native
// bands. Sampling a ten-band curve only at native centers would otherwise
// drop the 14 kHz control entirely (Kog has 12 kHz and 16 kHz centers).
fn set_interpolated_eq_gain(frequencies: &[f32], gains: &mut [f32], hz: f32, target: f32) {
    if hz <= frequencies[0] {
        gains[0] = target;
        return;
    }
    for i in 1..frequencies.len() {
        if hz <= frequencies[i] {
            let right = (hz / frequencies[i - 1]).ln() / (frequencies[i] / frequencies[i - 1]).ln();
            let left = 1.0 - right;
            let delta =
                (target - (left * gains[i - 1] + right * gains[i])) / (left * left + right * right);
            gains[i - 1] = (gains[i - 1] + left * delta).clamp(-20.0, 20.0);
            gains[i] = (gains[i] + right * delta).clamp(-20.0, 20.0);
            return;
        }
    }
    gains[gains.len() - 1] = target;
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
    fn add_path(&mut self, path: PathBuf) -> Result<AddPathResult, String> {
        let path = canonical_path(&path)?;
        if !path.is_file() {
            return Err(format!("{} is not a playable file", path.display()));
        }
        let expansion = self.decoders.expand_detailed(path)?;
        Ok(self.add_expansion(expansion))
    }

    fn add_remote_url(&mut self, url: &str) -> Result<AddPathResult, String> {
        let expansion = self.decoders.expand_remote_url(url)?;
        Ok(self.add_expansion(expansion))
    }

    fn add_expansion(&mut self, expansion: ExpansionResult) -> AddPathResult {
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
        result
    }

    fn add_directory(&mut self, directory: &Path) -> Result<AddPathResult, String> {
        let directory = canonical_path(directory)?;
        if !directory.is_dir() {
            return Err(format!("{} is not a folder", directory.display()));
        }

        let mut result = AddPathResult::default();
        for path in ordered_directory_files(&directory)? {
            match self.add_scanned_file(path) {
                Ok(added) => {
                    result.added += added.added;
                    if let Some(warning) = added.warning {
                        result.push_warning(warning);
                    }
                }
                Err(error) => result.push_warning(error),
            }
        }
        Ok(result)
    }

    fn add_scanned_file(&mut self, path: PathBuf) -> Result<AddPathResult, String> {
        if !self.decoders.accepts_path(&path) {
            return Ok(AddPathResult::default());
        }
        let extension = path
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or_default();
        if extension.eq_ignore_ascii_case("cue") && !self.read_cue_sheets_in_folders {
            return Ok(AddPathResult::default());
        }
        if matches!(
            extension.to_ascii_lowercase().as_str(),
            "m3u" | "m3u8" | "pls"
        ) && !self.read_playlists_in_folders
        {
            return Ok(AddPathResult::default());
        }
        self.add_path(path)
    }

    fn rebuild_visible_indices(&mut self) {
        self.visible_indices = self
            .tracks
            .iter()
            .enumerate()
            .filter(|(_, track)| track.matches(&self.filter))
            .map(|(index, _)| index)
            .collect();
        if self.sort_column != PlaylistSortColumn::Index {
            sort_visible_indices(
                &self.tracks,
                &mut self.visible_indices,
                self.sort_column,
                self.playlist_sort_ascending,
                &self.starred,
            );
        }
    }

    fn sync_tracks_in_playback_order(&mut self) -> usize {
        let current = usize::try_from(self.current_index).ok();
        self.playback_order.tracks_changed(&self.tracks, current);
        self.playback_order.queue_count()
    }

    fn total_duration_value(&self) -> QString {
        let duration = self
            .tracks
            .iter()
            .filter_map(|track| track.duration)
            .fold(Duration::ZERO, |total, duration| total + duration);
        qstring(total_duration_label(duration))
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

fn selected_sources(model: &qobject::AppController, indices: &str) -> Vec<PlaybackSource> {
    selected_source_indices(model, indices)
        .into_iter()
        .filter_map(|source_index| model.rust().tracks.get(source_index))
        .map(|track| track.source.clone())
        .collect()
}

fn selected_source_indices(model: &qobject::AppController, indices: &str) -> Vec<usize> {
    parse_row_indices(indices, model.rust().visible_indices.len())
        .into_iter()
        .filter_map(|row| model.rust().visible_indices.get(row).copied())
        .collect()
}

const fn selection_state_name(state: SelectionState) -> &'static str {
    match state {
        SelectionState::None => "none",
        SelectionState::Mixed => "mixed",
        SelectionState::All => "all",
    }
}

/// Best-effort list of this machine's reachable IPv4 addresses, so the
/// Preferences pane can show a URL a phone can actually open. Uses the
/// UDP-connect trick: no packets are sent, it just asks the routing table which
/// local address would be used.
fn local_ip_addresses() -> Result<Vec<std::net::IpAddr>, String> {
    let mut addresses = Vec::new();
    for probe in ["8.8.8.8:80", "1.1.1.1:80"] {
        let Ok(socket) = std::net::UdpSocket::bind("0.0.0.0:0") else {
            continue;
        };
        if socket.connect(probe).is_ok()
            && let Ok(local) = socket.local_addr()
        {
            let ip = local.ip();
            if !ip.is_loopback() && !addresses.contains(&ip) {
                addresses.push(ip);
            }
        }
    }
    if addresses.is_empty() {
        return Err("no network address was found".to_owned());
    }
    Ok(addresses)
}

fn json_result(result: Result<serde_json::Value, String>) -> QString {
    qstring(
        result
            .unwrap_or_else(|error| {
                serde_json::json!({
                    "ok": false,
                    "error": error,
                })
            })
            .to_string(),
    )
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
        let behavior = OpeningFilesBehavior::from_setting(
            &self.as_ref().rust().opening_files_behavior.to_string(),
        )
        .unwrap_or_default();
        self.as_mut().add_local_paths(paths, behavior);
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

    pub fn save_playlist(mut self: Pin<&mut Self>) {
        let rows = (0..self.as_ref().rust().visible_indices.len()).collect();
        self.as_mut().save_playlist_rows(rows, false);
    }

    pub fn save_playlist_selection(mut self: Pin<&mut Self>, indices: QString) {
        let rows = parse_row_indices(
            &indices.to_string(),
            self.as_ref().rust().visible_indices.len(),
        );
        if rows.is_empty() {
            self.as_mut()
                .set_status(qstring("Select at least one playlist track to save"));
            return;
        }
        self.as_mut().save_playlist_rows(rows, true);
    }

    fn save_playlist_rows(mut self: Pin<&mut Self>, rows: Vec<usize>, selection: bool) {
        if rows.is_empty() {
            self.as_mut()
                .set_status(qstring("There are no playlist tracks to save"));
            return;
        }
        let entries = {
            let model_ref = self.as_ref();
            let model = model_ref.rust();
            rows.iter()
                .map(|row| {
                    let source_index = model
                        .visible_indices
                        .get(*row)
                        .ok_or_else(|| format!("Playlist row {} no longer exists", row + 1))?;
                    let track = model.tracks.get(*source_index).ok_or_else(|| {
                        format!("Playlist source {} no longer exists", source_index + 1)
                    })?;
                    playlist_entry_for_track(track)
                })
                .collect::<Result<Vec<_>, String>>()
        };
        let entries = match entries {
            Ok(entries) => entries,
            Err(error) => {
                self.as_mut().set_status(qstring(error));
                return;
            }
        };
        let directory = self.as_ref().rust().directory.clone();
        let file_name = if selection {
            "selection.m3u"
        } else {
            "playlist.m3u"
        };
        let title = if selection {
            "Save Selection As Playlist"
        } else {
            "Save Playlist As"
        };
        let Some(path) = rfd::FileDialog::new()
            .set_title(title)
            .set_directory(directory)
            .set_file_name(file_name)
            .add_filter("M3U Playlist", &["m3u", "m3u8"])
            .add_filter("PLS Playlist", &["pls"])
            .save_file()
        else {
            return;
        };
        let path = match normalize_playlist_save_path(path) {
            Ok(path) => path,
            Err(error) => {
                self.as_mut().set_status(qstring(error));
                return;
            }
        };
        match Playlist::save(&path, &entries) {
            Ok(()) => self.as_mut().set_status(qstring(format!(
                "Saved {} {} to {}",
                entries.len(),
                if entries.len() == 1 {
                    "track"
                } else {
                    "tracks"
                },
                path.display()
            ))),
            Err(error) => self.as_mut().set_status(qstring(error)),
        }
    }

    pub fn add_file(mut self: Pin<&mut Self>, url: QUrl) {
        let Some(local_file) = url.to_local_file() else {
            self.as_mut()
                .set_status(qstring("Only local files can be added"));
            return;
        };
        self.as_mut().add_local_paths(
            vec![PathBuf::from(local_file.to_string())],
            OpeningFilesBehavior::Enqueue,
        );
    }

    pub fn activate_file(mut self: Pin<&mut Self>, url: QUrl) {
        let Some(local_file) = url.to_local_file() else {
            self.as_mut()
                .set_status(qstring("Only local files can be opened"));
            return;
        };
        let behavior = OpeningFilesBehavior::from_setting(
            &self.as_ref().rust().opening_files_behavior.to_string(),
        )
        .unwrap_or_default();
        self.as_mut()
            .add_local_paths(vec![PathBuf::from(local_file.to_string())], behavior);
    }

    pub fn add_local_path(mut self: Pin<&mut Self>, path: QString) {
        self.as_mut().add_local_paths(
            vec![PathBuf::from(path.to_string())],
            OpeningFilesBehavior::Enqueue,
        );
    }

    pub fn activate_local_path(mut self: Pin<&mut Self>, path: QString) {
        let behavior = OpeningFilesBehavior::from_setting(
            &self.as_ref().rust().opening_files_behavior.to_string(),
        )
        .unwrap_or_default();
        self.as_mut()
            .add_local_paths(vec![PathBuf::from(path.to_string())], behavior);
    }

    pub fn add_local_paths_json(mut self: Pin<&mut Self>, paths: QString) {
        let paths = match local_paths_from_json(&paths.to_string()) {
            Ok(paths) => paths,
            Err(error) => {
                self.as_mut().set_status(qstring(error));
                return;
            }
        };
        self.as_mut()
            .add_local_paths(paths, OpeningFilesBehavior::Enqueue);
    }

    pub fn activate_local_paths_json(mut self: Pin<&mut Self>, paths: QString) {
        let paths = match local_paths_from_json(&paths.to_string()) {
            Ok(paths) => paths,
            Err(error) => {
                self.as_mut().set_status(qstring(error));
                return;
            }
        };
        let behavior = OpeningFilesBehavior::from_setting(
            &self.as_ref().rust().opening_files_behavior.to_string(),
        )
        .unwrap_or_default();
        self.as_mut().add_local_paths(paths, behavior);
    }

    pub fn set_radio_enabled(mut self: Pin<&mut Self>, enabled: bool) {
        if enabled == self.as_ref().rust().radio_active {
            return;
        }
        if !enabled {
            self.as_mut().teardown_radio();
            if let Err(error) = AppSettings::save_radio_enabled(false) {
                self.as_mut().set_status(qstring(error));
                return;
            }
            self.as_mut().set_status(qstring("Random Radio off"));
            return;
        }
        if let Err(error) = AppSettings::save_radio_enabled(true) {
            self.as_mut().set_status(qstring(error));
            return;
        }
        self.as_mut().apply_repeat_mode(RepeatMode::Off);
        let root = self.as_ref().rust().directory.clone();
        if !root.is_dir() {
            self.as_mut().set_status(qstring(format!(
                "Music folder {} is unavailable",
                root.display()
            )));
            return;
        }
        // Manual toggle-on resumes the persisted round when one waits, so
        // positions (and the no-repeat promise) survive toggling. The Reset
        // control is the explicit reshuffle action, not this toggle.
        // Either way nothing autoplays before play.
        let (initial, resumed) = match load_radio_round(&root) {
            Some(initial) => (initial, true),
            None => (
                kog_audio::radio::RoundInitial::fresh(kog_audio::radio::random_seed()),
                false,
            ),
        };
        self.as_mut().begin_radio_session(
            root,
            initial,
            if resumed {
                "Random Radio on — resumed".to_owned()
            } else {
                "Random Radio on".to_owned()
            },
        );
    }

    /// Blacklist snapshot from the library store for radio staging:
    /// songs as locator keys, folders as paths.
    fn load_blacklist_snapshot(&self) -> kog_audio::radio::Blacklist {
        blacklist_snapshot(&self.rust().library_db)
    }

    /// Refresh the running staging thread's blacklist after menu edits.
    fn refresh_radio_blacklist(mut self: Pin<&mut Self>) {
        let snapshot = self.as_ref().load_blacklist_snapshot();
        if let Some(radio) = self.as_mut().rust_mut().radio.as_mut() {
            *radio
                .blacklist
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner()) = snapshot;
        }
    }

    /// Spawn the staging worker and install fresh radio state for `root`,
    /// clearing any ready buffer, deferred tracks, and in-flight jobs from
    /// the previous folder. Callers choose the round (resumed or fresh)
    /// and the status line.
    fn begin_radio_session(
        mut self: Pin<&mut Self>,
        root: PathBuf,
        initial: kog_audio::radio::RoundInitial,
        status: String,
    ) {
        let blacklist = self.as_ref().load_blacklist_snapshot();
        let staging = {
            let rust = self.as_ref();
            spawn_staging_worker(
                root,
                rust.rust().decoder_settings.clone(),
                rust.rust().read_cue_sheets_in_folders,
                initial,
                blacklist,
            )
        };
        let (staging, dead, blacklist) = match staging {
            Ok(staging) => staging,
            Err(error) => {
                self.as_mut().set_status(qstring(error));
                return;
            }
        };
        self.as_mut().rust_mut().radio = Some(RadioState {
            ready: VecDeque::new(),
            deferred: VecDeque::new(),
            staged_count: 0,
            expand_jobs: Vec::new(),
            dead,
            blacklist,
            staging: Some(staging),
            // Never autoplay on toggle: staging fills the hidden buffer,
            // and the first track moves (and plays) only after an explicit
            // play/next press or a natural track end.
            kickstart_armed: false,
            consecutive_dead: 0,
        });
        self.as_mut().set_radio_active(true);
        self.as_mut().set_status(qstring(status));
    }

    /// Fresh shuffle for the running radio session: new seed and cleared
    /// positions, keeping unplayability knowledge. Radio turns on if off.
    pub fn reshuffle_radio(mut self: Pin<&mut Self>) {
        let root = self.as_ref().rust().directory.clone();
        if !root.is_dir() {
            self.as_mut().set_status(qstring(format!(
                "Music folder {} is unavailable",
                root.display()
            )));
            return;
        }
        if let Err(error) = AppSettings::save_radio_enabled(true) {
            self.as_mut().set_status(qstring(error));
            return;
        }
        self.as_mut().apply_repeat_mode(RepeatMode::Off);
        self.as_mut().teardown_radio();
        let dead: Vec<String> = load_radio_round(&root)
            .map(|loaded| loaded.dead)
            .unwrap_or_default();
        let initial = kog_audio::radio::RoundInitial {
            seed: kog_audio::radio::random_seed(),
            counter: 0,
            cursors: HashMap::new(),
            dead,
        };
        self.as_mut().begin_radio_session(
            root,
            initial,
            "Random Radio — fresh shuffle".to_owned(),
        );
    }

    pub fn poll_radio(mut self: Pin<&mut Self>) {
        if !self.as_ref().rust().radio_active {
            return;
        }
        let mut events = Vec::new();
        // Finished lanes with the locator they expanded: empty completions
        // prove the pick unplayable and join the shared dead set.
        let mut finished: Vec<(usize, Option<String>)> = Vec::new();
        if let Some(radio) = self.as_ref().rust().radio.as_ref() {
            for (index, job) in radio.expand_jobs.iter().enumerate() {
                // Expand workers send exactly once, so a first event and a
                // disconnect both mean that lane is done.
                let mut done = false;
                let mut empty = false;
                while events.len() < 16 {
                    match job.receiver.try_recv() {
                        Ok(event) => {
                            empty = matches!(&event, RadioEvent::TracksReady { tracks, .. } if tracks.is_empty());
                            events.push(event);
                            done = true;
                            break;
                        }
                        Err(TryRecvError::Empty) => break,
                        Err(TryRecvError::Disconnected) => {
                            done = true;
                            break;
                        }
                    }
                }
                if done {
                    let key = empty.then(|| kog_audio::radio::radio_locator_key(&job.locator));
                    finished.push((index, key));
                }
            }
        } else {
            self.as_mut().set_radio_active(false);
            return;
        }
        for event in events {
            self.as_mut().handle_radio_event(event);
        }
        if let Some(radio) = self.as_mut().rust_mut().radio.as_mut() {
            for (index, key) in finished.into_iter().rev() {
                if let Some(dead) = key {
                    radio
                        .dead
                        .lock()
                        .unwrap_or_else(|poisoned| poisoned.into_inner())
                        .insert(dead);
                }
                if index < radio.expand_jobs.len() {
                    radio.expand_jobs.remove(index);
                }
            }
        }
        let kickstart = self
            .as_ref()
            .rust()
            .radio
            .as_ref()
            .is_some_and(|radio| radio.kickstart_armed)
            && self.as_ref().rust().tracks.is_empty()
            && self
                .as_ref()
                .rust()
                .radio
                .as_ref()
                .is_some_and(|radio| !radio.ready.is_empty());
        if kickstart {
            if let Some(index) = self.as_mut().shift_radio_track() {
                self.as_mut().play_shifted_radio_track(index);
            }
            if let Some(radio) = self.as_mut().rust_mut().radio.as_mut() {
                radio.kickstart_armed = false;
            }
        }
        self.as_mut().top_up_radio();
    }

    fn teardown_radio(mut self: Pin<&mut Self>) {
        if let Some(state) = self.as_mut().rust_mut().radio.take() {
            for job in state.expand_jobs {
                job.cancel.store(true, AtomicOrdering::Relaxed);
            }
            if let Some(staging) = state.staging {
                staging.cancel.store(true, AtomicOrdering::Relaxed);
            }
        }
        self.as_mut().set_radio_active(false);
    }

    fn request_radio_expand(mut self: Pin<&mut Self>, locator: PathBuf) {
        let (worker_decoders, read_cue, read_playlists) = {
            let this = self.as_ref();
            let rust = this.rust();
            (
                rust.decoders
                    .background_worker(rust.decoder_settings.clone()),
                rust.read_cue_sheets_in_folders,
                rust.read_playlists_in_folders,
            )
        };
        let (sender, receiver) = std::sync::mpsc::sync_channel(4);
        let cancel = Arc::new(AtomicBool::new(false));
        let job = RadioJob {
            receiver,
            cancel: Arc::clone(&cancel),
            locator: locator.clone(),
            started: Instant::now(),
        };
        let spawned = std::thread::Builder::new()
            .name("kog-radio-expand".to_owned())
            .spawn(move || {
                if cancel.load(AtomicOrdering::Relaxed) {
                    return;
                }
                // A panicking pick must free its lane instead of wedging
                // staging forever: catch it and report a skipped pick.
                let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    let prepared = prepare_scan_file(
                        locator.clone(),
                        &worker_decoders,
                        read_cue,
                        read_playlists,
                        false,
                    );
                    // Probe-open every candidate: only tracks that actually
                    // open reach the staging buffer, so an unplayable file can
                    // never surface as a playback error later.
                    let mut tracks = prepared.tracks;
                    let mut warnings = prepared.warnings;
                    if !cancel.load(AtomicOrdering::Relaxed) && !tracks.is_empty() {
                        tracks.retain(|track| match worker_decoders.probe(&track.source) {
                            Ok(_) => true,
                            Err(error) => {
                                warnings.push(format!(
                                    "{} cannot be played and was skipped: {error}",
                                    track.source.display_label()
                                ));
                                false
                            }
                        });
                    }
                    (tracks, warnings)
                }));
                match outcome {
                    Ok((tracks, warnings)) => {
                        let _ = sender.send(RadioEvent::TracksReady {
                            locator,
                            tracks,
                            warnings,
                        });
                    }
                    Err(_) => {
                        let _ = sender.send(RadioEvent::TracksReady {
                            locator,
                            tracks: Vec::new(),
                            warnings: vec![
                                "A radio pick failed unexpectedly and was skipped".to_owned(),
                            ],
                        });
                    }
                }
            })
            .is_ok();
        if spawned {
            if let Some(radio) = self.as_mut().rust_mut().radio.as_mut() {
                radio.expand_jobs.push(job);
            }
        }
    }

    fn handle_radio_event(mut self: Pin<&mut Self>, event: RadioEvent) {
        let RadioEvent::TracksReady {
            locator, tracks, ..
        } = event;
        // Multi-song single files (NSF and friends) expand to one track per
        // subsong. Stage a single shuffled pick now and defer the rest for
        // spaced staging, instead of queueing the whole file back-to-back.
        // Anything without uniform subsong indices stages whole, as before.
        let mut staged = tracks;
        let mut deferred = Vec::new();
        if staged.len() > 1 {
            let indices: Vec<u32> = staged
                .iter()
                .filter_map(|track| track.source.subsong)
                .collect();
            // Every track indexed, or nobody is: a partial set would orphan
            // the unindexed tracks, so those stage whole as before.
            if indices.len() == staged.len() {
                if let Some(order) = kog_audio::radio::shuffle_subsong_order(
                    &kog_audio::radio::radio_locator_key(&locator),
                    &indices,
                ) {
                    let mut ordered = Vec::with_capacity(staged.len());
                    for index in order {
                        let position = staged
                            .iter()
                            .position(|track| track.source.subsong == Some(index))
                            .expect("shuffled index comes from these tracks");
                        ordered.push(staged.remove(position));
                    }
                    deferred = ordered.split_off(1);
                    staged = ordered;
                }
            }
        }
        let live = !staged.is_empty();
        if let Some(radio) = self.as_mut().rust_mut().radio.as_mut() {
            radio.ready.extend(staged);
            radio.deferred.extend(deferred);
            if live {
                radio.consecutive_dead = 0;
            } else {
                // Dead picks (unlistable archives, over-claimed extensions,
                // unopenable files) are routine in big libraries: staging
                // flows past them silently. Only a long barren run reports,
                // so a misconfigured folder does not fail silently forever.
                radio.consecutive_dead = radio.consecutive_dead.saturating_add(1);
                if radio.consecutive_dead == 25 {
                    self.as_mut().set_status(qstring(
                        "Random Radio — no playable files found under the music folder",
                    ));
                }
            }
        }
    }

    /// Move one staged radio track into the playlist. Returns its new index.
    /// Callers play it immediately (kickstart, end-of-playlist advance).
    fn shift_radio_track(mut self: Pin<&mut Self>) -> Option<usize> {
        let track = self
            .as_mut()
            .rust_mut()
            .radio
            .as_mut()?
            .ready
            .pop_front()?;
        let index = self.as_ref().rust().tracks.len();
        self.as_mut().rust_mut().tracks.push(track);
        self.as_mut().refresh_playback_order();
        self.as_mut().rebuild_playlist();
        Some(index)
    }

    /// Keep the hidden staging buffer filled from the background staging
    /// thread: take staged locators off the channel and expand the first
    /// ones that are not already queued or proven dead. Up to three
    /// expansions fly at once so one slow archive cannot wedge staging;
    /// lanes that overrun their timeout join the shared dead set instead of
    /// blocking forever. Everything here is channel drains and key lookups;
    /// picking itself never runs on the UI thread.
    fn top_up_radio(mut self: Pin<&mut Self>) {
        const RADIO_READY_TARGET: usize = 10;
        const MAX_EXPANDERS: usize = 3;
        const MAX_DRAIN: usize = 128;
        const EXPAND_TIMEOUT: Duration = Duration::from_secs(300);
        let Some(_) = self.as_ref().rust().radio.as_ref() else {
            return;
        };
        // Retire lanes that overran: best-effort cancel, and their locators
        // join the shared dead set so no turn is spent on them again.
        let now = Instant::now();
        let mut timed_out = Vec::new();
        if let Some(radio) = self.as_mut().rust_mut().radio.as_mut() {
            let mut kept = Vec::new();
            for job in radio.expand_jobs.drain(..) {
                if now.duration_since(job.started) > EXPAND_TIMEOUT {
                    job.cancel.store(true, AtomicOrdering::Relaxed);
                    timed_out.push(kog_audio::radio::radio_locator_key(&job.locator));
                } else {
                    kept.push(job);
                }
            }
            radio.expand_jobs = kept;
            if !timed_out.is_empty() {
                let mut dead = radio
                    .dead
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
                for key in &timed_out {
                    dead.insert(key.clone());
                }
            }
        }
        if !timed_out.is_empty() {
            self.as_mut()
                .set_status(qstring("Random Radio — skipping an unresponsive file"));
        }
        let dead: Vec<String> = self
            .as_ref()
            .rust()
            .radio
            .as_ref()
            .map(|radio| {
                radio
                    .dead
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner())
                    .iter()
                    .cloned()
                    .collect()
            })
            .unwrap_or_default();
        let staged: HashSet<String> = self
            .as_ref()
            .rust()
            .tracks
            .iter()
            .map(kog_audio::radio::radio_track_key)
            .chain(
                self.as_ref()
                    .rust()
                    .radio
                    .as_ref()
                    .map(|radio| radio.ready.iter().map(kog_audio::radio::radio_track_key))
                    .into_iter()
                    .flatten(),
            )
            .chain(dead)
            .collect();
        let mut budget = MAX_DRAIN;
        loop {
            let (ready_len, active) = self
                .as_ref()
                .rust()
                .radio
                .as_ref()
                .map(|radio| (radio.ready.len(), radio.expand_jobs.len()))
                .unwrap_or((RADIO_READY_TARGET, MAX_EXPANDERS));
            if ready_len + active >= RADIO_READY_TARGET || active >= MAX_EXPANDERS {
                return;
            }
            // Spaced subsong staging: every SUBSONG_EVERY-th staging serves
            // the deferred queue instead of fresh picks, so one multi-song
            // file spreads across the session rather than playing back to
            // back. Deferred servings count like fresh ones below.
            const SUBSONG_EVERY: u64 = 4;
            let serve_deferred = self.as_ref().rust().radio.as_ref().is_some_and(|radio| {
                !radio.deferred.is_empty()
                    && radio.staged_count % SUBSONG_EVERY == SUBSONG_EVERY - 1
            });
            if serve_deferred {
                let mut this = self.as_mut();
                let mut rust = this.as_mut().rust_mut();
                if let Some(radio) = rust.radio.as_mut() {
                    if radio.ready.len() < RADIO_READY_TARGET {
                        if let Some(track) = radio.deferred.pop_front() {
                            radio.ready.push_back(track);
                            radio.staged_count += 1;
                        }
                    }
                }
                continue;
            }
            enum Drain {
                Pick(PathBuf),
                Empty,
                Barren,
                Dry,
                Gone,
            }
            let mut outcome = Drain::Dry;
            while budget > 0 {
                budget -= 1;
                let response = {
                    let mut this = self.as_mut();
                    let mut rust = this.as_mut().rust_mut();
                    let Some(radio) = rust.radio.as_mut() else {
                        return;
                    };
                    let Some(staging) = radio.staging.as_ref() else {
                        return;
                    };
                    staging.picks.try_recv()
                };
                match response {
                    Ok(kog_audio::radio::StagingResponse::Pick(locator)) => {
                        if staged.contains(&kog_audio::radio::radio_locator_key(&locator)) {
                            continue;
                        }
                        outcome = Drain::Pick(locator);
                        break;
                    }
                    Ok(kog_audio::radio::StagingResponse::Empty) => {
                        outcome = Drain::Empty;
                        break;
                    }
                    Ok(kog_audio::radio::StagingResponse::Barren) => {
                        outcome = Drain::Barren;
                        break;
                    }
                    Err(TryRecvError::Empty) => break,
                    Err(TryRecvError::Disconnected) => {
                        outcome = Drain::Gone;
                        break;
                    }
                }
            }
            match outcome {
                Drain::Pick(locator) => {
                    self.as_mut().request_radio_expand(locator);
                    if let Some(radio) = self.as_mut().rust_mut().radio.as_mut() {
                        radio.staged_count += 1;
                    }
                }
                Drain::Empty => {
                    // The worker exited after reporting: drop the handle so
                    // later polls do not overwrite this with a disconnect note.
                    if let Some(radio) = self.as_mut().rust_mut().radio.as_mut() {
                        radio.staging = None;
                    }
                    self.as_mut()
                        .set_status(qstring("Random Radio — music folder is empty"));
                    return;
                }
                Drain::Barren => {
                    // Everything reachable already failed: park with the dead
                    // set kept, so toggling is the way back if files appear.
                    if let Some(radio) = self.as_mut().rust_mut().radio.as_mut() {
                        radio.staging = None;
                    }
                    self.as_mut().set_status(qstring(
                        "Random Radio — no playable files found under the music folder",
                    ));
                    return;
                }
                Drain::Dry => return,
                Drain::Gone => {
                    // The staging thread is gone: park radio visibly instead
                    // of stalling silently. Toggling restarts it.
                    if let Some(radio) = self.as_mut().rust_mut().radio.as_mut() {
                        radio.staging = None;
                    }
                    self.as_mut().set_status(qstring(
                        "Random Radio worker stopped — toggle radio to restart",
                    ));
                    return;
                }
            }
        }
    }

    pub fn poll_directory_scan(mut self: Pin<&mut Self>) {
        if !self.as_ref().rust().directory_scan_active {
            return;
        }

        let cancel_requested = self
            .as_ref()
            .rust()
            .directory_scan
            .as_ref()
            .is_some_and(|scan| scan.cancel_requested);
        let limit = if cancel_requested { 256 } else { 128 };
        let mut events = Vec::new();
        let mut disconnected = false;
        if let Some(scan) = self.as_ref().rust().directory_scan.as_ref() {
            for _ in 0..limit {
                match scan.receiver.try_recv() {
                    Ok(event) => {
                        let complete = matches!(event, DirectoryScanEvent::Complete { .. });
                        events.push(event);
                        if complete {
                            break;
                        }
                    }
                    Err(TryRecvError::Empty) => break,
                    Err(TryRecvError::Disconnected) => {
                        disconnected = true;
                        break;
                    }
                }
            }
        }

        let mut completion = None;
        let mut scanned_increment = 0_i32;
        let mut last_scanned_path = None;
        let mut last_added_count = None;
        // Staged playlist imports live here: duplicates are legitimate
        // playlist content, so they bypass the known-source filter below.
        // Compare canonical paths since prepared paths are canonicalized.
        let staged_dir = kog_audio::track::canonical_path(&playlist_cache_dir())
            .unwrap_or_else(|_| playlist_cache_dir());
        for event in events {
            match event {
                DirectoryScanEvent::Prepared(prepared) => {
                    let skip = self
                        .as_ref()
                        .rust()
                        .directory_scan
                        .as_ref()
                        .is_none_or(|scan| scan.cancel_requested);
                    if skip {
                        continue;
                    }
                    scanned_increment = scanned_increment.saturating_add(1);
                    let staged_import = prepared.path.starts_with(&staged_dir);
                    last_scanned_path = Some(prepared.path);
                    let (new_tracks, added_count) = {
                        let mut rust = self.as_mut().rust_mut();
                        let Some(scan) = rust.directory_scan.as_mut() else {
                            continue;
                        };
                        let new_tracks = prepared
                            .tracks
                            .into_iter()
                            .filter(|track| {
                                staged_import || scan.known_sources.insert(track.source.clone())
                            })
                            .collect::<Vec<_>>();
                        let newly_added = new_tracks.len();
                        scan.combined.added += newly_added;
                        scan.playlist_dirty |= newly_added > 0;
                        for warning in prepared.warnings {
                            scan.combined.push_warning(warning);
                        }
                        (new_tracks, scan.combined.added)
                    };
                    if !new_tracks.is_empty() {
                        self.as_mut().rust_mut().tracks.extend(new_tracks);
                    }
                    last_added_count = Some(added_count);
                }
                DirectoryScanEvent::Warning(warning) => {
                    if let Some(scan) = self.as_mut().rust_mut().directory_scan.as_mut() {
                        scan.combined.push_warning(warning);
                    }
                }
                DirectoryScanEvent::Complete { cancelled } => completion = Some(cancelled),
            }
        }

        if scanned_increment > 0 {
            let scanned = self
                .as_ref()
                .rust()
                .directory_scan_files_scanned
                .saturating_add(scanned_increment);
            self.as_mut().set_directory_scan_files_scanned(scanned);
        }
        if let Some(path) = last_scanned_path {
            self.as_mut()
                .set_directory_scan_current_path(qstring(path.to_string_lossy()));
        }
        if let Some(added_count) = last_added_count {
            self.as_mut()
                .set_directory_scan_tracks_added(saturating_i32(added_count));
        }
        if disconnected && completion.is_none() {
            if !cancel_requested
                && let Some(scan) = self.as_mut().rust_mut().directory_scan.as_mut()
            {
                scan.combined
                    .push_warning("The folder scanner stopped unexpectedly");
            }
            completion = Some(cancel_requested);
        }
        let refresh_playlist = self
            .as_mut()
            .rust_mut()
            .directory_scan
            .as_mut()
            .is_some_and(|scan| {
                let due = scan.playlist_dirty
                    && (completion.is_some()
                        || scan.last_playlist_refresh.elapsed() >= Duration::from_millis(150));
                if due {
                    scan.playlist_dirty = false;
                    scan.last_playlist_refresh = Instant::now();
                }
                due
            });
        if refresh_playlist {
            self.as_mut().refresh_playback_order();
            self.as_mut().rebuild_playlist();
        }
        if let Some(cancelled) = completion {
            self.as_mut().finish_directory_scan(cancelled);
        }
    }

    pub fn cancel_directory_scan(mut self: Pin<&mut Self>) {
        {
            let mut rust = self.as_mut().rust_mut();
            let Some(scan) = rust.directory_scan.as_mut() else {
                return;
            };
            scan.cancel_requested = true;
            scan.cancel.store(true, AtomicOrdering::Relaxed);
        }
        self.as_mut()
            .set_directory_scan_current_path(qstring("Cancelling…"));
        self.as_mut().set_status(qstring("Cancelling music load…"));
    }

    pub fn start_tree_delete(mut self: Pin<&mut Self>, paths: QString, permanent: bool) -> bool {
        if self.as_ref().rust().tree_delete_active {
            self.as_mut()
                .set_status(qstring("A file delete is already running"));
            return false;
        }
        let paths = sanitize_delete_paths(parse_delete_paths_json(&paths.to_string()));
        if paths.is_empty() {
            self.as_mut().set_status(qstring("Nothing to delete"));
            return false;
        }
        let (sender, receiver) = std::sync::mpsc::sync_channel(64);
        let cancel = Arc::new(AtomicBool::new(false));
        let first = paths[0].to_string_lossy().into_owned();
        self.as_mut().rust_mut().tree_delete = Some(TreeDeleteState {
            receiver,
            cancel: Arc::clone(&cancel),
            cancel_requested: false,
            permanent,
            deleted: Vec::new(),
            failures: Vec::new(),
        });
        self.as_mut().set_tree_delete_done(0);
        self.as_mut()
            .set_tree_delete_total(saturating_i32(paths.len()));
        self.as_mut().set_tree_delete_current_path(qstring(&first));
        self.as_mut().set_tree_delete_error(QString::default());
        self.as_mut().set_tree_delete_active(true);
        self.as_mut().set_status(qstring(if permanent {
            "Deleting files permanently…"
        } else {
            "Moving files to trash…"
        }));
        if let Err(error) = std::thread::Builder::new()
            .name("kog-tree-delete".to_owned())
            .spawn(move || run_tree_delete_job(paths, permanent, sender, cancel))
        {
            self.as_mut().rust_mut().tree_delete = None;
            self.as_mut().set_tree_delete_active(false);
            self.as_mut()
                .set_status(qstring(format!("Starting file delete: {error}")));
            return false;
        }
        true
    }

    pub fn poll_tree_delete(mut self: Pin<&mut Self>) {
        if !self.as_ref().rust().tree_delete_active {
            return;
        }
        let mut events = Vec::new();
        let mut disconnected = false;
        if self.as_ref().rust().tree_delete.as_ref().is_some() {
            for _ in 0..64 {
                let event = match self.as_ref().rust().tree_delete.as_ref() {
                    Some(job) => match job.receiver.try_recv() {
                        Ok(event) => event,
                        Err(TryRecvError::Empty) => break,
                        Err(TryRecvError::Disconnected) => {
                            disconnected = true;
                            break;
                        }
                    },
                    None => break,
                };
                let complete = matches!(event, TreeDeleteEvent::Complete { .. });
                events.push(event);
                if complete {
                    break;
                }
            }
        }
        let mut completion = None;
        for event in events {
            match event {
                TreeDeleteEvent::Total { items } => {
                    self.as_mut().set_tree_delete_total(saturating_i32(items));
                }
                TreeDeleteEvent::Progress { path, done } => {
                    self.as_mut()
                        .set_tree_delete_current_path(qstring(path.to_string_lossy()));
                    self.as_mut().set_tree_delete_done(saturating_i32(done));
                }
                TreeDeleteEvent::Deleted { path } => {
                    if let Some(job) = self.as_mut().rust_mut().tree_delete.as_mut() {
                        job.deleted.push(path);
                    }
                }
                TreeDeleteEvent::Failed { path, error } => {
                    if let Some(job) = self.as_mut().rust_mut().tree_delete.as_mut() {
                        job.failures.push(format!("{}: {error}", path.display()));
                    }
                }
                TreeDeleteEvent::Complete { cancelled, files } => {
                    completion = Some((cancelled, files));
                }
            }
        }
        if disconnected && completion.is_none() {
            if !self
                .as_ref()
                .rust()
                .tree_delete
                .as_ref()
                .is_some_and(|job| job.cancel_requested)
                && let Some(job) = self.as_mut().rust_mut().tree_delete.as_mut()
            {
                job.failures
                    .push("The file delete stopped unexpectedly".to_owned());
            }
            completion = Some((true, 0));
        }
        if let Some((cancelled, files)) = completion {
            self.as_mut().finish_tree_delete(cancelled, files);
        }
    }

    pub fn cancel_tree_delete(mut self: Pin<&mut Self>) {
        {
            let mut rust = self.as_mut().rust_mut();
            let Some(job) = rust.tree_delete.as_mut() else {
                return;
            };
            job.cancel_requested = true;
            job.cancel.store(true, AtomicOrdering::Relaxed);
        }
        self.as_mut()
            .set_tree_delete_current_path(qstring("Cancelling…"));
        self.as_mut().set_status(qstring("Cancelling file delete…"));
    }

    fn finish_tree_delete(mut self: Pin<&mut Self>, worker_cancelled: bool, files: usize) {
        let Some(job) = self.as_mut().rust_mut().tree_delete.take() else {
            return;
        };
        let cancelled = worker_cancelled || job.cancel_requested;
        let permanent = job.permanent;
        let moved = job.deleted.len();
        let failed = job.failures.len();
        let contents = if files == 1 {
            " (1 file)".to_owned()
        } else if files > 1 {
            format!(" ({files} files)")
        } else {
            String::new()
        };

        // Drop playlist entries that pointed into the trashed paths so the
        // playlist never keeps dead files behind. Only currently visible
        // rows are mapped; rows hidden by an active search filter keep
        // today's external-deletion behavior.
        let doomed = purged_track_indices(&self.as_ref().rust().tracks, &job.deleted);
        let mut purged_tracks = 0_usize;
        if !doomed.is_empty() {
            let visible = self
                .as_ref()
                .rust()
                .visible_indices
                .iter()
                .enumerate()
                .filter(|(_, source)| doomed.binary_search(source).is_ok())
                .map(|(position, _)| position)
                .collect::<Vec<_>>();
            if !visible.is_empty() {
                purged_tracks = visible.len();
                let csv = visible
                    .iter()
                    .map(usize::to_string)
                    .collect::<Vec<_>>()
                    .join(",");
                self.as_mut().remove_tracks(qstring(csv));
            }
        }

        let items = if moved == 1 { "item" } else { "items" };
        let mut status = if cancelled {
            if permanent {
                format!("Delete cancelled after permanently deleting {moved} {items}{contents}")
            } else {
                format!("Delete cancelled after moving {moved} {items} to trash{contents}")
            }
        } else if permanent {
            format!("Permanently deleted {moved} {items}{contents}")
        } else {
            format!("Moved {moved} {items} to trash{contents}")
        };
        if failed > 0 {
            status.push_str(&format!(" — {failed} failed"));
            let shown = job
                .failures
                .iter()
                .take(3)
                .cloned()
                .collect::<Vec<_>>()
                .join("\n");
            self.as_mut().set_tree_delete_error(qstring(shown));
        }
        if purged_tracks > 0 {
            status.push_str(&format!(
                " — removed {purged_tracks} playlist track{}",
                if purged_tracks == 1 { "" } else { "s" }
            ));
        }
        self.as_mut().set_tree_delete_done(saturating_i32(moved));
        self.as_mut()
            .set_tree_delete_current_path(QString::default());
        self.as_mut().set_tree_delete_active(false);
        self.as_mut().set_status(qstring(status));
    }

    pub fn add_url(mut self: Pin<&mut Self>, url: QString) {
        let behavior = OpeningFilesBehavior::from_setting(
            &self.as_ref().rust().opening_files_behavior.to_string(),
        )
        .unwrap_or_default();
        self.as_mut()
            .add_remote_url_value(url.to_string(), behavior);
    }

    pub fn enqueue_url(mut self: Pin<&mut Self>, url: QString) {
        self.as_mut()
            .add_remote_url_value(url.to_string(), OpeningFilesBehavior::Enqueue);
    }

    pub fn enqueue_urls_json(mut self: Pin<&mut Self>, urls: QString) {
        let (paths, remote_urls) = match dropped_urls_from_json(&urls.to_string()) {
            Ok(inputs) => inputs,
            Err(error) => {
                self.as_mut().set_status(qstring(error));
                return;
            }
        };
        if !paths.is_empty() {
            self.as_mut()
                .add_local_paths(paths, OpeningFilesBehavior::Enqueue);
        }
        for url in remote_urls {
            self.as_mut()
                .add_remote_url_value(url, OpeningFilesBehavior::Enqueue);
        }
    }

    fn add_remote_url_value(
        mut self: Pin<&mut Self>,
        value: String,
        behavior: OpeningFilesBehavior,
    ) {
        if let Err(error) = self.as_ref().rust().decoders.expand_remote_url(&value) {
            self.as_mut().set_status(qstring(error));
            return;
        }
        if behavior.clears_playlist() {
            self.as_mut().clear_playlist();
        }
        let first_new_source_index = self.as_ref().rust().tracks.len();
        let result = match self.as_mut().rust_mut().add_remote_url(&value) {
            Ok(result) => result,
            Err(error) => {
                self.as_mut().set_status(qstring(error));
                return;
            }
        };
        if result.added == 0 {
            self.as_mut()
                .set_status(qstring("The URL is already in the playlist"));
            return;
        }
        self.as_mut().refresh_playback_order();
        self.as_mut().rebuild_playlist();
        self.as_mut().set_status(qstring(add_path_status(&result)));
        if behavior.starts_playback() {
            self.as_mut().play_source_index(first_new_source_index);
        }
    }

    pub fn remove_track(mut self: Pin<&mut Self>, index: i32) {
        let _ = self.as_mut().remove_tracks(qstring(index.to_string()));
    }

    pub fn remove_tracks(mut self: Pin<&mut Self>, indices: QString) -> i32 {
        let visible_indices = parse_row_indices(
            &indices.to_string(),
            self.as_ref().rust().visible_indices.len(),
        );
        if visible_indices.is_empty() {
            return -1;
        }

        let first_visible_index = visible_indices[0];
        let mut source_indices = visible_indices
            .iter()
            .filter_map(|&index| self.as_ref().rust().visible_indices.get(index).copied())
            .collect::<Vec<_>>();
        source_indices.sort_unstable();
        source_indices.dedup();

        let old_count = self.as_ref().rust().tracks.len();
        let old_current = usize::try_from(self.as_ref().rust().current_index).ok();
        let mut next_index = 0;
        let old_to_new = (0..old_count)
            .map(|index| {
                if source_indices.binary_search(&index).is_ok() {
                    None
                } else {
                    let mapped = next_index;
                    next_index += 1;
                    Some(mapped)
                }
            })
            .collect::<Vec<_>>();
        let removed_current = old_current.is_some_and(|index| old_to_new[index].is_none());

        if removed_current {
            self.as_mut().stop();
        }
        {
            let mut rust = self.as_mut().rust_mut();
            rust.playback_order.remap_tracks(&old_to_new);
            for &source_index in source_indices.iter().rev() {
                rust.tracks.remove(source_index);
            }
        }

        if removed_current {
            self.as_mut().set_current_index(-1);
            self.as_mut().reset_now_playing();
        } else if let Some(current) = old_current.and_then(|index| old_to_new[index]) {
            self.as_mut().set_current_index(saturating_i32(current));
        }
        self.as_mut().refresh_playback_order();
        self.as_mut().rebuild_playlist();
        self.as_mut().set_status(qstring(format!(
            "Removed {} track{}",
            source_indices.len(),
            if source_indices.len() == 1 { "" } else { "s" }
        )));

        let remaining = self.as_ref().rust().visible_indices.len();
        if remaining == 0 {
            -1
        } else {
            saturating_i32(first_visible_index.min(remaining - 1))
        }
    }

    pub fn move_tracks(mut self: Pin<&mut Self>, indices: QString, target_index: i32) -> QString {
        if !self.as_ref().rust().filter.is_empty() {
            self.as_mut().set_status(qstring(
                "Clear the playlist search before reordering tracks",
            ));
            return indices;
        }
        if self.as_ref().rust().sort_column != PlaylistSortColumn::Index {
            self.as_mut()
                .set_status(qstring("Restore playlist order before reordering tracks"));
            return indices;
        }

        let row_count = self.as_ref().rust().tracks.len();
        let selected_indices = parse_row_indices(&indices.to_string(), row_count);
        if selected_indices.is_empty() {
            return QString::default();
        }
        let target_slot = usize::try_from(target_index)
            .unwrap_or_default()
            .min(row_count);
        let old_current = usize::try_from(self.as_ref().rust().current_index).ok();
        let mut old_indices = (0..row_count).collect::<Vec<_>>();
        let new_indices = move_selected_items(
            &mut self.as_mut().rust_mut().tracks,
            &selected_indices,
            target_slot,
        );
        move_selected_items(&mut old_indices, &selected_indices, target_slot);
        let mut old_to_new = vec![None; row_count];
        for (new_index, old_index) in old_indices.into_iter().enumerate() {
            old_to_new[old_index] = Some(new_index);
        }
        self.as_mut()
            .rust_mut()
            .playback_order
            .remap_tracks(&old_to_new);

        if let Some(current) = old_current.and_then(|index| old_to_new[index]) {
            self.as_mut().set_current_index(saturating_i32(current));
        }
        self.as_mut().refresh_playback_order();
        self.as_mut().rebuild_playlist();
        self.as_mut().set_status(qstring(format!(
            "Moved {} track{}",
            new_indices.len(),
            if new_indices.len() == 1 { "" } else { "s" }
        )));
        encode_row_indices(&new_indices)
    }

    /// Empty the pane without touching playback: whatever is playing stays
    /// loaded and can still be paused, resumed, or stopped. The track is
    /// detached from the list, so when it ends there is nothing to advance to
    /// and playback stops.
    pub fn clear_playlist(mut self: Pin<&mut Self>) {
        {
            let mut rust = self.as_mut().rust_mut();
            rust.tracks.clear();
            rust.playback_order.clear_tracks();
        }
        self.as_mut().set_queue_count(0);
        self.as_mut().set_current_index(-1);
        self.as_mut().rebuild_playlist();
        let playing = self.as_ref().rust().playback.state() != PlaybackState::Stopped;
        self.as_mut().set_status(qstring(if playing {
            "Playlist cleared — the current track keeps playing"
        } else {
            "Playlist cleared"
        }));
    }

    pub fn filter_playlist(mut self: Pin<&mut Self>, query: QString) {
        let query = query.to_string().trim().to_lowercase();
        if self.as_ref().rust().filter == query {
            return;
        }
        self.as_mut().rust_mut().filter = query;
        self.as_mut().rebuild_playlist();
    }

    pub fn sort_playlist(
        mut self: Pin<&mut Self>,
        column: QString,
        selected_indices: QString,
    ) -> QString {
        let Some(column) = PlaylistSortColumn::from_identifier(column.to_string().trim()) else {
            self.as_mut()
                .set_status(qstring("That playlist column cannot be sorted"));
            return selected_indices;
        };
        let selected_sources = {
            let model = self.as_ref();
            let rows = parse_row_indices(
                &selected_indices.to_string(),
                model.rust().visible_indices.len(),
            );
            rows.iter()
                .filter_map(|row| model.rust().visible_indices.get(*row))
                .filter_map(|source_index| model.rust().tracks.get(*source_index))
                .map(|track| track.source.clone())
                .collect::<Vec<_>>()
        };
        let ascending = if column == PlaylistSortColumn::Index {
            true
        } else if self.as_ref().rust().sort_column == column {
            !self.as_ref().rust().playlist_sort_ascending
        } else {
            true
        };

        self.as_mut().rust_mut().sort_column = column;
        self.as_mut()
            .set_playlist_sort_column(qstring(column.identifier()));
        self.as_mut().set_playlist_sort_ascending(ascending);
        self.as_mut().rebuild_playlist();

        let selected_rows = {
            let model = self.as_ref();
            model
                .rust()
                .visible_indices
                .iter()
                .enumerate()
                .filter_map(|(row, source_index)| {
                    let source = &model.rust().tracks[*source_index].source;
                    selected_sources
                        .iter()
                        .any(|selected| selected == source)
                        .then_some(row)
                })
                .collect::<Vec<_>>()
        };
        if column == PlaylistSortColumn::Index {
            self.as_mut()
                .set_status(qstring("Restored original playlist order"));
        } else {
            self.as_mut().set_status(qstring(format!(
                "Sorted by {} {}",
                column.display_name(),
                if ascending { "ascending" } else { "descending" }
            )));
        }
        encode_row_indices(&selected_rows)
    }

    /// Custom playlists (plus the virtual Favorites at id 0) for the
    /// sidebar, as JSON for QML. Favorites reports the live star count.
    pub fn playlists_json(&self) -> QString {
        let mut items = vec![serde_json::json!({
            "id": 0,
            "name": "Favorites",
            "entryCount": self.rust().starred.len(),
        })];
        match self.rust().library_db.list_playlists() {
            Ok(playlists) => {
                for playlist in playlists {
                    items.push(serde_json::json!({
                        "id": playlist.id,
                        "name": playlist.name,
                        "entryCount": playlist.entry_count,
                    }));
                }
            }
            Err(error) => {
                return json_result(Err(error));
            }
        }
        json_result(Ok(serde_json::json!({
            "ok": true,
            "playlists": items,
        })))
    }

    fn playlist_result(
        mut self: Pin<&mut Self>,
        outcome: Result<serde_json::Value, String>,
        ok_status: Option<String>,
    ) -> QString {
        match &outcome {
            Ok(_) => {
                self.as_mut().bump_playlists_revision();
                if let Some(status) = ok_status {
                    self.as_mut().set_status(qstring(status));
                }
            }
            Err(error) => {
                self.as_mut().set_status(qstring(error));
            }
        }
        json_result(outcome)
    }

    pub fn create_playlist(mut self: Pin<&mut Self>, name: QString) -> QString {
        let name = name.to_string();
        let outcome = self
            .as_ref()
            .rust()
            .library_db
            .create_playlist(&name)
            .map(|id| {
                serde_json::json!({
                    "ok": true,
                    "id": id,
                    "name": name.trim(),
                })
            });
        let status = outcome
            .as_ref()
            .ok()
            .map(|_| format!("Created playlist {name:?}", name = name.trim()));
        self.playlist_result(outcome, status)
    }

    pub fn rename_playlist(mut self: Pin<&mut Self>, id: i32, name: QString) -> QString {
        if id == 0 {
            return json_result(Err("Favorites cannot be renamed".to_owned()));
        }
        let name = name.to_string();
        let outcome = self
            .as_ref()
            .rust()
            .library_db
            .rename_playlist(id as i64, &name)
            .map(|()| {
                serde_json::json!({
                    "ok": true,
                    "id": id,
                    "name": name.trim(),
                })
            });
        self.playlist_result(outcome, None)
    }

    pub fn duplicate_playlist(mut self: Pin<&mut Self>, id: i32, name: QString) -> QString {
        let outcome = if id == 0 {
            Err("Favorites cannot be duplicated".to_owned())
        } else {
            let name = name.to_string();
            self.as_ref()
                .rust()
                .library_db
                .duplicate_playlist(id as i64, &name)
                .map(|new_id| {
                    serde_json::json!({
                        "ok": true,
                        "id": new_id,
                        "name": name.trim(),
                    })
                })
        };
        self.playlist_result(outcome, None)
    }

    pub fn delete_playlist(mut self: Pin<&mut Self>, id: i32) {
        if id == 0 {
            self.as_mut()
                .set_status(qstring("Favorites cannot be deleted"));
            return;
        }
        match self.as_ref().rust().library_db.delete_playlist(id as i64) {
            Ok(()) => {
                self.as_mut().bump_playlists_revision();
                self.as_mut().set_status(qstring("Deleted playlist"));
            }
            Err(error) => {
                self.as_mut().set_status(qstring(error));
            }
        }
    }

    pub fn move_playlist(mut self: Pin<&mut Self>, id: i32, to_position: i32) {
        if id == 0 {
            self.as_mut()
                .set_status(qstring("Favorites always stays first"));
            return;
        }
        match self
            .as_ref()
            .rust()
            .library_db
            .move_playlist(id as i64, to_position.max(0) as usize)
        {
            Ok(()) => {
                self.as_mut().bump_playlists_revision();
            }
            Err(error) => {
                self.as_mut().set_status(qstring(error));
            }
        }
    }

    /// Resolve a stored playlist (or Favorites at id 0) to entries.
    fn playlist_stored_entries(&self, id: i64) -> Result<Vec<kog_core::db::StoredEntry>, String> {
        if id == 0 {
            return self.rust().library_db.starred_entries();
        }
        self.rust().library_db.playlist_entries(id)
    }

    /// Export a stored playlist (or Favorites) as a portable `.m3u` file:
    /// archives read as directories, subsongs as `#N` fragments. Kog
    /// re-resolves both on import; anything else sees plain paths.
    pub fn export_playlist(mut self: Pin<&mut Self>, id: i32) {
        let stored = match self.playlist_stored_entries(id as i64) {
            Ok(stored) => stored,
            Err(error) => {
                self.as_mut().set_status(qstring(error));
                return;
            }
        };
        if stored.is_empty() {
            self.as_mut().set_status(qstring("Playlist is empty"));
            return;
        }
        let mut entries = Vec::with_capacity(stored.len());
        let mut skipped = 0_usize;
        for stored_entry in &stored {
            match playlist_entry_from_stored(stored_entry) {
                Some(entry) => entries.push(entry),
                None => skipped += 1,
            }
        }
        if entries.is_empty() {
            self.as_mut()
                .set_status(qstring("Playlist has no exportable entries left"));
            return;
        }
        let default_name = if id == 0 {
            "favorites.m3u".to_owned()
        } else {
            let name = self
                .as_ref()
                .rust()
                .library_db
                .list_playlists()
                .ok()
                .and_then(|playlists| {
                    playlists
                        .into_iter()
                        .find(|playlist| playlist.id == id as i64)
                        .map(|playlist| playlist.name)
                })
                .unwrap_or_else(|| format!("playlist-{id}"));
            format!("{name}.m3u")
        };
        let directory = self.as_ref().rust().directory.clone();
        let Some(path) = rfd::FileDialog::new()
            .set_title("Export Playlist")
            .set_directory(directory)
            .set_file_name(default_name)
            .add_filter("M3U Playlist", &["m3u", "m3u8"])
            .save_file()
        else {
            return;
        };
        let mut path = path;
        if path
            .extension()
            .and_then(|extension| extension.to_str())
            .is_none_or(|extension| {
                !(extension.eq_ignore_ascii_case("m3u") || extension.eq_ignore_ascii_case("m3u8"))
            })
        {
            path.set_extension("m3u");
        }
        match kog_audio::playlist::Playlist::save_portable(&path, &entries) {
            Ok(()) => self.as_mut().set_status(qstring(format!(
                "Exported {} {} to {}{}",
                entries.len(),
                if entries.len() == 1 {
                    "track"
                } else {
                    "tracks"
                },
                path.display(),
                if skipped > 0 {
                    format!(" ({skipped} skipped)")
                } else {
                    String::new()
                },
            ))),
            Err(error) => self.as_mut().set_status(qstring(error)),
        }
    }

    /// Enqueue a stored playlist (or Favorites) through the normal
    /// background folder scanner via a cache `.m3u`, so progress, cancel,
    /// dedup, archive members, and play behavior all match adding files.
    /// Stage a stored playlist (or Favorites) as an importable `.m3u`
    /// cache file. Returns the cache path plus the count of entries that
    /// could not be addressed and were skipped.
    fn stage_playlist_cache_file(&self, id: i64) -> Result<(PathBuf, usize), String> {
        let stored = self.playlist_stored_entries(id)?;
        if stored.is_empty() {
            return Err("Playlist is empty".to_owned());
        }
        let mut entries = Vec::with_capacity(stored.len());
        let mut skipped = 0_usize;
        for stored_entry in &stored {
            match playlist_entry_from_stored(stored_entry) {
                Some(entry) => entries.push(entry),
                None => skipped += 1,
            }
        }
        if entries.is_empty() {
            return Err("Playlist has no playable entries left".to_owned());
        }
        let cache_file = playlist_cache_dir().join(if id == 0 {
            "favorites.m3u".to_owned()
        } else {
            format!("playlist-{id}.m3u")
        });
        if let Some(parent) = cache_file.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|_| "Could not stage the playlist for import".to_owned())?;
        }
        kog_audio::playlist::Playlist::save(&cache_file, &entries)?;
        Ok((cache_file, skipped))
    }

    pub fn enqueue_playlist(mut self: Pin<&mut Self>, id: i32, start_playback: bool) {
        let (cache_file, skipped) = match self.as_ref().stage_playlist_cache_file(id as i64) {
            Ok(staged) => staged,
            Err(error) => {
                self.as_mut().set_status(qstring(error));
                return;
            }
        };
        if skipped > 0 {
            self.as_mut()
                .set_status(qstring(format!("Skipped {skipped} unresolvable entries")));
        }
        self.as_mut().add_local_paths(
            vec![cache_file],
            if start_playback {
                OpeningFilesBehavior::EnqueueAndPlay
            } else {
                OpeningFilesBehavior::Enqueue
            },
        );
    }

    /// Blacklist file/folder paths from the tree so radio never picks
    /// them. Songs match radio picks exactly (archive members resolve to
    /// outer + member); folders match everything beneath, including
    /// archive contents addressed as `outer :: member`.
    pub fn blacklist_tree_paths(mut self: Pin<&mut Self>, paths: QString, folders: bool) -> QString {
        let outcome: Result<serde_json::Value, String> = (|| {
            let paths = local_paths_from_json(&paths.to_string())?;
            if paths.is_empty() {
                return Err("Nothing selected to blacklist".to_owned());
            }
            let mut entries = Vec::new();
            let mut skipped = 0_usize;
            for path in &paths {
                if folders {
                    if let Ok(Some(location)) = kog_audio::archive::tree_location(path) {
                        // Archive containers blacklist as folders: an empty
                        // member means the whole archive, otherwise the
                        // member subdirectory in key form.
                        if location.entry.is_empty() {
                            entries.push(blacklist_row(
                                kog_core::db::BLACKLIST_FOLDER,
                                &blacklist_path(&location.archive.to_string_lossy()),
                                "",
                            ));
                        } else {
                            entries.push(blacklist_row(
                                kog_core::db::BLACKLIST_FOLDER,
                                &format!(
                                    "{} :: {}",
                                    blacklist_path(&location.archive.to_string_lossy()),
                                    location.entry
                                ),
                                "",
                            ));
                        }
                        continue;
                    }
                    if !path.is_dir() {
                        skipped += 1;
                        continue;
                    }
                    entries.push(blacklist_row(
                        kog_core::db::BLACKLIST_FOLDER,
                        &blacklist_path(&path.to_string_lossy()),
                        "",
                    ));
                    continue;
                }
                if let Ok(Some(location)) = kog_audio::archive::tree_location(path) {
                    if location.entry.is_empty() {
                        skipped += 1;
                        continue;
                    }
                    entries.push(blacklist_row(
                        kog_core::db::BLACKLIST_SONG,
                        &blacklist_path(&location.archive.to_string_lossy()),
                        &location.entry,
                    ));
                    continue;
                }
                if path.is_dir() {
                    skipped += 1;
                    continue;
                }
                entries.push(blacklist_row(
                    kog_core::db::BLACKLIST_SONG,
                    &blacklist_path(&path.to_string_lossy()),
                    "",
                ));
            }
            self.as_mut().blacklist_commit(entries, skipped, folders)
        })();
        if let Err(error) = outcome.as_ref() {
            self.as_mut().set_status(qstring(error));
        }
        json_result(outcome)
    }

    /// Blacklist pane rows so radio never picks them (or, with folders,
    /// their parent folders for local files).
    pub fn blacklist_pane_selection(
        mut self: Pin<&mut Self>,
        indices: QString,
        folders: bool,
    ) -> QString {
        let outcome: Result<serde_json::Value, String> = (|| {
            let rows = parse_row_indices(
                &indices.to_string(),
                self.as_ref().rust().visible_indices.len(),
            );
            if rows.is_empty() {
                return Err("No rows are selected".to_owned());
            }
            let tracks: Vec<Track> = {
                let this = self.as_ref();
                let rust = this.rust();
                rows.iter()
                    .filter_map(|row| {
                        rust.visible_indices
                            .get(*row)
                            .and_then(|source_index| rust.tracks.get(*source_index))
                            .cloned()
                    })
                    .collect()
            };
            let mut entries = Vec::new();
            let mut skipped = 0_usize;
            let mut folders_seen = std::collections::HashSet::new();
            for track in &tracks {
                if folders {
                    let Some(parent) = track.source.path.parent().map(PathBuf::from) else {
                        skipped += 1;
                        continue;
                    };
                    if track.source.remote_url.is_some() || track.source.archive_origin.is_some() {
                        skipped += 1;
                        continue;
                    }
                    let folder = blacklist_path(&parent.to_string_lossy());
                    if folders_seen.insert(folder.clone()) {
                        entries.push(blacklist_row(kog_core::db::BLACKLIST_FOLDER, &folder, ""));
                    }
                    continue;
                }
                if let Some(url) = track.source.remote_url.as_deref() {
                    entries.push(blacklist_row(kog_core::db::BLACKLIST_SONG, url, ""));
                    continue;
                }
                if let Some(origin) = track.source.archive_origin.as_ref() {
                    if origin.entry_name.is_empty() {
                        skipped += 1;
                        continue;
                    }
                    entries.push(blacklist_row(
                        kog_core::db::BLACKLIST_SONG,
                        &blacklist_path(&origin.archive_path.to_string_lossy()),
                        &origin.entry_name,
                    ));
                    continue;
                }
                entries.push(blacklist_row(
                    kog_core::db::BLACKLIST_SONG,
                    &blacklist_path(&track.source.path.to_string_lossy()),
                    "",
                ));
            }
            self.as_mut().blacklist_commit(entries, skipped, folders)
        })();
        if let Err(error) = outcome.as_ref() {
            self.as_mut().set_status(qstring(error));
        }
        json_result(outcome)
    }

    /// Write blacklist rows, refresh live radio staging, and summarize.
    fn blacklist_commit(
        mut self: Pin<&mut Self>,
        entries: Vec<kog_core::db::BlacklistEntry>,
        skipped: usize,
        folders: bool,
    ) -> Result<serde_json::Value, String> {
        if entries.is_empty() {
            return Err(if skipped > 0 {
                "Nothing selected can be blacklisted".to_owned()
            } else {
                "Nothing selected to blacklist".to_owned()
            });
        }
        let added = self
            .as_ref()
            .rust()
            .library_db
            .add_blacklist_entries(&entries)?;
        self.as_mut().refresh_radio_blacklist();
        let noun = if folders { "folder" } else { "song" };
        let mut status = if added == 0 {
            format!("Already blacklisted")
        } else {
            format!(
                "Blacklisted {added} {noun}{}",
                if added == 1 { "" } else { "s" }
            )
        };
        if skipped > 0 {
            status.push_str(&format!(" ({skipped} skipped)"));
        }
        self.as_mut().set_status(qstring(status));
        Ok(serde_json::json!({
            "ok": true,
            "added": added,
            "skipped": skipped,
        }))
    }

    pub fn blacklist_json(&self) -> QString {
        let outcome = self.rust().library_db.list_blacklist().map(|entries| {
            serde_json::json!({
                "ok": true,
                "entries": entries
                    .iter()
                    .map(|entry| serde_json::json!({
                        "id": entry.id,
                        "kind": entry.kind,
                        "path": entry.path,
                        "entry": entry.entry,
                    }))
                    .collect::<Vec<_>>(),
            })
        });
        json_result(outcome)
    }

    pub fn remove_blacklist_entry(mut self: Pin<&mut Self>, id: i32) -> QString {
        let outcome = self
            .as_ref()
            .rust()
            .library_db
            .remove_blacklist_entry(id as i64)
            .map(|()| serde_json::json!({ "ok": true, "id": id }));
        if outcome.is_ok() {
            self.as_mut().refresh_radio_blacklist();
            self.as_mut().set_status(qstring("Removed from blacklist"));
        } else if let Err(error) = outcome.as_ref() {
            self.as_mut().set_status(qstring(error));
        }
        json_result(outcome)
    }
    /// Drop the entries of a stored playlist whose files are gone and
    /// report how many went. Never touches Favorites (id 0 lives in the
    /// stars table, not playlist rows).
    pub fn prune_missing_playlist_entries(mut self: Pin<&mut Self>, id: i32) -> QString {
        let outcome: Result<serde_json::Value, String> = (|| {
            if id == 0 {
                return Err("Favorites are cleaned from their starred files instead".to_owned());
            }
            let rows = self
                .as_ref()
                .rust()
                .library_db
                .playlist_entry_rows(id as i64)?;
            let doomed: Vec<i64> = rows
                .iter()
                .filter(|(_, entry)| stored_entry_is_missing(entry))
                .map(|(row_id, _)| *row_id)
                .collect();
            let removed = self
                .as_ref()
                .rust()
                .library_db
                .delete_entry_rows(id as i64, &doomed)?;
            Ok(serde_json::json!({
                "ok": true,
                "id": id,
                "removed": removed,
                "checked": rows.len(),
            }))
        })();
        let status = match &outcome {
            Ok(value) => {
                let removed = value
                    .get("removed")
                    .and_then(|removed| removed.as_u64())
                    .unwrap_or(0);
                Some(if removed == 0 {
                    "No missing files in the playlist".to_owned()
                } else {
                    format!(
                        "Removed {removed} missing file{} from the playlist",
                        if removed == 1 { "" } else { "s" }
                    )
                })
            }
            Err(_) => None,
        };
        self.playlist_result(outcome, status)
    }

    /// Replace the pane with a stored playlist (or Favorites) and start
    /// playing it from the top.
    pub fn load_playlist_into_pane(mut self: Pin<&mut Self>, id: i32) {
        let (cache_file, skipped) = match self.as_ref().stage_playlist_cache_file(id as i64) {
            Ok(staged) => staged,
            Err(error) => {
                self.as_mut().set_status(qstring(error));
                return;
            }
        };
        if skipped > 0 {
            self.as_mut()
                .set_status(qstring(format!("Skipped {skipped} unresolvable entries")));
        }
        self.as_mut()
            .add_local_paths(vec![cache_file], OpeningFilesBehavior::ClearAndPlay);
    }

    /// Save the current pane as a new playlist at the bottom of the list.
    pub fn save_pane_as_playlist(mut self: Pin<&mut Self>, name: QString) -> QString {
        let name = name.to_string();
        let outcome: Result<serde_json::Value, String> = (|| {
            let id = self.as_ref().rust().library_db.create_playlist(&name)?;
            let (entries, skipped) =
                collect_stored_entries(&self.as_ref().rust().tracks);
            if entries.is_empty() {
                let _ = self.as_ref().rust().library_db.delete_playlist(id);
                return Err("The current pane has no savable tracks".to_owned());
            }
            self.as_ref()
                .rust()
                .library_db
                .append_entries(id, &entries)?;
            let mut value = serde_json::json!({
                "ok": true,
                "id": id,
                "name": name.trim(),
            });
            if skipped > 0 {
                value["skipped"] = serde_json::json!(skipped);
            }
            Ok(value)
        })();
        let status = match &outcome {
            Ok(value) => Some(format!(
                "Saved {} as a new playlist",
                value
                    .get("name")
                    .and_then(|name| name.as_str())
                    .unwrap_or("playlist")
            )),
            Err(_) => None,
        };
        self.playlist_result(outcome, status)
    }

    /// Save the selected pane rows as a new playlist at the bottom of the
    /// list. Mirrors save_pane_as_playlist over the selection instead.
    pub fn save_selection_as_playlist(
        mut self: Pin<&mut Self>,
        indices: QString,
        name: QString,
    ) -> QString {
        let name = name.to_string();
        let outcome: Result<serde_json::Value, String> = (|| {
            let rows = parse_row_indices(
                &indices.to_string(),
                self.as_ref().rust().visible_indices.len(),
            );
            if rows.is_empty() {
                return Err("No rows are selected".to_owned());
            }
            let id = self.as_ref().rust().library_db.create_playlist(&name)?;
            let tracks: Vec<Track> = {
                let this = self.as_ref();
                let rust = this.rust();
                rows.iter()
                    .filter_map(|row| {
                        rust.visible_indices
                            .get(*row)
                            .and_then(|source_index| rust.tracks.get(*source_index))
                            .cloned()
                    })
                    .collect()
            };
            let (entries, skipped) = collect_stored_entries(&tracks);
            if entries.is_empty() {
                let _ = self.as_ref().rust().library_db.delete_playlist(id);
                return Err("The selection has no savable tracks".to_owned());
            }
            self.as_ref().rust().library_db.append_entries(id, &entries)?;
            let mut value = serde_json::json!({
                "ok": true,
                "id": id,
                "name": name.trim(),
            });
            if skipped > 0 {
                value["skipped"] = serde_json::json!(skipped);
            }
            Ok(value)
        })();
        let status = match &outcome {
            Ok(value) => Some(format!(
                "Saved {} as a new playlist",
                value
                    .get("name")
                    .and_then(|name| name.as_str())
                    .unwrap_or("playlist")
            )),
            Err(_) => None,
        };
        self.playlist_result(outcome, status)
    }

    pub fn save_playlist_column_layout(mut self: Pin<&mut Self>, layout: QString) {
        if let Err(error) = AppSettings::save_playlist_column_layout(&layout.to_string()) {
            self.as_mut().set_status(qstring(error));
            return;
        }
        self.as_mut().set_playlist_column_layout(layout);
    }

    /// Flip the star on each selected playlist row, persisting to the
    /// library database and refreshing the star column. Rows without a
    /// resolvable identity are skipped and reported.
    pub fn toggle_stars(mut self: Pin<&mut Self>, indices: QString) {
        let rows = parse_row_indices(
            &indices.to_string(),
            self.as_ref().rust().visible_indices.len(),
        );
        if rows.is_empty() {
            return;
        }
        let mut starred_count = 0_usize;
        let mut unstarred_count = 0_usize;
        let mut skipped = 0_usize;
        for row in rows {
            let track = {
                let this = self.as_ref();
                let rust = this.rust();
                rust.visible_indices
                    .get(row)
                    .copied()
                    .and_then(|source_index| rust.tracks.get(source_index))
                    .cloned()
            };
            let Some(track) = track else {
                skipped += 1;
                continue;
            };
            let key = star_key_for_track(&track);
            let Some(stored) = stored_entry_for_track(&track) else {
                skipped += 1;
                continue;
            };
            let starring = !self.as_ref().rust().starred.contains(&key);
            match self.as_ref().rust().library_db.set_star(
                &key,
                &stored.kind,
                &stored.path,
                &stored.entry,
                stored.fragment.as_deref(),
                starring,
            ) {
                Ok(()) => {
                    if starring {
                        self.as_mut().rust_mut().starred.insert(key);
                        starred_count += 1;
                    } else {
                        self.as_mut().rust_mut().starred.remove(&key);
                        unstarred_count += 1;
                    }
                }
                Err(error) => {
                    self.as_mut().set_status(qstring(error));
                    return;
                }
            }
        }
        self.as_mut().bump_playlist_revision();
        self.as_mut().bump_playlists_revision();
        if skipped == 0 {
            self.as_mut()
                .set_status(qstring(match (starred_count, unstarred_count) {
                    (0, 0) => "Nothing to star".to_owned(),
                    (starred, 0) => format!(
                        "Starred {starred} track{}",
                        if starred == 1 { "" } else { "s" }
                    ),
                    (0, unstarred) => format!(
                        "Unstarred {unstarred} track{}",
                        if unstarred == 1 { "" } else { "s" }
                    ),
                    (starred, unstarred) => {
                        format!("Starred {starred}, unstarred {unstarred}")
                    }
                }));
        } else {
            self.as_mut().set_status(qstring(format!(
                "Starred {starred_count}, unstarred {unstarred_count}, skipped {skipped}"
            )));
        }
    }

    pub fn tag_editor_data(&self, indices: QString) -> QString {
        let sources = selected_sources(self, &indices.to_string());
        json_result(snapshot_json(&sources))
    }

    pub fn choose_tag_artwork(&self) -> QString {
        let Some(path) = rfd::FileDialog::new()
            .set_title("Choose Cover Artwork")
            .set_directory(&self.rust().directory)
            .add_filter(
                "Images",
                &["jpg", "jpeg", "png", "gif", "bmp", "tif", "tiff"],
            )
            .pick_file()
        else {
            return qstring(serde_json::json!({ "ok": false, "cancelled": true }).to_string());
        };
        json_result(artwork_file_json(&path))
    }

    pub fn save_tags(mut self: Pin<&mut Self>, indices: QString, edits: QString) -> QString {
        let sources = selected_sources(self.as_ref().get_ref(), &indices.to_string());
        let edits = match parse_edits(&edits.to_string()) {
            Ok(edits) => edits,
            Err(error) => return json_result(Err(error)),
        };
        // Stop the decoder before changing the file it may have open. This is
        // required on Windows and avoids depending on inode replacement
        // behavior on Unix. The state and exact position are restored below.
        let current_index = usize::try_from(self.as_ref().rust().current_index).ok();
        let current_selected = {
            let model = self.as_ref();
            current_index
                .and_then(|index| model.rust().tracks.get(index))
                .is_some_and(|current| sources.iter().any(|source| source == &current.source))
        };
        let prior_state = self.as_ref().rust().playback.state();
        let prior_position = self.as_ref().rust().playback.position();
        if current_selected && prior_state != PlaybackState::Stopped {
            self.as_mut().rust_mut().playback.stop();
        }

        let outcome = write_tags(&sources, &edits);
        for path in &outcome.updated_paths {
            let matching_indices = self
                .as_ref()
                .rust()
                .tracks
                .iter()
                .enumerate()
                .filter_map(|(index, track)| {
                    (track.source.remote_url.is_none()
                        && track.source.archive_origin.is_none()
                        && track.source.subsong.is_none()
                        && track.source.path.canonicalize().as_ref().ok() == Some(path))
                    .then_some(index)
                })
                .collect::<Vec<_>>();
            for index in matching_indices {
                let source = self.as_ref().rust().tracks[index].source.clone();
                let refreshed = Track::from_source(source, &self.as_ref().rust().decoders);
                self.as_mut().rust_mut().tracks[index] = refreshed;
            }
        }
        if !outcome.updated_paths.is_empty() {
            self.as_mut().rebuild_playlist();
        }

        let mut resume_warning = None;
        if current_selected && let Some(index) = current_index {
            if prior_state == PlaybackState::Stopped {
                self.as_mut().populate_now_playing(index);
            } else {
                self.as_mut().play_source_index(index);
                if self.as_ref().rust().playback.state() == PlaybackState::Stopped {
                    resume_warning = Some("playback could not be resumed".to_owned());
                } else {
                    if !prior_position.is_zero()
                        && let Err(error) = self.as_ref().rust().playback.seek(prior_position)
                    {
                        resume_warning = Some(format!("restoring position failed: {error}"));
                    } else {
                        self.as_mut()
                            .set_position_seconds(prior_position.as_secs_f64());
                    }
                    if prior_state == PlaybackState::Paused {
                        self.as_mut().rust_mut().playback.play_pause();
                    }
                    self.as_mut().sync_playback_state();
                }
            }
        }

        let updated_count = outcome.updated_paths.len();
        let mut status = match &outcome.error {
            Some(error) => error.clone(),
            None => format!(
                "Updated tags for {updated_count} file{}",
                if updated_count == 1 { "" } else { "s" }
            ),
        };
        if let Some(warning) = &resume_warning {
            status.push_str(&format!("; {warning}"));
        }
        self.as_mut().set_status(qstring(&status));
        qstring(
            serde_json::json!({
                "ok": outcome.error.is_none(),
                "updatedCount": updated_count,
                "error": outcome.error,
                "warning": resume_warning,
                "message": status,
            })
            .to_string(),
        )
    }

    pub fn play_index(mut self: Pin<&mut Self>, index: i32) {
        let Some(source_index) = visible_source_index(self.as_ref().get_ref(), index) else {
            return;
        };
        self.as_mut().play_source_index(source_index);
    }

    pub fn activate_playlist_index(mut self: Pin<&mut Self>, index: i32) {
        let Some(source_index) = visible_source_index(self.as_ref().get_ref(), index) else {
            return;
        };
        if self.as_ref().rust().current_index == saturating_i32(source_index) {
            self.as_mut().play_pause();
        } else {
            self.as_mut().play_source_index(source_index);
        }
    }

    pub fn play_pause(mut self: Pin<&mut Self>) {
        // A track can outlive the pane: clearing the playlist detaches what is
        // playing, and pause/resume must keep working for it.
        let loaded = self.as_ref().rust().playback.state() != PlaybackState::Stopped;
        if self.as_ref().rust().tracks.is_empty() && !loaded {
            // Empty playlist with radio on: (re)arm kickstart so the next
            // staged track autoplays, or play one right now if buffered.
            // Without radio this stays a silent no-op as before.
            if self.as_ref().rust().radio_active {
                if let Some(radio) = self.as_mut().rust_mut().radio.as_mut() {
                    radio.kickstart_armed = true;
                }
                let buffered = self
                    .as_ref()
                    .rust()
                    .radio
                    .as_ref()
                    .is_some_and(|radio| !radio.ready.is_empty());
                if buffered {
                    if let Some(index) = self.as_mut().shift_radio_track() {
                        self.as_mut().play_shifted_radio_track(index);
                    }
                    if let Some(radio) = self.as_mut().rust_mut().radio.as_mut() {
                        radio.kickstart_armed = false;
                    }
                } else {
                    self.as_mut()
                        .set_status(qstring("Random Radio — finding a track…"));
                }
            }
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

    /// Best-effort shutdown for background synth helpers (currently the
    /// persistent SC-55 server): called on real application quit so no
    /// booted emulator outlives the player. An idle server only blocks on
    /// stdin, so a missed shutdown is harmless.
    /// The API server settings plus live status, for the Preferences pane.
    pub fn server_settings_json(&self) -> QString {
        let config = kog_server::config::load_config();
        let running = self.rust().api_server.as_ref();
        json_result(Ok(serde_json::json!({
            "ok": true,
            "enabled": config.enabled,
            "address": config.address.to_string(),
            "port": config.port,
            "auth": config.auth,
            "token": config.token,
            "username": config.credentials.username,
            "hasPassword": config.credentials.is_usable(),
            "tls": config.tls.mode,
            "certificatePath": config.tls.certificate_path.to_string_lossy(),
            "privateKeyPath": config.tls.private_key_path.to_string_lossy(),
            "defaultCodec": config.default_codec.setting_value(),
            "cacheBytes": config.cache_bytes,
            "problems": config.problems(),
            "status": running.map(|server| serde_json::json!({
                "running": true,
                "url": format!("{}://{}", server.scheme, server.address),
                "certificatePath": server
                    .certificate_path
                    .as_ref()
                    .map(|path| path.to_string_lossy().into_owned()),
            })),
        })))
    }

    /// Persist the API server settings. Token auth never leaves the token
    /// empty, and an unusable configuration is reported before it is saved.
    pub fn save_server_settings(mut self: Pin<&mut Self>, json: QString) -> QString {
        let outcome: Result<serde_json::Value, String> = (|| {
            let value: serde_json::Value = serde_json::from_str(&json.to_string())
                .map_err(|error| format!("reading the server settings: {error}"))?;
            let mut config = kog_server::config::load_config();
            if let Some(enabled) = value.get("enabled").and_then(|value| value.as_bool()) {
                config.enabled = enabled;
            }
            if let Some(address) = value.get("address").and_then(|value| value.as_str()) {
                config.address = address
                    .trim()
                    .parse()
                    .map_err(|_| format!("{address} is not a valid bind address"))?;
            }
            if let Some(port) = value.get("port").and_then(|value| value.as_u64()) {
                config.port = u16::try_from(port)
                    .map_err(|_| "the port must be between 1 and 65535".to_owned())?;
            }
            if let Some(auth) = value.get("auth").and_then(|value| value.as_str()) {
                config.auth = match auth {
                    "none" => kog_server::AuthMode::None,
                    "token" => kog_server::AuthMode::Token,
                    "basic" => kog_server::AuthMode::Basic,
                    other => return Err(format!("unknown authentication mode: {other}")),
                };
            }
            if let Some(token) = value.get("token").and_then(|value| value.as_str()) {
                config.token = token.trim().to_owned();
            }
            if let Some(username) = value.get("username").and_then(|value| value.as_str()) {
                config.credentials.username = username.trim().to_owned();
            }
            // The password arrives in the clear once and is stored hashed.
            if let Some(password) = value.get("password").and_then(|value| value.as_str()) {
                if !password.is_empty() {
                    config.credentials.set_password(password)?;
                }
            }
            if let Some(tls) = value.get("tls").and_then(|value| value.as_str()) {
                config.tls.mode = match tls {
                    "off" => kog_server::TlsMode::Off,
                    "selfSigned" => kog_server::TlsMode::SelfSigned,
                    "pem" => kog_server::TlsMode::Pem,
                    other => return Err(format!("unknown TLS mode: {other}")),
                };
            }
            if let Some(codec) = value.get("defaultCodec").and_then(|value| value.as_str()) {
                config.default_codec = kog_server::StreamCodec::from_setting(codec)
                    .ok_or_else(|| format!("unknown codec: {codec}"))?;
            }
            if let Some(bytes) = value.get("cacheBytes").and_then(|value| value.as_u64()) {
                config.cache_bytes = bytes;
            }
            config.ensure_credentials();
            config.validate()?;
            kog_server::config::save_config(&config)?;
            Ok(serde_json::json!({ "ok": true, "problems": config.problems() }))
        })();
        self.as_mut().server_result(outcome)
    }

    pub fn generate_api_token(&self) -> QString {
        json_result(kog_server::auth::generate_token().map(|token| {
            serde_json::json!({ "ok": true, "token": token })
        }))
    }

    /// Copy a user-supplied certificate and key into Kog's TLS directory.
    pub fn import_server_certificate(
        mut self: Pin<&mut Self>,
        certificate: QUrl,
        private_key: QUrl,
    ) -> QString {
        let outcome: Result<serde_json::Value, String> = (|| {
            let certificate = certificate
                .to_local_file()
                .ok_or_else(|| "Choose a local certificate file".to_owned())?;
            let private_key = private_key
                .to_local_file()
                .ok_or_else(|| "Choose a local private key file".to_owned())?;
            let certificate = PathBuf::from(certificate.to_string());
            let private_key = PathBuf::from(private_key.to_string());
            let tls = kog_server::import_pem_pair(&certificate, &private_key)?;
            let mut config = kog_server::config::load_config();
            config.tls = tls;
            kog_server::config::save_config(&config)?;
            Ok(serde_json::json!({
                "ok": true,
                "certificatePath": config.tls.certificate_path.to_string_lossy(),
            }))
        })();
        self.as_mut().server_result(outcome)
    }

    /// Start the API server on a dedicated runtime thread. Stopping it signals
    /// the same server for a graceful shutdown.
    pub fn start_api_server(mut self: Pin<&mut Self>) -> QString {
        if self.rust().api_server.is_some() {
            return json_result(Ok(serde_json::json!({ "ok": true, "alreadyRunning": true })));
        }
        let outcome: Result<serde_json::Value, String> = (|| {
            let config = kog_server::config::load_config();
            config.validate()?;
            let settings = kog_audio::settings::AppSettings::load();
            let state = kog_server::routes::AppState::new(
                config.clone(),
                env!("CARGO_PKG_VERSION"),
                kog_server::routes::AppState::stream_service(
                    &config,
                    settings.decoder_settings(),
                ),
                kog_server::api::Library::open(),
            );
            let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();
            let address = config.socket_address();
            let scheme = if config.tls.mode == kog_server::TlsMode::Off {
                "http"
            } else {
                "https"
            };
            let certificate_path = match config.tls.mode {
                kog_server::TlsMode::SelfSigned => kog_server::tls::self_signed_certificate_path().ok(),
                kog_server::TlsMode::Pem => Some(config.tls.certificate_path.clone()),
                kog_server::TlsMode::Off => None,
            };
            // Fail fast on a port that is already taken: the server thread's
            // error would otherwise only reach stderr, leaving the pane
            // claiming the server is running.
            std::net::TcpListener::bind(address)
                .map_err(|error| format!("could not bind {address}: {error}"))?;
            std::thread::Builder::new()
                .name("kog-api-server".to_owned())
                .spawn(move || {
                    let runtime = match tokio::runtime::Builder::new_multi_thread()
                        .enable_all()
                        .build()
                    {
                        Ok(runtime) => runtime,
                        Err(error) => {
                            eprintln!("kog-server: could not start the runtime: {error}");
                            return;
                        }
                    };
                    runtime.block_on(async move {
                        if let Err(error) = kog_server::routes::serve_with_shutdown(
                            state,
                            async move {
                                let _ = shutdown_rx.await;
                            },
                        )
                        .await
                        {
                            eprintln!("kog-server: {error}");
                        }
                    });
                })
                .map_err(|error| format!("starting the API server thread: {error}"))?;
            self.as_mut().rust_mut().api_server = Some(ApiServerHandle {
                shutdown: Some(shutdown_tx),
                address,
                scheme,
                certificate_path,
            });
            Ok(serde_json::json!({
                "ok": true,
                "url": format!("{scheme}://{address}"),
            }))
        })();
        self.as_mut().server_result(outcome)
    }

    pub fn stop_api_server(mut self: Pin<&mut Self>) -> QString {
        match self.as_mut().rust_mut().api_server.take() {
            Some(mut server) => {
                if let Some(shutdown) = server.shutdown.take() {
                    let _ = shutdown.send(());
                }
                self.as_mut()
                    .set_status(qstring("API server stopped"));
                json_result(Ok(serde_json::json!({ "ok": true, "running": false })))
            }
            None => json_result(Ok(serde_json::json!({ "ok": true, "running": false }))),
        }
    }

    /// URLs a phone or another machine can use, so the UI can show something
    /// copyable instead of making the user guess their host address.
    pub fn server_addresses_json(&self) -> QString {
        let config = kog_server::config::load_config();
        let scheme = if config.tls.mode == kog_server::TlsMode::Off {
            "http"
        } else {
            "https"
        };
        // Only advertise addresses the server is actually bound to. A loopback
        // bind answers nowhere else, and handing out a LAN address for it was
        // the bug: the address looked usable and could never connect.
        let mut addresses = Vec::new();
        if config.address.is_loopback() {
            addresses.push(format!("{scheme}://127.0.0.1:{}", config.port));
        } else if config.address.is_unspecified() {
            if let Ok(interfaces) = local_ip_addresses() {
                for ip in interfaces {
                    addresses.push(format!("{scheme}://{ip}:{}", config.port));
                }
            }
        } else {
            addresses.push(format!("{scheme}://{}:{}", config.address, config.port));
        }
        addresses.sort();
        addresses.dedup();
        json_result(Ok(serde_json::json!({
            "ok": true,
            "addresses": addresses,
            "port": config.port,
            "shared": !config.address.is_loopback(),
        })))
    }

    /// The revision stamped into this build, for the title bar and About box.
    pub fn build_revision(&self) -> QString {
        qstring(env!("KOG_BUILD_REV"))
    }

    /// When this binary was linked, in milliseconds since the epoch, so QML can
    /// format it for the user's locale. Zero means "not known": a nix store
    /// path carries a 1970 mtime, and showing that would be worse than a blank.
    pub fn build_timestamp(&self) -> f64 {
        const EARLIEST_PLAUSIBLE_MS: f64 = 946_684_800_000.0; // 2000-01-01
        let millis = std::env::current_exe()
            .ok()
            .and_then(|path| std::fs::metadata(path).ok())
            .and_then(|metadata| metadata.modified().ok())
            .and_then(|modified| modified.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|since| since.as_millis() as f64)
            .unwrap_or(0.0);
        if millis < EARLIEST_PLAUSIBLE_MS {
            0.0
        } else {
            millis
        }
    }

    fn server_result(
        mut self: Pin<&mut Self>,
        outcome: Result<serde_json::Value, String>,
    ) -> QString {
        if let Err(error) = &outcome {
            self.as_mut().set_status(qstring(error));
        }
        json_result(outcome)
    }

    pub fn shutdown_synth_helpers(&self) {
        kog_audio::sc55::shutdown_sc55_servers();
    }

    pub fn stop(mut self: Pin<&mut Self>) {
        self.as_mut().rust_mut().playback.stop();
        self.as_mut().set_position_seconds(0.0);
        self.as_mut().set_status(qstring("Stopped"));
        // Cancel any in-flight artwork lookup so a late download cannot
        // repaint the thumbnail after the stop cleared it.
        if let Some(job) = self.as_mut().rust_mut().cover_art.take() {
            job.cancel.store(true, AtomicOrdering::Relaxed);
        }
        self.as_mut().set_current_artwork_path(QString::default());
        self.as_mut().sync_playback_state();
    }

    pub fn previous(mut self: Pin<&mut Self>) {
        // Like advancement, walk past verifiably-gone tracks instead of
        // stopping on them.
        let budget = self.as_ref().rust().tracks.len().saturating_add(1).max(1);
        let mut skips = 0_usize;
        loop {
            let tracks = self.as_ref().rust().tracks.clone();
            let current = usize::try_from(self.as_ref().rust().current_index).ok();
            let target = {
                self.as_mut()
                    .rust_mut()
                    .playback_order
                    .previous(&tracks, current)
            };
            let Some(target) = target else {
                return;
            };
            if self
                .as_ref()
                .rust()
                .tracks
                .get(target)
                .is_some_and(|track| track.missing)
            {
                skips += 1;
                if skips >= budget {
                    return;
                }
                continue;
            }
            match self.as_mut().play_source_index(target) {
                PlayOutcome::Started => return,
                PlayOutcome::Skipped => {
                    skips += 1;
                    if skips >= budget {
                        return;
                    }
                }
            }
        }
    }

    pub fn next(mut self: Pin<&mut Self>) {
        self.as_mut().advance_playback(false);
    }

    pub fn seek(mut self: Pin<&mut Self>, seconds: f64) {
        let seconds = seconds.clamp(0.0, self.as_ref().rust().duration_seconds.max(0.0));
        match self
            .as_ref()
            .rust()
            .playback
            .seek(Duration::from_secs_f64(seconds))
        {
            Ok(()) => {
                self.as_mut().set_position_seconds(seconds);
                self.as_ref().rust().mpris.seeked(seconds);
            }
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

    pub fn refresh_output_devices(mut self: Pin<&mut Self>) {
        let devices = match available_output_devices() {
            Ok(devices) => devices,
            Err(error) => {
                self.as_mut().set_output_device_status(qstring(&error));
                self.as_mut().set_status(qstring(error));
                return;
            }
        };
        self.as_mut()
            .set_output_devices_json(qstring(output_devices_json(&devices)));
        let selected_id = self.as_ref().rust().output_device_id.to_string();
        let selected = devices.iter().find(|device| device.id == selected_id);
        if !selected_id.is_empty() && selected.is_none() {
            self.as_mut().apply_output_device(
                None,
                &format!(
                    "Audio output {selected_id} is no longer available; using the system default"
                ),
            );
            return;
        }
        let status = if let Some(selected) = selected {
            format!("Using audio output: {}", selected.label)
        } else {
            format!(
                "Following the system default audio output; {} available device{}",
                devices.len(),
                if devices.len() == 1 { "" } else { "s" }
            )
        };
        self.as_mut().set_output_device_status(qstring(&status));
        self.as_mut().set_status(qstring(status));
    }

    pub fn select_output_device(mut self: Pin<&mut Self>, id: QString) {
        let id = id.to_string();
        let selection = if id.is_empty() {
            None
        } else {
            let devices = match available_output_devices() {
                Ok(devices) => devices,
                Err(error) => {
                    self.as_mut().set_output_device_status(qstring(&error));
                    self.as_mut().set_status(qstring(error));
                    return;
                }
            };
            let Some(device) = devices.into_iter().find(|device| device.id == id) else {
                let error = format!("That audio output is no longer available: {id}");
                self.as_mut().set_output_device_status(qstring(&error));
                self.as_mut().set_status(qstring(error));
                return;
            };
            Some(device)
        };
        let label = selection
            .as_ref()
            .map(|device| device.label.as_str())
            .unwrap_or("the system default");
        let status = format!("Switched audio output to {label}");
        self.as_mut().apply_output_device(selection, &status);
    }

    pub fn cycle_shuffle_mode(mut self: Pin<&mut Self>) {
        let mode = self.as_ref().rust().playback_order.shuffle_mode().next();
        self.as_mut().apply_shuffle_mode(mode);
    }

    pub fn select_shuffle_mode(mut self: Pin<&mut Self>, mode: QString) {
        let Some(mode) = ShuffleMode::from_setting(&mode.to_string()) else {
            self.as_mut()
                .set_status(qstring("That shuffle mode is unavailable"));
            return;
        };
        self.as_mut().apply_shuffle_mode(mode);
    }

    pub fn cycle_repeat_mode(mut self: Pin<&mut Self>) {
        let mode = self.as_ref().rust().playback_order.repeat_mode().next();
        self.as_mut().apply_repeat_mode(mode);
    }

    pub fn select_repeat_mode(mut self: Pin<&mut Self>, mode: QString) {
        let Some(mode) = RepeatMode::from_setting(&mode.to_string()) else {
            self.as_mut()
                .set_status(qstring("That repeat mode is unavailable"));
            return;
        };
        self.as_mut().apply_repeat_mode(mode);
    }

    pub fn toggle_queue(mut self: Pin<&mut Self>, indices: QString) {
        let source_indices = selected_source_indices(self.as_ref().get_ref(), &indices.to_string());
        if source_indices.is_empty() {
            return;
        }
        self.as_mut()
            .rust_mut()
            .playback_order
            .toggle_queue(&source_indices);
        let count = self.as_ref().rust().playback_order.queue_count();
        self.as_mut().set_queue_count(saturating_i32(count));
        self.as_mut().bump_playlist_revision();
        self.as_mut().set_status(qstring(format!(
            "Playback queue now contains {count} track{}",
            if count == 1 { "" } else { "s" }
        )));
    }

    pub fn clear_queue(mut self: Pin<&mut Self>) {
        self.as_mut().rust_mut().playback_order.clear_queue();
        self.as_mut().set_queue_count(0);
        self.as_mut().bump_playlist_revision();
        self.as_mut().set_status(qstring("Playback queue cleared"));
    }

    pub fn queue_selection_state(&self, indices: QString) -> QString {
        let source_indices = selected_source_indices(self, &indices.to_string());
        qstring(selection_state_name(
            self.rust()
                .playback_order
                .queue_selection_state(&source_indices),
        ))
    }

    pub fn toggle_stop_after(mut self: Pin<&mut Self>, indices: QString) {
        let source_indices = selected_source_indices(self.as_ref().get_ref(), &indices.to_string());
        if source_indices.is_empty() {
            return;
        }
        self.as_mut()
            .rust_mut()
            .playback_order
            .toggle_stop_after(&source_indices);
        self.as_mut().bump_playlist_revision();
        self.as_mut().set_status(qstring(format!(
            "Toggled Stop After for {} track{}",
            source_indices.len(),
            if source_indices.len() == 1 { "" } else { "s" }
        )));
    }

    pub fn stop_after_selection_state(&self, indices: QString) -> QString {
        let source_indices = selected_source_indices(self, &indices.to_string());
        qstring(selection_state_name(
            self.rust()
                .playback_order
                .stop_after_selection_state(&source_indices),
        ))
    }

    fn handle_mpris_command(mut self: Pin<&mut Self>, command: MprisCommand) {
        match command {
            MprisCommand::Raise => {
                let serial = self.as_ref().rust().mpris_raise_serial.wrapping_add(1);
                self.as_mut().set_mpris_raise_serial(serial);
            }
            MprisCommand::Next => self.as_mut().next(),
            MprisCommand::Previous => self.as_mut().previous(),
            MprisCommand::Pause => {
                if self.as_ref().rust().playback.state() == PlaybackState::Playing {
                    self.as_mut().play_pause();
                }
            }
            MprisCommand::PlayPause => self.as_mut().play_pause(),
            MprisCommand::Stop => self.as_mut().stop(),
            MprisCommand::Play => {
                if self.as_ref().rust().playback.state() != PlaybackState::Playing {
                    self.as_mut().play_pause();
                }
            }
            MprisCommand::SeekBy(offset_seconds) => {
                let position = self.as_ref().rust().position_seconds;
                let duration = self.as_ref().rust().duration_seconds;
                let target = position + offset_seconds;
                if duration > 0.0 && target > duration {
                    self.as_mut().next();
                } else {
                    self.as_mut().seek(target.max(0.0));
                }
            }
            MprisCommand::SetPosition { track_id, seconds } => {
                let current_track_id = usize::try_from(self.as_ref().rust().current_index)
                    .ok()
                    .map(mpris_track_id);
                let duration = self.as_ref().rust().duration_seconds;
                if current_track_id.as_deref() == Some(track_id.as_str())
                    && seconds >= 0.0
                    && seconds <= duration
                {
                    self.as_mut().seek(seconds);
                }
            }
            MprisCommand::OpenUri(uri) => match url::Url::parse(&uri) {
                Ok(url) if url.scheme() == "file" => match url.to_file_path() {
                    Ok(path) => self
                        .as_mut()
                        .add_local_paths(vec![path], OpeningFilesBehavior::EnqueueAndPlay),
                    Err(()) => self
                        .as_mut()
                        .set_status(qstring("The MPRIS file URI is not a local path")),
                },
                Ok(url) if matches!(url.scheme(), "http" | "https") => self
                    .as_mut()
                    .add_remote_url_value(uri, OpeningFilesBehavior::EnqueueAndPlay),
                Ok(url) => self.as_mut().set_status(qstring(format!(
                    "MPRIS cannot open the unsupported {} URI scheme",
                    url.scheme()
                ))),
                Err(error) => self
                    .as_mut()
                    .set_status(qstring(format!("Opening MPRIS URI: {error}"))),
            },
            MprisCommand::SetLoopStatus(status) => self.as_mut().apply_repeat_mode(match status {
                MprisLoopStatus::None => RepeatMode::Off,
                MprisLoopStatus::Track => RepeatMode::One,
                MprisLoopStatus::Playlist => RepeatMode::All,
            }),
            MprisCommand::SetShuffle(shuffle) => self.as_mut().apply_shuffle_mode(if shuffle {
                ShuffleMode::All
            } else {
                ShuffleMode::Off
            }),
            MprisCommand::SetVolume(volume) => self.as_mut().set_volume_level(volume),
        }
    }

    pub fn poll_playback(mut self: Pin<&mut Self>) {
        let mut handled_mpris_command = false;
        while let Some(command) = self.as_ref().rust().mpris.try_command() {
            self.as_mut().handle_mpris_command(command);
            handled_mpris_command = true;
        }
        if handled_mpris_command {
            self.as_ref()
                .rust()
                .mpris
                .publish(mpris_snapshot(self.as_ref().rust()));
            return;
        }
        if let Some(Err(error)) = self.as_ref().rust().playback.take_seek_result() {
            self.as_mut().set_status(qstring(error));
        }
        if self.as_ref().rust().playback.finished() {
            let current = usize::try_from(self.as_ref().rust().current_index).ok();
            if current
                .is_some_and(|index| self.as_ref().rust().playback_order.should_stop_after(index))
            {
                self.as_mut().stop();
            } else {
                self.as_mut().advance_playback(true);
            }
            self.as_ref()
                .rust()
                .mpris
                .publish(mpris_snapshot(self.as_ref().rust()));
            return;
        }
        if self.as_ref().rust().playback.state() == PlaybackState::Stopped {
            self.as_ref()
                .rust()
                .mpris
                .publish(mpris_snapshot(self.as_ref().rust()));
            return;
        }
        let position = self.as_ref().rust().playback.position().as_secs_f64();
        self.as_mut().set_position_seconds(position);
        self.as_ref()
            .rust()
            .mpris
            .publish(mpris_snapshot(self.as_ref().rust()));
    }

    pub fn poll_audio_levels(mut self: Pin<&mut Self>) {
        let levels = if self.as_ref().rust().playback.state() == PlaybackState::Playing {
            self.as_ref().rust().playback.audio_levels()
        } else {
            [0.0; 5]
        };
        self.as_mut().set_audio_level_low(f64::from(levels[0]));
        self.as_mut().set_audio_level_low_mid(f64::from(levels[1]));
        self.as_mut().set_audio_level_mid(f64::from(levels[2]));
        self.as_mut().set_audio_level_high_mid(f64::from(levels[3]));
        self.as_mut().set_audio_level_high(f64::from(levels[4]));
    }

    pub fn visualizer_frame(&self) -> QString {
        QString::from(self.rust().playback.visualizer_frame())
    }

    pub fn skin_state(&self, include_tracks: bool) -> QString {
        let state = self.rust();
        let current = usize::try_from(state.current_index)
            .ok()
            .and_then(|source| state.visible_indices.iter().position(|i| *i == source))
            .map(|i| i as i64)
            .unwrap_or(-1);
        // Only metadata is exposed to the renderer, never local audio paths.
        let mut snapshot = serde_json::json!({
            "playback": state.playback_state.to_string(), "position": state.position_seconds,
            "duration": state.duration_seconds, "volume": state.volume,
            "songInfo": ([state.current_codec.to_string(), state.current_sample_rate.to_string(),
                state.current_channels.to_string(), state.current_bitrate.to_string()]
                .into_iter().filter(|text| !text.is_empty()).collect::<Vec<_>>().join(" · ")),
            "currentIndex": current, "revision": state.playlist_revision,
            "shuffle": state.shuffle_mode.to_string(), "repeat": state.repeat_mode.to_string(),
            "eqEnabled": state.equalizer_enabled, "eqPreamp": state.equalizer_preamp_db,
            "eq": SKIN_EQ_FREQUENCIES.map(|hz| interpolate_eq(&kog_core::equalizer::EQUALIZER_FREQUENCIES, &state.equalizer_settings.gains_db, hz)),
            "visualization": serde_json::from_str::<serde_json::Value>(&state.playback.visualizer_frame()).unwrap_or_default()
        });
        if include_tracks {
            snapshot["tracks"] = state.visible_indices.iter().filter_map(|i| state.tracks.get(*i).map(|track| {
                serde_json::json!({"id":i.to_string(), "title":track.title, "artist":track.artist,
                    "album":track.album, "duration":track.duration.map(|d| d.as_secs_f64()).unwrap_or(0.0)})
            })).collect::<Vec<_>>().into();
        }
        QString::from(snapshot.to_string())
    }

    pub fn update_skin_equalizer_band(mut self: Pin<&mut Self>, index: i32, gain_db: f64) {
        let Some(index) = usize::try_from(index).ok().filter(|i| *i < 10) else {
            return;
        };
        let Some(gain) = valid_equalizer_gain(gain_db) else {
            return;
        };
        let mut settings = self.rust().equalizer_settings.clone();
        set_interpolated_eq_gain(
            &kog_core::equalizer::EQUALIZER_FREQUENCIES,
            &mut settings.gains_db,
            SKIN_EQ_FREQUENCIES[index],
            gain,
        );
        self.as_mut()
            .commit_equalizer_settings(settings, "Modern skin equalizer updated");
    }

    pub fn show_now_playing_notification(mut self: Pin<&mut Self>) {
        let has_track = usize::try_from(self.as_ref().rust().current_index)
            .ok()
            .is_some_and(|index| index < self.as_ref().rust().tracks.len());
        if !has_track {
            return;
        }
        let serial = self.as_ref().rust().notification_serial.wrapping_add(1);
        self.as_mut().set_notification_serial(serial);
    }

    pub fn equalizer_band_gain(&self, index: i32) -> f64 {
        usize::try_from(index)
            .ok()
            .and_then(|index| self.rust().equalizer_settings.gains_db.get(index))
            .copied()
            .map(f64::from)
            .unwrap_or_default()
    }

    pub fn update_equalizer_enabled(mut self: Pin<&mut Self>, enabled: bool) {
        let mut settings = self.as_ref().rust().equalizer_settings.clone();
        settings.enabled = enabled;
        self.as_mut().commit_equalizer_settings(
            settings,
            if enabled {
                "Equalizer enabled"
            } else {
                "Equalizer disabled"
            },
        );
    }

    pub fn update_equalizer_tracking(mut self: Pin<&mut Self>, enabled: bool) {
        let mut settings = self.as_ref().rust().equalizer_settings.clone();
        settings.track_genre = enabled;
        if enabled {
            let genre = {
                let this = self.as_ref();
                let rust = this.rust();
                usize::try_from(rust.current_index)
                    .ok()
                    .and_then(|index| rust.tracks.get(index))
                    .map(|track| track.genre.clone())
                    .unwrap_or_default()
            };
            apply_preset(&mut settings, preset_for_genre(&genre));
        }
        self.as_mut().commit_equalizer_settings(
            settings,
            if enabled {
                "Equalizer will track genre tags"
            } else {
                "Equalizer genre tracking disabled"
            },
        );
    }

    pub fn update_equalizer_preamp(mut self: Pin<&mut Self>, gain_db: f64) {
        let Some(gain_db) = valid_equalizer_gain(gain_db) else {
            self.as_mut()
                .set_status(qstring("Equalizer gain must be between -20 and +20 dB"));
            return;
        };
        let mut settings = self.as_ref().rust().equalizer_settings.clone();
        settings.preamp_db = gain_db;
        settings.preset_name = "Custom".to_owned();
        self.as_mut()
            .commit_equalizer_settings(settings, "Equalizer preamp adjusted");
    }

    pub fn update_equalizer_band(mut self: Pin<&mut Self>, index: i32, gain_db: f64) {
        let Some(index) = usize::try_from(index)
            .ok()
            .filter(|index| *index < kog_core::equalizer::EQUALIZER_FREQUENCIES.len())
        else {
            self.as_mut()
                .set_status(qstring("That equalizer band does not exist"));
            return;
        };
        let Some(gain_db) = valid_equalizer_gain(gain_db) else {
            self.as_mut()
                .set_status(qstring("Equalizer gain must be between -20 and +20 dB"));
            return;
        };
        let mut settings = self.as_ref().rust().equalizer_settings.clone();
        settings.gains_db[index] = gain_db;
        settings.preset_name = "Custom".to_owned();
        self.as_mut()
            .commit_equalizer_settings(settings, "Equalizer curve adjusted");
    }

    pub fn select_equalizer_preset(mut self: Pin<&mut Self>, name: QString) {
        let name = name.to_string();
        let mut settings = self.as_ref().rust().equalizer_settings.clone();
        if name == "Custom" {
            settings.preset_name = name.clone();
        } else if let Some(preset) = preset_named(&name) {
            apply_preset(&mut settings, preset);
        } else {
            self.as_mut()
                .set_status(qstring(format!("Unknown equalizer preset: {name}")));
            return;
        }
        self.as_mut()
            .commit_equalizer_settings(settings, &format!("Equalizer preset changed to {name}"));
    }

    pub fn flatten_equalizer(mut self: Pin<&mut Self>) {
        let mut settings = self.as_ref().rust().equalizer_settings.clone();
        apply_preset(
            &mut settings,
            preset_named("Flat").expect("bundled equalizer presets include Flat"),
        );
        self.as_mut()
            .commit_equalizer_settings(settings, "Equalizer flattened");
    }

    pub fn level_equalizer_preamp(mut self: Pin<&mut Self>) {
        let mut settings = self.as_ref().rust().equalizer_settings.clone();
        let maximum = settings.gains_db.iter().copied().fold(0.0_f32, f32::max);
        if maximum <= 0.0 || settings.preamp_db == -maximum {
            self.as_mut().set_status(qstring(
                "Equalizer preamp already leaves headroom for the current curve",
            ));
            return;
        }
        settings.preamp_db = -maximum;
        settings.preset_name = "Custom".to_owned();
        self.as_mut()
            .commit_equalizer_settings(settings, "Equalizer preamp leveled");
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
        if self.rust().playback_order.should_stop_after(source_index) {
            qstring("■")
        } else if self.rust().current_index == saturating_i32(source_index) {
            match self.rust().playback.state() {
                PlaybackState::Playing => qstring("▶"),
                PlaybackState::Paused => qstring("Ⅱ"),
                PlaybackState::Stopped => QString::default(),
            }
        } else if self
            .rust()
            .playback_order
            .queue_position(source_index)
            .is_some()
        {
            qstring("+")
        } else {
            QString::default()
        }
    }

    pub fn track_status_message_at(&self, index: i32) -> QString {
        let Some(source_index) = visible_source_index(self, index) else {
            return QString::default();
        };
        if self.rust().playback_order.should_stop_after(source_index) {
            qstring("Playback will stop after this track")
        } else if self.rust().current_index == saturating_i32(source_index) {
            qstring(match self.rust().playback.state() {
                PlaybackState::Playing => "Playing",
                PlaybackState::Paused => "Paused",
                PlaybackState::Stopped => "Current track",
            })
        } else if let Some(position) = self.rust().playback_order.queue_position(source_index) {
            qstring(format!("Queued at position {}", position + 1))
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

    pub fn track_missing_at(&self, index: i32) -> bool {
        visible_source_index(self, index)
            .and_then(|source_index| self.rust().tracks.get(source_index))
            .is_some_and(|track| track.missing)
    }

    pub fn track_value_at(&self, index: i32, column: QString) -> QString {
        let Some(source_index) = visible_source_index(self, index) else {
            return QString::default();
        };
        let Some(track) = self.rust().tracks.get(source_index) else {
            return QString::default();
        };
        match column.to_string().as_str() {
            "index" => qstring((source_index + 1).to_string()),
            "star" => {
                if self.rust().starred.contains(&star_key_for_track(track)) {
                    qstring("★")
                } else {
                    QString::default()
                }
            }
            "status" => self.track_status_at(index),
            "rating" | "playcount" => QString::default(),
            "title" => qstring(&track.title),
            "albumartist" => qstring(&track.album_artist),
            "artist" => qstring(&track.artist),
            "composer" => qstring(&track.composer),
            "album" => qstring(&track.album),
            "length" => qstring(track.duration_label()),
            "date" => track
                .year
                .map(|year| qstring(year.to_string()))
                .unwrap_or_default(),
            "genre" => qstring(&track.genre),
            "track" => track
                .track_number
                .map(|number| qstring(number.to_string()))
                .unwrap_or_default(),
            "path" => qstring(track_path(track)),
            "filename" => qstring(track_filename(track)),
            "codec" => qstring(&track.codec),
            "samplerate" => qstring(sample_rate_label(track.sample_rate)),
            "bitspersample" => track
                .bits_per_sample
                .map(|bits| qstring(bits.to_string()))
                .unwrap_or_default(),
            "bitrate" => track
                .bitrate
                .map(|bitrate| qstring(format!("{bitrate} kbps")))
                .unwrap_or_default(),
            _ => QString::default(),
        }
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

    pub fn import_sc55_rom_archive(mut self: Pin<&mut Self>, url: QUrl) {
        let Some(local_file) = url.to_local_file() else {
            self.as_mut()
                .set_midi_status(qstring("Only a local SC-55 ROM archive can be selected"));
            return;
        };
        let archive = PathBuf::from(local_file.to_string());
        let imported = match import_rom_archive(&archive, RomKind::Sc55) {
            Ok(imported) => imported,
            Err(error) => {
                self.as_mut().set_midi_status(qstring(error));
                return;
            }
        };
        let model = match kog_audio::sc55::validate_rom_directory(&imported.directory) {
            Ok(model) => model,
            Err(error) => {
                let _ = std::fs::remove_dir_all(&imported.directory);
                self.as_mut().set_midi_status(qstring(format!(
                    "The imported SC-55 archive is not a complete supported ROM set: {error}"
                )));
                return;
            }
        };
        let directory = QUrl::from_local_file(&qstring(imported.directory.to_string_lossy()));
        self.as_mut().set_sc55_rom_directory(directory);
        self.as_mut().set_status(qstring(rom_import_status(
            "SC-55", &model, &archive, &imported,
        )));
    }

    pub fn choose_sc55_rom_archive(mut self: Pin<&mut Self>) {
        let initial_directory = self.as_ref().rust().directory.clone();
        let Some(path) = rfd::FileDialog::new()
            .set_title("Import a compressed SC-55 ROM set")
            .set_directory(initial_directory)
            .add_filter(
                "ROM archives",
                &[
                    "zip", "7z", "rar", "tar", "tgz", "tbz", "tbz2", "txz", "gz", "bz2", "xz",
                ],
            )
            .pick_file()
        else {
            return;
        };
        let url = QUrl::from_local_file(&qstring(path.to_string_lossy()));
        self.as_mut().import_sc55_rom_archive(url);
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

    pub fn import_mt32_rom_archive(mut self: Pin<&mut Self>, url: QUrl) {
        let Some(local_file) = url.to_local_file() else {
            self.as_mut()
                .set_midi_status(qstring("Only a local MT-32 ROM archive can be selected"));
            return;
        };
        let archive = PathBuf::from(local_file.to_string());
        let imported = match import_rom_archive(&archive, RomKind::Mt32) {
            Ok(imported) => imported,
            Err(error) => {
                self.as_mut().set_midi_status(qstring(error));
                return;
            }
        };
        let model = match kog_audio::mt32::validate_rom_directory(&imported.directory) {
            Ok(model) => model,
            Err(error) => {
                let _ = std::fs::remove_dir_all(&imported.directory);
                self.as_mut().set_midi_status(qstring(format!(
                    "The imported MT-32 archive has no complete compatible ROM pair: {error}"
                )));
                return;
            }
        };
        let directory = QUrl::from_local_file(&qstring(imported.directory.to_string_lossy()));
        self.as_mut().set_mt32_rom_directory(directory);
        self.as_mut().set_status(qstring(rom_import_status(
            "MT-32/CM-32L",
            &model,
            &archive,
            &imported,
        )));
    }

    pub fn choose_mt32_rom_archive(mut self: Pin<&mut Self>) {
        let initial_directory = self.as_ref().rust().directory.clone();
        let Some(path) = rfd::FileDialog::new()
            .set_title("Import a compressed MT-32 or CM-32L ROM set")
            .set_directory(initial_directory)
            .add_filter(
                "ROM archives",
                &[
                    "zip", "7z", "rar", "tar", "tgz", "tbz", "tbz2", "txz", "gz", "bz2", "xz",
                ],
            )
            .pick_file()
        else {
            return;
        };
        let url = QUrl::from_local_file(&qstring(path.to_string_lossy()));
        self.as_mut().import_mt32_rom_archive(url);
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

    pub fn update_mt32_gm_program_mapping(mut self: Pin<&mut Self>, enabled: bool) {
        if let Err(error) = AppSettings::save_mt32_gm_program_mapping(enabled) {
            self.as_mut().set_midi_status(qstring(error));
            return;
        }
        self.as_ref()
            .rust()
            .decoder_settings
            .set_mt32_gm_program_mapping(enabled);
        self.as_mut().set_mt32_gm_program_mapping(enabled);
        self.as_mut().set_status(qstring(if enabled {
            "General MIDI programs will be mapped to compatible MT-32 patches"
        } else {
            "MT-32 MIDI will use its original program numbers"
        }));
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
            OpeningFilesBehavior::ClearAndPlay => {
                "Opening files will clear the playlist and start playback"
            }
            OpeningFilesBehavior::Enqueue => "Opening files will enqueue without starting playback",
            OpeningFilesBehavior::EnqueueAndPlay => "Opening files will enqueue and start playback",
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

    pub fn update_show_tray_icon(mut self: Pin<&mut Self>, enabled: bool) {
        if let Err(error) = AppSettings::save_show_tray_icon(enabled) {
            self.as_mut().set_status(qstring(error));
            return;
        }
        self.as_mut().set_show_tray_icon(enabled);
    }

    pub fn update_close_to_tray(mut self: Pin<&mut Self>, enabled: bool) {
        if let Err(error) = AppSettings::save_close_to_tray(enabled) {
            self.as_mut().set_status(qstring(error));
            return;
        }
        self.as_mut().set_close_to_tray(enabled);
    }

    pub fn update_minimize_to_tray(mut self: Pin<&mut Self>, enabled: bool) {
        if let Err(error) = AppSettings::save_minimize_to_tray(enabled) {
            self.as_mut().set_status(qstring(error));
            return;
        }
        self.as_mut().set_minimize_to_tray(enabled);
    }

    pub fn update_track_notifications(mut self: Pin<&mut Self>, enabled: bool) {
        if let Err(error) = AppSettings::save_track_notifications(enabled) {
            self.as_mut().set_status(qstring(error));
            return;
        }
        self.as_mut().set_track_notifications(enabled);
        self.as_mut().set_status(qstring(if enabled {
            "Track change notifications enabled"
        } else {
            "Track change notifications disabled"
        }));
    }

    /// Load whichever synthesis backend the user selected ahead of the
    /// first MIDI track, on a background thread: SoundFont parsing and
    /// SC-55 emulator boot are both slow one-time costs that would
    /// otherwise be paid on the first play.
    pub fn prewarm_synths(&self) {
        let settings = self.rust().decoder_settings.clone();
        let engine = settings.midi_engine();
        let spawned = std::thread::Builder::new()
            .name("kog-synth-prewarm".to_owned())
            .spawn(move || match engine {
                MidiEngine::Sc55 => {
                    if let Some(rom_directory) = settings.sc55_rom_path() {
                        kog_audio::sc55::warm_sc55_server(&rom_directory);
                    }
                }
                MidiEngine::RustySynth => {
                    if let Some(path) = settings.soundfont_path() {
                        let _ = kog_audio::decoder::warm_soundfont(&path);
                    }
                }
                // The remaining engines synthesize in-process with no
                // startup cost worth caching.
                MidiEngine::Opl3Windows | MidiEngine::Mt32 => {}
            });
        let _ = spawned;
    }

    pub fn update_download_cover_art(mut self: Pin<&mut Self>, enabled: bool) {
        if let Err(error) = AppSettings::save_download_cover_art(enabled) {
            self.as_mut().set_status(qstring(error));
            return;
        }
        self.as_mut().set_download_cover_art(enabled);
        if !enabled {
            if let Some(job) = self.as_mut().rust_mut().cover_art.take() {
                job.cancel.store(true, AtomicOrdering::Relaxed);
            }
            self.as_mut().set_status(qstring("Cover art downloads disabled"));
            return;
        }
        self.as_mut()
            .set_status(qstring("Cover art downloads enabled"));
        let current = usize::try_from(self.as_ref().rust().current_index)
            .ok()
            .and_then(|index| self.as_ref().rust().tracks.get(index).cloned());
        if let Some(track) = current {
            self.as_mut().refresh_cover_art(
                track.artist.clone(),
                track.album.clone(),
                track.source.path.clone(),
            );
        }
    }

    pub fn poll_cover_art(mut self: Pin<&mut Self>) {
        let generation = match self.as_ref().rust().cover_art.as_ref() {
            Some(job) => job.generation,
            None => return,
        };
        let mut resolved = None;
        let mut received = false;
        if let Some(job) = self.as_ref().rust().cover_art.as_ref() {
            while let Ok(event) = job.receiver.try_recv() {
                if event.generation == generation {
                    resolved = event.path;
                    received = true;
                }
            }
        }
        if !received {
            return;
        }
        if self
            .as_ref()
            .rust()
            .cover_art
            .as_ref()
            .is_some_and(|job| job.generation == generation)
        {
            self.as_mut().rust_mut().cover_art = None;
        }
        if let Some(path) = resolved {
            self.as_mut()
                .set_current_artwork_path(qstring(path.to_string_lossy()));
        }
    }

    fn refresh_cover_art(mut self: Pin<&mut Self>, artist: String, album: String, file: PathBuf) {
        let generation = self
            .as_ref()
            .rust()
            .cover_art_generation
            .wrapping_add(1);
        self.as_mut().rust_mut().cover_art_generation = generation;
        if let Some(job) = self.as_mut().rust_mut().cover_art.take() {
            job.cancel.store(true, AtomicOrdering::Relaxed);
        }
        self.as_mut()
            .set_current_artwork_path(QString::default());
        let tagged_album = album.clone();
        let album = kog_audio::cover_art::fallback_album(&file, &album);
        if album.is_empty() {
            return;
        }
        let cache_dir = cover_art_cache_dir();
        let (key, may_download) = cover_art_key(&artist, &tagged_album, &album, &file);
        if let Some(cached) = kog_audio::cover_art::cache_lookup(&cache_dir, &key) {
            self.as_mut()
                .set_current_artwork_path(qstring(cached.to_string_lossy()));
            return;
        }
        if let Some(bytes) = kog_audio::cover_art::embedded_cover_bytes(&file) {
            if let Some(stored) = kog_audio::cover_art::store_cache(&cache_dir, &key, &bytes) {
                self.as_mut()
                    .set_current_artwork_path(qstring(stored.to_string_lossy()));
                return;
            }
        }
        // Sibling scans beside the file (folder.jpg, a covers/ folder):
        // folder-scoped so one album shares one cached copy without
        // leaking across folders. Untitled rips with art in a covers/
        // subdirectory resolve here instead of downloading blind.
        if file.is_file() {
            if let Some(folder) = file.parent() {
                let folder_key = kog_audio::cover_art::folder_cover_key(folder);
                if let Some(cached) = kog_audio::cover_art::cache_lookup(&cache_dir, &folder_key) {
                    self.as_mut()
                        .set_current_artwork_path(qstring(cached.to_string_lossy()));
                    return;
                }
                if let Some(bytes) = kog_audio::cover_art::sibling_cover_bytes(&file) {
                    if let Some(stored) =
                        kog_audio::cover_art::store_cache(&cache_dir, &folder_key, &bytes)
                    {
                        self.as_mut()
                            .set_current_artwork_path(qstring(stored.to_string_lossy()));
                        return;
                    }
                }
            }
        }
        if !may_download || !self.as_ref().rust().download_cover_art {
            return;
        }
        let (sender, receiver) = std::sync::mpsc::sync_channel(2);
        let cancel = Arc::new(AtomicBool::new(false));
        self.as_mut().rust_mut().cover_art = Some(CoverArtState {
            receiver,
            cancel: Arc::clone(&cancel),
            generation,
        });
        let request = CoverArtRequest {
            generation,
            artist,
            album,
            cache_dir,
        };
        if std::thread::Builder::new()
            .name("kog-cover-art".to_owned())
            .spawn(move || run_cover_art_job(request, sender, cancel))
            .is_err()
        {
            self.as_mut().rust_mut().cover_art = None;
        }
    }

    fn commit_equalizer_settings(
        mut self: Pin<&mut Self>,
        settings: EqualizerSettings,
        status: &str,
    ) {
        if let Err(error) = AppSettings::save_equalizer(&settings) {
            self.as_mut().set_status(qstring(error));
            return;
        }
        self.as_ref()
            .rust()
            .playback
            .set_equalizer(settings.clone());
        let revision = self.as_ref().rust().equalizer_revision.wrapping_add(1);
        self.as_mut().rust_mut().equalizer_settings = settings.clone();
        self.as_mut().set_equalizer_enabled(settings.enabled);
        self.as_mut()
            .set_equalizer_track_genre(settings.track_genre);
        self.as_mut()
            .set_equalizer_preamp_db(f64::from(settings.preamp_db));
        self.as_mut()
            .set_equalizer_preset(qstring(settings.preset_name));
        self.as_mut().set_equalizer_revision(revision);
        self.as_mut().set_status(qstring(status));
    }

    fn apply_output_device(
        mut self: Pin<&mut Self>,
        output_device: Option<OutputDevice>,
        success_status: &str,
    ) {
        let output_device_id = output_device.as_ref().map(|device| device.id.clone());
        let current = self.as_ref().rust().output_device_id.to_string();
        if current == output_device_id.as_deref().unwrap_or_default() {
            self.as_mut()
                .set_output_device_status(qstring(success_status));
            self.as_mut().set_status(qstring(success_status));
            return;
        }

        let (previous_state, source_index, position) = {
            let this = self.as_ref();
            let rust = this.rust();
            (
                rust.playback.state(),
                usize::try_from(rust.current_index).ok(),
                rust.playback.position(),
            )
        };
        if let Err(error) = self
            .as_mut()
            .rust_mut()
            .playback
            .switch_output_device(output_device_id.clone())
        {
            self.as_mut().set_output_device_status(qstring(&error));
            self.as_mut().set_status(qstring(error));
            return;
        }

        self.as_mut()
            .set_output_device_id(output_device_id.as_deref().map(qstring).unwrap_or_default());
        let preference = output_device.as_ref().map(|device| OutputDevicePreference {
            id: device.id.clone(),
            name: device.name.clone(),
        });
        let save_error = AppSettings::save_output_device(preference.as_ref()).err();
        let mut status = success_status.to_owned();

        if previous_state != PlaybackState::Stopped
            && let Some(source_index) = source_index
        {
            self.as_mut().play_source_index(source_index);
            if self.as_ref().rust().playback.state() == PlaybackState::Stopped {
                status.push_str("; playback could not be resumed");
            } else {
                if !position.is_zero() {
                    match self.as_ref().rust().playback.seek(position) {
                        Ok(()) => self.as_mut().set_position_seconds(position.as_secs_f64()),
                        Err(error) => {
                            status.push_str(&format!("; restoring position failed: {error}"));
                        }
                    }
                }
                if previous_state == PlaybackState::Paused {
                    self.as_mut().rust_mut().playback.play_pause();
                }
                self.as_mut().sync_playback_state();
            }
        }
        if let Some(error) = save_error {
            status.push_str(&format!("; the selection could not be saved: {error}"));
        }
        self.as_mut().set_output_device_status(qstring(&status));
        self.as_mut().set_status(qstring(status));
    }

    fn rebuild_playlist(mut self: Pin<&mut Self>) {
        self.as_mut().rust_mut().rebuild_visible_indices();
        let count = saturating_i32(self.as_ref().rust().visible_indices.len());
        let revision = self.as_ref().rust().playlist_revision.wrapping_add(1);
        let duration = self.as_ref().rust().total_duration_value();
        self.as_mut().set_playlist_count(count);
        self.as_mut().set_playlist_revision(revision);
        self.as_mut().set_total_duration(duration);
    }

    fn add_local_paths(
        mut self: Pin<&mut Self>,
        paths: Vec<PathBuf>,
        behavior: OpeningFilesBehavior,
    ) {
        if paths.is_empty() {
            return;
        }
        if self.as_ref().rust().directory_scan_active {
            self.as_mut().set_status(qstring(
                "A folder scan is already running; cancel it before adding more files",
            ));
            return;
        }
        self.as_mut().begin_directory_scan(paths, behavior);
    }

    fn begin_directory_scan(
        mut self: Pin<&mut Self>,
        paths: Vec<PathBuf>,
        behavior: OpeningFilesBehavior,
    ) {
        if behavior.clears_playlist() {
            self.as_mut().clear_playlist();
        }
        let first_new_source_index = self.as_ref().rust().tracks.len();
        let (sender, receiver) = std::sync::mpsc::sync_channel(64);
        let cancel = Arc::new(AtomicBool::new(false));
        let decoder_settings = self.as_ref().rust().decoder_settings.clone();
        let worker_decoders = self
            .as_ref()
            .rust()
            .decoders
            .background_worker(decoder_settings.clone());
        let known_sources = self
            .as_ref()
            .rust()
            .tracks
            .iter()
            .map(|track| track.source.clone())
            .collect();
        let read_cue_sheets = self.as_ref().rust().read_cue_sheets_in_folders;
        let read_playlists = self.as_ref().rust().read_playlists_in_folders;
        self.as_mut().rust_mut().directory_scan = Some(DirectoryScanState {
            receiver,
            cancel: Arc::clone(&cancel),
            cancel_requested: false,
            behavior,
            first_new_source_index,
            combined: AddPathResult::default(),
            known_sources,
            playlist_dirty: false,
            last_playlist_refresh: Instant::now(),
        });
        self.as_mut().set_directory_scan_files_scanned(0);
        self.as_mut().set_directory_scan_tracks_added(0);
        self.as_mut()
            .set_directory_scan_current_path(qstring("Finding files…"));
        self.as_mut().set_directory_scan_active(true);
        self.as_mut().set_status(qstring("Loading music…"));

        if let Err(error) = std::thread::Builder::new()
            .name("kog-directory-scan".to_owned())
            .spawn(move || {
                scan_directory_paths(
                    paths,
                    sender,
                    cancel,
                    worker_decoders,
                    decoder_settings,
                    read_cue_sheets,
                    read_playlists,
                )
            })
        {
            self.as_mut().rust_mut().directory_scan = None;
            self.as_mut().set_directory_scan_active(false);
            self.as_mut()
                .set_status(qstring(format!("Starting the folder scanner: {error}")));
        }
    }

    fn finish_directory_scan(mut self: Pin<&mut Self>, worker_cancelled: bool) {
        let Some(scan) = self.as_mut().rust_mut().directory_scan.take() else {
            return;
        };
        let cancelled = worker_cancelled || scan.cancel_requested;
        let files_scanned = self.as_ref().rust().directory_scan_files_scanned;
        let tracks_added = scan.combined.added;
        self.as_mut().set_directory_scan_active(false);
        self.as_mut()
            .set_directory_scan_current_path(QString::default());

        let mut status = if cancelled {
            format!(
                "Music load cancelled — {files_scanned} files scanned, {tracks_added} tracks added"
            )
        } else {
            format!(
                "Music load complete — {files_scanned} files scanned, {tracks_added} tracks added"
            )
        };
        if let Some(warning) = scan.combined.warning {
            status.push_str("; ");
            status.push_str(&warning);
        }
        self.as_mut().set_status(qstring(status));

        if !cancelled && tracks_added > 0 && scan.behavior.starts_playback() {
            self.as_mut().play_source_index(scan.first_new_source_index);
        }
    }

    fn refresh_playback_order(mut self: Pin<&mut Self>) {
        let queue_count = self.as_mut().rust_mut().sync_tracks_in_playback_order();
        self.as_mut().set_queue_count(saturating_i32(queue_count));
    }

    fn advance_playback(mut self: Pin<&mut Self>, honor_repeat_one: bool) {
        // Walk past verifiably-gone tracks instead of stopping on them.
        // The budget caps the walk below two full passes so repeat-all
        // over an all-missing pane still reaches the end handling.
        let budget = self.as_ref().rust().tracks.len().saturating_add(1).max(1);
        let mut skips = 0_usize;
        loop {
            let tracks = self.as_ref().rust().tracks.clone();
            let current = usize::try_from(self.as_ref().rust().current_index).ok();
            let (target, queue_count) = {
                let mut rust = self.as_mut().rust_mut();
                let target = rust.playback_order.next(&tracks, current, honor_repeat_one);
                (target, rust.playback_order.queue_count())
            };
            self.as_mut().set_queue_count(saturating_i32(queue_count));
            let Some(target) = target else {
                self.as_mut().advance_past_end();
                return;
            };
            if self
                .as_ref()
                .rust()
                .tracks
                .get(target)
                .is_some_and(|track| track.missing)
            {
                skips += 1;
                if skips >= budget {
                    self.as_mut().advance_past_end();
                    return;
                }
                continue;
            }
            match self.as_mut().play_source_index(target) {
                PlayOutcome::Started => return,
                PlayOutcome::Skipped => {
                    skips += 1;
                    if skips >= budget {
                        self.as_mut().advance_past_end();
                        return;
                    }
                }
            }
        }
    }

    /// End-of-playlist handling for advance: radio pulls a staged track,
    /// otherwise playback stops. Shared by advancement and by skip
    /// exhaustion over an all-missing pane.
    fn advance_past_end(mut self: Pin<&mut Self>) {
        if self.as_ref().rust().radio_active {
            // End of the playlist with radio on: pull one staged track into
            // the playlist instead of stopping. Stop-after never reaches here;
            // it stops explicitly before advancing.
            let pulled = self
                .as_ref()
                .rust()
                .radio
                .as_ref()
                .is_some_and(|radio| !radio.ready.is_empty());
            if pulled {
                if let Some(index) = self.as_mut().shift_radio_track() {
                    self.as_mut().play_shifted_radio_track(index);
                } else {
                    self.as_mut().stop();
                }
            } else if self.as_ref().rust().tracks.is_empty() {
                // Explicit next on an empty radio playlist with nothing
                // staged yet: arm kickstart so the next staged track
                // autoplays instead of sitting stopped.
                self.as_mut().stop();
                if let Some(radio) = self.as_mut().rust_mut().radio.as_mut() {
                    radio.kickstart_armed = true;
                }
                self.as_mut()
                    .set_status(qstring("Random Radio — finding a track…"));
            } else {
                self.as_mut().stop();
            }
        } else {
            self.as_mut().stop();
        }
    }

    fn apply_shuffle_mode(mut self: Pin<&mut Self>, mode: ShuffleMode) {
        if self.as_ref().rust().playback_order.shuffle_mode() == mode {
            return;
        }
        if let Err(error) = AppSettings::save_shuffle_mode(mode) {
            self.as_mut().set_status(qstring(error));
            return;
        }
        let tracks = self.as_ref().rust().tracks.clone();
        let current = usize::try_from(self.as_ref().rust().current_index).ok();
        {
            self.as_mut()
                .rust_mut()
                .playback_order
                .set_shuffle_mode(mode, &tracks, current);
        }
        self.as_mut()
            .set_shuffle_mode(qstring(mode.setting_value()));
        self.as_mut().set_status(qstring(match mode {
            ShuffleMode::Off => "Shuffle off",
            ShuffleMode::Albums => "Shuffle albums",
            ShuffleMode::All => "Shuffle all tracks",
        }));
    }

    fn apply_repeat_mode(mut self: Pin<&mut Self>, mode: RepeatMode) {
        if self.as_ref().rust().playback_order.repeat_mode() == mode {
            return;
        }
        if let Err(error) = AppSettings::save_repeat_mode(mode) {
            self.as_mut().set_status(qstring(error));
            return;
        }
        self.as_mut()
            .rust_mut()
            .playback_order
            .set_repeat_mode(mode);
        self.as_mut().set_repeat_mode(qstring(mode.setting_value()));
        if mode != RepeatMode::Off && self.as_ref().rust().radio_active {
            self.as_mut().teardown_radio();
            if let Err(error) = AppSettings::save_radio_enabled(false) {
                self.as_mut().set_status(qstring(error));
                return;
            }
            self.as_mut()
                .set_status(qstring("Random Radio off — repeat on"));
            return;
        }
        self.as_mut().set_status(qstring(match mode {
            RepeatMode::Off => "Repeat off",
            RepeatMode::One => "Repeat one track",
            RepeatMode::Album => "Repeat album",
            RepeatMode::All => "Repeat all tracks",
        }));
    }

    /// Play a freshly-shifted radio pick. If it fails to open, drop the dead
    /// pick and try the next staged track (bounded), so one bad file never
    /// stops radio. Manual plays keep the existing stop-with-error behavior.
    fn play_shifted_radio_track(mut self: Pin<&mut Self>, mut index: usize) {
        for _ in 0..5 {
            self.as_mut().play_source_index(index);
            if self.as_ref().rust().playback.state() != PlaybackState::Stopped {
                return;
            }
            // The corpse appended at `index` is addressed by visible row for
            // removal; a filter-hidden corpse is simply left behind unplayed.
            let row = self
                .as_ref()
                .rust()
                .visible_indices
                .iter()
                .position(|&source| source == index);
            if let Some(row) = row {
                self.as_mut().remove_track(saturating_i32(row));
            }
            let Some(next) = self.as_mut().shift_radio_track() else {
                return;
            };
            index = next;
        }
    }

    fn play_source_index(mut self: Pin<&mut Self>, source_index: usize) -> PlayOutcome {
        let Some((source, genre, notification)) = self
            .as_ref()
            .get_ref()
            .rust()
            .tracks
            .get(source_index)
            .map(|track| {
                let rust = self.as_ref().get_ref().rust();
                let is_new_start = rust.playback.state() == PlaybackState::Stopped
                    || usize::try_from(rust.current_index).ok() != Some(source_index);
                let notification = rust.track_notifications && is_new_start;
                (track.source.clone(), track.genre.clone(), notification)
            })
        else {
            return PlayOutcome::Started;
        };
        if self.as_ref().rust().equalizer_settings.track_genre {
            let mut settings = self.as_ref().rust().equalizer_settings.clone();
            apply_preset(&mut settings, preset_for_genre(&genre));
            let preset_name = settings.preset_name.clone();
            self.as_mut().commit_equalizer_settings(
                settings,
                &format!("Equalizer matched genre with {preset_name}"),
            );
        }
        match self.as_mut().rust_mut().playback.play_source(&source) {
            Ok(backend) => {
                let previous = usize::try_from(self.as_ref().rust().current_index).ok();
                self.as_mut()
                    .rust_mut()
                    .playback_order
                    .clear_stop_after_when_leaving(previous, source_index);
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
                if notification {
                    self.as_mut().show_now_playing_notification();
                }
                PlayOutcome::Started
            }
            Err(error) => {
                // A verifiably-gone file grays out for skipping instead of
                // stopping the whole playlist on it; anything else keeps
                // the existing stop-with-error behavior.
                let title = self
                    .as_ref()
                    .rust()
                    .tracks
                    .get(source_index)
                    .map(|track| track.title.clone())
                    .unwrap_or_default();
                let gone = self
                    .as_ref()
                    .rust()
                    .tracks
                    .get(source_index)
                    .is_some_and(|track| playback_source_is_missing(&track.source));
                if gone {
                    if let Some(track) = self.as_mut().rust_mut().tracks.get_mut(source_index) {
                        track.missing = true;
                    }
                    self.as_mut().bump_playlist_revision();
                    let trimmed = title.trim();
                    self.as_mut().set_status(qstring(if trimmed.is_empty() {
                        "Skipping missing file".to_owned()
                    } else {
                        format!("Skipping missing file: {trimmed}")
                    }));
                    return PlayOutcome::Skipped;
                }
                self.as_mut().set_status(qstring(error));
                self.as_mut().rust_mut().playback.stop();
                self.as_mut().sync_playback_state();
                PlayOutcome::Started
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
        self.as_mut().refresh_cover_art(
            track.artist.clone(),
            track.album.clone(),
            track.source.path.clone(),
        );
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
        let generation = self
            .as_ref()
            .rust()
            .cover_art_generation
            .wrapping_add(1);
        self.as_mut().rust_mut().cover_art_generation = generation;
        if let Some(job) = self.as_mut().rust_mut().cover_art.take() {
            job.cancel.store(true, AtomicOrdering::Relaxed);
        }
        self.as_mut()
            .set_current_artwork_path(QString::default());
    }

    fn sync_playback_state(mut self: Pin<&mut Self>) {
        let state = self.as_ref().rust().playback.state();
        self.as_mut().set_playback_state(qstring(state.as_str()));
        self.as_mut().bump_playlist_revision();
    }

    fn bump_playlist_revision(mut self: Pin<&mut Self>) {
        let revision = self.as_ref().rust().playlist_revision.wrapping_add(1);
        self.as_mut().set_playlist_revision(revision);
    }

    fn bump_playlists_revision(mut self: Pin<&mut Self>) {
        let revision = self.as_ref().rust().playlists_revision.wrapping_add(1);
        self.as_mut().set_playlists_revision(revision);
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
        // A new tree root invalidates everything radio staged from the old
        // one: drop the ready buffer and in-flight jobs, then stage from
        // the new folder (resuming its saved round when one waits).
        if self.as_ref().rust().radio_active {
            self.as_mut().teardown_radio();
            let (initial, resumed) = match load_radio_round(&path) {
                Some(initial) => (initial, true),
                None => (
                    kog_audio::radio::RoundInitial::fresh(kog_audio::radio::random_seed()),
                    false,
                ),
            };
            self.as_mut().begin_radio_session(
                path,
                initial,
                if resumed {
                    "Random Radio — new folder, resumed".to_owned()
                } else {
                    "Random Radio — new folder, staging fresh".to_owned()
                },
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        AddPathResult, DirectoryScanEvent, PlaylistSortColumn, add_path_status, compare_tracks,
        count_delete_entries, cover_art_key, dropped_urls_from_json, local_paths_from_json,
        move_selected_items, natural_compare, normalize_playlist_save_path, ordered_directory_files,
        output_devices_json, parse_delete_paths_json, parse_row_indices, playback_source_is_missing,
        playlist_entry_for_track,
        purged_track_indices, remove_path_permanent, prepare_scan_file, resolve_output_device,
        sample_rate_label, sanitize_delete_paths, scan_directory_paths, sort_visible_indices,
        star_key_for_track, stored_entry_is_missing, track_filename, track_path,
        valid_equalizer_gain,
    };
    use kog_audio::decoder::{ArchiveOrigin, DecoderRegistry, DecoderSettings, PlaybackSource};
    use kog_audio::playback::OutputDevice;
    use kog_audio::playlist::PlaylistLocation;
    use kog_audio::settings::OutputDevicePreference;
    use kog_audio::track::Track;
    use std::cmp::Ordering;
    use std::collections::HashSet;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::Arc;
    use std::sync::atomic::AtomicBool;
    use tempfile::tempdir;

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

    #[test]
    fn directory_scan_preserves_sorted_depth_first_file_order() {
        let temporary = tempdir().expect("create temporary music folder");
        let root = temporary.path();
        fs::create_dir(root.join("__MACOSX")).unwrap();
        fs::write(root.join("__MACOSX/ghost.flac"), []).unwrap();
        fs::write(root.join("._ghost.flac"), []).unwrap();
        fs::write(root.join("desktop.ini"), []).unwrap();
        fs::create_dir(root.join("02-disc")).expect("create nested album folder");
        fs::write(root.join("01-first.flac"), []).expect("create first track");
        fs::write(root.join("02-disc/01-middle.flac"), []).expect("create nested first track");
        fs::write(root.join("02-disc/02-middle.flac"), []).expect("create nested second track");
        fs::write(root.join("03-last.flac"), []).expect("create last track");

        let relative = ordered_directory_files(root)
            .expect("scan the music folder")
            .into_iter()
            .map(|path| path.strip_prefix(root).unwrap().to_owned())
            .collect::<Vec<_>>();

        assert_eq!(
            relative,
            [
                PathBuf::from("01-first.flac"),
                PathBuf::from("02-disc/01-middle.flac"),
                PathBuf::from("02-disc/02-middle.flac"),
                PathBuf::from("03-last.flac"),
            ]
        );
    }

    #[test]
    fn background_directory_scan_preserves_visual_file_order() {
        let temporary = tempdir().expect("create temporary music folder");
        let root = temporary.path();
        fs::create_dir(root.join("__MACOSX")).unwrap();
        fs::write(root.join("__MACOSX/ghost.flac"), []).unwrap();
        fs::write(root.join("._ghost.flac"), []).unwrap();
        fs::create_dir(root.join("02-disc")).expect("create nested album folder");
        fs::write(root.join("01-first.flac"), []).expect("create first track");
        fs::write(root.join("02-disc/01-middle.flac"), []).expect("create nested first track");
        fs::write(root.join("02-disc/02-middle.flac"), []).expect("create nested second track");
        fs::write(root.join("03-last.flac"), []).expect("create last track");

        let (sender, receiver) = std::sync::mpsc::sync_channel(16);
        scan_directory_paths(
            vec![root.to_owned()],
            sender,
            Arc::new(AtomicBool::new(false)),
            DecoderRegistry::default(),
            DecoderSettings::default(),
            true,
            true,
        );
        let relative = receiver
            .into_iter()
            .filter_map(|event| match event {
                DirectoryScanEvent::Prepared(prepared) => {
                    Some(prepared.path.strip_prefix(root).unwrap().to_owned())
                }
                DirectoryScanEvent::Warning(warning) => panic!("unexpected warning: {warning}"),
                DirectoryScanEvent::Complete { cancelled } => {
                    assert!(!cancelled);
                    None
                }
            })
            .collect::<Vec<_>>();

        assert_eq!(
            relative,
            [
                PathBuf::from("01-first.flac"),
                PathBuf::from("02-disc/01-middle.flac"),
                PathBuf::from("02-disc/02-middle.flac"),
                PathBuf::from("03-last.flac"),
            ]
        );
    }

    #[test]
    fn file_tree_path_batches_preserve_selection_order() {
        assert_eq!(
            local_paths_from_json(r#"["/music/01.flac","/music/02.flac"]"#).unwrap(),
            [
                PathBuf::from("/music/01.flac"),
                PathBuf::from("/music/02.flac"),
            ]
        );
        assert!(local_paths_from_json(r#"{"path":"/music/01.flac"}"#).is_err());
    }

    #[test]
    fn sanitize_delete_paths_drops_roots_empties_duplicates_and_nested() {
        assert_eq!(
            sanitize_delete_paths(vec![
                PathBuf::from(""),
                PathBuf::from("/"),
                PathBuf::from("relative/song.flac"),
                PathBuf::from("/other"),
                PathBuf::from("/music/pop"),
                PathBuf::from("/music/pop"),
                PathBuf::from("/music/rock/song.flac"),
                PathBuf::from("/music/rock"),
            ]),
            [
                PathBuf::from("/music/pop"),
                PathBuf::from("/music/rock"),
                PathBuf::from("/other"),
            ]
        );
        assert!(sanitize_delete_paths(vec![PathBuf::from("")]).is_empty());
    }

    #[test]
    fn count_delete_entries_counts_files_without_following_links() {
        let temporary = tempdir().expect("create temporary music folder");
        let root = temporary.path();
        fs::create_dir(root.join("sub")).unwrap();
        fs::write(root.join("a.flac"), []).unwrap();
        fs::write(root.join("sub/b.flac"), []).unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(root.join("sub"), root.join("link")).unwrap();

        let cancel = AtomicBool::new(false);
        let (files, directories) = count_delete_entries(root, &cancel);
        #[cfg(unix)]
        assert_eq!((files, directories), (3, 2));
        #[cfg(not(unix))]
        assert_eq!((files, directories), (2, 2));
        assert_eq!(
            count_delete_entries(&root.join("missing.flac"), &cancel),
            (0, 0)
        );
        assert_eq!(count_delete_entries(&root.join("a.flac"), &cancel), (1, 0));
    }

    #[test]
    fn remove_path_permanent_removes_files_and_trees() {
        let temporary = tempdir().expect("create temporary music folder");
        let root = temporary.path();
        let file = root.join("gone.flac");
        fs::write(&file, []).unwrap();
        remove_path_permanent(&file).expect("remove file");
        assert!(!file.exists());

        let tree = root.join("tree");
        fs::create_dir(&tree).unwrap();
        fs::write(tree.join("nested.flac"), []).unwrap();
        remove_path_permanent(&tree).expect("remove directory tree");
        assert!(!tree.exists());

        assert!(remove_path_permanent(&root.join("missing.flac")).is_err());
    }

    #[test]
    fn explicit_playlist_files_bypass_the_folder_playlist_filter() {
        // Staged playlists (e.g. loading a stored playlist into the pane)
        // are explicit scan roots: they must parse even when the folder
        // preference disables playlist discovery.
        let temporary = tempdir().expect("create temporary music folder");
        let playlist = temporary.path().join("staged.m3u");
        fs::write(&playlist, "missing.flac\n").expect("write staged playlist");
        let skipped = prepare_scan_file(
            playlist.clone(),
            &DecoderRegistry::default(),
            true,
            false,
            false,
        );
        assert!(skipped.tracks.is_empty());
        assert!(skipped.warnings.is_empty());
        let forced = prepare_scan_file(
            playlist,
            &DecoderRegistry::default(),
            true,
            false,
            true,
        );
        assert!(forced.tracks.is_empty());
        assert!(
            forced
                .warnings
                .iter()
                .any(|warning| warning.contains("missing.flac")),
            "unexpected warnings: {:?}",
            forced.warnings
        );
    }

    #[test]
    fn purged_track_indices_match_files_dirs_and_archives() {
        fn local(path: &str) -> Track {
            Track {
                source: PlaybackSource::from_path(PathBuf::from(path)),
                ..Track::default()
            }
        }
        let mut archived = local("/music/pack.zip/Disc/a.wav");
        archived
            .source
            .set_archive_origin(PathBuf::from("/music/pack.zip"), "Disc/a.wav".to_owned());
        let remote = Track {
            source: PlaybackSource {
                remote_url: Some("https://example.com/stream".to_owned()),
                path: PathBuf::from("/stream"),
                ..PlaybackSource::default()
            },
            ..Track::default()
        };
        let tracks = vec![
            local("/music/rock/song.flac"),
            local("/music/pop/song.flac"),
            archived,
            remote,
            local("/music/rock2/song.flac"),
        ];
        assert_eq!(
            purged_track_indices(&tracks, &[PathBuf::from("/music/rock")]),
            [0]
        );
        assert_eq!(
            purged_track_indices(&tracks, &[PathBuf::from("/music/pack.zip")]),
            [2]
        );
        assert_eq!(
            purged_track_indices(&tracks, &[PathBuf::from("/music")]),
            [0, 1, 2, 4]
        );
        assert!(purged_track_indices(&tracks, &[PathBuf::from("/videos")]).is_empty());
    }

    #[test]
    fn parse_delete_paths_json_handles_arrays() {
        assert_eq!(
            parse_delete_paths_json(r#"["/music/a.flac", "/music/b"]"#),
            [PathBuf::from("/music/a.flac"), PathBuf::from("/music/b")]
        );
        assert!(parse_delete_paths_json("not json").is_empty());
        assert!(parse_delete_paths_json(r#"{"path": 1}"#).is_empty());
    }


    #[test]
    fn track_path_shows_full_song_path() {
        let local = Track {
            source: PlaybackSource::from_path(PathBuf::from("/music/song.flac")),
            ..Track::default()
        };
        assert_eq!(track_path(&local), "/music/song.flac");
        let mut archived = Track {
            source: PlaybackSource::from_path(PathBuf::from("inner.zip/track.flac")),
            ..Track::default()
        };
        archived.source.set_archive_origin(
            PathBuf::from("/music/pack.zip"),
            "inner.zip/track.flac".to_owned(),
        );
        assert_eq!(
            track_path(&archived),
            format!(
                "/music/pack.zip{}inner.zip/track.flac",
                std::path::MAIN_SEPARATOR
            )
        );
        assert_eq!(
            track_filename(&archived),
            "track.flac",
            "filename column keeps showing just the file"
        );
    }

    #[test]
    fn cover_art_key_scopes_untagged_files_and_skips_downloads() {
        let file = Path::new("/music/midi/new/song.mid");
        let (key, may_download) = cover_art_key("", "", "new", file);
        assert!(!may_download, "untagged files never hit providers");
        let (other_key, _) = cover_art_key("", "", "new", Path::new("/music/midi/new/other.mid"));
        assert_ne!(
            key, other_key,
            "sibling untagged files must not share one cached image"
        );
        let (tagged_key, tagged_download) =
            cover_art_key("Artist", "Album", "Album", Path::new("/music/a.flac"));
        assert!(tagged_download);
        assert_ne!(key, tagged_key);
    }


    #[test]
    fn archive_tree_selection_flows_through_background_import() {
        let fixture = tempfile::tempdir().unwrap();
        let archive = fixture.path().join("songs.zip");
        let wav = kog_audio::archive::tests::wav_bytes(100);
        kog_audio::archive::tests::write_stored_zip(
            &archive,
            &[("Disc/b.wav", &wav), ("Disc/a.wav", &wav)],
        );
        let paths = vec![kog_audio::archive::tests::tree_url(&archive, "Disc", true)];
        let (sender, receiver) = std::sync::mpsc::sync_channel(64);
        scan_directory_paths(
            paths,
            sender,
            Arc::new(AtomicBool::new(false)),
            DecoderRegistry::default(),
            DecoderSettings::default(),
            true,
            true,
        );
        let prepared = receiver
            .try_iter()
            .find_map(|event| match event {
                DirectoryScanEvent::Prepared(prepared) => Some(prepared),
                _ => None,
            })
            .expect("Archive tree selection reaches import preparation");
        assert!(prepared.warnings.is_empty(), "{:?}", prepared.warnings);
        assert_eq!(prepared.tracks.len(), 2);
        assert_eq!(
            prepared.tracks[0]
                .source
                .archive_origin
                .as_ref()
                .unwrap()
                .entry_name,
            "Disc/a.wav"
        );
        assert_eq!(
            prepared.tracks[1]
                .source
                .archive_origin
                .as_ref()
                .unwrap()
                .entry_name,
            "Disc/b.wav"
        );
    }

    #[test]
    fn dropped_file_urls_are_decoded_as_one_ordered_batch() {
        let (paths, remotes) =
            dropped_urls_from_json(r#"["file:///music/01%20intro.flac","file:///music/02.flac"]"#)
                .expect("parse file URL batch");
        assert_eq!(
            paths,
            [
                PathBuf::from("/music/01 intro.flac"),
                PathBuf::from("/music/02.flac"),
            ]
        );
        assert!(remotes.is_empty());
    }

    #[test]
    fn output_device_resolution_prefers_stable_id_then_falls_back_to_name() {
        let devices = vec![
            OutputDevice {
                id: "new-speakers-id".to_owned(),
                name: "Studio Speakers".to_owned(),
                label: "Studio Speakers — ALSA".to_owned(),
                is_default: true,
            },
            OutputDevice {
                id: "saved-id".to_owned(),
                name: "Headphones".to_owned(),
                label: "Headphones — ALSA".to_owned(),
                is_default: false,
            },
        ];

        let exact = resolve_output_device(
            &devices,
            &OutputDevicePreference {
                id: "saved-id".to_owned(),
                name: "Studio Speakers".to_owned(),
            },
        )
        .expect("resolve the stable ID before the matching name");
        assert_eq!(exact.id, "saved-id");

        let remapped = resolve_output_device(
            &devices,
            &OutputDevicePreference {
                id: "stale-speakers-id".to_owned(),
                name: "Studio Speakers".to_owned(),
            },
        )
        .expect("recover a changed backend ID from the saved device name");
        assert_eq!(remapped.id, "new-speakers-id");
    }

    #[test]
    fn output_device_json_exposes_only_safe_ui_fields() {
        let json = output_devices_json(&[OutputDevice {
            id: "alsa:output:1".to_owned(),
            name: "Raw backend name".to_owned(),
            label: "Studio Speakers — ALSA".to_owned(),
            is_default: true,
        }]);
        let value: serde_json::Value = serde_json::from_str(&json).expect("valid device JSON");
        let device = &value[0];

        assert_eq!(device["id"], "alsa:output:1");
        assert_eq!(device["label"], "Studio Speakers — ALSA");
        assert_eq!(device["isDefault"], true);
        assert!(device.get("name").is_none());
    }

    #[test]
    fn row_index_parser_sorts_deduplicates_and_bounds_input() {
        assert_eq!(parse_row_indices("3, 1,3,garbage,8,0", 5), [0, 1, 3]);
        assert!(parse_row_indices("-1,wrong", 5).is_empty());
    }

    #[test]
    fn missing_entry_check_ignores_remotes_and_checks_archive_outers() {
        let temporary = tempdir().expect("create temporary music folder");
        let present = temporary.path().join("song.flac");
        fs::write(&present, []).expect("write present file");
        let stored = |kind: &str, path: &str| kog_core::db::StoredEntry {
            kind: kind.to_owned(),
            path: path.to_owned(),
            entry: String::new(),
            fragment: None,
        };
        assert!(!stored_entry_is_missing(&stored(
            kog_core::db::KIND_LOCAL,
            present.to_str().unwrap()
        )));
        assert!(stored_entry_is_missing(&stored(
            kog_core::db::KIND_LOCAL,
            temporary.path().join("gone.flac").to_str().unwrap()
        )));
        assert!(stored_entry_is_missing(&stored(kog_core::db::KIND_LOCAL, "")));
        assert!(!stored_entry_is_missing(&stored(
            kog_core::db::KIND_ARCHIVE,
            present.to_str().unwrap()
        )));
        assert!(stored_entry_is_missing(&stored(
            kog_core::db::KIND_ARCHIVE,
            temporary.path().join("gone.zip").to_str().unwrap()
        )));
        assert!(!stored_entry_is_missing(&stored(
            kog_core::db::KIND_REMOTE,
            "https://example.invalid/stream"
        )));
        assert!(!stored_entry_is_missing(&stored("bogus-kind", "/music/x.flac")));
    }

    #[test]
    fn missing_source_check_never_flags_remotes() {
        let temporary = tempdir().expect("create temporary music folder");
        let present = temporary.path().join("song.flac");
        std::fs::write(&present, []).expect("write present file");
        let local = |path: PathBuf| PlaybackSource {
            path,
            remote_url: None,
            subsong: None,
            archive_origin: None,
        };
        assert!(!playback_source_is_missing(&local(present)));
        assert!(playback_source_is_missing(&local(
            temporary.path().join("gone.flac")
        )));
        let mut archived = local(temporary.path().join("member.wav"));
        archived.set_archive_origin(
            temporary.path().join("gone.zip"),
            "member.wav".to_owned(),
        );
        assert!(playback_source_is_missing(&archived));
        let outer = temporary.path().join("pack.zip");
        std::fs::write(&outer, []).expect("write present outer");
        archived.set_archive_origin(outer, "member.wav".to_owned());
        assert!(!playback_source_is_missing(&archived));
        let remote = PlaybackSource {
            path: PathBuf::from("/stream"),
            remote_url: Some("https://example.invalid/stream".to_owned()),
            subsong: None,
            archive_origin: None,
        };
        assert!(!playback_source_is_missing(&remote));
    }

    #[test]
    fn playlist_export_preserves_cue_archive_remote_and_subsong_identities() {
        let cue = Track {
            source: PlaybackSource {
                path: PathBuf::from("/music/album.cue"),
                remote_url: None,
                subsong: Some(1),
                archive_origin: None,
            },
            track_number: Some(7),
            backend_id: "cuesheet".to_owned(),
            ..Track::default()
        };
        let cue_entry = playlist_entry_for_track(&cue).expect("serialize CueSheet track");
        assert_eq!(cue_entry.fragment.as_deref(), Some("7"));
        assert_eq!(
            cue_entry.location,
            PlaylistLocation::Local(PathBuf::from("/music/album.cue"))
        );

        let archived = Track {
            source: PlaybackSource {
                path: PathBuf::from("/tmp/kog-archive/song.jxs"),
                remote_url: None,
                subsong: Some(1),
                archive_origin: Some(ArchiveOrigin {
                    archive_path: PathBuf::from("/music/set.zip"),
                    entry_name: "disc/song.jxs".to_owned(),
                }),
            },
            backend_id: "syntrax".to_owned(),
            ..Track::default()
        };
        let archived_entry =
            playlist_entry_for_track(&archived).expect("serialize archived subsong");
        assert_eq!(archived_entry.fragment.as_deref(), Some("1"));
        assert_eq!(
            archived_entry.location,
            PlaylistLocation::Archive {
                archive_path: PathBuf::from("/music/set.zip"),
                entry_name: "disc/song.jxs".to_owned(),
            }
        );

        let remote = Track {
            source: PlaybackSource::from_remote_url(
                url::Url::parse("https://example.invalid/radio").unwrap(),
            ),
            ..Track::default()
        };
        assert_eq!(
            playlist_entry_for_track(&remote).unwrap().location,
            PlaylistLocation::Remote("https://example.invalid/radio".to_owned())
        );
    }

    #[test]
    fn playlist_save_paths_default_to_m3u_and_reject_unrelated_extensions() {
        assert_eq!(
            normalize_playlist_save_path(PathBuf::from("mix")).unwrap(),
            PathBuf::from("mix.m3u")
        );
        assert!(normalize_playlist_save_path(PathBuf::from("mix.M3U8")).is_ok());
        assert!(normalize_playlist_save_path(PathBuf::from("mix.PLS")).is_ok());
        assert!(normalize_playlist_save_path(PathBuf::from("mix.txt")).is_err());
    }

    #[test]
    fn moving_multiple_rows_preserves_their_relative_order() {
        let mut values = vec!['a', 'b', 'c', 'd', 'e', 'f'];
        let moved = move_selected_items(&mut values, &[1, 3], 6);

        assert_eq!(values, ['a', 'c', 'e', 'f', 'b', 'd']);
        assert_eq!(moved, [4, 5]);
    }

    #[test]
    fn dropping_inside_a_selection_does_not_scramble_it() {
        let mut values = vec!['a', 'b', 'c', 'd', 'e'];
        let moved = move_selected_items(&mut values, &[1, 2], 2);

        assert_eq!(values, ['a', 'b', 'c', 'd', 'e']);
        assert_eq!(moved, [1, 2]);
    }

    #[test]
    fn natural_comparison_is_case_insensitive_and_orders_digit_runs_numerically() {
        assert_eq!(natural_compare("Track 2", "track 10"), Ordering::Less);
        assert_eq!(natural_compare("SONG 01", "song 1"), Ordering::Equal);
        assert_eq!(natural_compare("Alpha", "beta"), Ordering::Less);
    }

    #[test]
    fn cogs_complete_playlist_column_schema_is_sortable() {
        let identifiers = [
            "index",
            "star",
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
        for identifier in identifiers {
            let column = PlaylistSortColumn::from_identifier(identifier)
                .unwrap_or_else(|| panic!("missing playlist column {identifier}"));
            assert_eq!(column.identifier(), identifier);
        }
    }

    #[test]
    fn sample_rate_labels_use_compact_cog_style_units() {
        assert_eq!(sample_rate_label(None), "");
        assert_eq!(sample_rate_label(Some(500)), "500 Hz");
        assert_eq!(sample_rate_label(Some(44_100)), "44.1 kHz");
        assert_eq!(sample_rate_label(Some(48_000)), "48 kHz");
    }

    #[test]
    fn equalizer_gain_validation_rejects_nonfinite_and_out_of_range_values() {
        assert_eq!(valid_equalizer_gain(-20.0), Some(-20.0));
        assert_eq!(valid_equalizer_gain(20.0), Some(20.0));
        assert_eq!(valid_equalizer_gain(20.01), None);
        assert_eq!(valid_equalizer_gain(f64::NAN), None);
    }

    #[test]
    fn track_sort_matches_cogs_album_disc_track_sequence() {
        let disc_two = Track {
            album_artist: "The Artist".to_owned(),
            album: "Record".to_owned(),
            disc_number: Some(2),
            track_number: Some(1),
            ..Track::default()
        };
        let track_ten = Track {
            album_artist: "the artist".to_owned(),
            album: "record".to_owned(),
            disc_number: Some(1),
            track_number: Some(10),
            ..Track::default()
        };

        assert_eq!(
            compare_tracks(
                &track_ten,
                &disc_two,
                PlaylistSortColumn::Track,
                &HashSet::new()
            ),
            Ordering::Less
        );
    }

    #[test]
    fn star_sort_groups_starred_first_when_ascending() {
        let plain = Track {
            source: kog_audio::decoder::PlaybackSource::from_path(PathBuf::from("/music/a.flac")),
            ..Track::default()
        };
        let favorite = Track {
            source: kog_audio::decoder::PlaybackSource::from_path(PathBuf::from("/music/b.flac")),
            ..Track::default()
        };
        let starred: HashSet<String> = [star_key_for_track(&favorite)].into_iter().collect();
        assert_eq!(
            compare_tracks(&plain, &favorite, PlaylistSortColumn::Star, &starred),
            Ordering::Greater
        );
        assert_eq!(
            compare_tracks(&favorite, &plain, PlaylistSortColumn::Star, &starred),
            Ordering::Less
        );
        assert_eq!(
            compare_tracks(&plain, &plain, PlaylistSortColumn::Star, &starred),
            Ordering::Equal
        );
    }

    #[test]
    fn visible_sort_is_stable_and_index_mode_preserves_original_order() {
        let tracks = [
            Track {
                title: "Song 10".to_owned(),
                ..Track::default()
            },
            Track {
                title: "song 2".to_owned(),
                ..Track::default()
            },
            Track {
                title: "SONG 02".to_owned(),
                ..Track::default()
            },
        ];
        let mut visible = vec![0, 1, 2];

        sort_visible_indices(
            &tracks,
            &mut visible,
            PlaylistSortColumn::Title,
            true,
            &HashSet::new(),
        );
        assert_eq!(visible, [1, 2, 0]);

        let mut original = vec![0, 1, 2];
        sort_visible_indices(
            &tracks,
            &mut original,
            PlaylistSortColumn::Index,
            false,
            &HashSet::new(),
        );
        assert_eq!(original, [0, 1, 2]);
    }
}
