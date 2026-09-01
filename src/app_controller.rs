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
        #[qproperty(QString, playlist_sort_column)]
        #[qproperty(bool, playlist_sort_ascending)]
        #[qproperty(QString, playlist_column_layout)]
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
        #[qproperty(QString, midi_engine)]
        #[qproperty(QString, midi_status)]
        #[qproperty(QString, opening_files_behavior)]
        #[qproperty(bool, read_cue_sheets_in_folders)]
        #[qproperty(bool, read_playlists_in_folders)]
        #[qproperty(bool, show_tray_icon)]
        #[qproperty(bool, close_to_tray)]
        #[qproperty(bool, minimize_to_tray)]
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
        fn add_url(self: Pin<&mut AppController>, url: QString);
        #[qinvokable]
        fn enqueue_url(self: Pin<&mut AppController>, url: QString);
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
        fn save_playlist_column_layout(self: Pin<&mut AppController>, layout: QString);
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
        #[qinvokable]
        fn update_show_tray_icon(self: Pin<&mut AppController>, enabled: bool);
        #[qinvokable]
        fn update_close_to_tray(self: Pin<&mut AppController>, enabled: bool);
        #[qinvokable]
        fn update_minimize_to_tray(self: Pin<&mut AppController>, enabled: bool);
    }
}

use std::cmp::Ordering;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::time::Duration;

use cxx_qt::CxxQtType;
use cxx_qt_lib::{QString, QUrl};

use crate::decoder::{
    DecoderRegistry, DecoderSettings, ExpansionResult, PlaybackSource, validate_soundfont,
};
use crate::equalizer::{
    EqualizerSettings, apply_preset, preset_for_genre, preset_named, preset_names,
};
use crate::playback::{OutputDevice, PlaybackEngine, PlaybackState, available_output_devices};
use crate::playback_order::{PlaybackOrder, SelectionState};
use crate::playlist::{Playlist, PlaylistEntry, PlaylistLocation};
use crate::settings::{
    AppSettings, MidiEngine, OpeningFilesBehavior, OutputDevicePreference, RepeatMode, ShuffleMode,
};
use crate::tag_editor::{artwork_file_json, parse_edits, snapshot_json, write_tags};
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

fn ordered_directory_files(directory: &Path) -> Result<Vec<PathBuf>, String> {
    let mut files = Vec::new();
    let mut pending = vec![(directory.to_owned(), true)];

    while let Some((path, is_directory)) = pending.pop() {
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

fn track_path(track: &Track) -> String {
    if let Some(url) = &track.source.remote_url {
        return url.clone();
    }
    let path = track
        .source
        .archive_origin
        .as_ref()
        .map_or(&track.source.path, |origin| &origin.archive_path);
    path.parent()
        .map(|parent| parent.to_string_lossy().into_owned())
        .unwrap_or_default()
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

fn compare_tracks(left: &Track, right: &Track, column: PlaylistSortColumn) -> Ordering {
    match column {
        PlaylistSortColumn::Index
        | PlaylistSortColumn::Rating
        | PlaylistSortColumn::PlayCount
        | PlaylistSortColumn::Status => Ordering::Equal,
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

fn sort_visible_indices(
    tracks: &[Track],
    visible_indices: &mut [usize],
    column: PlaylistSortColumn,
    ascending: bool,
) {
    if column == PlaylistSortColumn::Index {
        return;
    }
    visible_indices.sort_by(|left, right| {
        let ordering = compare_tracks(&tracks[*left], &tracks[*right], column);
        if ascending {
            ordering
        } else {
            ordering.reverse()
        }
    });
}

pub struct AppControllerRust {
    playlist_count: i32,
    playlist_revision: i32,
    playlist_sort_column: QString,
    playlist_sort_ascending: bool,
    playlist_column_layout: QString,
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
    midi_engine: QString,
    midi_status: QString,
    opening_files_behavior: QString,
    read_cue_sheets_in_folders: bool,
    read_playlists_in_folders: bool,
    show_tray_icon: bool,
    close_to_tray: bool,
    minimize_to_tray: bool,
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
    directory: PathBuf,
    decoder_settings: DecoderSettings,
    decoders: DecoderRegistry,
    playback: PlaybackEngine,
    equalizer_settings: EqualizerSettings,
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
            playlist_sort_column: qstring(PlaylistSortColumn::Index.identifier()),
            playlist_sort_ascending: true,
            playlist_column_layout,
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
            midi_engine,
            midi_status,
            opening_files_behavior: qstring(app_settings.opening_files_behavior.setting_value()),
            read_cue_sheets_in_folders: app_settings.read_cue_sheets_in_folders,
            read_playlists_in_folders: app_settings.read_playlists_in_folders,
            show_tray_icon: app_settings.show_tray_icon,
            close_to_tray: app_settings.close_to_tray,
            minimize_to_tray: app_settings.minimize_to_tray,
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
            directory,
            decoder_settings,
            decoders,
            playback,
            equalizer_settings,
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
        controller
    }
}

fn qstring(value: impl AsRef<str>) -> QString {
    QString::from(value.as_ref())
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
            if !self.decoders.accepts_path(&path) {
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
        if self.sort_column != PlaylistSortColumn::Index {
            sort_visible_indices(
                &self.tracks,
                &mut self.visible_indices,
                self.sort_column,
                self.playlist_sort_ascending,
            );
        }
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

    pub fn clear_playlist(mut self: Pin<&mut Self>) {
        self.as_mut().stop();
        {
            let mut rust = self.as_mut().rust_mut();
            rust.tracks.clear();
            rust.playback_order.clear_tracks();
        }
        self.as_mut().set_queue_count(0);
        self.as_mut().set_current_index(-1);
        self.as_mut().reset_now_playing();
        self.as_mut().rebuild_playlist();
        self.as_mut().set_status(qstring("Playlist cleared"));
    }

    pub fn filter_playlist(mut self: Pin<&mut Self>, query: QString) {
        self.as_mut().rust_mut().filter = query.to_string().trim().to_lowercase();
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

    pub fn save_playlist_column_layout(mut self: Pin<&mut Self>, layout: QString) {
        if let Err(error) = AppSettings::save_playlist_column_layout(&layout.to_string()) {
            self.as_mut().set_status(qstring(error));
            return;
        }
        self.as_mut().set_playlist_column_layout(layout);
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
        let tracks = self.as_ref().rust().tracks.clone();
        let current = usize::try_from(self.as_ref().rust().current_index).ok();
        let target = {
            self.as_mut()
                .rust_mut()
                .playback_order
                .previous(&tracks, current)
        };
        if let Some(target) = target {
            self.as_mut().play_source_index(target);
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

    pub fn poll_playback(mut self: Pin<&mut Self>) {
        if self.as_ref().rust().playback.finished() {
            let current = usize::try_from(self.as_ref().rust().current_index).ok();
            if current
                .is_some_and(|index| self.as_ref().rust().playback_order.should_stop_after(index))
            {
                self.as_mut().stop();
            } else {
                self.as_mut().advance_playback(true);
            }
            return;
        }
        if self.as_ref().rust().playback.state() == PlaybackState::Stopped {
            return;
        }
        let position = self.as_ref().rust().playback.position().as_secs_f64();
        self.as_mut().set_position_seconds(position);
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
            .filter(|index| *index < crate::equalizer::EQUALIZER_FREQUENCIES.len())
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

    pub fn track_value_at(&self, index: i32, column: QString) -> QString {
        let Some(source_index) = visible_source_index(self, index) else {
            return QString::default();
        };
        let Some(track) = self.rust().tracks.get(source_index) else {
            return QString::default();
        };
        match column.to_string().as_str() {
            "index" => qstring((source_index + 1).to_string()),
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
        if behavior.clears_playlist() {
            self.as_mut().clear_playlist();
        }

        let first_new_source_index = self.as_ref().rust().tracks.len();
        let mut combined = AddPathResult::default();
        for path in paths {
            let result = if path.is_dir() {
                self.as_mut().rust_mut().add_directory(&path)
            } else {
                self.as_mut().rust_mut().add_path(path)
            };
            match result {
                Ok(result) => {
                    combined.added += result.added;
                    if let Some(warning) = result.warning {
                        combined.push_warning(warning);
                    }
                }
                Err(error) => combined.push_warning(error),
            }
        }

        if combined.added == 0 && combined.warning.is_none() {
            self.as_mut()
                .set_status(qstring("The file is already in the playlist"));
            return;
        }
        if combined.added == 0 {
            self.as_mut()
                .set_status(qstring(add_path_status(&combined)));
            return;
        }

        self.as_mut().refresh_playback_order();
        self.as_mut().rebuild_playlist();
        self.as_mut()
            .set_status(qstring(add_path_status(&combined)));
        if behavior.starts_playback() {
            self.as_mut().play_source_index(first_new_source_index);
        }
    }

    fn refresh_playback_order(mut self: Pin<&mut Self>) {
        let tracks = self.as_ref().rust().tracks.clone();
        let current = usize::try_from(self.as_ref().rust().current_index).ok();
        let queue_count = {
            let mut rust = self.as_mut().rust_mut();
            rust.playback_order.tracks_changed(&tracks, current);
            rust.playback_order.queue_count()
        };
        self.as_mut().set_queue_count(saturating_i32(queue_count));
    }

    fn advance_playback(mut self: Pin<&mut Self>, honor_repeat_one: bool) {
        let tracks = self.as_ref().rust().tracks.clone();
        let current = usize::try_from(self.as_ref().rust().current_index).ok();
        let (target, queue_count) = {
            let mut rust = self.as_mut().rust_mut();
            let target = rust.playback_order.next(&tracks, current, honor_repeat_one);
            (target, rust.playback_order.queue_count())
        };
        self.as_mut().set_queue_count(saturating_i32(queue_count));
        if let Some(target) = target {
            self.as_mut().play_source_index(target);
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
        self.as_mut().set_status(qstring(match mode {
            RepeatMode::Off => "Repeat off",
            RepeatMode::One => "Repeat one track",
            RepeatMode::Album => "Repeat album",
            RepeatMode::All => "Repeat all tracks",
        }));
    }

    fn play_source_index(mut self: Pin<&mut Self>, source_index: usize) {
        let Some((source, genre)) = self
            .as_ref()
            .get_ref()
            .rust()
            .tracks
            .get(source_index)
            .map(|track| (track.source.clone(), track.genre.clone()))
        else {
            return;
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
    use super::{
        AddPathResult, PlaylistSortColumn, add_path_status, compare_tracks, local_paths_from_json,
        move_selected_items, natural_compare, normalize_playlist_save_path,
        ordered_directory_files, output_devices_json, parse_row_indices, playlist_entry_for_track,
        resolve_output_device, sample_rate_label, sort_visible_indices, valid_equalizer_gain,
    };
    use crate::decoder::{ArchiveOrigin, PlaybackSource};
    use crate::playback::OutputDevice;
    use crate::playlist::PlaylistLocation;
    use crate::settings::OutputDevicePreference;
    use crate::track::Track;
    use std::cmp::Ordering;
    use std::fs;
    use std::path::PathBuf;
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
            compare_tracks(&track_ten, &disc_two, PlaylistSortColumn::Track),
            Ordering::Less
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

        sort_visible_indices(&tracks, &mut visible, PlaylistSortColumn::Title, true);
        assert_eq!(visible, [1, 2, 0]);

        let mut original = vec![0, 1, 2];
        sort_visible_indices(&tracks, &mut original, PlaylistSortColumn::Index, false);
        assert_eq!(original, [0, 1, 2]);
    }
}
