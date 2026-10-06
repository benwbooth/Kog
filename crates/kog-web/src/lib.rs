//! Kog's web player.
//!
//! The shared Rust Session owns this browser player's queue and transport.
//! Leptos renders its snapshots; browser ports play API stream URLs through
//! an `<audio>` element and return tokened lifecycle events.
//!
//! The layout mirrors the desktop window: a 48px toolbar, a sidebar holding the
//! file tree and the playlists, the playlist pane with a column header, and a
//! 92px transport bar. The shell is a fixed-height grid, so only the pane
//! scrolls, never the page. Phones use full-width Library, Queue, and Playlists
//! views with a mini player above the bottom navigation.
//!
//! The toolbar carries the desktop's two separate controls: `☰` opens an
//! application menu mirroring `qml/Main.qml`'s `hamburgerMenu`, and a checkable
//! `«`/`»` toggles the file tree. The transport buttons inline the same
//! monochrome SVGs `CogButton` shows on the desktop, so no control depends on
//! the browser emoji font. The header context menu exposes the full
//! column set from `qml/PlaylistHeader.qml`; tags arrive from
//! `POST /api/metadata` in one batch per refresh and are cached by locator, so
//! re-renders never refetch. A background poll of `/kog_web.js`'s content-hash
//! ETag reloads the page once a newer build is being served, deferring while a
//! track plays.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use gloo_net::http::Request;
use kog_playback_policy::session::{
    Command as SessionCommand, Effect as SessionEffect, IoResult, OutputEvent, Session, Token,
    Transport,
};
use kog_playback_policy::sort::SortRow;
use kog_playback_policy::workspace::QueueAction;
use kog_playback_policy::{NavigationEvent, RepeatMode, ShuffleMode};
use leptos::prelude::*;
use serde::{Deserialize, Serialize};
use wasm_bindgen::JsCast;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::prelude::wasm_bindgen;

mod drag;
use drag::{clear_track_drop_marker, track_drop_target};
mod selection;
mod session;
mod workspace;
mod menu;
mod persistence;
mod artwork;

/// The desktop transport's SVG icons (`qml/icons/`), inlined verbatim. CSS
/// tints them with the button's text color where the desktop picks the
/// `-light` variant from the toolbar luminance.
mod icons {
    pub const MENU: &str = include_str!("../../../qml/icons/application-menu.svg");
    pub const SHUFFLE: &str = include_str!("../../../qml/icons/media-playlist-shuffle.svg");
    pub const SKIP_BACKWARD: &str = include_str!("../../../qml/icons/media-skip-backward.svg");
    pub const PLAY: &str = include_str!("../../../qml/icons/media-playback-start.svg");
    pub const PAUSE: &str = include_str!("../../../qml/icons/media-playback-pause.svg");
    pub const STOP: &str = include_str!("../../../qml/icons/media-playback-stop.svg");
    pub const SKIP_FORWARD: &str = include_str!("../../../qml/icons/media-skip-forward.svg");
    pub const REPEAT: &str = include_str!("../../../qml/icons/media-playlist-repeat.svg");
    /// Desktop's volume icon, plus a muted variant in the same style for the
    /// mute toggle (the desktop keeps the high icon, but a mute button that
    /// never changes reads as broken).
    pub const VOLUME: &str = include_str!("../../../qml/icons/audio-volume-high.svg");
    pub const VOLUME_MUTED: &str = "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 24 24\"><path fill=\"#000\" d=\"M4.27 3 3 4.27 7.73 9H3v6h4l5 4v-6.73l4.25 4.25c-.67.52-1.42.93-2.25 1.18v2.06c1.38-.31 2.63-.95 3.69-1.81L19.73 21 21 19.73l-9-9L4.27 3zM12 4 9.91 6.09 12 8.18V4z\"/></svg>";
    pub const FIND: &str = include_str!("../../../qml/icons/edit-find.svg");
    /// The search busy gear, currentColor so it follows the theme; its
    /// geometry is centered by construction (teeth rotated about the
    /// viewBox center), so it spins true on its axis.
    pub const GEAR: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><g fill="currentColor"><path fill-rule="evenodd" d="M12 5.8a6.2 6.2 0 1 1 0 12.4 6.2 6.2 0 0 1 0-12.4zm0 2a4.2 4.2 0 1 0 0 8.4 4.2 4.2 0 0 0 0-8.4z"/><rect x="10.6" y="2" width="2.8" height="5" rx="1.4"/><rect x="10.6" y="2" width="2.8" height="5" rx="1.4" transform="rotate(45 12 12)"/><rect x="10.6" y="2" width="2.8" height="5" rx="1.4" transform="rotate(90 12 12)"/><rect x="10.6" y="2" width="2.8" height="5" rx="1.4" transform="rotate(135 12 12)"/><rect x="10.6" y="2" width="2.8" height="5" rx="1.4" transform="rotate(180 12 12)"/><rect x="10.6" y="2" width="2.8" height="5" rx="1.4" transform="rotate(225 12 12)"/><rect x="10.6" y="2" width="2.8" height="5" rx="1.4" transform="rotate(270 12 12)"/><rect x="10.6" y="2" width="2.8" height="5" rx="1.4" transform="rotate(315 12 12)"/></g></svg>"#;
    pub const GO_UP: &str = include_str!("../../../qml/icons/go-up.svg");
    pub const FOLDER_OPEN: &str = include_str!("../../../qml/icons/folder-open.svg");
    pub const VIEW_LIST_TREE: &str = include_str!("../../../qml/icons/view-list-tree.svg");
    pub const CLEAR_LIST: &str = include_str!("../../../qml/icons/edit-clear-list.svg");
    pub const QUEUE: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path fill="currentColor" d="M3 5h13v2H3zm0 6h13v2H3zm0 6h9v2H3zm13-3 6 4-6 4z"/></svg>"#;
    /// Per-format tree art, the same files the Qt tree shows
    /// (qml/icons/kog-format-*.svg), inlined so the stylesheet can tint them
    /// with the row text color. Only the dark-theme variant is inlined: the
    /// CSS recolors every path with currentColor, so one copy serves both.
    pub const FMT_GAMEBOY: &str = include_str!("../../../qml/icons/kog-format-gameboy.svg");
    pub const FMT_NES: &str = include_str!("../../../qml/icons/kog-format-nes.svg");
    pub const FMT_SNES: &str = include_str!("../../../qml/icons/kog-format-snes.svg");
    pub const FMT_GBA: &str = include_str!("../../../qml/icons/kog-format-gba.svg");
    pub const FMT_DS: &str = include_str!("../../../qml/icons/kog-format-ds.svg");
    pub const FMT_PSX: &str = include_str!("../../../qml/icons/kog-format-psx.svg");
    pub const FMT_PS2: &str = include_str!("../../../qml/icons/kog-format-ps2.svg");
    pub const FMT_SATURN: &str = include_str!("../../../qml/icons/kog-format-saturn.svg");
    pub const FMT_N64: &str = include_str!("../../../qml/icons/kog-format-n64.svg");
    pub const FMT_ARCADE: &str = include_str!("../../../qml/icons/kog-format-arcade.svg");
    pub const FMT_MSX: &str = include_str!("../../../qml/icons/kog-format-msx.svg");
    pub const FMT_PCENGINE: &str = include_str!("../../../qml/icons/kog-format-pcengine.svg");
    pub const FMT_SPECTRUM: &str = include_str!("../../../qml/icons/kog-format-spectrum.svg");
    pub const FMT_ATARI: &str = include_str!("../../../qml/icons/kog-format-atari.svg");
    pub const FMT_C64: &str = include_str!("../../../qml/icons/kog-format-c64.svg");
    pub const FMT_AMIGA: &str = include_str!("../../../qml/icons/kog-format-amiga.svg");
    pub const FMT_CHIP: &str = include_str!("../../../qml/icons/kog-format-chip.svg");
    pub const FMT_TRACKER: &str = include_str!("../../../qml/icons/kog-format-tracker.svg");
    pub const FMT_MIDI: &str = include_str!("../../../qml/icons/kog-format-midi.svg");
    pub const FMT_AUDIO: &str = include_str!("../../../qml/icons/kog-format-audio.svg");
    pub const FMT_ARCHIVE: &str = include_str!("../../../qml/icons/kog-format-archive.svg");
    pub const FMT_PLAYLIST: &str = include_str!("../../../qml/icons/kog-format-playlist.svg");
    pub const FMT_CUE: &str = include_str!("../../../qml/icons/kog-format-cue.svg");
    pub const FMT_PAPER: &str = include_str!("../../../qml/icons/kog-format-paper.svg");
}

/// A file's mark in the tree and the playlist title column: dedicated art,
/// the paper badge carrying the extension, or nothing (folders and the `..`
/// row keep their own marks).
enum FileIcon {
    Svg(&'static str),
    Badge(String),
    None,
}

/// Lowercased suffix without the dot, or "" when there is none. Mirrors the
/// suffix extraction in kogFormatIconName
/// (native/kog_desktop_integration.cpp); the extension table below must stay
/// in agreement with it.
fn suffix_of(name: &str) -> String {
    let base = name.rsplit('/').next().unwrap_or(name);
    match base.rsplit_once('.') {
        Some((_, ext)) if !ext.is_empty() => ext.to_ascii_lowercase(),
        _ => String::new(),
    }
}

/// Dedicated-art key for a suffix, or None for the paper badge. Extension
/// families mirror the decoder backends' static allow-lists; tracker/MIDI
/// membership follows backend priority (mus/xmf are MIDI, dsf is Saturn).
fn format_key(suffix: &str) -> Option<&'static str> {
    Some(match suffix {
        "gbs" => "gameboy",
        "nsf" | "nsfe" => "nes",
        "spc" | "snsf" | "minisnsf" => "snes",
        "gsf" | "minigsf" => "gba",
        "2sf" | "mini2sf" | "ncsf" | "minincsf" => "ds",
        "psf" | "minipsf" => "psx",
        "psf2" | "minipsf2" => "ps2",
        "ssf" | "minissf" | "dsf" | "minidsf" => "saturn",
        "usf" | "miniusf" => "n64",
        "qsf" | "miniqsf" => "arcade",
        "kss" => "msx",
        "hes" => "pcengine",
        "ay" => "spectrum",
        "sap" => "atari",
        "sid" => "c64",
        "hvl" | "ahx" => "amiga",
        "vgm" | "vgz" | "gym" | "s98" | "dro" | "sfm" => "chip",
        "mptm" | "mod" | "s3m" | "xm" | "it" | "667" | "669" | "amf" | "ams" | "c67" | "cba"
        | "dbm" | "digi" | "dmf" | "dsm" | "dsym" | "dtm" | "etx" | "far" | "fc" | "fc13"
        | "fc14" | "fmt" | "fst" | "ftm" | "imf" | "ims" | "ice" | "j2b" | "m15" | "mdl"
        | "med" | "mms" | "mt2" | "mtm" | "nst" | "okt" | "plm" | "psm" | "pt36" | "ptm"
        | "puma" | "rtm" | "sfx" | "sfx2" | "smod" | "st26" | "stk" | "stm" | "stx" | "stp"
        | "symmod" | "tcb" | "gmc" | "gtk" | "gt2" | "ult" | "unic" | "wow" | "gdm" | "mo3"
        | "oxm" | "umx" | "xpk" | "ppm" | "mmcmp" | "org" | "jxs" => "tracker",
        "kar" | "mid" | "midi" | "rmi" | "mids" | "mds" | "lds" | "xmf" | "mxmf" | "hmi"
        | "hmp" | "hmq" | "mus" | "xmi" => "midi",
        "aac" | "adts" | "aif" | "aifc" | "aiff" | "alac" | "caf" | "flac" | "m4a" | "m4b"
        | "mka" | "mkv" | "mp1" | "mp2" | "mp3" | "mp4" | "oga" | "ogg" | "ogv" | "opus"
        | "wav" | "wave" | "webm" | "wma" | "asf" | "tak" | "m4r" | "m2a" | "mpa" | "ape"
        | "ac3" | "dts" | "dtshd" | "tta" | "vqf" | "vqe" | "vql" | "ra" | "rm" | "rmj"
        | "weba" | "dsdiff" | "dff" | "wsd" | "wv" | "wvp" | "mpc" | "shn" | "iff" | "apl" => {
            "audio"
        }
        "zip" | "rar" | "7z" | "rsn" | "vgm7z" | "gz" | "mdz" | "mdr" | "s3z" | "xmz" | "itz"
        | "mptmz" => "archive",
        "m3u" | "m3u8" | "pls" => "playlist",
        "cue" => "cue",
        _ => return None,
    })
}

/// Inline SVG for a dedicated-art key.
fn format_svg(key: &str) -> &'static str {
    match key {
        "gameboy" => icons::FMT_GAMEBOY,
        "nes" => icons::FMT_NES,
        "snes" => icons::FMT_SNES,
        "gba" => icons::FMT_GBA,
        "ds" => icons::FMT_DS,
        "psx" => icons::FMT_PSX,
        "ps2" => icons::FMT_PS2,
        "saturn" => icons::FMT_SATURN,
        "n64" => icons::FMT_N64,
        "arcade" => icons::FMT_ARCADE,
        "msx" => icons::FMT_MSX,
        "pcengine" => icons::FMT_PCENGINE,
        "spectrum" => icons::FMT_SPECTRUM,
        "atari" => icons::FMT_ATARI,
        "c64" => icons::FMT_C64,
        "amiga" => icons::FMT_AMIGA,
        "chip" => icons::FMT_CHIP,
        "tracker" => icons::FMT_TRACKER,
        "midi" => icons::FMT_MIDI,
        "audio" => icons::FMT_AUDIO,
        "archive" => icons::FMT_ARCHIVE,
        "playlist" => icons::FMT_PLAYLIST,
        "cue" => icons::FMT_CUE,
        _ => icons::FMT_PAPER,
    }
}

/// The mark for a tree row or playlist entry. Archive members resolve by
/// their member name; anything without a suffix keeps the caller's default.
fn file_icon(path: &str, entry: &str) -> FileIcon {
    let name = if entry.is_empty() { path } else { entry };
    let suffix = suffix_of(name);
    if suffix.is_empty() {
        return FileIcon::None;
    }
    match format_key(&suffix) {
        Some(key) => FileIcon::Svg(format_svg(key)),
        // Badged like the desktop's paper fallback, capitals, three glyphs.
        None => FileIcon::Badge(suffix.to_ascii_uppercase().chars().take(3).collect()),
    }
}

/// One playable entry, addressed the way the whole API addresses tracks.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct Entry {
    kind: String,
    path: String,
    entry: String,
    fragment: Option<String>,
    /// Display name; "dir" entries carry one, playlist entries derive it.
    name: String,
    /// Location as shown in the subtitle (relative where known).
    location: String,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum MobileView {
    Library,
    Queue,
    Playlists,
}

impl Entry {
    fn is_dir(&self) -> bool {
        self.kind == "dir"
    }
}

/// One rendered tree line: a flattened view of the expanded directories.
/// The locator fields are carried through so two playlist-expanded rows that
/// share a path (one per subsong) stay distinct and queue their own track.
#[derive(Clone, Debug, PartialEq)]
struct TreeRow {
    name: String,
    path: String,
    parent: String,
    is_dir: bool,
    depth: usize,
    expanded: bool,
    kind: String,
    entry: String,
    fragment: Option<String>,
}

/// The desktop search's progress counters, straight from the walk: items
/// scanned in the filesystem pass, then archive listings, for the status
/// line under the search box.
#[derive(Clone, Copy, Default)]
struct SearchProgress {
    scanned: u64,
    archive_count: u64,
    archives_scanned: u64,
    unreadable: u64,
    scanning_archives: bool,
}

/// The web stand-in for the desktop's playlist dialogs: naming a new
/// playlist, naming a duplicate, and confirming a delete.
#[derive(Clone, Debug, PartialEq)]
enum PlaylistDialogMode {
    CreateFromPane { selected_only: Option<bool> },
    Duplicate,
    ConfirmDelete,
}

#[derive(Clone, Debug, PartialEq)]
struct PlaylistDialog {
    mode: PlaylistDialogMode,
    title: &'static str,
    /// Prefilled name for duplicates, empty for a new playlist.
    value: String,
    /// The playlist a duplicate or delete acts on.
    id: i64,
    /// The playlist's current name, for the confirm message.
    name: String,
}

/// One row of `POST /api/metadata`, shaped like the playlist columns.
///
/// Every field is present in the API response; the ones the web pane does not
/// render yet are still parsed so the struct matches the wire shape.
#[allow(dead_code)]
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MetaRow {
    title: Option<String>,
    artist: Option<String>,
    album: Option<String>,
    album_artist: Option<String>,
    composer: Option<String>,
    genre: Option<String>,
    year: Option<u32>,
    track_number: Option<u32>,
    disc_number: Option<u32>,
    duration: Option<f64>,
    sample_rate: Option<u32>,
    channels: Option<u16>,
    bits_per_sample: Option<u8>,
    codec: Option<String>,
    bitrate: Option<u32>,
    file_size_bytes: Option<u64>,
}

/// The columns the pane sorts by, matching the visible header order.
#[derive(Clone, Copy, PartialEq, Eq)]
enum SortKey {
    Index,
    Star,
    Status,
    Rating,
    Title,
    AlbumArtist,
    Artist,
    Composer,
    Album,
    Length,
    FileSizeBytes,
    FileSize,
    Year,
    Genre,
    Track,
    PlayCount,
    Path,
    Filename,
    Codec,
    SampleRate,
    BitsPerSample,
    Bitrate,
}

/// Every column the pane can show, in the fixed order they render.
///
/// The order mirrors `qml/PlaylistHeader.qml`'s `defaultColumns`, so a Move
/// Column and a Reset Columns land where the desktop would put them. Rating
/// and Play Count have no value in the API: they render empty exactly as
/// `AppController::track_value_at` does on the desktop.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum ColumnId {
    Index,
    Star,
    Status,
    Rating,
    Title,
    AlbumArtist,
    Artist,
    Composer,
    Album,
    Length,
    FileSizeBytes,
    FileSize,
    Year,
    Genre,
    Track,
    PlayCount,
    Path,
    Filename,
    Codec,
    SampleRate,
    BitsPerSample,
    Bitrate,
}

impl ColumnId {
    const ALL: [ColumnId; 22] = [
        ColumnId::Index,
        ColumnId::Star,
        ColumnId::Status,
        ColumnId::Rating,
        ColumnId::Title,
        ColumnId::AlbumArtist,
        ColumnId::Artist,
        ColumnId::Composer,
        ColumnId::Album,
        ColumnId::Length,
        ColumnId::FileSizeBytes,
        ColumnId::FileSize,
        ColumnId::Year,
        ColumnId::Genre,
        ColumnId::Track,
        ColumnId::PlayCount,
        ColumnId::Path,
        ColumnId::Filename,
        ColumnId::Codec,
        ColumnId::SampleRate,
        ColumnId::BitsPerSample,
        ColumnId::Bitrate,
    ];

    /// The order the header context menu lists the visibility toggles in,
    /// alphabetised by menu label exactly as `qml/PlaylistHeader.qml` does.
    /// Like the desktop, the menu does not offer to hide the Star column.
    const MENU_ORDER: [ColumnId; 21] = [
        ColumnId::Album,
        ColumnId::AlbumArtist,
        ColumnId::Artist,
        ColumnId::Bitrate,
        ColumnId::BitsPerSample,
        ColumnId::Codec,
        ColumnId::Composer,
        ColumnId::Filename,
        ColumnId::FileSize,
        ColumnId::FileSizeBytes,
        ColumnId::Genre,
        ColumnId::Index,
        ColumnId::Length,
        ColumnId::Path,
        ColumnId::PlayCount,
        ColumnId::Rating,
        ColumnId::SampleRate,
        ColumnId::Status,
        ColumnId::Title,
        ColumnId::Track,
        ColumnId::Year,
    ];

    fn key(self) -> &'static str {
        match self {
            Self::Index => "index",
            Self::Star => "star",
            Self::Status => "status",
            Self::Rating => "rating",
            Self::Title => "title",
            Self::AlbumArtist => "albumartist",
            Self::Artist => "artist",
            Self::Composer => "composer",
            Self::Album => "album",
            Self::Length => "length",
            Self::FileSizeBytes => "filesizebytes",
            Self::FileSize => "filesize",
            Self::Year => "year",
            Self::Genre => "genre",
            Self::Track => "track",
            Self::PlayCount => "playcount",
            Self::Path => "path",
            Self::Filename => "filename",
            Self::Codec => "codec",
            Self::SampleRate => "samplerate",
            Self::BitsPerSample => "bitspersample",
            Self::Bitrate => "bitrate",
        }
    }

    fn from_key(key: &str) -> Option<Self> {
        // The desktop names the Year column "date" in its layouts.
        if key == "date" {
            return Some(Self::Year);
        }
        Self::ALL.into_iter().find(|column| column.key() == key)
    }

    /// The header text, matching `defaultColumns()` in `PlaylistHeader.qml`.
    fn label(self) -> &'static str {
        match self {
            Self::Index => "#",
            // The desktop's star and status headers are empty; a glyph keeps
            // the web header discoverable and its auto-fit width sane.
            Self::Star => "★",
            Self::Status => "●",
            Self::Rating => "Rating",
            Self::Title => "Title",
            Self::AlbumArtist => "Album Artist",
            Self::Artist => "Artist",
            Self::Composer => "Composer",
            Self::Album => "Album",
            Self::Length => "Length",
            Self::FileSizeBytes => "Size (bytes)",
            Self::FileSize => "Size",
            Self::Year => "Year",
            Self::Genre => "Genre",
            Self::Track => "№",
            Self::PlayCount => "Plays",
            Self::Path => "Path",
            Self::Filename => "Filename",
            Self::Codec => "Codec",
            Self::SampleRate => "Sample Rate",
            Self::BitsPerSample => "Bits",
            Self::Bitrate => "Bitrate",
        }
    }

    /// The name the header menu uses, matching `defaultColumns()`'s menuLabel.
    fn menu_label(self) -> &'static str {
        match self {
            Self::Index => "Index",
            Self::Star => "Star",
            Self::Status => "Status",
            Self::Rating => "Rating",
            Self::Title => "Title",
            Self::AlbumArtist => "Album Artist",
            Self::Artist => "Artist",
            Self::Composer => "Composer",
            Self::Album => "Album",
            Self::Length => "Length",
            Self::FileSizeBytes => "File Size (Bytes)",
            Self::FileSize => "File Size",
            Self::Year => "Year",
            Self::Genre => "Genre",
            Self::Track => "Track",
            Self::PlayCount => "Play Count",
            Self::Path => "Path",
            Self::Filename => "Filename",
            Self::Codec => "Codec",
            Self::SampleRate => "Sample Rate",
            Self::BitsPerSample => "Bits Per Sample",
            Self::Bitrate => "Bitrate",
        }
    }

    fn class(self) -> &'static str {
        match self {
            Self::Index => "index-cell",
            Self::Star => "star-cell",
            Self::Status => "status-cell",
            Self::Rating => "rating-cell",
            Self::Title => "title-cell",
            Self::AlbumArtist => "albumartist-cell",
            Self::Artist => "artist-cell",
            Self::Composer => "composer-cell",
            Self::Album => "album-cell",
            Self::Length => "duration-cell",
            Self::FileSizeBytes => "filesizebytes-cell",
            Self::FileSize => "filesize-cell",
            Self::Year => "year-cell",
            Self::Genre => "genre-cell",
            Self::Track => "trackno-cell",
            Self::PlayCount => "playcount-cell",
            Self::Path => "path-cell",
            Self::Filename => "filename-cell",
            Self::Codec => "codec-cell",
            Self::SampleRate => "samplerate-cell",
            Self::BitsPerSample => "bitspersample-cell",
            Self::Bitrate => "bitrate-cell",
        }
    }

    fn sort_key(self) -> SortKey {
        match self {
            Self::Index => SortKey::Index,
            Self::Star => SortKey::Star,
            Self::Status => SortKey::Status,
            Self::Rating => SortKey::Rating,
            Self::Title => SortKey::Title,
            Self::AlbumArtist => SortKey::AlbumArtist,
            Self::Artist => SortKey::Artist,
            Self::Composer => SortKey::Composer,
            Self::Album => SortKey::Album,
            Self::Length => SortKey::Length,
            Self::FileSizeBytes => SortKey::FileSizeBytes,
            Self::FileSize => SortKey::FileSize,
            Self::Year => SortKey::Year,
            Self::Genre => SortKey::Genre,
            Self::Track => SortKey::Track,
            Self::PlayCount => SortKey::PlayCount,
            Self::Path => SortKey::Path,
            Self::Filename => SortKey::Filename,
            Self::Codec => SortKey::Codec,
            Self::SampleRate => SortKey::SampleRate,
            Self::BitsPerSample => SortKey::BitsPerSample,
            Self::Bitrate => SortKey::Bitrate,
        }
    }

    fn default_width(self) -> f64 {
        match self {
            Self::Index => 68.0,
            Self::Star => 52.0,
            Self::Status => 38.0,
            Self::Rating => 78.0,
            Self::Title => 220.0,
            Self::AlbumArtist => 150.0,
            Self::Artist => 190.0,
            Self::Composer => 151.0,
            Self::Album => 220.0,
            Self::Length => 70.0,
            Self::FileSizeBytes => 110.0,
            Self::FileSize => 88.0,
            Self::Year => 58.0,
            Self::Genre => 120.0,
            Self::Track => 54.0,
            Self::PlayCount => 71.0,
            Self::Path => 180.0,
            Self::Filename => 180.0,
            Self::Codec => 80.0,
            Self::SampleRate => 92.0,
            Self::BitsPerSample => 64.0,
            Self::Bitrate => 84.0,
        }
    }

    fn min_width(self) -> f64 {
        match self {
            Self::Index => 68.0,
            Self::Star => 52.0,
            Self::Status => 38.0,
            Self::Rating => 48.0,
            Self::Title => 96.0,
            Self::AlbumArtist => 96.0,
            Self::Artist => 96.0,
            Self::Composer => 96.0,
            Self::Album => 96.0,
            Self::Length => 44.0,
            Self::FileSizeBytes => 82.0,
            Self::FileSize => 60.0,
            Self::Year => 42.0,
            Self::Genre => 48.0,
            Self::Track => 32.0,
            Self::PlayCount => 42.0,
            Self::Path => 64.0,
            Self::Filename => 64.0,
            Self::Codec => 48.0,
            Self::SampleRate => 64.0,
            Self::BitsPerSample => 48.0,
            Self::Bitrate => 56.0,
        }
    }

    /// Text columns share the spare width the way the desktop flexes them.
    fn flexible(self) -> bool {
        matches!(
            self,
            Self::Title
                | Self::AlbumArtist
                | Self::Artist
                | Self::Composer
                | Self::Album
                | Self::Genre
                | Self::Path
                | Self::Filename
        )
    }

    fn align_right(self) -> bool {
        matches!(
            self,
            Self::Index
                | Self::Year
                | Self::Length
                | Self::FileSizeBytes
                | Self::FileSize
                | Self::Track
                | Self::PlayCount
                | Self::SampleRate
                | Self::BitsPerSample
                | Self::Bitrate
        )
    }

    fn align_center(self) -> bool {
        matches!(self, Self::Star | Self::Status)
    }

    /// The web's default layout: the desktop's visible set minus Genre, Year
    /// and the ten columns this pass added.
    fn default_visible(self) -> bool {
        matches!(
            self,
            Self::Index
                | Self::Star
                | Self::Status
                | Self::Title
                | Self::Artist
                | Self::Album
                | Self::Length
                | Self::FileSize
                | Self::Track
        )
    }
}

/// One persisted column: its width and whether it is shown.
#[derive(Clone, Copy, PartialEq)]
struct Column {
    id: ColumnId,
    width: f64,
    visible: bool,
}

fn default_columns() -> Vec<Column> {
    ColumnId::ALL
        .into_iter()
        .map(|id| Column {
            id,
            width: id.default_width(),
            visible: id.default_visible(),
        })
        .collect()
}

/// `id:width:visible;...`, the layout the pane caches.
fn encode_columns(columns: &[Column]) -> String {
    columns
        .iter()
        .map(|column| {
            format!(
                "{},{:.2},{}",
                column.id.key(),
                column.width,
                if column.visible { 1 } else { 0 }
            )
        })
        .collect::<Vec<_>>()
        .join(";")
}

/// Restore a persisted layout in its saved order, so a Move Left/Right survives
/// a reload. Any column the saved string missed (one added since) is inserted
/// at its default position.
fn decode_columns(raw: &str) -> Vec<Column> {
    let mut columns = Vec::new();
    for entry in raw.split(';') {
        // Desktop layouts write `id,width,visible`; the pane's own cache
        // writes `id:width:visible`. Both are accepted here.
        let fields: Vec<&str> = entry.split([':', ',']).collect();
        if fields.len() != 3 {
            continue;
        }
        let Some(id) = ColumnId::from_key(fields[0].trim()) else {
            continue;
        };
        let Ok(width) = fields[1].trim().parse::<f64>() else {
            continue;
        };
        if columns.iter().any(|column: &Column| column.id == id) {
            continue;
        }
        columns.push(Column {
            id,
            width: width.max(id.min_width()),
            visible: fields[2].trim() == "1",
        });
    }
    for id in ColumnId::ALL {
        if columns.iter().any(|column| column.id == id) {
            continue;
        }
        let position = ColumnId::ALL
            .iter()
            .position(|candidate| *candidate == id)
            .unwrap_or(columns.len())
            .min(columns.len());
        columns.insert(
            position,
            Column {
                id,
                width: id.default_width(),
                visible: id.default_visible(),
            },
        );
    }
    if !columns.iter().any(|column| column.visible) {
        return default_columns();
    }
    columns
}

/// A content-based width: the widest of the header label (with its sort
/// arrow) and the visible values, measured with the real fonts through a
/// cached canvas. A per-character guess fit proportional text badly in both
/// directions — wide glyphs overflowed, narrow ones left dead space.
fn content_width(id: ColumnId, texts: &[String]) -> f64 {
    let Some(context) = measure_context() else {
        // No canvas to measure with: the old per-character guess, padded.
        let widest = texts
            .iter()
            .map(|text| text.chars().count())
            .max()
            .unwrap_or(0) as f64;
        return (widest * 7.0 + 22.0).clamp(id.min_width(), 1024.0);
    };
    // Cells inherit the 13px body font; the header label sits in the sort
    // button at its own 11px. Both get their cell padding back, plus a
    // little slack so a measurement a hair under the rendered width does
    // not clip into an ellipsis.
    let cell_font = styled_font(".track .cell", "13px system-ui, sans-serif");
    context.set_font(&cell_font);
    // The header is laid out, not estimated: sort arrows come from a
    // fallback symbol font that canvas measuring under-reports, so the
    // button's own element measures the label-with-arrow exactly.
    let head_text = format!("{} ▲", id.label());
    let mut widest = dom_text_width(".columns .col-sort", &head_text).unwrap_or_else(|| {
        let head_font = styled_font(".columns .col-sort", "11px system-ui, sans-serif");
        context.set_font(&head_font);
        measured(&context, &head_text)
    }) + 16.0
        + 3.0;
    context.set_font(&cell_font);
    for text in texts {
        let width = measured(&context, text) + 16.0 + 3.0;
        if width > widest {
            widest = width;
        }
    }
    widest.clamp(id.min_width(), 1024.0)
}

/// The canvas 2D context of a detached canvas, created once and reused: the
/// text measurer for auto-fit widths.
fn measure_context() -> Option<web_sys::CanvasRenderingContext2d> {
    thread_local! {
        static CONTEXT: RefCell<Option<web_sys::CanvasRenderingContext2d>> =
            RefCell::new(None);
    }
    CONTEXT.with(|slot| {
        let mut held = slot.borrow_mut();
        if held.is_none() {
            let document = web_sys::window().and_then(|window| window.document())?;
            let canvas = document.create_element("canvas").ok()?;
            let canvas: web_sys::HtmlCanvasElement = canvas.dyn_into().ok()?;
            let context = canvas.get_context("2d").ok()??;
            let context: web_sys::CanvasRenderingContext2d = context.dyn_into().ok()?;
            *held = Some(context);
        }
        held.clone()
    })
}

/// Measured text width, or zero when measurement fails (then the floor wins).
fn measured(context: &web_sys::CanvasRenderingContext2d, text: &str) -> f64 {
    context.measure_text(text).map(|m| m.width()).unwrap_or(0.0)
}

/// The laid-out width of `text` inside the first element matching
/// `selector`: the text inherits that element's font by measure, so fallback
/// glyphs (sort arrows, star glyphs) come out exactly as wide as they
/// render. None when the element does not exist yet.
fn dom_text_width(selector: &str, text: &str) -> Option<f64> {
    let document = web_sys::window().and_then(|window| window.document())?;
    let host = document.query_selector(selector).ok().flatten()?;
    let probe = document.create_element("span").ok()?;
    let _ = probe.set_attribute(
        "style",
        "position: absolute; visibility: hidden; white-space: pre; \
         padding: 0; margin: 0; border: 0; font: inherit;",
    );
    probe.set_text_content(Some(text));
    host.append_child(&probe).ok()?;
    let html: web_sys::HtmlElement = probe.dyn_into().ok()?;
    let width = html.offset_width() as f64;
    let _ = host.remove_child(&html);
    Some(width)
}

/// The computed font of the first element matching `selector`, so auto-fit
/// measures with exactly what the cells render, staying correct if the CSS
/// font changes. Falls back to a plain font when nothing matches yet.
fn styled_font(selector: &str, fallback: &str) -> String {
    let Some(window) = web_sys::window() else {
        return fallback.to_owned();
    };
    let Some(document) = window.document() else {
        return fallback.to_owned();
    };
    let Ok(Some(element)) = document.query_selector(selector) else {
        return fallback.to_owned();
    };
    let Ok(Some(style)) = window.get_computed_style(&element) else {
        return fallback.to_owned();
    };
    let family = style.get_property_value("font-family").unwrap_or_default();
    if family.trim().is_empty() {
        return fallback.to_owned();
    }
    let size = style.get_property_value("font-size").unwrap_or_default();
    let weight = style.get_property_value("font-weight").unwrap_or_default();
    format!("{} {} {}", weight.trim(), size.trim(), family.trim())
}

/// The text a column shows for one queue row, shared by the cell and auto-fit.
fn column_text(
    id: ColumnId,
    index: usize,
    entry: &Entry,
    meta: Option<&MetaRow>,
    live_duration: Option<f64>,
    starred: bool,
    status: &str,
) -> String {
    match id {
        ColumnId::Index => (index + 1).to_string(),
        // Filled vs outline star, the desktop's two icon states rendered as
        // glyphs.
        ColumnId::Star => if starred { "★" } else { "☆" }.to_owned(),
        ColumnId::Status => status.to_owned(),
        // No rating/play-count value crosses the API, so those stay blank; the
        // desktop renders rating and play count blank too.
        ColumnId::Rating | ColumnId::PlayCount => String::new(),
        ColumnId::AlbumArtist => meta
            .and_then(|meta| meta.album_artist.clone())
            .unwrap_or_default(),
        ColumnId::Composer => meta
            .and_then(|meta| meta.composer.clone())
            .unwrap_or_default(),
        ColumnId::Title => meta
            .and_then(|meta| meta.title.clone())
            .unwrap_or_else(|| entry.name.clone()),
        ColumnId::Artist => meta
            .and_then(|meta| meta.artist.clone())
            .unwrap_or_default(),
        ColumnId::Album => meta.and_then(|meta| meta.album.clone()).unwrap_or_default(),
        ColumnId::Genre => meta.and_then(|meta| meta.genre.clone()).unwrap_or_default(),
        ColumnId::Year => meta
            .and_then(|meta| meta.year)
            .map(|year| year.to_string())
            .unwrap_or_default(),
        ColumnId::Length => meta
            .and_then(|meta| meta.duration)
            .map(clock)
            .or_else(|| live_duration.map(clock))
            .unwrap_or_default(),
        ColumnId::FileSizeBytes => meta
            .and_then(|meta| meta.file_size_bytes)
            .map(|bytes| bytes.to_string())
            .unwrap_or_default(),
        ColumnId::FileSize => meta
            .and_then(|meta| meta.file_size_bytes)
            .map(file_size_label)
            .unwrap_or_default(),
        ColumnId::Track => meta
            .and_then(|meta| meta.track_number)
            .map(|number| number.to_string())
            .unwrap_or_default(),
        ColumnId::Path => entry_path(entry),
        ColumnId::Filename => entry_filename(entry),
        ColumnId::Codec => meta.and_then(|meta| meta.codec.clone()).unwrap_or_default(),
        ColumnId::SampleRate => sample_rate_label(meta.and_then(|meta| meta.sample_rate)),
        ColumnId::BitsPerSample => meta
            .and_then(|meta| meta.bits_per_sample)
            .map(|bits| bits.to_string())
            .unwrap_or_default(),
        ColumnId::Bitrate => meta
            .and_then(|meta| meta.bitrate)
            .map(|bitrate| format!("{bitrate} kbps"))
            .unwrap_or_default(),
    }
}

/// The full location for the Path column, mirroring `track_path`: an archive's
/// outer file with its member appended, or the local path / remote URL.
fn entry_path(entry: &Entry) -> String {
    if entry.kind == "archive" && !entry.entry.trim().is_empty() {
        format!("{}::{}", entry.path, entry.entry)
    } else {
        entry.path.clone()
    }
}

/// Tree location tooltips show the basename, including on Windows servers.
fn tree_location_name(path: &str) -> &str {
    path.trim_end_matches(['/', '\\'])
        .rsplit(['/', '\\'])
        .next()
        .filter(|name| !name.is_empty())
        .unwrap_or(path)
}

/// The file name for the Filename column, mirroring `track_filename`: an
/// archive shows the member name, everything else its own last path segment.
fn entry_filename(entry: &Entry) -> String {
    if entry.kind == "archive" && !entry.entry.trim().is_empty() {
        last_segment(&entry.entry)
    } else {
        last_segment(&entry.path)
    }
}

/// `44.1 kHz`, `48 kHz` or `500 Hz`, matching the desktop's
/// `sample_rate_label`.
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

fn file_size_label(bytes: u64) -> String {
    if bytes < 1024 {
        return format!("{bytes} B");
    }
    let mut value = bytes as f64;
    let mut unit = "B";
    for next in ["KiB", "MiB", "GiB", "TiB", "PiB", "EiB"] {
        value /= 1024.0;
        unit = next;
        if value < 1024.0 {
            break;
        }
    }
    format!("{value:.1} {unit}")
}

type Repeat = RepeatMode;

#[derive(Clone, Debug)]
enum Auth {
    None,
    Token(String),
    Basic(String, String),
}

impl Auth {
    fn header(&self) -> Option<String> {
        match self {
            Self::None => None,
            Self::Token(token) => Some(format!("Bearer {token}")),
            Self::Basic(user, password) => {
                let raw = format!("{user}:{password}");
                Some(format!("Basic {}", base64(raw.as_bytes())))
            }
        }
    }
}

/// Minimal base64 so the app needs no dependency for one header.
fn base64(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for chunk in bytes.chunks(3) {
        let b = [
            chunk[0],
            *chunk.get(1).unwrap_or(&0),
            *chunk.get(2).unwrap_or(&0),
        ];
        let n = ((b[0] as u32) << 16) | ((b[1] as u32) << 8) | b[2] as u32;
        out.push(TABLE[((n >> 18) & 63) as usize] as char);
        out.push(TABLE[((n >> 12) & 63) as usize] as char);
        out.push(if chunk.len() > 1 {
            TABLE[((n >> 6) & 63) as usize] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            TABLE[(n & 63) as usize] as char
        } else {
            '='
        });
    }
    out
}

fn storage() -> Option<web_sys::Storage> {
    web_sys::window()?.local_storage().ok()?
}

fn store(key: &str, value: &str) {
    if let Some(storage) = storage() {
        let _ = storage.set_item(key, value);
    }
}

fn load(key: &str) -> Option<String> {
    storage()?.get_item(key).ok()?
}

/// Yield to the event loop for `ms` milliseconds (waiting on lazy tree loads).
fn sleep_ms(ms: i32) -> wasm_bindgen_futures::JsFuture {
    let promise = js_sys::Promise::new(&mut |resolve, _| {
        let _ = web_sys::window()
            .expect("window")
            .set_timeout_with_callback_and_timeout_and_arguments_0(&resolve, ms);
    });
    wasm_bindgen_futures::JsFuture::from(promise)
}

/// A stable id for this browser, so the server's device registry — and the
/// desktop's connected-devices settings — can tell clients apart. Generated
/// once and kept in localStorage.
fn device_id() -> String {
    if let Some(existing) = load("kog.device")
        && !existing.trim().is_empty()
    {
        return existing;
    }
    let generated = format!(
        "web-{:08x}{:08x}",
        (js_sys::Math::random() * 4_294_967_295.0) as u32,
        (js_sys::Math::random() * 4_294_967_295.0) as u32,
    );
    store("kog.device", &generated);
    generated
}

/// Legacy browser session import and the session's current SQLite UI record.
/// The application checkpoint is stored under its independent session ID.
struct RestoredSession {
    queue: Vec<Entry>,
    current: usize,
    list_name: String,
    tree_root: String,
    expanded: Vec<String>,
    volume: Option<f64>,
    shuffle: ShuffleMode,
    repeat: Repeat,
    radio_on: bool,
}

impl Default for RestoredSession {
    fn default() -> Self {
        Self {
            queue: Vec::new(),
            current: usize::MAX,
            list_name: String::new(),
            tree_root: String::new(),
            expanded: Vec::new(),
            volume: None,
            shuffle: ShuffleMode::Off,
            repeat: Repeat::Off,
            radio_on: false,
        }
    }
}

/// Parse the persisted session, skipping anything that no longer resolves.
/// Missing or malformed values are not fatal: the pane simply comes back empty
/// or with defaults, exactly as the desktop tolerates a missing session.json.
fn decode_session(raw: &str) -> RestoredSession {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(raw) else {
        return RestoredSession::default();
    };
    let mut session = RestoredSession::default();
    if let Some(entries) = value["queue"].as_array() {
        // A stored row that no longer resolves is dropped rather than aborting
        // the whole restore.
        session.queue = entries
            .iter()
            .filter_map(|entry| {
                let parsed = entry_from_json(entry);
                if parsed.path.trim().is_empty() {
                    None
                } else {
                    Some(parsed)
                }
            })
            .collect();
    }
    session.current = value["current"]
        .as_u64()
        .map(|index| index as usize)
        .filter(|index| *index < session.queue.len())
        .unwrap_or(usize::MAX);
    session.list_name = value["listName"].as_str().unwrap_or_default().to_owned();
    session.tree_root = value["treeRoot"].as_str().unwrap_or_default().to_owned();
    session.expanded = value["expanded"]
        .as_array()
        .map(|paths| {
            paths
                .iter()
                .filter_map(|path| path.as_str().map(str::to_owned))
                .filter(|path| !path.trim().is_empty())
                .collect()
        })
        .unwrap_or_default();
    session.volume = value["volume"].as_f64().filter(|volume| volume.is_finite());
    session.shuffle = value["shuffle"]
        .as_str()
        .and_then(ShuffleMode::from_setting)
        .unwrap_or_else(|| {
            if value["shuffle"].as_bool().unwrap_or(false) {
                ShuffleMode::All
            } else {
                ShuffleMode::Off
            }
        });
    session.repeat = value["repeat"]
        .as_str()
        .and_then(RepeatMode::from_setting)
        .unwrap_or_default();
    session.radio_on = value["radioOn"].as_bool().unwrap_or(false);
    session
}

// Source replacement or Stop can legitimately cancel an outstanding play
// promise. Media failures still arrive through the tokened `error` callback.
fn play_audio(audio: &web_sys::HtmlAudioElement) {
    if let Ok(promise) = audio.play() {
        leptos::task::spawn_local(async move {
            let _ = wasm_bindgen_futures::JsFuture::from(promise).await;
        });
    }
}

/// A usable media duration. ADTS and FLAC report `Infinity` (or `NaN`) until
/// the browser has buffered enough, so those must never reach the transport.
fn finite_duration(value: f64) -> Option<f64> {
    if value.is_finite() && value > 0.0 {
        Some(value)
    } else {
        None
    }
}

/// The browser forwards Media Session data to the phone's Now Playing UI and,
/// when supported by the OS, to Bluetooth receivers. Keep this optional: the
/// web player still works in browsers without the API.
fn media_session() -> Option<wasm_bindgen::JsValue> {
    let navigator = js_sys::Reflect::get(&js_sys::global(), &"navigator".into()).ok()?;
    let session = js_sys::Reflect::get(&navigator, &"mediaSession".into()).ok()?;
    (!session.is_null() && !session.is_undefined()).then_some(session)
}

fn media_session_metadata(
    session: &wasm_bindgen::JsValue,
    title: &str,
    artist: &str,
    album: &str,
    artwork: &str,
) {
    let init = js_sys::Object::new();
    for (key, value) in [("title", title), ("artist", artist), ("album", album)] {
        let _ = js_sys::Reflect::set(&init, &key.into(), &value.into());
    }
    let image = js_sys::Object::new();
    let _ = js_sys::Reflect::set(&image, &"src".into(), &artwork.into());
    let images = js_sys::Array::new();
    images.push(&image);
    let _ = js_sys::Reflect::set(&init, &"artwork".into(), &images);
    let Some(constructor) = js_sys::Reflect::get(&js_sys::global(), &"MediaMetadata".into())
        .ok()
        .and_then(|value| value.dyn_into::<js_sys::Function>().ok())
    else {
        return;
    };
    let args = js_sys::Array::new();
    args.push(&init);
    if let Ok(metadata) = js_sys::Reflect::construct(&constructor, &args) {
        let _ = js_sys::Reflect::set(session, &"metadata".into(), &metadata);
    }
}

fn media_session_album<'a>(meta: Option<&'a MetaRow>, entry: &'a Entry) -> &'a str {
    meta.and_then(|meta| meta.album.as_deref())
        .filter(|album| !album.trim().is_empty())
        .or_else(|| {
            (entry.kind == "local")
                .then(|| {
                    std::path::Path::new(&entry.path)
                        .parent()?
                        .file_name()?
                        .to_str()
                })
                .flatten()
        })
        .unwrap_or_default()
}

fn media_session_property(session: &wasm_bindgen::JsValue, key: &str, value: &str) {
    let _ = js_sys::Reflect::set(session, &key.into(), &value.into());
}

fn media_session_position(session: &wasm_bindgen::JsValue, duration: f64, position: f64) {
    let Some(duration) = finite_duration(duration) else {
        return;
    };
    let Ok(method) = js_sys::Reflect::get(session, &"setPositionState".into()) else {
        return;
    };
    let Some(method) = method.dyn_ref::<js_sys::Function>() else {
        return;
    };
    let state = js_sys::Object::new();
    let _ = js_sys::Reflect::set(&state, &"duration".into(), &duration.into());
    let _ = js_sys::Reflect::set(
        &state,
        &"position".into(),
        &position.max(0.0).min(duration).into(),
    );
    let _ = method.call1(session, &state);
}

fn media_session_action(
    session: &wasm_bindgen::JsValue,
    name: &str,
    handler: impl FnMut(wasm_bindgen::JsValue) + 'static,
) {
    let Ok(method) = js_sys::Reflect::get(session, &"setActionHandler".into()) else {
        return;
    };
    let Some(method) = method.dyn_ref::<js_sys::Function>() else {
        return;
    };
    let callback = Closure::<dyn FnMut(wasm_bindgen::JsValue)>::new(handler);
    // Some browsers expose Media Session but reject individual actions.
    if method
        .call2(session, &name.into(), callback.as_ref())
        .is_ok()
    {
        callback.forget();
    }
}

/// `m:ss`, or `h:mm:ss` past an hour, matching the desktop's `timeLabel`.
fn clock(seconds: f64) -> String {
    if !seconds.is_finite() || seconds < 0.0 {
        return "-:--".to_owned();
    }
    let total = seconds.round() as u64;
    let (hours, minutes, seconds) = (total / 3600, (total % 3600) / 60, total % 60);
    if hours > 0 {
        format!("{hours}:{minutes:02}:{seconds:02}")
    } else {
        format!("{minutes}:{seconds:02}")
    }
}

fn url_encode(value: &str) -> String {
    value
        .bytes()
        .map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (byte as char).to_string()
            }
            other => format!("%{other:02X}"),
        })
        .collect()
}

fn last_segment(path: &str) -> String {
    path.rsplit('/').next().unwrap_or(path).to_owned()
}

/// Split `name` into (text, highlighted) runs: every occurrence of every
/// whitespace-separated token in `query` is marked, the way the desktop's
/// highlightedName paints matches. Folding is per character so the runs keep
/// their offsets into the original spelling.
fn highlight_runs(name: &str, query: &str) -> Vec<(String, bool)> {
    let tokens: Vec<Vec<char>> = query
        .split_whitespace()
        .map(|word| word.to_lowercase().chars().collect())
        .collect();
    let chars: Vec<char> = name.chars().collect();
    if tokens.is_empty() {
        return vec![(name.to_owned(), false)];
    }
    let folded: Vec<char> = chars
        .iter()
        .map(|c| c.to_lowercase().next().unwrap_or(*c))
        .collect();
    let mut marked = vec![false; chars.len()];
    for token in &tokens {
        if token.is_empty() || token.len() > folded.len() {
            continue;
        }
        for start in 0..=folded.len() - token.len() {
            if folded[start..start + token.len()].iter().eq(token.iter()) {
                for mark in &mut marked[start..start + token.len()] {
                    *mark = true;
                }
            }
        }
    }
    let mut runs: Vec<(String, bool)> = Vec::new();
    for (at, hit) in marked.iter().enumerate() {
        match runs.last_mut() {
            Some((text, run_hit)) if *run_hit == *hit => text.push(chars[at]),
            _ => runs.push((chars[at].to_string(), *hit)),
        }
    }
    runs
}

/// The label text with matched tokens wrapped in a badge, or the plain text
/// when the query is empty. Returns a single view either way.
fn highlight_label(name: String, query: String) -> AnyView {
    let runs = highlight_runs(&name, &query);
    runs.into_iter()
        .map(|(text, hit)| {
            if hit {
                view! { <mark class="search-hit">{text}</mark> }.into_any()
            } else {
                text.into_any()
            }
        })
        .collect_view()
        .into_any()
}

/// Post a "Kog — Now Playing" notification: the web twin of the desktop's
/// popup, without its transport controls. Silent, one at a time (a newer
/// track replaces the older notification), auto-dismissed like the desktop's
/// eight second popup, and clicking it brings the player forward.
fn show_now_playing(title: &str, body: &str) {
    let mut options = web_sys::NotificationOptions::new();
    options.body(body);
    options.icon("/icons/kog.svg");
    options.silent(Some(true));
    options.tag("kog-now-playing");
    let Ok(notification) = web_sys::Notification::new_with_options(title, &options) else {
        return;
    };
    // Clicking the notification focuses the player, like the desktop
    // popup's openPlayer.
    let notification_for_click = notification.clone();
    let onclick = Closure::<dyn FnMut()>::new(move || {
        notification_for_click.close();
        if let Some(window) = web_sys::window() {
            let _ = window.focus();
        }
    });
    notification.set_onclick(Some(onclick.as_ref().unchecked_ref()));
    onclick.forget();
    let window = web_sys::window();
    if let Some(window) = window {
        let notification_for_close = notification.clone();
        let close = Closure::<dyn FnMut()>::new(move || notification_for_close.close());
        let _ = window.set_timeout_with_callback_and_timeout_and_arguments_0(
            close.as_ref().unchecked_ref(),
            8000,
        );
        close.forget();
    }
}

/// Apply the now-playing-notification preference. Enabling asks the browser
/// for permission (a click is the only moment it will); denial keeps the box
/// off. Signals are `Copy`, so handlers calling this stay `Fn`.
fn set_track_notifications_pref(
    enabled: bool,
    set_enabled: leptos::prelude::WriteSignal<bool>,
    set_message: leptos::prelude::WriteSignal<String>,
) {
    if !enabled {
        set_enabled.set(false);
        return;
    }
    leptos::task::spawn_local(async move {
        // Requesting when already granted resolves immediately.
        let granted = match web_sys::Notification::request_permission() {
            Ok(promise) => wasm_bindgen_futures::JsFuture::from(promise)
                .await
                .ok()
                .and_then(|value| value.as_string())
                .map(|value| value == "granted")
                .unwrap_or(false),
            Err(_) => false,
        };
        if granted {
            set_enabled.set(true);
        } else {
            set_message.set("Notifications were blocked by the browser".to_owned());
        }
    });
}

/// The desktop playing-row meter's band analysis, client-side: the same
/// cascaded one-pole splits, windowed RMS in dB mapped to 0..1, and
/// asymmetric smoothing as `AudioMeterSource` in kog-audio, fed from the
/// audio element's Web Audio tap.
#[derive(Default)]
struct BandState {
    rate: u32,
    alphas: [f32; 4],
    low_pass: [f32; 4],
    energy: [f64; 5],
    frames_in_window: u32,
    frames_per_window: u32,
    smoothed: [f32; 5],
}

impl BandState {
    /// Split frequencies and window length tuned for `sample_rate`.
    fn reset(&mut self, sample_rate: u32) {
        const SPLITS_HZ: [f32; 4] = [180.0, 700.0, 2_500.0, 7_000.0];
        let rate = sample_rate as f32;
        self.rate = sample_rate;
        self.alphas = SPLITS_HZ.map(|frequency| {
            let frequency = frequency.min(rate * 0.45);
            1.0 - (-2.0 * std::f32::consts::PI * frequency / rate).exp()
        });
        self.low_pass = [0.0; 4];
        self.energy = [0.0; 5];
        self.frames_in_window = 0;
        self.frames_per_window = (sample_rate / 50).max(64);
    }

    fn observe(&mut self, sample: f32) {
        for (low_pass, alpha) in self.low_pass.iter_mut().zip(self.alphas) {
            *low_pass += alpha * (sample - *low_pass);
        }
        let bands = [
            self.low_pass[0],
            self.low_pass[1] - self.low_pass[0],
            self.low_pass[2] - self.low_pass[1],
            self.low_pass[3] - self.low_pass[2],
            sample - self.low_pass[3],
        ];
        for (energy, band) in self.energy.iter_mut().zip(bands) {
            *energy += f64::from(band) * f64::from(band);
        }
        self.frames_in_window += 1;
        if self.frames_in_window < self.frames_per_window {
            return;
        }
        const GAINS: [f32; 5] = [1.35, 1.2, 1.0, 1.05, 1.2];
        let frames = f64::from(self.frames_in_window);
        for index in 0..5 {
            let rms = (self.energy[index] / frames).sqrt() as f32 * GAINS[index];
            let decibels = 20.0 * rms.max(0.000_001).log10();
            let target = ((decibels + 60.0) / 60.0).clamp(0.0, 1.0);
            let smoothing = if target > self.smoothed[index] {
                0.72
            } else {
                0.16
            };
            self.smoothed[index] += smoothing * (target - self.smoothed[index]);
            if self.smoothed[index] < 0.004 {
                self.smoothed[index] = 0.0;
            }
        }
        self.energy.fill(0.0);
        self.frames_in_window = 0;
    }
}

/// Hand `url` to the browser as a download: a hidden same-origin anchor with
/// the download attribute, so an empty `filename` keeps the server's
/// attachment disposition and a non-empty one names the file directly (a
/// blob URL has no disposition to lean on).
fn trigger_browser_download(url: &str, filename: &str) {
    if let Some(document) = web_sys::window().and_then(|window| window.document()) {
        if let Ok(anchor) = document.create_element("a") {
            let _ = anchor.set_attribute("href", url);
            let _ = anchor.set_attribute("download", filename);
            if let Some(body) = document.body() {
                let _ = body.append_child(&anchor);
                if let Ok(element) = anchor.dyn_into::<web_sys::HtmlElement>() {
                    element.click();
                    let _ = body.remove_child(&element);
                }
            }
        }
    }
}

fn entry_from_json(value: &serde_json::Value) -> Entry {
    let path = value["path"].as_str().unwrap_or_default().to_owned();
    let entry = value["entry"].as_str().unwrap_or_default().to_owned();
    let fragment = value["fragment"].as_str().map(str::to_owned);
    let location = value["relative"]
        .as_str()
        .map(str::to_owned)
        .filter(|relative| !relative.is_empty())
        .unwrap_or_else(|| {
            if entry.is_empty() {
                path.clone()
            } else {
                format!("{path}::{entry}")
            }
        });
    Entry {
        kind: value["kind"].as_str().unwrap_or("local").to_owned(),
        name: last_segment(&path),
        path,
        entry,
        fragment,
        location,
    }
}

/// Radio entries come back with the same locator fields plus a `relative`
/// location; the pane's row parser handles both.
fn radio_entries(value: &serde_json::Value) -> Vec<Entry> {
    value["entries"]
        .as_array()
        .map(|items| items.iter().map(entry_from_json).collect())
        .unwrap_or_default()
}

/// The star locator, matching `kog_server::media_filter::locator_for`: `path`
/// for local/remote, `archive::member` for archives, `#fragment` for subsongs.
fn star_locator(kind: &str, path: &str, entry: &str, fragment: &str) -> String {
    let base = if kind == "archive" {
        format!("{path}::{entry}")
    } else {
        path.to_owned()
    };
    if fragment.trim().is_empty() {
        base
    } else {
        format!("{base}#{}", fragment.trim())
    }
}

/// The star locator for one playlist entry.
fn entry_star_locator(entry: &Entry) -> String {
    star_locator(
        &entry.kind,
        &entry.path,
        &entry.entry,
        entry.fragment.as_deref().unwrap_or_default(),
    )
}

/// The metadata cache key: the same locator fields the server hashes.
fn meta_key(entry: &Entry) -> String {
    format!(
        "{}|{}|{}|{}",
        entry.kind,
        entry.path,
        entry.entry,
        entry.fragment.clone().unwrap_or_default()
    )
}

/// A cached metadata row for an entry, if one has been fetched.
fn entry_sort_row(
    index: usize,
    entry: &Entry,
    cache: &HashMap<String, Option<MetaRow>>,
    starred: &HashSet<String>,
) -> SortRow {
    let meta = meta_for(cache, entry).unwrap_or_default();
    SortRow {
        original: Some(index as f64),
        title: meta.title.unwrap_or_else(|| entry.name.clone()),
        artist: meta.artist.unwrap_or_default(),
        album: meta.album.unwrap_or_default(),
        album_artist: meta.album_artist.unwrap_or_default(),
        composer: meta.composer.unwrap_or_default(),
        genre: meta.genre.unwrap_or_default(),
        year: meta.year.map(f64::from),
        disc_number: meta.disc_number.map(f64::from),
        track_number: meta.track_number.map(f64::from),
        duration: meta.duration,
        file_size_bytes: meta.file_size_bytes.map(|value| value as f64),
        sample_rate: meta.sample_rate.map(f64::from),
        bits_per_sample: meta.bits_per_sample.map(f64::from),
        bitrate: meta.bitrate.map(f64::from),
        channels: meta.channels.map(f64::from),
        codec: meta.codec.unwrap_or_default(),
        path: entry_path(entry),
        filename: entry_filename(entry),
        star: starred.contains(&entry_star_locator(entry)),
    }
}

fn meta_for(cache: &HashMap<String, Option<MetaRow>>, entry: &Entry) -> Option<MetaRow> {
    cache.get(&meta_key(entry)).and_then(|row| row.clone())
}

fn metadata_ready(
    cache: &HashMap<String, Option<MetaRow>>,
    failed: &HashSet<String>,
    entry: &Entry,
) -> bool {
    let key = meta_key(entry);
    cache.contains_key(&key) || failed.contains(&key)
}

/// No title is visible until the metadata lookup completes. A completed
/// lookup with no title (or a failed request) uses the filename fallback.
fn display_title(
    cache: &HashMap<String, Option<MetaRow>>,
    failed: &HashSet<String>,
    entry: &Entry,
) -> Option<String> {
    let key = meta_key(entry);
    match cache.get(&key) {
        None if !failed.contains(&key) => None,
        None => Some(entry.name.clone()),
        Some(Some(meta)) => Some(
            meta.title
                .as_ref()
                .filter(|title| !title.trim().is_empty())
                .cloned()
                .unwrap_or_else(|| entry.name.clone()),
        ),
        Some(None) => Some(entry.name.clone()),
    }
}

/// The parent of an absolute path, or the empty string at the filesystem root.
/// Whether `child` equals or lives below `root` ("" is never a root).
fn is_under(child: &str, root: &str) -> bool {
    if root.trim().is_empty() {
        return false;
    }
    let root = root.trim_end_matches('/');
    child == root || child.starts_with(&format!("{root}/"))
}

fn parent_path(path: &str) -> String {
    let trimmed = path.trim_end_matches('/');
    if trimmed.is_empty() {
        return String::new();
    }
    match trimmed.rfind('/') {
        Some(0) => "/".to_owned(),
        Some(index) => trimmed[..index].to_owned(),
        None => String::new(),
    }
}

/// Whether `child` is `root` or lives inside it.
/// `POST` a JSON body and decode a JSON response, carrying the same auth and
/// errors as the `get_json` wrapper.
async fn post_json(
    url: String,
    header: Option<String>,
    body: serde_json::Value,
) -> Result<serde_json::Value, String> {
    let mut request = Request::post(&url);
    if let Some(header) = header {
        request = request.header("Authorization", &header);
    }
    let request = request
        .header("X-Kog-Device", &device_id())
        .json(&body)
        .map_err(|error| error.to_string())?;
    let response = request.send().await.map_err(|error| error.to_string())?;
    if response.status() == 401 {
        return Err("Sign in to continue".to_owned());
    }
    if !response.ok() {
        return Err(error_text(response).await);
    }
    response
        .json::<serde_json::Value>()
        .await
        .map_err(|error| error.to_string())
}

/// The server's error message for refused requests (a blocked device says
/// exactly what happened); falls back to the bare status.
async fn error_text(response: gloo_net::http::Response) -> String {
    let status = response.status();
    if let Ok(value) = response.json::<serde_json::Value>().await {
        if let Some(text) = value["error"].as_str() {
            return text.to_owned();
        }
    }
    format!("Request failed ({status})")
}

/// Walk the expanded directories into a flat list of tree lines.
/// One in-flight long press. Phones have no right click and HTML5 drag never
/// starts from a finger, so a held touch stands in for both: pending means the
/// timer is still running, armed means a queued track held long enough to be
/// dragged or menu-opened, dragging means the finger is reordering the queue.
enum TouchPress {
    Idle,
    Pending {
        row: web_sys::Element,
        x: i32,
        y: i32,
        is_track: bool,
        timer: i32,
    },
    Armed {
        row: web_sys::Element,
        x: i32,
        y: i32,
    },
    Dragging {
        from: usize,
        to: Option<usize>,
    },
}

/// Drop whatever the press was doing back to idle: stop its timer, unarm its
/// row, and tear down drag state so nothing is committed by a dead gesture.
fn cancel_touch_press(
    press: &RefCell<TouchPress>,
    window: &web_sys::Window,
    cleanup_drag: &dyn Fn(),
) {
    let taken = std::mem::replace(&mut *press.borrow_mut(), TouchPress::Idle);
    if let TouchPress::Pending { timer, .. } = taken {
        window.clear_timeout_with_handle(timer);
    }
    if let TouchPress::Armed { row, .. } = &taken {
        let _ = row.class_list().remove_1("drag-armed");
    }
    if matches!(taken, TouchPress::Dragging { .. }) {
        cleanup_drag();
    }
}

/// Shape flat search matches into browse-cache form, the way the desktop's
/// search tree nests matches under their real folders: every match lands in
/// its parent folder's bucket, missing ancestors become folder rows, and the
/// ancestor chain up to the first folder that matched by name starts
/// expanded — a matched folder itself starts collapsed, so its (already
/// reported) contents appear when it is opened, like the desktop's browse
/// nodes. Buckets sort case-insensitively by name, folders and files
/// together, as the desktop's tree rows do.
fn build_search_tree(
    matches: &[Entry],
    root: &str,
) -> (HashMap<String, Vec<Entry>>, HashSet<String>) {
    let root = root.trim_end_matches('/');
    let key = |parent: &str| -> String {
        if parent == root {
            String::new()
        } else {
            parent.to_owned()
        }
    };
    let matched: HashSet<String> = matches
        .iter()
        .filter(|entry| entry.is_dir())
        .map(|entry| entry.path.clone())
        .collect();
    let mut map: HashMap<String, Vec<Entry>> = HashMap::new();
    let push = |map: &mut HashMap<String, Vec<Entry>>, parent: String, entry: Entry| {
        let bucket = map.entry(parent).or_default();
        if !bucket
            .iter()
            .any(|item| item.path == entry.path && item.entry == entry.entry)
        {
            bucket.push(entry);
        }
    };
    let mut expanded = HashSet::new();
    for match_ in matches {
        // An archive member's hierarchy lives inside its container: the
        // member nests under the archive node and virtual subfolders spelled
        // "archive/sub", the same paths the browse endpoint serves.
        let hier = if match_.kind == "archive" && !match_.entry.is_empty() {
            format!("{}/{}", match_.path.trim_end_matches('/'), match_.entry)
        } else {
            match_.path.clone()
        };
        // Ancestors from the root down: expansion stops at (and below) the
        // first folder that matched by name, so a matched folder stays
        // collapsed while plain folders on the way to a match open up.
        let mut chain: Vec<String> = Vec::new();
        let mut parent = parent_path(&hier);
        while !parent.is_empty() && parent != root {
            chain.push(parent.clone());
            parent = parent_path(&parent);
        }
        chain.reverse();
        let mut blocked = false;
        for ancestor in &chain {
            push(
                &mut map,
                key(&parent_path(ancestor)),
                Entry {
                    kind: "dir".to_owned(),
                    path: ancestor.clone(),
                    entry: String::new(),
                    fragment: None,
                    name: last_segment(ancestor),
                    location: ancestor.clone(),
                },
            );
            if !blocked {
                if matched.contains(ancestor) {
                    blocked = true;
                } else {
                    expanded.insert(ancestor.clone());
                }
            }
        }
        push(&mut map, key(&parent_path(&hier)), match_.clone());
    }
    for bucket in map.values_mut() {
        bucket.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    }
    (map, expanded)
}

fn flatten(
    children: &HashMap<String, Vec<Entry>>,
    expanded: &HashSet<String>,
    parent: &str,
    depth: usize,
    out: &mut Vec<TreeRow>,
) {
    let Some(items) = children.get(parent) else {
        return;
    };
    for item in items {
        let is_expanded = expanded.contains(&item.path);
        out.push(TreeRow {
            name: item.name.clone(),
            path: item.path.clone(),
            parent: parent.to_owned(),
            is_dir: item.is_dir(),
            depth,
            expanded: is_expanded,
            kind: item.kind.clone(),
            entry: item.entry.clone(),
            fragment: item.fragment.clone(),
        });
        if item.is_dir() && is_expanded {
            flatten(children, expanded, &item.path, depth + 1, out);
        }
    }
}

/// One `/api/library?path=` response as entries: directories first, then files,
/// exactly as the API returns them.
fn library_entries(value: &serde_json::Value) -> Vec<Entry> {
    let mut items: Vec<Entry> = Vec::new();
    if let Some(dirs) = value["directories"].as_array() {
        for dir in dirs {
            let path = dir["path"].as_str().unwrap_or_default().to_owned();
            let name = dir["name"]
                .as_str()
                .map(str::to_owned)
                .unwrap_or_else(|| last_segment(&path));
            let where_ = dir["relative"]
                .as_str()
                .map(str::to_owned)
                .unwrap_or_else(|| name.clone());
            items.push(Entry {
                kind: "dir".to_owned(),
                path,
                entry: String::new(),
                fragment: None,
                name,
                location: where_,
            });
        }
    }
    if let Some(files) = value["files"].as_array() {
        items.extend(files.iter().map(browse_file_entry));
    }
    items
}

/// One file line (a library file or an expanded track) as a pane entry: the
/// server expands folder playlists into `kind`/`entry`/`fragment` locators;
/// older responses (and plain files) fall back to a bare local entry.
fn browse_file_entry(file: &serde_json::Value) -> Entry {
    let path = file["path"].as_str().unwrap_or_default().to_owned();
    let name = file["name"]
        .as_str()
        .map(str::to_owned)
        .unwrap_or_else(|| last_segment(&path));
    let where_ = file["relative"]
        .as_str()
        .map(str::to_owned)
        .unwrap_or_else(|| name.clone());
    let kind = file["kind"]
        .as_str()
        .filter(|kind| !kind.is_empty())
        .unwrap_or("local")
        .to_owned();
    let entry = file["entry"].as_str().unwrap_or_default().to_owned();
    let fragment = file["fragment"]
        .as_str()
        .filter(|fragment| !fragment.is_empty())
        .map(str::to_owned);
    Entry {
        kind,
        path,
        entry,
        fragment,
        name,
        location: where_,
    }
}

/// The playable files in a library response.

fn visualizer_spectrum_bins(frequencies: &[u8], sample_rate: f32, fft_size: usize) -> Vec<f32> {
    if frequencies.is_empty() || sample_rate <= 0.0 {
        return vec![0.0; 40];
    }
    let top = (sample_rate / 2.0).min(20_000.0).max(40.0);
    (0..40)
        .map(|band| {
            let edge = |step: usize| {
                ((30.0 * (top / 30.0).powf(step as f32 / 40.0) * fft_size as f32 / sample_rate)
                    .round() as usize)
                    .min(frequencies.len() - 1)
            };
            let low = edge(band);
            let high = edge(band + 1).max(low + 1).min(frequencies.len());
            f32::from(*frequencies[low..high].iter().max().unwrap_or(&0)) / 255.0
        })
        .collect()
}

fn draw_web_visualizer(
    canvas: &web_sys::HtmlCanvasElement,
    wave: &[f32],
    spectrum: &[f32],
    spectrum_mode: bool,
) {
    let Ok(Some(context)) = canvas.get_context("2d") else {
        return;
    };
    let Ok(context) = context.dyn_into::<web_sys::CanvasRenderingContext2d>() else {
        return;
    };
    let width = f64::from(canvas.width());
    let height = f64::from(canvas.height());
    let padding = 24.0;
    let plot_width = width - padding * 2.0;
    let plot_height = height - padding * 2.0;
    context.set_fill_style_str("#10191f");
    context.fill_rect(0.0, 0.0, width, height);
    context.set_stroke_style_str("#263944");
    context.set_line_width(1.0);
    for line in 1..4 {
        let y = padding + plot_height * f64::from(line) / 4.0;
        context.begin_path();
        context.move_to(padding, y);
        context.line_to(width - padding, y);
        context.stroke();
    }
    if spectrum_mode {
        if spectrum.is_empty() {
            return;
        }
        let stride = plot_width / spectrum.len() as f64;
        for (index, &level) in spectrum.iter().enumerate() {
            let fraction = index as f64 / spectrum.len() as f64;
            let red = (66.0 + 3.0 * fraction).round() as u8;
            let green = (223.0 - 37.0 * fraction).round() as u8;
            let blue = (163.0 + 92.0 * fraction).round() as u8;
            context.set_fill_style_str(&format!("rgb({red},{green},{blue})"));
            let bar = f64::from(level.clamp(0.0, 1.0)) * plot_height;
            context.fill_rect(
                padding + index as f64 * stride,
                height - padding - bar,
                (stride - 2.0).max(1.0),
                bar,
            );
        }
    } else if wave.len() > 1 {
        context.set_stroke_style_str("#42dfa3");
        context.set_line_width(2.0);
        context.begin_path();
        for (index, &sample) in wave.iter().enumerate() {
            let x = padding + plot_width * index as f64 / (wave.len() - 1) as f64;
            let y = height / 2.0 - f64::from(sample.clamp(-1.0, 1.0)) * plot_height * 0.48;
            if index == 0 {
                context.move_to(x, y);
            } else {
                context.line_to(x, y);
            }
        }
        context.stroke();
    }
}

#[wasm_bindgen(start)]
pub fn start() {
    console_error_panic_hook::set_once();
    leptos::mount::mount_to_body(App);
}

#[component]
fn App() -> impl IntoView {
    // A phone opening the server's own address is already pointed at it.
    let origin = web_sys::window()
        .and_then(|window| window.location().origin().ok())
        .unwrap_or_default();
    let (server, set_server) = signal(load("kog.server").unwrap_or(origin));
    let (token, set_token) = signal(load("kog.token").unwrap_or_default());
    let (user, set_user) = signal(String::new());
    let (password, set_password) = signal(String::new());
    let (use_basic, set_use_basic) = signal(false);
    let (codec, set_codec) = signal(load("kog.codec").unwrap_or_else(|| "aac".to_owned()));
    let (connected, set_connected) = signal(false);
    let (message, set_message) = signal(String::new());
    let (settings_open, set_settings_open) = signal(false);
    // The application (`☰`) menu, the separate About dialog, and the version
    // the server reports, shown in About.
    let (menu_open, set_menu_open) = signal(false);
    let (add_url_open, set_add_url_open) = signal(false);
    let (add_url_text, set_add_url_text) = signal(String::new());
    let (about_open, set_about_open) = signal(false);
    // The server-side folder picker opened by the tree toolbar's folder
    // button: `picker_open` shows it, `picker_dir` is the directory it is
    // currently listing, and `picker_entries`/`picker_parent` mirror the
    // browse response for that directory.
    let (picker_open, set_picker_open) = signal(false);
    let (picker_dir, set_picker_dir) = signal(String::new());
    let (picker_entries, set_picker_entries) = signal(Vec::<(String, String)>::new());
    let (picker_parent, set_picker_parent) = signal(Option::<String>::None);
    // The MIDI synthesizer the server decodes with, editable like the
    // desktop's Preferences > Synthesis backend.
    let (midi_engine, set_midi_engine) = signal(String::new());
    let (midi_options, set_midi_options) = signal(Vec::<(String, String)>::new());
    let (version, set_version) = signal(String::new());
    // Queue indices the pane actions act on. The anchor is a queue index too;
    // Shift-click resolves its range through the current visible row order.
    let (selected, set_selected) = signal(HashSet::<usize>::new());
    let (selection_anchor, set_selection_anchor) = signal(Option::<usize>::None);
    // Live-reload: the content hash of the running `/kog_web.js`, and whether a
    // newer build has appeared. A change reloads on the next pause or idle.
    let (asset_etag, set_asset_etag) = signal(Option::<String>::None);
    let (update_ready, set_update_ready) = signal(false);
    // When the pending build first appeared. A deferred reload (music
    // playing) force-applies after a grace period, or a tab with hours of
    // playing never reaches the new build at all.
    let (update_since, set_update_since) = signal(Option::<f64>::None);
    // Desktop shows the sidebar inline. Phones use full-width views with a
    // persistent bottom tab bar; their navigation state is local to the tab.
    let (sidebar_open, set_sidebar_open) = signal(false);
    let (mobile_view, set_mobile_view) = signal(MobileView::Queue);
    // Draggable tree-pane width (the splitter between the tree and the pane).
    let (sidebar_width, set_sidebar_width) = signal(
        load("kog.sidebar-width")
            .and_then(|value| value.trim().parse::<f64>().ok())
            .map(|value| value.clamp(180.0, 600.0))
            .unwrap_or(260.0),
    );
    let (resizing_sidebar, set_resizing_sidebar) = signal(false);
    let (sidebar_visible, set_sidebar_visible) = signal(
        load("kog.sidebar")
            .map(|value| value != "0")
            .unwrap_or(true),
    );
    let (files_expanded, set_files_expanded) = signal(true);
    let (playlists_expanded, set_playlists_expanded) = signal(true);

    // Use the saved tree root on the first radio request, even before its
    // directory listing has finished restoring.
    let restored = load("kog.session")
        .map(|raw| decode_session(&raw))
        .unwrap_or_default();

    // Library tree, loaded one directory at a time as it is expanded.
    let (children, set_children) = signal(HashMap::<String, Vec<Entry>>::new());
    let (expanded, set_expanded) = signal(HashSet::<String>::new());
    let (library_root, set_library_root) = signal(String::new());
    // The directory the tree is rooted at: "" means the server's library root.
    let (tree_root, set_tree_root) = signal(restored.tree_root.clone());
    let (tree_selected, set_tree_selected) = signal(String::new());
    // Whether the selected tree row is a folder, so the root button knows
    // whether to re-root or step up.
    let (tree_selected_dir, set_tree_selected_dir) = signal(false);
    let (tree_search, set_tree_search) = signal(String::new());
    // Server-side search results for the tree box, shaped like the browse
    // cache so the pane renders one tree either way: matches sit under their
    // real parent folders ("" is the tree root's own level), and a folder
    // with no bucket yet — an archive container, whose members the walk never
    // entered — fetches its listing when expanded, like the desktop's lazy
    // browse nodes.
    let (search_children, set_search_children) = signal(HashMap::<String, Vec<Entry>>::new());
    // Which search folders are expanded. Ancestors of matches start expanded,
    // a folder that matched by name starts collapsed, exactly like the
    // desktop's TreeSearchLayout.
    let (search_expanded, set_search_expanded) = signal(HashSet::<String>::new());
    // How many matches the walk has reported and whether the client stopped
    // pulling early, for the status line under the search box.
    let (search_count, set_search_count) = signal(0usize);
    let (search_capped, set_search_capped) = signal(false);
    // The desktop's progress counters: items scanned on the filesystem pass,
    // then archive listings, exactly the numbers its status line shows.
    let (search_progress, set_search_progress) = signal(SearchProgress::default());
    let (search_paused, set_search_paused) = signal(false);
    // Now-playing notifications, the web twin of the desktop's Track
    // Notifications preference. Off until enabled from settings, which is
    // also where the browser's permission question is asked.
    let (track_notifications, set_track_notifications) =
        signal(load("kog.track_notifications").as_deref() == Some("1"));
    // True while a query's walk is still streaming results in.
    let (tree_search_pending, set_tree_search_pending) = signal(false);
    // Right-click menu anchor and target row.
    let (tree_menu, set_tree_menu) = signal(Option::<(f64, f64, TreeRow)>::None);
    // Right-click on a playlist row: the Qt playlist context menu (play,
    // remove, select all, clear, reveal in the file tree).
    let (song_menu, set_song_menu) = signal(Option::<(f64, f64, usize, Entry)>::None);
    // Right-click on a playlist row in the sidebar: the desktop's playlist
    // context menu. `renaming_playlist` swaps that row's label for an edit
    // field.
    let (playlist_menu, set_playlist_menu) = signal(Option::<(f64, f64, i64, String)>::None);
    let (renaming_playlist, set_renaming_playlist) = signal(Option::<i64>::None);
    let (rename_text, set_rename_text) = signal(String::new());
    let rename_input = NodeRef::<leptos::html::Input>::new();
    // The enlarged cover view, opened by clicking the transport art like
    // the desktop's cover dialog.
    let (cover_open, set_cover_open) = signal(false);
    // Modal prompts where the desktop opens dialogs: naming a new playlist
    // (the + button), naming a duplicate, and confirming a delete.
    let (playlist_dialog, set_playlist_dialog) = signal(Option::<PlaylistDialog>::None);
    let playlist_dialog_input = NodeRef::<leptos::html::Input>::new();
    // Bumped only when a dialog opens: the focus effect must not re-run
    // while the visitor types (re-selecting the text would overwrite it).
    let (playlist_dialog_opened, set_playlist_dialog_opened) = signal(0u32);
    // The row being dragged from the tree onto the playlist pane.
    let (dragging_tree, set_dragging_tree) = signal(Option::<TreeRow>::None);
    // A playlist row dragged toward the pane (append its tracks), and a queue
    // row dragged to a new position (reorder).
    let (dragging_playlist, set_dragging_playlist) = signal(Option::<i64>::None);
    // Reordering the saved playlists by dragging rows in their section.
    let (playlist_reorder_to, set_playlist_reorder_to) = signal(Option::<usize>::None);
    let (dragging_track, set_dragging_track) = signal(Option::<usize>::None);
    let (reorder_to, set_reorder_to) = signal(Option::<usize>::None);
    let (playlist_drop_active, set_playlist_drop_active) = signal(false);
    // Playlist columns, their widths and visibility, persisted like the Qt
    // window's saved column layout.
    let (columns, set_columns) = signal(
        load("kog.columns")
            .map(|raw| decode_columns(&raw))
            .unwrap_or_else(default_columns),
    );
    // (x, y, the column the menu acts on). The target drives Move Left/Right
    // and Auto-Fit Column, matching the Qt header menu.
    let (column_menu, set_column_menu) = signal(Option::<(f64, f64, ColumnId)>::None);
    let (mobile_sort_open, set_mobile_sort_open) = signal(false);
    let (track_details, set_track_details) = signal(Option::<(usize, Entry)>::None);
    // (column, pointer start x, width at pointer-down) while a divider drags.
    let (resizing, set_resizing) = signal(Option::<(ColumnId, f64, f64)>::None);

    let (playlists, set_playlists) = signal(Vec::<(i64, String, i64)>::new());
    // The remembered pane and transport modes appear on the first paint.
    // `restoring` suppresses persistence until the initial restore settles.
    // While this is true the session effect ignores signal changes, so the
    // empty defaults never overwrite the saved session before the restore has
    // run.
    let restoring = RwSignal::new(true);
    let persist = persistence::Writer::new("session-ui", Callback::new(move |error| set_message.set(error)));
    let ui_loaded_scope = StoredValue::new(None::<String>);
    let restored_tree = RwSignal::new(Some((restored.tree_root.clone(), restored.expanded.clone())));
    let (queue, set_queue) = signal(restored.queue.clone());
    let (list_name, set_list_name) = signal(restored.list_name.clone());
    let (current, set_current) = signal(restored.current);
    let (playing, set_playing) = signal(false);
    // Band levels of the currently streaming track, for the playing row's
    // meter: five 0..1 values polled from the server while it decodes.
    let (audio_levels, set_audio_levels) = signal([0.0_f32; 5]);
    let (visualizer_open, set_visualizer_open) = signal(false);
    let (visualizer_spectrum_mode, set_visualizer_spectrum_mode) = signal(false);
    let (visualizer_wave, set_visualizer_wave) = signal(Vec::<f32>::new());
    let (visualizer_spectrum, set_visualizer_spectrum) = signal(Vec::<f32>::new());
    let visualizer_canvas = NodeRef::<leptos::html::Canvas>::new();
    Effect::new(move |_| {
        if !visualizer_open.get() {
            return;
        }
        let wave = visualizer_wave.get();
        let spectrum = visualizer_spectrum.get();
        let mode = visualizer_spectrum_mode.get();
        if let Some(canvas) = visualizer_canvas.get() {
            draw_web_visualizer(&canvas, &wave, &spectrum, mode);
        }
    });
    // Stopped is stricter than paused: nothing was played and nothing is held
    // mid-song. A fresh page load starts stopped, and Stop returns here, so
    // the current row shows no playing or paused glyph.
    let (stopped, set_stopped) = signal(true);
    // Browsing down on a phone leaves a small now-playing strip; scrolling
    // back up or tapping its expand button restores seeking and volume.
    let (transport_compact, set_transport_compact) = signal(true);
    // Touch mode: coarse-pointer devices (phones, tablets) get single-tap
    // activation — taps play and enqueue, so nothing requires a double
    // click, a hold, or a drag. Holds still open the context menus.
    // ?touch=1 / ?touch=0 force the mode (trying it on a desktop, or a
    // fine-pointer device that still wants taps).
    let touch_mode = {
        let coarse = web_sys::window()
            .and_then(|window| window.match_media("(pointer: coarse)").ok().flatten())
            .map(|media| media.matches())
            .unwrap_or(false);
        match web_sys::window().and_then(|window| window.location().search().ok()) {
            Some(search) if search.contains("touch=0") => false,
            Some(search) if search.contains("touch=1") => true,
            _ => coarse,
        }
    };
    // The stylesheet keys the touch affordances (+ buttons) off this class,
    // so the forced mode and the detected mode look the same.
    if touch_mode {
        if let Some(body) = web_sys::window()
            .and_then(|window| window.document())
            .map(|document| document.body())
            .flatten()
        {
            let _ = body.class_list().add_1("touch");
        }
    }
    // One-line outcomes for actions the pane itself shows nothing about,
    // e.g. "Added to playlist"; shown where the track count normally lives.
    let (status_note, set_status_note) = signal(String::new());
    let (filter, set_filter) = signal(String::new());
    let (sort_key, set_sort_key) = signal(SortKey::Index);
    let (sort_asc, set_sort_asc) = signal(true);
    let (volume, set_volume) = signal(restored.volume.unwrap_or(0.9).clamp(0.0, 1.0));
    let (volume_before_mute, set_volume_before_mute) = signal(0.9_f64);
    let (position, set_position) = signal(0.0_f64);
    // The media element's own duration, reset per track. ADTS and FLAC report
    // Infinity here, so the effective duration falls back to the tag duration
    // until the browser can measure it.
    let (media_duration, set_media_duration) = signal(Option::<f64>::None);
    let (shuffle, set_shuffle) = signal(restored.shuffle);
    let (repeat_mode, set_repeat_mode) = signal(restored.repeat);
    let (radio_on, set_radio_on) = signal(restored.radio_on);
    let (radio_waiting, set_radio_waiting) = signal(false);
    // Tag cache keyed by locator. An `Rc` so reading it clones a pointer, not
    // the map, on every cell render.
    let (metadata, set_metadata) = signal_local(Rc::new(HashMap::<String, Option<MetaRow>>::new()));
    // A failed request uses the filename until another queue change retries
    // the lookup. Keep failures separate from cached "no tags" results.
    let (metadata_failed, set_metadata_failed) = signal_local(Rc::new(HashSet::<String>::new()));
    let audio_ref = NodeRef::<leptos::html::Audio>::new();
    let workspace_dispatch =
        StoredValue::new(None::<Callback<kog_playback_policy::workspace::Command>>);

    let (policy_revision, set_policy_revision) = signal(0u64);
    // Starred locators from `GET /api/stars`, in the same scheme the server
    // stores them under. An `Rc` for the same reason as `metadata`.
    let (stars, set_stars) = signal_local(Rc::new(HashSet::<String>::new()));

    let base = move || server.get().trim_end_matches('/').to_owned();
    let auth = move || {
        if use_basic.get() {
            Auth::Basic(user.get(), password.get())
        } else if token.get().trim().is_empty() {
            Auth::None
        } else {
            Auth::Token(token.get())
        }
    };

    let named_session = web_sys::window()
        .and_then(|w| w.location().search().ok())
        .and_then(|query| web_sys::UrlSearchParams::new_with_str(&query).ok())
        .and_then(|query| query.get("session"))
        .filter(|id| !id.is_empty() && id.len() <= 240)
        .map(|id| format!("web:{id}"));
    let session_id = named_session.clone().unwrap_or_else(|| {
        load("kog.backend-session-id").unwrap_or_else(|| format!("web:{}", device_id()))
    });
    if named_session.is_none() {
        store("kog.backend-session-id", &session_id);
    }
    let mut application = Session::new(
        session_id.clone(),
        js_sys::Date::now() as u64,
        restored.shuffle,
        restored.repeat,
    );
    let resumed = load(&format!("kog.backend-session.{session_id}"))
        .and_then(|raw| serde_json::from_str(&raw).ok())
        .is_some_and(|saved| {
            application
                .restore(saved, |value| {
                    serde_json::from_value(value.clone())
                        .or_else(|_| Ok::<Entry, String>(entry_from_json(value)))
                })
                .is_ok()
        });
    if !resumed && named_session.is_none() {
        let _ = application.dispatch(SessionCommand::Replace {
            tracks: restored.queue.clone(),
            current: (restored.current < restored.queue.len()).then_some(restored.current),
        });
        let _ = application.dispatch(SessionCommand::Volume {
            value: restored.volume.unwrap_or(0.9),
        });
        if let Some(value) =
            load("kog.playlist-tabs").and_then(|raw| serde_json::from_str(&raw).ok())
        {
            let _ = application.dispatch(SessionCommand::WorkspaceRestore { value });
        }
    }
    let session_model = StoredValue::new(application);
    let output_token = RwSignal::new(None::<Token>);
    let output_start = StoredValue::new(0.0_f64);
    let backend = session::Controller {
        model: session_model,
        revision: RwSignal::new(0),
        output: StoredValue::new(None),
        saved: StoredValue::new(None),
        persistence: persistence::Writer::new("sessions", Callback::new(move |error| set_message.set(error))),
        error: Callback::new(move |error| set_message.set(error)),
        loaded_scope: StoredValue::new(None),
        base: Callback::new(move |()| base()),
        auth: Callback::new(move |()| auth().header()),
        changed: Callback::new(move |(queue_changed, progress): (bool, bool)| {
            session_model.with_value(|model| {
                let v = model.snapshot();
                if progress {
                    set_position.set(v.position);
                    return;
                }
                if queue_changed {
                    set_queue.set(v.queue.to_vec());
                }
                let next_current = v.current.unwrap_or(usize::MAX);
                if current.get_untracked() != next_current {
                    set_current.set(next_current);
                }
                set_playing.set(matches!(
                    v.transport,
                    Transport::Playing | Transport::Starting
                ));
                set_stopped.set(v.transport == Transport::Stopped);
                set_position.set(v.position);
                set_volume.set(v.volume);
                set_selected.set(v.selection.indices.iter().copied().collect());
                set_selection_anchor.set(v.selection.anchor);
                set_radio_on.set(v.radio_enabled);
                set_radio_waiting.set(v.radio_waiting);
                set_shuffle.set(v.shuffle);
                set_repeat_mode.set(v.repeat);
                set_filter.set(v.filter.to_owned());
                set_sort_asc.set(!v.descending);
                let key = ColumnId::ALL
                    .into_iter()
                    .find(|c| c.key() == v.sort_column)
                    .map(|c| c.sort_key())
                    .unwrap_or(SortKey::Index);
                set_sort_key.set(key);
                if let Some(error) = v.error {
                    set_message.set(error.to_owned());
                }
                set_policy_revision.set(v.revision);
            })
        }),
    };
    backend.changed.run((true, false));
    if let Some(window) = web_sys::window() {
        let callback = Closure::<dyn FnMut(web_sys::Event)>::new(move |event: web_sys::Event| {
            if backend.persistence.dirty() || persist.dirty() {
                event.prevent_default();
                let _ = js_sys::Reflect::set(event.as_ref(), &"returnValue".into(), &"".into());
            }
        });
        let _ = window.add_event_listener_with_callback("beforeunload", callback.as_ref().unchecked_ref());
        callback.forget();
    }

    let playlist_workspace = workspace::Controller::new(backend);
    // One table renders either the live queue or the active saved-playlist
    // draft. Transport continues to read `queue`; pane actions use these views.
    let pane_key = Memo::new(move |_| playlist_workspace.snapshot().active.clone());
    let pane_entries = Memo::new(move |_| {
        if pane_key.get() == "queue" {
            queue.get()
        } else {
            playlist_workspace.snapshot().entries.iter().map(entry_from_json).collect()
        }
    });
    let pane_selected = Memo::new(move |_| {
        playlist_workspace.snapshot().selected.iter().copied().collect::<HashSet<_>>()
    });
    let pane_current = Memo::new(move |_| playlist_workspace.snapshot().current);
    let draft_filters = RwSignal::new(HashMap::<String, String>::new());
    let draft_sorts = RwSignal::new(HashMap::<String, (SortKey, bool)>::new());
    let pane_filter = Memo::new(move |_| {
        let key = pane_key.get();
        if key == "queue" { filter.get() }
        else { draft_filters.with(|filters| filters.get(&key).cloned().unwrap_or_default()) }
    });
    let pane_visible = Memo::new(move |_| {
        if pane_key.get() == "queue" {
            backend.revision.track();
            session_model.with_value(|model| model.visible().to_vec())
        } else {
            let query = pane_filter.get();
            let entries = pane_entries.read();
            if query.trim().is_empty() { return (0..entries.len()).collect(); }
            let cache = metadata.read();
            let starred = stars.read();
            entries.iter().enumerate()
                .filter_map(|(index, entry)| entry_sort_row(index, entry, &cache, &starred)
                    .matches(&query).then_some(index))
                .collect()
        }
    });
    let filter_pane = move |query: String| {
        let key = pane_key.get_untracked();
        if key == "queue" {
            backend.send(SessionCommand::Filter { query });
        } else {
            draft_filters.update(|filters| { filters.insert(key, query); });
            playlist_workspace.send(kog_playback_policy::workspace::Command::Selection {
                command: kog_playback_policy::selection::Command::Clear,
            });
        }
    };
    Effect::new(move |_| {
        pane_key.track();
        set_song_menu.set(None);
        set_dragging_track.set(None);
        set_reorder_to.set(None);
        clear_track_drop_marker();
    });
    let select_row = move |index: usize, shift: bool, toggle: bool| {
        use kog_playback_policy::selection::Command;
        let selection = session_model.with_value(|model| {
            if pane_key.get_untracked() == "queue" { model.selection().clone() }
            else { model.workspace_model().selection() }
        });
        let (indices, anchor) = selection::click_selection(
            &selection.indices.into_iter().collect(), selection.anchor,
            &pane_visible.get_untracked(), index, shift, toggle,
        );
        playlist_workspace.send(kog_playback_policy::workspace::Command::Selection {
            command: Command::Set { indices: indices.into_iter().collect(), anchor },
        });
    };

    Effect::new(move |_| {
        let entries = queue.get();
        let cache = metadata.get();
        let starred = stars.get();
        let rows = entries
            .iter()
            .enumerate()
            .map(|(i, e)| entry_sort_row(i, e, &cache, &starred))
            .collect();
        backend.send(SessionCommand::Metadata { rows });
    });

    // One place that talks to the API, so every call carries the same auth and
    // reports the same failures.
    let get_json = move |route: String| {
        let mut url = format!("{}{route}", base());
        if route.starts_with("/api/library/search") {
            let id = session_model.with_value(|s| s.id().to_owned());
            url.push_str(&format!(
                "{}session={}",
                if route.contains('?') { "&" } else { "?" },
                url_encode(&id)
            ));
        }
        let header = auth().header();
        let device = device_id();
        async move {
            let mut request = Request::get(&url);
            if let Some(header) = header {
                request = request.header("Authorization", &header);
            }
            request = request.header("X-Kog-Device", &device);
            let response = request.send().await.map_err(|error| error.to_string())?;
            if response.status() == 401 {
                return Err("Sign in to continue".to_owned());
            }
            if !response.ok() {
                return Err(error_text(response).await);
            }
            response
                .json::<serde_json::Value>()
                .await
                .map_err(|error| error.to_string())
        }
    };

    // The tree box searches the whole music folder on the server (like the
    // desktop's tree search), not just the folders already loaded. Debounced:
    // a stale reply for an older query is dropped when the text has moved on.
    let run_tree_search = {
        let get_json = get_json;
        move |query: String| {
            let trimmed = query.trim().to_owned();
            leptos::task::spawn_local(async move {
                sleep_ms(300).await;
                if tree_search.get_untracked().trim() != trimmed {
                    return;
                }
                if trimmed.is_empty() {
                    set_search_children.set(HashMap::new());
                    set_search_expanded.set(HashSet::new());
                    set_search_count.set(0);
                    set_search_capped.set(false);
                    set_search_progress.set(SearchProgress::default());
                    set_tree_search_pending.set(false);
                    return;
                }
                // A huge library takes a while to cross: the pane must say it
                // is searching rather than declare "no matches" prematurely.
                // Stale numbers from the previous query go first.
                set_search_count.set(0);
                set_search_capped.set(false);
                set_search_progress.set(SearchProgress::default());
                set_search_paused.set(false);
                set_tree_search_pending.set(true);
                let parse = |value: &serde_json::Value| -> Vec<Entry> {
                    let mut rows = Vec::new();
                    if let Some(list) = value["results"].as_array() {
                        for item in list {
                            let path = item["path"].as_str().unwrap_or_default().to_owned();
                            if path.is_empty() {
                                continue;
                            }
                            let name = item["name"]
                                .as_str()
                                .map(str::to_owned)
                                .unwrap_or_else(|| last_segment(&path));
                            // A folder whose own name matched is itself a
                            // result; the walk already reported everything
                            // under it, so its bucket fills from matches. An
                            // archive container matches as a folder too, but
                            // its members were never walked, so its bucket
                            // stays missing and expanding fetches the listing.
                            let is_dir = item["is_dir"].as_bool().unwrap_or(false);
                            let member = item["entry"].as_str().unwrap_or_default();
                            let member_dir = is_dir && !member.is_empty();
                            let kind = if is_dir {
                                "dir".to_owned()
                            } else {
                                item["kind"].as_str().unwrap_or("local").to_owned()
                            };
                            rows.push(Entry {
                                name,
                                path: if member_dir {
                                    format!("{}/{}", path.trim_end_matches('/'), member)
                                } else {
                                    path
                                },
                                kind,
                                entry: if member_dir {
                                    String::new()
                                } else {
                                    member.to_owned()
                                },
                                fragment: None,
                                location: member.to_owned(),
                            });
                        }
                    }
                    rows
                };
                // Rebuild the results tree from the matches gathered so far.
                // Ran after every batch: matches nest progressively deeper as
                // the walk streams in, and folders the visitor expanded while
                // waiting stay expanded.
                let parse_progress = |value: &serde_json::Value| -> SearchProgress {
                    SearchProgress {
                        scanned: value["scanned"].as_u64().unwrap_or(0),
                        archive_count: value["archive_count"].as_u64().unwrap_or(0),
                        archives_scanned: value["archives_scanned"].as_u64().unwrap_or(0),
                        unreadable: value["unreadable_archives"].as_u64().unwrap_or(0),
                        scanning_archives: value["scanning_archives"].as_bool().unwrap_or(false),
                    }
                };
                let publish = |rows: &[Entry], progress: SearchProgress| {
                    let root = {
                        let rooted = tree_root.get_untracked();
                        if rooted.is_empty() {
                            library_root.get_untracked()
                        } else {
                            rooted
                        }
                    };
                    let (map, auto_expanded) = build_search_tree(rows, &root);
                    let mut expanded = search_expanded.get_untracked();
                    expanded.extend(auto_expanded);
                    set_search_children.set(map);
                    set_search_expanded.set(expanded);
                    set_search_count.set(rows.len());
                    set_search_progress.set(progress);
                };
                match get_json(format!("/api/library/search?q={}", url_encode(&trimmed))).await {
                    Ok(value) => {
                        // The walk runs on its own thread and matches
                        // accumulate in the shared buffer; the client pulls
                        // slices by offset until the walk reports done. A
                        // newer query supersedes this one both here and on
                        // the server.
                        let generation = value["generation"].as_u64().unwrap_or_default();
                        let mut limited = value["limited"].as_bool().unwrap_or(false);
                        let mut progress = parse_progress(&value);
                        let mut rows = parse(&value);
                        // Members of one archive share the archive's path, so
                        // identity includes the member name.
                        let identity = |row: &Entry| format!("{}|{}", row.path, row.entry);
                        let mut seen: HashSet<String> =
                            rows.iter().map(|row| identity(row)).collect();
                        let mut done = value["done"].as_bool().unwrap_or(true);
                        if tree_search.get_untracked().trim() == trimmed {
                            publish(&rows, progress);
                        }
                        // The desktop's search stops at its own matchLimit;
                        // mirror that 2000 instead of ending after the first
                        // batch, so the same query reaches the same matches.
                        let mut offset = rows.len();
                        while !done && rows.len() < 2000 {
                            if tree_search.get_untracked().trim() != trimmed {
                                break;
                            }
                            // Poll at the desktop's cadence: the numbers are
                            // a progress readout, not a per-request flicker.
                            // A paused walk is not advancing; poll gently.
                            sleep_ms(250).await;
                            while search_paused.get() {
                                sleep_ms(200).await;
                            }
                            match get_json(format!(
                                "/api/library/search/more?g={generation}&offset={offset}"
                            ))
                            .await
                            {
                                Ok(value) => {
                                    if value["generation"].as_u64() != Some(generation) {
                                        break;
                                    }
                                    for row in parse(&value) {
                                        if seen.insert(identity(&row)) {
                                            rows.push(row);
                                        }
                                    }
                                    progress = parse_progress(&value);
                                    offset = rows.len();
                                    done = value["done"].as_bool().unwrap_or(true);
                                    if value["limited"].as_bool().unwrap_or(false) {
                                        limited = true;
                                    }
                                    if tree_search.get_untracked().trim() == trimmed {
                                        publish(&rows, progress);
                                    }
                                }
                                Err(_) => break,
                            }
                        }
                        if done || rows.len() >= 2000 {
                            set_tree_search_pending.set(false);
                        }
                    }
                    Err(error) => {
                        set_tree_search_pending.set(false);
                        set_message.set(error);
                    }
                }
            });
        }
    };

    // Shared buffering policy; HTTP is only this client's transport adapter.
    let radio_scope = move || {
        let root = tree_root.get();
        if root.is_empty() {
            library_root.get()
        } else {
            root
        }
    };
    let change_radio = move |enabled: bool, reshuffle: bool| {
        backend.send(SessionCommand::Radio {
            enabled,
            scope: base(),
            root: radio_scope(),
            reshuffle,
        })
    };
    let set_radio = move |enabled| change_radio(enabled, false);
    let reshuffle_radio = move || change_radio(true, true);
    let select_shuffle = move |mode| backend.send(SessionCommand::Shuffle { mode });
    let select_repeat = move |mode| backend.send(SessionCommand::Repeat { mode });

    // Load one directory level. The library root is keyed as "". When the load
    // was a re-root, a rejection puts the previous root back instead of leaving
    // the tree pointing at a directory the server will not browse.
    let load_dir = {
        let get_json = get_json;
        move |directory: String, revert_root: Option<String>| {
            let route = if directory.is_empty() {
                "/api/library".to_owned()
            } else {
                format!("/api/library?path={}", url_encode(&directory))
            };
            leptos::task::spawn_local(async move {
                match get_json(route).await {
                    Ok(value) => {
                        if directory.is_empty() {
                            let relative = value["path"].as_str().unwrap_or_default().to_owned();
                            set_library_root.set(relative);
                        }
                        let items = library_entries(&value);
                        set_children.update(|map| {
                            map.insert(directory.clone(), items);
                        });
                    }
                    Err(error) => {
                        set_message.set(error);
                        if let Some(previous) = revert_root {
                            set_tree_root.set(previous);
                        }
                    }
                }
            });
        }
    };

    let toggle_dir = {
        let load_dir = load_dir.clone();
        move |path: String| {
            let mut now_open = false;
            set_expanded.update(|set| {
                if set.remove(&path) {
                    now_open = false;
                } else {
                    set.insert(path.clone());
                    now_open = true;
                }
            });
            if now_open {
                let loaded = children.get().contains_key(&path);
                if !loaded {
                    load_dir(path, None);
                }
            }
        }
    };

    // Expand or collapse a folder inside search results. Matched folders'
    // contents are already part of the match set; an archive container's
    // members never are, so opening one fetches its listing through the same
    // browse endpoint the loaded tree uses — the desktop's search tree lazily
    // browses its containers the same way.
    let toggle_search_dir = {
        let get_json = get_json;
        move |path: String| {
            let mut now_open = false;
            set_search_expanded.update(|set| {
                if set.remove(&path) {
                    now_open = false;
                } else {
                    set.insert(path.clone());
                    now_open = true;
                }
            });
            if now_open && !search_children.get_untracked().contains_key(&path) {
                leptos::task::spawn_local(async move {
                    match get_json(format!("/api/library?path={}", url_encode(&path))).await {
                        Ok(value) => {
                            let items = library_entries(&value);
                            set_search_children.update(|map| {
                                map.insert(path, items);
                            });
                        }
                        Err(error) => set_message.set(error),
                    }
                });
            }
        }
    };

    // Root the tree at `path` ("" is the server's library root), collapsing the
    // old expansion the way the desktop tree resets when its root changes.
    let goto_root = {
        let load_dir = load_dir.clone();
        move |path: String| {
            let previous = tree_root.get_untracked();
            set_tree_selected.set(String::new());
            set_tree_selected_dir.set(false);
            set_expanded.set(HashSet::new());
            let loaded = children.get().contains_key(&path);
            set_tree_root.set(path.clone());
            if !loaded {
                load_dir(path, Some(previous));
            }
        }
    };

    let go_up = {
        let goto_root = goto_root.clone();
        move || {
            let current = tree_root.get();
            if current.is_empty() {
                return;
            }
            // The music directory set in the desktop's Server settings is the
            // ceiling: the tree climbs within it, never past it.
            let library = library_root.get();
            let parent = parent_path(&current);
            let target = if parent.is_empty() || parent == library || !is_under(&parent, &library) {
                String::new()
            } else {
                parent
            };
            goto_root(target);
        }
    };

    // The folder picker lists directories through the same browse endpoint,
    // so it can climb anywhere on the server; outside the music directory the
    // server answers with subfolders only.
    {
        let url_encode = url_encode;
        let get_json = get_json;
        Effect::new(move |_| {
            if !picker_open.get() {
                return;
            }
            let dir = picker_dir.get();
            if dir.is_empty() {
                return;
            }
            let url = format!("/api/library?path={}", url_encode(&dir));
            leptos::task::spawn_local(async move {
                if let Ok(value) = get_json(url).await {
                    let mut entries = Vec::new();
                    if let Some(dirs) = value["directories"].as_array() {
                        for entry in dirs {
                            let name = entry["name"].as_str().unwrap_or_default().to_owned();
                            let path = entry["path"].as_str().unwrap_or_default().to_owned();
                            if !path.is_empty() {
                                entries.push((name, path));
                            }
                        }
                    }
                    set_picker_entries.set(entries);
                    set_picker_parent.set(value["parent"].as_str().map(str::to_owned));
                }
            });
        });
    }

    // Reveal a song's location in the tree: re-root if the file lives outside
    // the current root, expand every folder on the way down, then select and
    // scroll to the song's row. Levels load lazily, so each step waits for the
    // previous fetch to land.
    let reveal_in_tree = {
        let goto_root = goto_root.clone();
        let load_dir = load_dir.clone();
        move |entry: Entry| {
            let goto_root = goto_root.clone();
            let load_dir = load_dir.clone();
            leptos::task::spawn_local(async move {
                let song = entry.path.clone();
                if entry.kind == "remote" || song.is_empty() {
                    return;
                }
                let parent = parent_path(&song);
                if parent.is_empty() {
                    return;
                }
                let library = library_root.get_untracked();
                // Inside the music directory the tree roots at the library
                // root; a song outside it can only be approached to its own
                // folder, since those listings carry no files.
                let target_root = if is_under(&parent, &library) {
                    String::new()
                } else {
                    parent.clone()
                };
                if tree_root.get_untracked() != target_root {
                    goto_root(target_root.clone());
                }
                let root_dir = if target_root.is_empty() {
                    library.clone()
                } else {
                    target_root.clone()
                };
                let mut chain = Vec::new();
                let mut cursor = parent.clone();
                while !cursor.is_empty() && cursor != root_dir {
                    chain.push(cursor.clone());
                    cursor = parent_path(&cursor);
                }
                chain.reverse();
                // The root level is keyed the way `load_dir` stored it: ""
                // for the music directory.
                for _ in 0..50 {
                    if children.get_untracked().contains_key(&target_root) {
                        break;
                    }
                    sleep_ms(100).await;
                }
                for dir in chain {
                    set_expanded.update(|set| {
                        set.insert(dir.clone());
                    });
                    if !children.get_untracked().contains_key(&dir) {
                        load_dir(dir.clone(), None);
                        let mut waited = 0;
                        while !children.get_untracked().contains_key(&dir) && waited < 50 {
                            sleep_ms(100).await;
                            waited += 1;
                        }
                        if !children.get_untracked().contains_key(&dir) {
                            return;
                        }
                    }
                }
                set_tree_selected.set(song);
                set_tree_selected_dir.set(false);
                // The selected row renders a tick after the state change, and
                // only once the expanded folders have re-rendered. Poll until
                // it exists, then land it mid-pane so it is plainly visible.
                for _ in 0..20 {
                    sleep_ms(100).await;
                    let scrolled = js_sys::eval(
                        "(() => { const row = document.querySelector('.tree-row.selected');\
                          if (!row) return 'no';\
                          row.scrollIntoView({ block: 'center' });\
                          return 'yes'; })()",
                    );
                    if let Ok(value) = scrolled
                        && value.as_string() == Some("yes".to_owned())
                    {
                        break;
                    }
                }
            });
        }
    };

    let open_folder_picker = move |_| {
        set_picker_dir.set(library_root.get());
        set_picker_entries.set(Vec::new());
        set_picker_open.set(true);
    };

    let use_picked_folder = {
        let goto_root = goto_root.clone();
        move |_| {
            let dir = picker_dir.get_untracked();
            set_picker_open.set(false);
            if dir.is_empty() {
                return;
            }
            // The music directory itself stays the "" root the tree began with.
            if dir == library_root.get_untracked() {
                goto_root(String::new());
            } else {
                goto_root(dir);
            }
        }
    };

    // Root at one of the listed folders directly from its row.
    let root_at_folder = {
        let goto_root = goto_root.clone();
        move |dir: String| {
            set_picker_open.set(false);
            if dir == library_root.get_untracked() {
                goto_root(String::new());
            } else {
                goto_root(dir);
            }
        }
    };

    // Rebuild the remembered tree once the library root is known. Children
    // load lazily, so a folder is only re-expanded after its parent level has
    // arrived; the effect runs again as each level lands and converges on the
    // saved expansion without fetching anything the user had not already
    // opened. The queue itself is restored synchronously above, so the pane is
    // already correct while the tree is still filling in.
    {
        let load_dir = load_dir.clone();
        let restore_flag = restoring.clone();
        Effect::new(move |_| {
            if !connected.get() { return; }
            let Some((root, targets)) = restored_tree.get() else { return; };
            let loaded = children.get();
            if !loaded.contains_key(&root) {
                // The library root ("") is loaded by `connect`; a deeper root
                // is fetched here and the effect re-runs when it arrives.
                if !root.is_empty() {
                    load_dir(root.clone(), None);
                }
                return;
            }
            set_tree_root.set(root.clone());
            let missing: Vec<String> = targets
                .iter()
                .filter(|path| !loaded.contains_key(*path))
                .cloned()
                .collect();
            if !missing.is_empty() {
                for path in missing {
                    load_dir(path, None);
                }
                return;
            }
            set_expanded.set(targets.iter().cloned().collect());
            restore_flag.set(false);
            restored_tree.set(None);
        });
        // A stored root that the server will not browse must not leave the
        // session permanently unpersisted: after a grace period the restore is
        // declared finished so changes start being saved again.
        let restoring = restoring.clone();
        if let Some(window) = web_sys::window() {
            let callback = Closure::<dyn FnMut()>::new(move || {
                restoring.set(false);
            });
            let _ = window.set_timeout_with_callback_and_timeout_and_arguments_0(
                callback.as_ref().unchecked_ref(),
                10_000,
            );
            callback.forget();
        }
    }

    // Persist the session whenever a remembered value changes. The effect reads
    // every persisted signal, so a change to any of them schedules a write;
    // `restoring` keeps the pre-restore defaults from overwriting the saved
    // session, and the SQLite writer drops writes that do not change the value.
    {
        let restoring = restoring.clone();
        let persist = persist.clone();
        Effect::new(move |_| {
            let list_name = list_name.get();
            let tree_root = tree_root.get();
            let expanded = expanded.get();
            let codec = codec.get();
            let sidebar_width = sidebar_width.get();
            let sidebar_visible = sidebar_visible.get();
            let columns = encode_columns(&columns.get());
            let notifications = track_notifications.get();
            let connected = connected.get();
            if restoring.get() || !connected { return; }
            let mut expanded = expanded.into_iter().collect::<Vec<_>>();
            expanded.sort();
            persist.save(
                serde_json::json!({"listName":list_name,"treeRoot":tree_root,"expanded":expanded,
                    "codec":codec,"sidebar_width":sidebar_width,"sidebar_visible":sidebar_visible,
                    "columns":columns,"track_notifications":notifications}),
            );
        });
    }

    let load_playlists = {
        let get_json = get_json;
        move || {
            leptos::task::spawn_local(async move {
                if let Ok(value) = get_json("/api/playlists".to_owned()).await {
                    let lists = value["playlists"]
                        .as_array()
                        .map(|items| {
                            items
                                .iter()
                                .map(|item| {
                                    (
                                        item["id"].as_i64().unwrap_or_default(),
                                        item["name"].as_str().unwrap_or_default().to_owned(),
                                        item["entryCount"].as_i64().unwrap_or_default(),
                                    )
                                })
                                .collect()
                        })
                        .unwrap_or_default();
                    set_playlists.set(lists);
                }
            });
        }
    };

    let move_track = move |from: usize, to: usize| {
        if !playlist_workspace.snapshot().actions.append { return; }
        if !pane_selected.get_untracked().contains(&from) { select_row(from, false, false); }
        if pane_key.get_untracked() == "queue" {
            backend.send(SessionCommand::Move {
                indices: pane_selected.get_untracked().into_iter().collect(), target: to,
            });
        } else {
            playlist_workspace.send(kog_playback_policy::workspace::Command::Move { target: to });
        }
    };

    let open_create_playlist_dialog = move || {
        set_playlist_dialog_opened.update(|epoch| *epoch += 1);
        set_playlist_dialog.set(Some(PlaylistDialog {
            mode: PlaylistDialogMode::CreateFromPane { selected_only: None },
            title: "Save Playlist",
            value: String::new(),
            id: 0,
            name: String::new(),
        }));
    };

    let open_save_playlist_dialog = move |selected_only| {
        open_create_playlist_dialog();
        set_playlist_dialog.update(|dialog| {
            if let Some(dialog) = dialog {
                dialog.mode = PlaylistDialogMode::CreateFromPane { selected_only: Some(selected_only) };
                dialog.title = if selected_only { "Save Selection As" } else { "Save As" };
            }
        });
    };

    // Stars live on the server (`GET/POST /api/stars`) under the desktop's
    // locator scheme, so a star set from the web shows up on the desktop.
    let load_shared_columns = {
        let get_json = get_json;
        move || {
            leptos::task::spawn_local(async move {
                if let Ok(value) = get_json("/api/columns".to_owned()).await {
                    let layout = value["layout"].as_str().unwrap_or_default().to_owned();
                    if !layout.trim().is_empty() {
                        set_columns.set(decode_columns(&layout));
                    }
                }
            });
        }
    };

    let load_midi = {
        let get_json = get_json;
        move || {
            leptos::task::spawn_local(async move {
                if let Ok(value) = get_json("/api/settings/midi".to_owned()).await {
                    set_midi_engine.set(value["engine"].as_str().unwrap_or_default().to_owned());
                    let mut options = Vec::new();
                    if let Some(list) = value["options"].as_array() {
                        for option in list {
                            let value = option["value"].as_str().unwrap_or_default().to_owned();
                            let label = option["label"].as_str().unwrap_or_default().to_owned();
                            if !value.is_empty() {
                                options.push((value, label));
                            }
                        }
                    }
                    set_midi_options.set(options);
                }
            });
        }
    };

    let load_stars = {
        let get_json = get_json;
        move || {
            leptos::task::spawn_local(async move {
                if let Ok(value) = get_json("/api/stars".to_owned()).await {
                    let locators: HashSet<String> = value["entries"]
                        .as_array()
                        .map(|items| {
                            items
                                .iter()
                                .filter_map(|item| {
                                    Some(star_locator(
                                        item["kind"].as_str()?,
                                        item["path"].as_str()?,
                                        item["entry"].as_str().unwrap_or_default(),
                                        item["fragment"].as_str().unwrap_or_default(),
                                    ))
                                })
                                .collect()
                        })
                        .unwrap_or_default();
                    set_stars.set(Rc::new(locators));
                }
            });
        }
    };

    let toggle_star = {
        let base = base.clone();
        let auth = auth.clone();
        move |entry: Entry, currently: bool| {
            let starred = !currently;
            // Optimistic: the row flips now; the server is the source of truth
            // on the next `/api/stars` refresh.
            set_stars.update(|set| {
                let set = Rc::make_mut(set);
                let key = entry_star_locator(&entry);
                if starred {
                    set.insert(key);
                } else {
                    set.remove(&key);
                }
            });
            let url = format!("{}/api/stars", base());
            let header = auth().header();
            let body = serde_json::json!({
                "kind": entry.kind,
                "path": entry.path,
                "entry": entry.entry,
                // The server's star request takes a string; `null` is a 422.
                "fragment": entry.fragment.clone().unwrap_or_default(),
                "starred": starred,
            });
            leptos::task::spawn_local(async move {
                if let Err(error) = post_json(url, header, body).await {
                    set_message.set(error);
                }
            });
        }
    };

    let connect = {
        let restoring = restoring.clone();
        let load_dir = load_dir.clone();
        let load_playlists = load_playlists.clone();
        let load_stars = load_stars.clone();
        let load_midi = load_midi.clone();
        let load_shared_columns = load_shared_columns.clone();
        move || {
            let restoring = restoring.clone();
            let header = auth().header();
            // A protected endpoint: the version call is open to everyone, so it
            // cannot tell a missing token from a working connection — and with
            // token auth on, that silence left the tree quietly empty.
            let connection_base = base();
            let url = format!("{connection_base}/api/codecs");
            let basic = use_basic.get();
            store("kog.server", &server.get());
            if !basic {
                store("kog.token", &token.get());
            }
            let load_dir = load_dir.clone();
            let load_playlists = load_playlists.clone();
            let load_stars = load_stars.clone();
            let load_midi = load_midi.clone();
            let load_shared_columns = load_shared_columns.clone();
            leptos::task::spawn_local(async move {
                let mut request = Request::get(&url);
                if let Some(header) = header {
                    request = request.header("Authorization", &header);
                }
                match request.send().await {
                    Ok(response) if response.ok() => {
                        if base() != connection_base { return; }
                        if (backend.persistence.dirty() || persist.dirty()) && !backend.persistence.ready(&connection_base) {
                            set_message.set("Wait for the current session to save before changing servers".into());
                            return;
                        }
                        set_connected.set(false);
                        let id = session_model.with_value(|s| s.id().to_owned());
                        let initialized = async {
                            backend.connect().await?;
                            persist.connect(base(), id, auth().header()).await
                        }.await;
                        match initialized {
                            Err(error) => { backend.persistence.warning.set(error.clone()); set_message.set(error); return; }
                            Ok(Some(Some(value))) => {
                                if base() != connection_base { return; }
                                if ui_loaded_scope.get_value().as_ref() != Some(&connection_base) {
                                    set_children.set(HashMap::new());
                                    set_expanded.set(HashSet::new());
                                }
                                let tree = decode_session(&value.to_string());
                                set_list_name.set(tree.list_name);
                                set_tree_root.set(tree.tree_root.clone());
                                restoring.set(true);
                                restored_tree.set(Some((tree.tree_root, tree.expanded)));
                                if let Some(value) = value["codec"].as_str() { set_codec.set(value.into()); }
                                if let Some(value) = value["sidebar_width"].as_f64() { set_sidebar_width.set(value.clamp(180.0, 600.0)); }
                                if let Some(value) = value["sidebar_visible"].as_bool() { set_sidebar_visible.set(value); }
                                if let Some(value) = value["columns"].as_str() { set_columns.set(decode_columns(value)); }
                                if let Some(value) = value["track_notifications"].as_bool() { set_track_notifications.set(value); }
                            }
                            Ok(Some(None)) if ui_loaded_scope.get_value().is_some() => {
                                set_children.set(HashMap::new());
                                set_expanded.set(HashSet::new());
                                set_tree_root.set(String::new());
                                set_list_name.set(String::new());
                                restored_tree.set(Some((String::new(), Vec::new())));
                                restoring.set(true);
                                set_codec.set("aac".into());
                                set_columns.set(default_columns());
                                set_track_notifications.set(false);
                            }
                            _ => {}
                        }
                        if base() != connection_base { return; }
                        ui_loaded_scope.set_value(Some(connection_base));
                        set_connected.set(true);
                        set_settings_open.set(false);
                        set_message.set(String::new());
                        // Informational only: the codecs call above is what
                        // proved the token.
                        if let Ok(reply) = Request::get(&format!("{}/api/version", base()))
                            .send()
                            .await
                        {
                            if let Ok(value) = reply.json::<serde_json::Value>().await {
                                if let Some(version) = value["version"].as_str().map(str::to_owned)
                                {
                                    set_version.set(version);
                                }
                            }
                        }
                        load_dir(String::new(), None);
                        load_playlists();
                        load_stars();
                        load_midi();
                        load_shared_columns();
                    }
                    Ok(response) if response.status() == 401 => {
                        set_connected.set(false);
                        set_message.set("That token or password was rejected".to_owned());
                        set_settings_open.set(true);
                    }
                    Ok(response) => {
                        set_message.set(format!("Server returned {}", response.status()))
                    }
                    Err(error) => set_message.set(format!("Could not reach the server: {error}")),
                }
            });
        }
    };

    // Root changes invalidate outstanding replies through the shared policy.
    {
        let last_scope = Rc::new(RefCell::new(None::<String>));
        Effect::new(move |_| {
            let scope = connected
                .get()
                .then(radio_scope)
                .filter(|root| !root.is_empty());
            if *last_scope.borrow() == scope {
                return;
            }
            *last_scope.borrow_mut() = scope.clone();
            let enabled = radio_on.get_untracked() || (!resumed && restored.radio_on);
            if scope.is_some() {
                change_radio(enabled, false);
            }
        });
    }

    // The page is served by the Kog server itself, so connect on load rather
    // than landing on an empty shell until the user finds the server settings.
    let (auto_connected, set_auto_connected) = signal(false);
    let auto_connect = connect.clone();
    Effect::new(move |_| {
        if auto_connected.get_untracked() {
            return;
        }
        set_auto_connected.set(true);
        auto_connect();
    });

    // Touch gestures: a long press (about half a second) is the right click.
    // Holding a tree, queue, or column-header row opens the same context menu
    // a mouse would. Holding a queued track instead arms it; from there the
    // finger reorders the queue and a lift without movement opens the menu.
    // Moving before the press settles cancels it, so scrolling still works.
    {
        let window = web_sys::window().expect("window for touch gestures");
        let document = window.document().expect("document for touch gestures");
        let move_track = move_track.clone();
        let press: Rc<RefCell<TouchPress>> = Rc::new(RefCell::new(TouchPress::Idle));
        let suppress_click = Rc::new(std::cell::Cell::new(false));

        let fire_menu = {
            let suppress_click = suppress_click.clone();
            move |row: &web_sys::Element, x: i32, y: i32| {
                suppress_click.set(true);
                let init = web_sys::MouseEventInit::new();
                init.set_client_x(x);
                init.set_client_y(y);
                init.set_bubbles(true);
                init.set_cancelable(true);
                if let Ok(event) =
                    web_sys::MouseEvent::new_with_mouse_event_init_dict("contextmenu", &init)
                {
                    let _ = row.dispatch_event(&event);
                }
            }
        };
        let cleanup_drag = {
            let set_dragging_track = set_dragging_track.clone();
            let set_reorder_to = set_reorder_to.clone();
            move || {
                set_dragging_track.set(None);
                set_reorder_to.set(None);
                clear_track_drop_marker();
            }
        };

        // Android fires its own contextmenu partway into a long press; while
        // one of our gestures is in flight that native event is swallowed so
        // the row handlers run once, from the timer. A mouse right click sees
        // an idle state and passes through untouched.
        let swallow = {
            let press = press.clone();
            Closure::<dyn FnMut(web_sys::MouseEvent)>::new(move |event: web_sys::MouseEvent| {
                if !matches!(&*press.borrow(), TouchPress::Idle) {
                    event.prevent_default();
                    event.stop_propagation();
                }
            })
        };
        document
            .add_event_listener_with_callback_and_bool(
                "contextmenu",
                swallow.as_ref().unchecked_ref(),
                true,
            )
            .expect("contextmenu capture listener");
        swallow.forget();

        let on_timer = {
            let press = press.clone();
            let fire_menu = fire_menu.clone();
            Closure::<dyn FnMut()>::new(move || {
                let taken = std::mem::replace(&mut *press.borrow_mut(), TouchPress::Idle);
                if let TouchPress::Pending {
                    row,
                    x,
                    y,
                    is_track,
                    ..
                } = taken
                {
                    if is_track {
                        // A queued track waits for the lift: menu if the
                        // finger stays, drag if it moves.
                        let _ = row.class_list().add_1("drag-armed");
                        *press.borrow_mut() = TouchPress::Armed { row, x, y };
                    } else {
                        fire_menu(&row, x, y);
                    }
                }
            })
        };

        // The timer callback outlives every gesture, so the touchstart
        // handler carries a plain function handle rather than the closure.
        let timer_callback = on_timer
            .as_ref()
            .unchecked_ref::<js_sys::Function>()
            .clone();
        on_timer.forget();

        let on_touch_start = {
            let press = press.clone();
            let window = window.clone();
            let suppress_click = suppress_click.clone();
            Closure::<dyn FnMut(web_sys::TouchEvent)>::new(move |event: web_sys::TouchEvent| {
                suppress_click.set(false);
                // A second finger is a pinch, never a press.
                if event.touches().length() != 1 {
                    cancel_touch_press(&press, &window, &|| cleanup_drag());
                    return;
                }
                let Some(touch) = event.touches().get(0) else {
                    cancel_touch_press(&press, &window, &|| cleanup_drag());
                    return;
                };
                let row = event
                    .target()
                    .and_then(|target| target.dyn_into::<web_sys::Element>().ok())
                    .and_then(|target| {
                        // The explicit add and reorder controls own their
                        // gestures. A hold there must not open the row menu.
                        if target
                            .closest(".tree-add, .drag-grip")
                            .ok()
                            .flatten()
                            .is_some()
                        {
                            return None;
                        }
                        target
                            .closest(".tree-row, .track, .col-head")
                            .ok()
                            .flatten()
                    });
                let Some(row) = row else {
                    cancel_touch_press(&press, &window, &|| cleanup_drag());
                    return;
                };
                cancel_touch_press(&press, &window, &|| cleanup_drag());
                let is_track = row.class_list().contains("track");
                let timer = window
                    .set_timeout_with_callback_and_timeout_and_arguments_0(&timer_callback, 550)
                    .unwrap_or_default();
                *press.borrow_mut() = TouchPress::Pending {
                    row,
                    x: touch.client_x(),
                    y: touch.client_y(),
                    is_track,
                    timer,
                };
            })
        };
        document
            .add_event_listener_with_callback("touchstart", on_touch_start.as_ref().unchecked_ref())
            .expect("touchstart listener");
        on_touch_start.forget();

        // Registered non-passive: once a drag is running the page must not
        // scroll under the finger.
        let on_touch_move = {
            let press = press.clone();
            let window = window.clone();
            let set_dragging_track = set_dragging_track.clone();
            let set_reorder_to = set_reorder_to.clone();
            Closure::<dyn FnMut(web_sys::TouchEvent)>::new(move |event: web_sys::TouchEvent| {
                let Some(touch) = event.touches().get(0) else {
                    return;
                };
                let taken = std::mem::replace(&mut *press.borrow_mut(), TouchPress::Idle);
                match taken {
                    TouchPress::Idle => {}
                    TouchPress::Pending {
                        row,
                        x,
                        y,
                        is_track,
                        timer,
                    } => {
                        let moved =
                            (touch.client_x() - x).abs() + (touch.client_y() - y).abs() > 10;
                        if moved {
                            window.clear_timeout_with_handle(timer);
                        } else {
                            *press.borrow_mut() = TouchPress::Pending {
                                row,
                                x,
                                y,
                                is_track,
                                timer,
                            };
                        }
                    }
                    TouchPress::Armed { row, x, y } => {
                        event.prevent_default();
                        let moved = (touch.client_x() - x).abs() + (touch.client_y() - y).abs() > 6;
                        if moved {
                            let _ = row.class_list().remove_1("drag-armed");
                            if let Some(from) = row
                                .get_attribute("data-index")
                                .and_then(|value| value.parse::<usize>().ok())
                            {
                                set_dragging_track.set(Some(from));
                                set_reorder_to.set(None);
                                *press.borrow_mut() = TouchPress::Dragging { from, to: None };
                            }
                        } else {
                            *press.borrow_mut() = TouchPress::Armed { row, x, y };
                        }
                    }
                    TouchPress::Dragging { from, .. } => {
                        event.prevent_default();
                        let to = track_drop_target(
                            touch.client_x(),
                            touch.client_y(),
                            pane_entries.get_untracked().len(),
                        );
                        set_reorder_to.set(to);
                        *press.borrow_mut() = TouchPress::Dragging { from, to };
                    }
                }
            })
        };
        let move_options = web_sys::AddEventListenerOptions::new();
        move_options.set_passive(false);
        document
            .add_event_listener_with_callback_and_add_event_listener_options(
                "touchmove",
                on_touch_move.as_ref().unchecked_ref(),
                &move_options,
            )
            .expect("touchmove listener");
        on_touch_move.forget();

        let on_touch_end = {
            let press = press.clone();
            let window = window.clone();
            let fire_menu = fire_menu.clone();
            let move_track = move_track.clone();
            let set_dragging_track = set_dragging_track.clone();
            let set_reorder_to = set_reorder_to.clone();
            let suppress_click = suppress_click.clone();
            Closure::<dyn FnMut(web_sys::TouchEvent)>::new(move |_| {
                let taken = std::mem::replace(&mut *press.borrow_mut(), TouchPress::Idle);
                match taken {
                    TouchPress::Idle => {}
                    // A lift before the timer is an ordinary tap; the click
                    // goes through untouched.
                    TouchPress::Pending { timer, .. } => {
                        window.clear_timeout_with_handle(timer);
                    }
                    TouchPress::Armed { row, x, y } => {
                        let _ = row.class_list().remove_1("drag-armed");
                        fire_menu(&row, x, y);
                    }
                    TouchPress::Dragging { from, to } => {
                        set_dragging_track.set(None);
                        set_reorder_to.set(None);
                        clear_track_drop_marker();
                        if let Some(to) = to {
                            move_track(from, to);
                        }
                        // The release must not also click the row underneath.
                        suppress_click.set(true);
                    }
                }
            })
        };
        document
            .add_event_listener_with_callback("touchend", on_touch_end.as_ref().unchecked_ref())
            .expect("touchend listener");
        on_touch_end.forget();

        let on_touch_cancel = {
            let press = press.clone();
            let window = window.clone();
            Closure::<dyn FnMut(web_sys::TouchEvent)>::new(move |_| {
                cancel_touch_press(&press, &window, &|| cleanup_drag());
            })
        };
        document
            .add_event_listener_with_callback(
                "touchcancel",
                on_touch_cancel.as_ref().unchecked_ref(),
            )
            .expect("touchcancel listener");
        on_touch_cancel.forget();

        let on_click = {
            let suppress_click = suppress_click.clone();
            Closure::<dyn FnMut(web_sys::MouseEvent)>::new(move |event: web_sys::MouseEvent| {
                if suppress_click.get() {
                    suppress_click.set(false);
                    event.stop_propagation();
                    event.prevent_default();
                }
            })
        };
        document
            .add_event_listener_with_callback_and_bool(
                "click",
                on_click.as_ref().unchecked_ref(),
                true,
            )
            .expect("click capture listener");
        on_click.forget();
    }

    // Remove the selected rows from the pane. The pane is client-owned, so
    // neither the menu nor the Delete key needs a server request.
    let remove_selected = move || {
        playlist_workspace.send(kog_playback_policy::workspace::Command::Remove);
        set_menu_open.set(false);
        set_song_menu.set(None);
    };
    let select_all_results = move || {
        playlist_workspace.send(kog_playback_policy::workspace::Command::Select {
            indices: pane_visible.get_untracked(),
        });
    };

    // Global keys, from anywhere on the page. Escape dismisses any open menu
    // or dialog; Ctrl/Cmd+A selects the whole pane like the desktop's
    // Select All — except inside a text field, where it keeps selecting text.
    let key_handle =
        window_event_listener(leptos::ev::keydown, move |ev: web_sys::KeyboardEvent| {
            let in_text = ev
                .target()
                .and_then(|target| target.dyn_into::<web_sys::HtmlElement>().ok())
                .map(|element| {
                    let tag = element.tag_name().to_ascii_lowercase();
                    tag == "input"
                        || tag == "textarea"
                        || tag == "select"
                        || element.is_content_editable()
                })
                .unwrap_or(false);
            if !in_text && (ev.ctrl_key() || ev.meta_key()) && !ev.alt_key()
                && matches!(ev.key().to_lowercase().as_str(), "z" | "y") {
                use kog_playback_policy::workspace::Command;
                ev.prevent_default();
                if let Some(dispatch) = workspace_dispatch.get_value() {
                    dispatch.run(if ev.key().eq_ignore_ascii_case("y") || ev.shift_key() {
                        Command::Redo
                    } else { Command::Undo });
                }
                return;
            }
            if !in_text
                && session_model
                    .with_value(|model| model.workspace_model().snapshot().active != "queue")
            {
                use kog_playback_policy::workspace::Command;
                let command = if ev.ctrl_key() || ev.meta_key() {
                    match ev.key().to_lowercase().as_str() {
                        "s" => Some(Command::Save),
                        "w" => Some(Command::Close {
                            key: session_model
                                .with_value(|model| model.workspace_model().snapshot().active),
                        }),
                        "a" => Some(Command::Select { indices: pane_visible.get_untracked() }),
                        "z" if ev.shift_key() => Some(Command::Redo),
                        "z" => Some(Command::Undo),
                        "y" => Some(Command::Redo),
                        _ => None,
                    }
                } else if ev.key() == "Delete" {
                    Some(Command::Remove)
                } else {
                    None
                };
                if let Some(command) = command {
                    ev.prevent_default();
                    if let Some(dispatch) = workspace_dispatch.get_value() {
                        dispatch.run(command);
                    }
                    return;
                }
            }
            if (ev.ctrl_key() || ev.meta_key())
                && !ev.alt_key()
                && ev.key().eq_ignore_ascii_case("a")
            {
                if !in_text && !queue.get_untracked().is_empty() {
                    ev.prevent_default();
                    select_all_results();
                }
                return;
            }
            if ev.key() == "Delete"
                && !in_text
                && !ev.ctrl_key()
                && !ev.meta_key()
                && !ev.alt_key()
                && !settings_open.get_untracked()
                && !picker_open.get_untracked()
                && !add_url_open.get_untracked()
                && track_details.get_untracked().is_none()
                && playlist_dialog.get_untracked().is_none()
                && !pane_selected.get_untracked().is_empty()
            {
                ev.prevent_default();
                remove_selected();
                return;
            }
            if ev.key() == "Escape" {
                set_visualizer_open.set(false);
                set_cover_open.set(false);
                set_menu_open.set(false);
                set_mobile_sort_open.set(false);
                set_track_details.set(None);
                set_add_url_open.set(false);
                set_column_menu.set(None);
                set_tree_menu.set(None);
                set_about_open.set(false);
                set_settings_open.set(false);
                set_picker_open.set(false);
                set_song_menu.set(None);
                set_playlist_menu.set(None);
                set_renaming_playlist.set(None);
            }
        });
    on_cleanup(move || key_handle.remove());

    // ------------------------------------------------------------ live reload
    // Poll the running `/kog_web.js` for its content-hash ETag and reload once
    // the server serves a different build. A transient failure (say, the dev
    // loop restarting) counts as "unchanged", and a reload waits until no track
    // is playing so an update never interrupts playback.
    let poll_assets = {
        move || {
            let baseline = asset_etag.get_untracked();
            let set_asset_etag = set_asset_etag;
            let set_update_ready = set_update_ready;
            let set_update_since = set_update_since;
            leptos::task::spawn_local(async move {
                // A cache-busting query forces a fresh 200 with the ETag; the
                // asset handler ignores the query and serves the same bytes.
                let url = format!("/kog_web.js?_={}", js_sys::Date::now());
                let Ok(response) = Request::get(&url).send().await else {
                    return;
                };
                if !response.ok() {
                    return;
                }
                let Some(etag) = response.headers().get("etag") else {
                    return;
                };
                match baseline {
                    None => set_asset_etag.set(Some(etag)),
                    Some(previous) if previous != etag => {
                        let now = js_sys::Date::now();
                        if update_since.get_untracked().is_none() {
                            set_update_since.set(Some(now));
                        }
                        set_update_ready.set(true);
                        // Never stay stale for long: a deferred reload
                        // force-applies after the grace period, even mid-song.
                        if let Some(since) = update_since.get_untracked()
                            && now - since > 45_000.0
                            && !backend.persistence.dirty() && !persist.dirty()
                            && let Some(window) = web_sys::window()
                        {
                            let _ = window.location().reload();
                        }
                    }
                    _ => {}
                }
            });
        }
    };
    poll_assets();
    let poll_interval = {
        let poll_assets = poll_assets.clone();
        Closure::<dyn FnMut()>::new(move || poll_assets())
    };
    if let Some(window) = web_sys::window() {
        let _ = window.set_interval_with_callback_and_timeout_and_arguments_0(
            poll_interval.as_ref().unchecked_ref(),
            5_000,
        );
    }
    poll_interval.forget();

    Effect::new(move |_| {
        if update_ready.get() && !playing.get() && !backend.persistence.dirty() && !persist.dirty() {
            if let Some(window) = web_sys::window() {
                let _ = window.location().reload();
            }
        }
    });

    let stream_url = move |entry: &Entry| {
        // A selected synth changes the rendered bytes. Give MIDI a new media
        // URL when it changes so the current track reloads, including when
        // the browser has cached the old response.
        let suffix = suffix_of(if entry.entry.is_empty() {
            &entry.path
        } else {
            &entry.entry
        });
        let synth = if matches!(
            suffix.as_str(),
            "kar" | "mid" | "midi" | "rmi" | "mids" | "mds" | "lds" | "xmf" | "mxmf"
        ) {
            let engine = midi_engine.get();
            (!engine.is_empty()).then(|| format!("&midi_engine={}", url_encode(&engine)))
        } else {
            None
        };
        format!(
            "{}/api/stream?kind={}&path={}&entry={}&codec={}{}{}&device={}&token={}",
            base(),
            url_encode(&entry.kind),
            url_encode(&entry.path),
            url_encode(&entry.entry),
            url_encode(&codec.get()),
            entry
                .fragment
                .as_deref()
                .filter(|fragment| !fragment.is_empty())
                .map(|fragment| format!("&fragment={}", url_encode(fragment)))
                .unwrap_or_default(),
            synth.unwrap_or_default(),
            url_encode(&device_id()),
            // The audio element cannot send the Authorization header.
            url_encode(&token.get()),
        )
    };

    let current_entry = move || queue.get().get(current.get()).cloned();
    let audio_src = move || {
        current_entry()
            .map(|entry| stream_url(&entry))
            .unwrap_or_default()
    };

    // The transport's duration: the element's value when it is finite, else the
    // current track's tag duration from the metadata cache. Reactive, so the
    // bar and the total label are right before the media has been measured.
    let duration = {
        let current_entry = current_entry.clone();
        Memo::new(move |_| {
            if let Some(value) = media_duration.get() {
                return value;
            }
            current_entry()
                .as_ref()
                .and_then(|entry| meta_for(&metadata.get(), entry))
                .and_then(|meta| meta.duration)
                .and_then(finite_duration)
                .unwrap_or(0.0)
        })
    };

    // Measure the element once it is real. Progressive streams (ADTS, FLAC)
    // can report `Infinity` for `duration` while their `seekable` range already
    // knows the end, so fall back to that before letting the tag guess win.
    let refresh_media_duration = move |_event: web_sys::Event| {
        if let Some(audio) = audio_ref.get() {
            let measured = finite_duration(audio.duration()).or_else(|| {
                let seekable = audio.seekable();
                if seekable.length() > 0 {
                    seekable.end(0).ok().and_then(finite_duration)
                } else {
                    None
                }
            });
            set_media_duration.set(measured);
        }
    };

    // The element's src is written only when it actually changes. Assigning
    // even the same URL restarts the media load algorithm, and the URL is
    // derived from the whole queue — so an append (dragging a song in),
    // a reorder, or a removal would reload the element mid-song and restart
    // it from zero. Only a change of the row at `current`, or of the token
    // baked into the stream URL, may reload.
    // Loading another track can pause the element before its replacement is
    // ready. That pause is not a remote transport command.
    let source_changing = RwSignal::new(false);
    {
        let source_changing = source_changing.clone();
        Effect::new(move |_| {
            let desired = audio_src();
            let Some(output) = output_token.get() else {
                return;
            };
            if let Some(audio) = audio_ref.get() {
                if audio.get_attribute("data-output").as_deref() != Some(&output.serial.to_string())
                {
                    return;
                }
                if let Some(applied) = audio.get_attribute("data-loaded-source") {
                    if applied == desired {
                        return;
                    }
                    backend.send(SessionCommand::Output {
                        token: output.clone(),
                        event: OutputEvent::Progress {
                            seconds: audio.current_time(),
                            duration: duration.get_untracked(),
                        },
                    });
                    backend.send(SessionCommand::ReloadOutput);
                    return;
                }
                source_changing.set(true);
                audio.set_src(&desired);
                let _ = audio.set_attribute("data-loaded-source", &desired);
            }
        });
    }

    // A new output request creates a new element. Start it once the source is
    // ready; explicit Resume acts directly on the existing element.
    let resume_when_ready = {
        let refresh_media_duration = refresh_media_duration.clone();
        let source_changing = source_changing.clone();
        move |event: web_sys::Event| {
            refresh_media_duration(event);
            if let Some(audio) = audio_ref.get() {
                let seconds = output_start.get_value();
                if seconds > 0.0 {
                    audio.set_current_time(seconds);
                    output_start.set_value(0.0);
                }
            }
            if playing.get_untracked() {
                if let Some(audio) = audio_ref.get() {
                    play_audio(&audio);
                }
            }
            source_changing.set(false);
        }
    };

    Effect::new(move |_| {
        if let Some(audio) = audio_ref.get() {
            audio.set_volume(volume.get());
        }
    });

    backend
        .output
        .set_value(Some(Callback::new(move |effect| match effect {
            SessionEffect::Play { token, seconds, .. } => {
                output_start.set_value(seconds);
                set_media_duration.set(None);
                output_token.set(Some(token));
            }
            SessionEffect::Stop => {
                if let Some(audio) = audio_ref.get() {
                    let _ = audio.pause();
                    audio.remove_attribute("src").ok();
                    audio.load();
                }
                output_token.set(None);
            }
            SessionEffect::Pause => {
                if let Some(audio) = audio_ref.get() {
                    let _ = audio.pause();
                }
            }
            SessionEffect::Resume => {
                if let Some(audio) = audio_ref.get() {
                    play_audio(&audio);
                }
            }
            SessionEffect::Seek { seconds } => {
                if let Some(audio) = audio_ref.get() {
                    audio.set_current_time(seconds);
                }
            }
            SessionEffect::Volume { value } => {
                if let Some(audio) = audio_ref.get() {
                    audio.set_volume(value);
                }
            }
            _ => {}
        })));

    // A stream request that fails around a track change — the server
    // restarting, a flaky network — leaves the element errored or paused
    // while the transport still says playing: the pane highlights the next
    // row and nothing sounds until the visitor pokes it again. Watch the
    // element so playback self-heals: re-issue play after a spurious pause,
    // and force a fresh load when the source itself failed.
    {
        let window = web_sys::window().expect("window for the playback watchdog");
        let playing = playing.clone();
        let audio_ref = audio_ref.clone();
        let current = current.clone();
        let queue = queue.clone();
        let source_changing = source_changing.clone();
        let watchdog = Closure::<dyn FnMut()>::new(move || {
            let Some(audio) = audio_ref.get() else {
                return;
            };
            if !playing.get_untracked() || current.get_untracked() >= queue.get_untracked().len() {
                return;
            }
            if audio.error().is_some() || source_changing.get_untracked() {
                return;
            }
            if audio.paused() && !audio.ended() {
                play_audio(&audio);
            }
        });
        window
            .set_interval_with_callback_and_timeout_and_arguments_0(
                watchdog.as_ref().unchecked_ref(),
                2000,
            )
            .expect("playback watchdog interval");
        watchdog.forget();
    }

    // The playing row's level meter, tapping the audio element's captured
    // stream through Web Audio (the desktop meters its local playback the
    // same way). The element itself is never routed — captureStream taps it
    // in parallel — so playback always sounds directly and the meter can
    // never mute it, whatever a browser does to Web Audio. The context is
    // warmed in the visitor's first pointer gesture (the one moment it
    // legally starts Running); the tap is built once playback is actually
    // running, because Chromium does not flow tracks created before media
    // starts, and rebuilt if it goes quiet mid-playback.
    {
        let audio_ref = audio_ref.clone();
        let playing = playing.clone();
        let graph: Rc<RefCell<Option<(web_sys::AudioContext, web_sys::AnalyserNode)>>> =
            Rc::new(RefCell::new(None));
        // Warmed context, ready before the tap exists.
        let context_slot: Rc<RefCell<Option<web_sys::AudioContext>>> = Rc::new(RefCell::new(None));
        // Band analysis state, mirroring the desktop's per-stream state.
        let analysis = Rc::new(RefCell::new(BandState::default()));
        // Consecutive silent reads while audibly playing: a stale tap.
        let dry_reads = Rc::new(std::cell::Cell::new(0u32));

        // Warm the AudioContext on the first pointer press, when it legally
        // starts Running. Never touches the element.
        {
            let context_slot = context_slot.clone();
            let warm = Closure::<dyn FnMut()>::new(move || {
                let mut slot = context_slot.borrow_mut();
                if slot.is_some() {
                    return;
                }
                if let Ok(context) = web_sys::AudioContext::new() {
                    // resume() is a promise: the state flips a tick later,
                    // so store the context now and let the poller re-check
                    // (and keep re-asking) once it has settled.
                    let _ = context.resume();
                    *slot = Some(context);
                }
            });
            let document = web_sys::window()
                .and_then(|window| window.document())
                .expect("document for the meter warmer");
            let _ = document
                .add_event_listener_with_callback("pointerdown", warm.as_ref().unchecked_ref());
            warm.forget();
        }

        // Build (or rebuild) the capture tap on the warmed context. Every
        // failure path leaves the element untouched.
        let build_tap = {
            let audio_ref = audio_ref.clone();
            let graph = graph.clone();
            let context_slot = context_slot.clone();
            Rc::new(move || {
                // Get or create the context right here: the first playing
                // tick follows the click that started playback, which is
                // itself the gesture a context legally needs — no reliance
                // on a separate warmer having run. A not-yet-running
                // context just retries on the next tick.
                // Bind the snapshot first: a match scrutinee's temporary
                // lives through the whole match, so borrowing the slot
                // mutably inside an arm would double-borrow and panic.
                let existing = context_slot.borrow().clone();
                let context = match existing {
                    Some(context) => context,
                    None => match web_sys::AudioContext::new() {
                        Ok(context) => {
                            *context_slot.borrow_mut() = Some(context.clone());
                            context
                        }
                        Err(_) => return,
                    },
                };
                let _ = context.resume();
                if context.state() != web_sys::AudioContextState::Running {
                    return;
                }
                if audio_ref.get().is_none() {
                    return;
                }
                let Ok(analyser) = context.create_analyser() else {
                    return;
                };
                analyser.set_fft_size(2048);
                // Tap the element's output without diverting it. web-sys
                // has no captureStream binding, so call it on the element
                // itself (the page's only audio element, class-marked).
                let Ok(stream_value) =
                    js_sys::eval("document.querySelector('audio.audio').captureStream()")
                else {
                    return;
                };
                let Ok(stream) = stream_value.dyn_into::<web_sys::MediaStream>() else {
                    return;
                };
                let Ok(source) = context.create_media_stream_source(&stream) else {
                    return;
                };
                // A zero-gain tail keeps the analyser pulled by the graph
                // without adding any sound of its own.
                let Ok(sink) = context.create_gain() else {
                    return;
                };
                sink.gain().set_value(0.0);
                if source.connect_with_audio_node(&analyser).is_err()
                    || analyser.connect_with_audio_node(&sink).is_err()
                    || sink
                        .connect_with_audio_node(&context.destination())
                        .is_err()
                {
                    return;
                }
                *graph.borrow_mut() = Some((context, analyser));
            })
        };

        let levels_poll = Closure::<dyn FnMut()>::new(move || {
            if !playing.get_untracked() {
                set_audio_levels.set([0.0; 5]);
                if visualizer_open.get_untracked() {
                    set_visualizer_wave.set(Vec::new());
                    set_visualizer_spectrum.set(Vec::new());
                }
                return;
            }
            let Some(audio) = audio_ref.get() else {
                return;
            };
            if audio.paused() || audio.muted() || audio.volume() <= 0.0 {
                set_audio_levels.set([0.0; 5]);
                if visualizer_open.get_untracked() {
                    set_visualizer_wave.set(Vec::new());
                    set_visualizer_spectrum.set(Vec::new());
                }
                return;
            }
            if graph.borrow().is_none() {
                build_tap();
                if graph.borrow().is_none() {
                    return;
                }
            }
            // Cloned handles: no borrow is held across the stale-tap drop.
            let Some((context, analyser)) = graph.borrow().clone() else {
                return;
            };
            let _ = context.resume();
            let mut samples = vec![0.0_f32; analyser.fft_size() as usize];
            analyser.get_float_time_domain_data(&mut samples);
            if visualizer_open.get_untracked() {
                set_visualizer_wave.set(samples.iter().step_by(4).copied().collect());
                let mut frequencies = vec![0_u8; analyser.frequency_bin_count() as usize];
                analyser.get_byte_frequency_data(&mut frequencies);
                set_visualizer_spectrum.set(visualizer_spectrum_bins(
                    &frequencies,
                    context.sample_rate(),
                    analyser.fft_size() as usize,
                ));
            }
            let live = samples.iter().any(|sample| sample.abs() > 0.00001);
            if !live {
                // Audibly playing but silent reads: the tap went stale
                // (a track change can drop its tracks). Drop it; the next
                // tick rebuilds on the still-warm context.
                let dry = dry_reads.get() + 1;
                dry_reads.set(dry);
                if dry > 14 {
                    *graph.borrow_mut() = None;
                    dry_reads.set(0);
                }
                return;
            }
            dry_reads.set(0);
            let mut state = analysis.borrow_mut();
            if state.rate == 0 {
                state.reset(context.sample_rate() as u32);
            }
            for sample in samples {
                state.observe(sample);
            }
            set_audio_levels.set(state.smoothed);
        });
        web_sys::window()
            .expect("window for the level meter")
            .set_interval_with_callback_and_timeout_and_arguments_0(
                levels_poll.as_ref().unchecked_ref(),
                70,
            )
            .expect("level meter interval");
        levels_poll.forget();
    }

    // One batched tag lookup for the queue and displayed playlist. The cache
    // is read untracked so a successful fetch does not immediately schedule the
    // same request again; a re-render of the rows is the only effect.
    Effect::new(move |_| {
        if !connected.get() {
            return;
        }
        let mut entries = queue.get();
        entries.extend(pane_entries.get());
        let mut seen = HashSet::new();
        entries.retain(|entry| seen.insert(meta_key(entry)));
        if entries.is_empty() {
            return;
        }
        let known = metadata.get_untracked();
        let missing: Vec<Entry> = entries
            .into_iter()
            .filter(|entry| !known.contains_key(&meta_key(entry)))
            .collect();
        if missing.is_empty() {
            return;
        }
        let url = format!("{}/api/metadata", base());
        let header = auth().header();
        for chunk in missing.chunks(1000) {
            let body = serde_json::Value::Array(
                chunk
                    .iter()
                    .map(|entry| {
                        serde_json::json!({
                            "kind": entry.kind,
                            "path": entry.path,
                            "entry": entry.entry,
                            "fragment": entry.fragment,
                        })
                    })
                    .collect(),
            );
            let keys: Vec<String> = chunk.iter().map(meta_key).collect();
            let url = url.clone();
            let header = header.clone();
            leptos::task::spawn_local(async move {
                let rows = post_json(url, header, body)
                    .await
                    .ok()
                    .and_then(|value| serde_json::from_value::<Vec<Option<MetaRow>>>(value).ok());
                let Some(rows) = rows else {
                    set_metadata_failed.update(|failed| {
                        Rc::make_mut(failed).extend(keys);
                    });
                    return;
                };
                let updates: Vec<(String, Option<MetaRow>)> = keys
                    .into_iter()
                    .enumerate()
                    .map(|(index, key)| (key, rows.get(index).cloned().flatten()))
                    .collect();
                set_metadata.update(|map| {
                    let map = Rc::make_mut(map);
                    for (key, row) in &updates {
                        map.insert(key.clone(), row.clone());
                    }
                });
                set_metadata_failed.update(|failed| {
                    let failed = Rc::make_mut(failed);
                    for (key, _) in updates {
                        failed.remove(&key);
                    }
                });
            });
        }
    });

    let play_row = move |index: usize| {
        if pane_key.get_untracked() == "queue" {
            backend.send(SessionCommand::Play { index });
        } else {
            select_row(index, false, false);
            playlist_workspace.send(kog_playback_policy::workspace::Command::Queue { action: QueueAction::PlayNow });
        }
    };

    // Missing art is searched in the background. A single short-polling
    // fetch supplies the thumbnail, enlarged cover and Media Session image.
    let art_request = Memo::new(move |_| {
        if !connected.get() {
            return None;
        }
        queue.with(|q| q.get(current.get()).cloned())
            .filter(|entry| !entry.is_dir())
            .map(|entry| {
                format!(
                    "{}/api/art?kind={}&path={}&token={}&background=true",
                    base(),
                    url_encode(&entry.kind),
                    url_encode(&entry.path),
                    url_encode(&token.get()),
                )
            })
    });
    let art_source = artwork::source(art_request);
    let art_src = move || art_source.get();

    // Now-playing notifications: when the playing track changes while the
    // preference is on, post the web twin of the desktop's popup — title,
    // artist and album, no controls. Skips page load (the restored track is
    // not news) and any change while paused or stopped.
    let notify_last_index = Rc::new(std::cell::Cell::new(None::<usize>));
    Effect::new(move |_| {
        let index = current.get();
        let playing = playing.get();
        if !track_notifications.get() {
            notify_last_index.set(Some(index));
            return;
        }
        if notify_last_index.get() == Some(index) {
            return;
        }
        let Some(entry) = queue.with_untracked(|q| q.get(index).cloned()) else {
            return;
        };
        notify_last_index.set(Some(index));
        if !playing {
            return;
        }
        let cache = metadata.get_untracked();
        let meta = meta_for(&cache, &entry);
        let title = meta
            .as_ref()
            .and_then(|meta| meta.title.clone())
            .unwrap_or_else(|| entry.name.clone());
        let detail = [
            meta.as_ref().and_then(|meta| meta.artist.clone()),
            meta.as_ref().and_then(|meta| meta.album.clone()),
        ]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join("  ·  ");
        let body = if detail.is_empty() {
            "Local music · Kog".to_owned()
        } else {
            detail
        };
        show_now_playing(&title, &body);
    });

    // Follow the playing track: Next, Previous, a track ending, or a radio
    // shift all land the current row in view. `nearest` scrolls only as far
    // as it must and nothing at all when the row is already on screen, so
    // picking a song with the mouse or browsing the pane while a song plays
    // is never yanked around. The row renders a tick after the state change
    // (a radio shift appends it), so poll briefly, like the tree's reveal.
    Effect::new(move |_| {
        let expected = current.get();
        let expected_pane = pane_key.get_untracked();
        if queue.get_untracked().is_empty() {
            return;
        }
        leptos::task::spawn_local(async move {
            for _ in 0..10 {
                sleep_ms(50).await;
                if current.get_untracked() != expected || pane_key.get_untracked() != expected_pane {
                    break;
                }
                let scrolled = js_sys::eval(
                    "(() => { const row = document.querySelector('#playlist-rows .track.current');\
                      if (!row) return 'no';\
                      row.scrollIntoView({ block: 'nearest', inline: 'nearest' });\
                      return 'yes'; })()",
                );
                if let Ok(value) = scrolled
                    && value.as_string() == Some("yes".to_owned())
                {
                    break;
                }
            }
        });
    });

    let stop_playback = move || backend.send(SessionCommand::Stop);
    let toggle_play = move || backend.send(SessionCommand::Toggle);
    let navigate = move |event| backend.send(SessionCommand::Navigate { event });
    let step = move |delta: i64| {
        navigate(if delta < 0 {
            NavigationEvent::Previous
        } else {
            NavigationEvent::Next
        })
    };

    // Publish the same resolved tags as the visible transport. A track change
    // clears the previous song immediately, then publishes the new song only
    // after its metadata request has completed (no transient filename title).
    Effect::new(move |_| {
        let Some(session) = media_session() else {
            return;
        };
        let entry = queue.get().get(current.get()).cloned();
        let stopped = stopped.get();
        let cache = metadata.get();
        let failed = metadata_failed.get();
        let Some(entry) = entry.filter(|_| !stopped) else {
            let _ =
                js_sys::Reflect::set(&session, &"metadata".into(), &wasm_bindgen::JsValue::NULL);
            return;
        };
        let Some(title) = display_title(&cache, &failed, &entry) else {
            let _ =
                js_sys::Reflect::set(&session, &"metadata".into(), &wasm_bindgen::JsValue::NULL);
            return;
        };
        let meta = meta_for(&cache, &entry);
        let artist = meta
            .as_ref()
            .and_then(|meta| meta.artist.as_ref().or(meta.album_artist.as_ref()))
            .map(String::as_str)
            .unwrap_or_default();
        let album = media_session_album(meta.as_ref(), &entry);
        let artwork = art_src();
        media_session_metadata(&session, &title, artist, album, &artwork);
    });

    Effect::new(move |_| {
        let Some(session) = media_session() else {
            return;
        };
        let state = if stopped.get() || queue.get().is_empty() {
            "none"
        } else if playing.get() {
            "playing"
        } else {
            "paused"
        };
        media_session_property(&session, "playbackState", state);
    });

    Effect::new(move |_| {
        if stopped.get() {
            return;
        }
        if let Some(session) = media_session() {
            media_session_position(&session, duration.get(), position.get());
        }
    });

    if let Some(session) = media_session() {
        media_session_action(&session, "play", move |_| {
            backend.send(SessionCommand::Resume)
        });
        media_session_action(&session, "pause", move |_| {
            backend.send(SessionCommand::Pause)
        });
        media_session_action(&session, "stop", move |_| stop_playback());
        media_session_action(&session, "previoustrack", move |_| step(-1));
        media_session_action(&session, "nexttrack", move |_| step(1));
        media_session_action(&session, "seekto", move |details| {
            let Some(requested) = js_sys::Reflect::get(&details, &"seekTime".into())
                .ok()
                .and_then(|value| value.as_f64())
            else {
                return;
            };
            if let Some(audio) = audio_ref.get() {
                let end = finite_duration(audio.duration())
                    .or_else(|| {
                        let seekable = audio.seekable();
                        (seekable.length() > 0)
                            .then(|| seekable.end(0).ok().and_then(finite_duration))
                            .flatten()
                    })
                    .unwrap_or_else(|| duration.get_untracked());
                let target = if end.is_finite() && end > 0.0 {
                    requested.clamp(0.0, end)
                } else {
                    requested.max(0.0)
                };
                backend.send(SessionCommand::Seek { seconds: target });
            }
        });
    }

    let toggle_mute = move |_| {
        if volume.get() > 0.0 {
            set_volume_before_mute.set(volume.get());
            backend.send(SessionCommand::Volume { value: 0.0 });
        } else {
            let restored = volume_before_mute.get();
            backend.send(SessionCommand::Volume {
                value: if restored > 0.0 { restored } else { 0.75 },
            });
        }
    };

    let tree_rows = move || {
        let mut out = Vec::new();
        // A query swaps the pane to the results tree: matches under their
        // real folders, ancestors open, exactly like the desktop swapping in
        // its search model. No query: the loaded tree as usual.
        if tree_search.get().trim().is_empty() {
            let children = children.get();
            let expanded = expanded.get();
            let root = tree_root.get();
            flatten(&children, &expanded, &root, 0, &mut out);
        } else {
            let children = search_children.get();
            let expanded = search_expanded.get();
            flatten(&children, &expanded, "", 0, &mut out);
        }
        out
    };

    // Expand/collapse dispatch for a tree row: inside a search the results
    // tree owns the folders, outside it the loaded tree does. Read at event
    // time — a row rendered by one mode can survive into the other (keyed
    // reuse), so the branch cannot be baked in at creation.
    let tree_toggle = move |path: String| {
        if tree_search.get_untracked().trim().is_empty() {
            toggle_dir(path);
        } else {
            toggle_search_dir(path);
        }
    };

    let current_root = move || {
        let root = tree_root.get();
        if root.is_empty() {
            library_root.get()
        } else {
            root
        }
    };

    // Every tab shares the queue's rows, columns, metadata, and input handling.
    // Only the source of entries and the destination of edits changes.
    let activate_row = move |index| {
        playlist_workspace.send(kog_playback_policy::workspace::Command::Activate { index });
    };
    let view_rows = move || {
        let entries = pane_entries.read();
        let cache = metadata.read();
        let failed = metadata_failed.read();
        pane_visible.read().iter().copied().filter_map(|index| {
            entries.get(index).filter(|entry| metadata_ready(&cache, &failed, entry))
                .map(|entry| (index, entry.clone()))
        }).collect::<Vec<_>>()
    };

    // The pane's status line, shared by the header and the transport: how many
    // tracks the pane shows (all of the queue, or the filter's matches) and
    // their total probed duration, in the desktop's footer spirit.
    let status_line = move || -> String {
        let cache = metadata.read();
        let failed = metadata_failed.read();
        let total = pane_entries
            .read()
            .iter()
            .filter(|entry| metadata_ready(&cache, &failed, entry))
            .count();
        let rows = view_rows();
        let shown = rows.len();
        let mut text = if shown == total {
            if total == 1 {
                "1 track".to_owned()
            } else {
                format!("{total} tracks")
            }
        } else {
            format!("{shown} of {total}")
        };
        let seconds: f64 = rows
            .iter()
            .filter_map(|(_, entry)| meta_for(&cache, entry))
            .filter_map(|meta| meta.duration)
            .sum();
        if seconds > 0.0 {
            text.push_str(" · ");
            text.push_str(&clock(seconds));
        }
        text
    };

    let queue_files = move |directory: &str| -> Vec<Entry> {
        children
            .get()
            .get(directory)
            .map(|items| {
                items
                    .iter()
                    .filter(|item| !item.is_dir())
                    .cloned()
                    .collect()
            })
            .unwrap_or_default()
    };

    let pane_sort = Memo::new(move |_| {
        let key = pane_key.get();
        if key == "queue" { (sort_key.get(), sort_asc.get()) }
        else { draft_sorts.with(|sorts| sorts.get(&key).copied().unwrap_or((SortKey::Index, true))) }
    });
    let toggle_sort = move |key: SortKey| {
        if !playlist_workspace.snapshot().actions.append { return; }
        let column = ColumnId::ALL.into_iter().find(|c| c.sort_key() == key)
            .unwrap_or(ColumnId::Index).key().to_owned();
        let previous = pane_sort.get_untracked();
        let descending = previous.0 == key && previous.1;
        let tab = pane_key.get_untracked();
        if tab == "queue" {
            backend.send(SessionCommand::Sort { column, descending, physical: true });
        } else {
            let cache = metadata.get_untracked();
            let starred = stars.get_untracked();
            let rows = pane_entries.get_untracked().iter().enumerate()
                .map(|(index, entry)| entry_sort_row(index, entry, &cache, &starred)).collect();
            playlist_workspace.send(kog_playback_policy::workspace::Command::Sort { rows, column, descending });
            draft_sorts.update(|sorts| { sorts.insert(tab, (key, !descending)); });
        }
    };
    let sort_arrow = move |key: SortKey| -> &'static str {
        let (sorted, ascending) = pane_sort.get();
        if sorted == key { if ascending { " ▲" } else { " ▼" } } else { "" }
    };

    let visible_columns = move || -> Vec<Column> {
        columns
            .get()
            .into_iter()
            .filter(|column| column.visible)
            .collect()
    };

    // Header and rows read the same custom property, so they never drift.
    let grid_template = move || {
        visible_columns()
            .into_iter()
            .map(|column| {
                if column.id.flexible() {
                    format!("minmax({:.0}px, {:.0}fr)", column.width, column.width)
                } else {
                    format!("{:.0}px", column.width)
                }
            })
            .collect::<Vec<_>>()
            .join(" ")
    };

    // The pane's scrollable content is never narrower than the columns it
    // holds: when the column set is wider than the pane, the shared scroll
    // container overflows and the header and rows scroll together.
    let table_width = move || -> f64 {
        visible_columns()
            .into_iter()
            .map(|column| column.width)
            .sum()
    };

    let persist_columns = move |columns: &[Column]| {
        let layout = encode_columns(columns);
        // Share the layout so every client - and the desktop - agrees.
        let url = format!("{}/api/columns", base());
        let header = auth().header();
        leptos::task::spawn_local(async move {
            let _ = post_json(url, header, serde_json::json!({ "layout": layout })).await;
        });
    };

    backend
        .saved
        .set_value(Some(Callback::new(move |()| load_playlists())));
    workspace_dispatch.set_value(Some(Callback::new(move |command| {
        playlist_workspace.send(command)
    })));
    let append_playlist = move |id: i64| {
        let destination = playlist_workspace.snapshot();
        if !destination.actions.append {
            return;
        }
        let key = destination.active.clone();
        let scope = base();
        if key == "queue" {
            backend.send(SessionCommand::AppendToTab {
                key,
                scope,
                entries: vec![serde_json::json!({"playlist_id":id})],
            });
        } else {
            let header = auth().header();
            leptos::task::spawn_local(async move {
                match session::request("GET", format!("{scope}/api/playlists/{id}"), header, None)
                    .await
                {
                    Ok(value) if base() == scope => {
                        let entries = value["entries"].as_array().cloned().unwrap_or_default();
                        if !entries.is_empty() {
                            backend.send(SessionCommand::AppendToTab {
                                key,
                                scope,
                                entries,
                            });
                        }
                    }
                    Ok(_) => set_message.set("Connect to the original playlist server".into()),
                    Err(error) => set_message.set(error),
                }
            });
        }
    };
    let playlist_append_title = move || {
        let state = playlist_workspace.snapshot();
        let name = state
            .tabs
            .iter()
            .find(|tab| tab.key == state.active)
            .map(|tab| tab.name.as_str())
            .unwrap_or("Play Queue");
        if state.actions.append {
            format!("Append to “{name}”")
        } else {
            format!("“{name}” cannot be edited")
        }
    };

    let commit_rename = {
        let get_json = get_json;
        let post_json = post_json;
        let load_playlists = load_playlists.clone();
        move || {
            if let Some(id) = renaming_playlist.get_untracked() {
                let name = rename_text.get_untracked().trim().to_owned();
                set_renaming_playlist.set(None);
                if name.is_empty() {
                    return;
                }
                let url = format!("{}/api/playlists/{id}/rename", base());
                let header = auth().header();
                leptos::task::spawn_local(async move {
                    match post_json(url, header, serde_json::json!({ "name": name })).await {
                        Ok(_) => playlist_workspace.send(
                            kog_playback_policy::workspace::Command::Renamed {
                                key: format!("{}:{id}", base()),
                                name: name.clone(),
                            },
                        ),
                        Err(error) => set_message.set(error),
                    }
                    load_playlists();
                });
            }
        }
    };

    let delete_playlist = {
        let get_json = get_json;
        let load_playlists = load_playlists.clone();
        move |id: i64| {
            let url = format!("{}/api/playlists/{id}", base());
            let header = auth().header();
            leptos::task::spawn_local(async move {
                let mut request = Request::delete(&url);
                if let Some(header) = header {
                    request = request.header("Authorization", &header);
                }
                match request.send().await {
                    Ok(response) if !response.ok() => {
                        set_message.set(format!("Request failed ({})", response.status()))
                    }
                    Ok(_) => {
                        playlist_workspace.send(kog_playback_policy::workspace::Command::Deleted {
                            key: format!("{}:{id}", base()),
                        });
                        load_playlists();
                    }
                    Err(error) => set_message.set(error.to_string()),
                }
            });
        }
    };

    // The desktop sidebar's Play: append the playlist's tracks and start
    // playing the first of them.
    let play_playlist = move |id: i64| {
        backend.send(SessionCommand::Expand {
            scope: base(),
            entries: vec![serde_json::json!({"playlist_id":id})],
            action: QueueAction::PlayNow,
        })
    };

    let replace_pane_with_playlist = move |id: i64, name: String| playlist_workspace.open(id, name);

    // Remove Missing Files, with the desktop's status strings.
    let prune_playlist_missing = {
        let post_json = post_json;
        move |id: i64| {
            let url = format!("{}/api/playlists/{id}/prune-missing", base());
            let header = auth().header();
            leptos::task::spawn_local(async move {
                match post_json(url, header, serde_json::json!({})).await {
                    Ok(value) => {
                        let removed = value["removed"].as_u64().unwrap_or(0);
                        set_status_note.set(if removed == 0 {
                            "No missing files in the playlist".to_owned()
                        } else {
                            format!(
                                "Removed {removed} missing file{} from the playlist",
                                if removed == 1 { "" } else { "s" }
                            )
                        });
                    }
                    Err(error) => set_message.set(error),
                }
            });
        }
    };

    // Duplicate, through the server's create-and-copy endpoint.
    let duplicate_playlist = {
        let post_json = post_json;
        let load_playlists = load_playlists.clone();
        move |id: i64, name: String| {
            let url = format!("{}/api/playlists/{id}/duplicate", base());
            let header = auth().header();
            leptos::task::spawn_local(async move {
                match post_json(url, header, serde_json::json!({ "name": name })).await {
                    Ok(_) => {
                        set_status_note.set(format!("Saved {name} as a new playlist"));
                    }
                    Err(error) => set_message.set(error),
                }
                load_playlists();
            });
        }
    };

    // Export as m3u: build the playlist file in the page and hand it to the
    // browser as a download, where the desktop opens a save dialog.
    let export_playlist_m3u = {
        let get_json = get_json;
        move |id: i64, name: String| {
            leptos::task::spawn_local(async move {
                match get_json(format!("/api/playlists/{id}")).await {
                    Ok(value) => {
                        let entries: Vec<Entry> = value["entries"]
                            .as_array()
                            .map(|items| items.iter().map(entry_from_json).collect())
                            .unwrap_or_default();
                        if entries.is_empty() {
                            set_status_note.set("Playlist is empty".to_owned());
                            return;
                        }
                        let cache = metadata.get_untracked();
                        let mut m3u = String::from("#EXTM3U\n");
                        for entry in &entries {
                            let meta = meta_for(&cache, entry);
                            let duration =
                                meta.as_ref().and_then(|meta| meta.duration).unwrap_or(0.0) as i64;
                            let artist = meta
                                .as_ref()
                                .and_then(|meta| meta.artist.clone())
                                .unwrap_or_default();
                            let title = meta
                                .as_ref()
                                .and_then(|meta| meta.title.clone())
                                .unwrap_or_else(|| entry.name.clone());
                            m3u.push_str(&format!(
                                "#EXTINF:{duration},{artist} - {title}\n{}\n",
                                entry_path(entry)
                            ));
                        }
                        let bytes = js_sys::Uint8Array::from(m3u.as_bytes());
                        let parts = js_sys::Array::new();
                        parts.push(&bytes);
                        match web_sys::Blob::new_with_u8_slice_sequence(&parts.into()) {
                            Ok(blob) => {
                                let staged = web_sys::Url::create_object_url_with_blob(&blob);
                                match staged {
                                    Ok(url) => {
                                        trigger_browser_download(&url, &format!("{name}.m3u"));
                                        set_status_note.set(format!(
                                            "Exported {} tracks to {}.m3u",
                                            entries.len(),
                                            name
                                        ));
                                    }
                                    Err(_) => {
                                        set_message.set("could not stage the m3u file".to_owned())
                                    }
                                }
                            }
                            Err(_) => set_message.set("could not build the m3u file".to_owned()),
                        }
                    }
                    Err(error) => set_message.set(error),
                }
            });
        }
    };

    // The + button's flow, the desktop's Save Playlist dialog: name a new
    // playlist and save the pane into it — or just the selected rows, when
    // the pane has a selection.
    let create_playlist_from_pane = {
        let post_json = post_json;
        let load_playlists = load_playlists.clone();
        move |name: String, selected_only: Option<bool>| {
            let url = format!("{}/api/playlists", base());
            let header = auth().header();
            let selected: HashSet<usize> = selected.get_untracked();
            let entries: Vec<Entry> = queue
                .get_untracked()
                .into_iter()
                .enumerate()
                .filter(|(index, _)| !selected_only.unwrap_or(!selected.is_empty()) || selected.contains(index))
                .map(|(_, entry)| entry)
                .collect();
            leptos::task::spawn_local(async move {
                let payload = serde_json::json!({ "name": name, "entries": entries.iter().map(|entry| {
                    serde_json::json!({ "kind": entry.kind, "path": entry.path, "entry": entry.entry,
                        "fragment": entry.fragment.clone().unwrap_or_default() })
                }).collect::<Vec<_>>() });
                match post_json(url, header, payload).await {
                    Ok(_) => {
                        set_status_note.set(format!("Saved {name} as a new playlist"));
                        load_playlists();
                    }
                    Err(error) => set_message.set(error),
                }
            });
        }
    };

    // Saving onto an existing playlist: replace its tracks with the pane
    // (or the selection) through the overwrite endpoint.
    let overwrite_playlist_with_pane = {
        let load_playlists = load_playlists.clone();
        move |id: i64, name: String, selected_only: Option<bool>| {
            let url = format!("{}/api/playlists/{id}/entries", base());
            let header = auth().header();
            let selected: HashSet<usize> = selected.get_untracked();
            let entries: Vec<Entry> = queue
                .get_untracked()
                .into_iter()
                .enumerate()
                .filter(|(index, _)| !selected_only.unwrap_or(!selected.is_empty()) || selected.contains(index))
                .map(|(_, entry)| entry)
                .collect();
            let count = entries.len();
            let payload = serde_json::json!({
                "entries": entries
                    .iter()
                    .map(|entry| {
                        serde_json::json!({
                            "kind": entry.kind,
                            "path": entry.path,
                            "entry": entry.entry,
                            "fragment": entry.fragment.clone().unwrap_or_default(),
                        })
                    })
                    .collect::<Vec<_>>(),
            });
            leptos::task::spawn_local(async move {
                let mut request = Request::put(&url);
                if let Some(header) = header {
                    request = request.header("Authorization", &header);
                }
                request = request.header("X-Kog-Device", &device_id());
                let sent = match request.json(&payload) {
                    Ok(request) => request.send().await,
                    Err(error) => Err(error),
                };
                match sent {
                    Ok(response) if response.ok() => {
                        set_status_note.set(format!("Overwrote {name} with {count} tracks"));
                        load_playlists();
                    }
                    Ok(response) => set_message.set(error_text(response).await),
                    Err(error) => set_message.set(error.to_string()),
                }
            });
        }
    };

    // A trimmed name that matches a saved playlist (Favorites excluded):
    // saving onto it overwrites instead of creating.
    let playlist_name_exists = move |name: String| -> bool {
        playlists
            .get()
            .iter()
            .any(|(id, existing, _)| *id != 0 && *existing == name)
    };
    let existing_playlist_id = move |name: &str| -> Option<i64> {
        playlists
            .get_untracked()
            .iter()
            .find(|(id, existing, _)| *id != 0 && existing == name)
            .map(|(id, _, _)| *id)
    };

    // Spinner click: pause or resume the walk on the server.
    let toggle_search_paused = move || {
        let paused = !search_paused.get_untracked();
        set_search_paused.set(paused);
        let url = format!(
            "{}/api/library/search/pause?session={}",
            base(),
            url_encode(&session_model.with_value(|s| s.id().to_owned()))
        );
        let header = auth().header();
        leptos::task::spawn_local(async move {
            if let Err(error) =
                post_json(url, header, serde_json::json!({ "paused": paused })).await
            {
                set_message.set(error);
            }
        });
    };

    // A trimmed name that matches a saved playlist (Favorites excluded):
    // saving onto it overwrites instead of creating.
    let playlist_name_exists = move |name: String| -> bool {
        playlists
            .get()
            .iter()
            .any(|(id, existing, _)| *id != 0 && *existing == name)
    };
    let existing_playlist_id = move |name: &str| -> Option<i64> {
        playlists
            .get_untracked()
            .iter()
            .find(|(id, existing, _)| *id != 0 && existing == name)
            .map(|(id, _, _)| *id)
    };

    // Spinner click: pause or resume the walk on the server.
    let toggle_search_paused = move || {
        let paused = !search_paused.get_untracked();
        set_search_paused.set(paused);
        let url = format!(
            "{}/api/library/search/pause?session={}",
            base(),
            url_encode(&session_model.with_value(|s| s.id().to_owned()))
        );
        let header = auth().header();
        leptos::task::spawn_local(async move {
            if let Err(error) =
                post_json(url, header, serde_json::json!({ "paused": paused })).await
            {
                set_message.set(error);
            }
        });
    };

    // The dialog's OK (and Enter) action, per mode.
    let accept_playlist_dialog = move || {
        let Some(dialog) = playlist_dialog.get_untracked() else {
            return;
        };
        let name = dialog.value.trim().to_owned();
        match dialog.mode {
            PlaylistDialogMode::CreateFromPane { selected_only } => {
                if !name.is_empty() {
                    set_playlist_dialog.set(None);
                    if let Some(id) = existing_playlist_id(&name) {
                        overwrite_playlist_with_pane(id, name, selected_only);
                    } else {
                        create_playlist_from_pane(name, selected_only);
                    }
                }
            }
            PlaylistDialogMode::Duplicate => {
                if !name.is_empty() {
                    set_playlist_dialog.set(None);
                    duplicate_playlist(dialog.id, name);
                }
            }
            PlaylistDialogMode::ConfirmDelete => {
                set_playlist_dialog.set(None);
                delete_playlist(dialog.id);
            }
        }
    };

    // The dialog shows the name field for create/duplicate and a confirm
    // label for delete; both stay mounted and toggle visibility, so typing
    // never recreates the field mid-word.
    let confirm_delete_mode = move || {
        playlist_dialog
            .get()
            .map(|dialog| dialog.mode == PlaylistDialogMode::ConfirmDelete)
            .unwrap_or(true)
    };

    // Focus and select the dialog's name field when it opens, like the
    // desktop's dialog grabbing its text field.
    Effect::new(move |_| {
        if playlist_dialog_opened.get() == 0 {
            return;
        }
        if let Some(input) = playlist_dialog_input.get() {
            let _ = input.focus();
            let _ = input.select();
        }
    });

    Effect::new(move |_| {
        if renaming_playlist.get().is_some() {
            if let Some(input) = rename_input.get() {
                let _ = input.focus();
                let _ = input.select();
            }
        }
    });

    // ------------------------------------------------------- shell actions
    let add_url = move || {
        let url = add_url_text.get_untracked().trim().to_owned();
        if url.is_empty() {
            return;
        }
        let entry = Entry {
            kind: "remote".to_owned(),
            path: url.clone(),
            entry: String::new(),
            fragment: None,
            name: last_segment(&url),
            location: url,
        };
        backend.send(SessionCommand::Expand {
            scope: base(),
            entries: vec![serde_json::to_value(entry).unwrap()],
            action: QueueAction::AddToQueue,
        });
        set_add_url_open.set(false);
        if touch_mode {
            set_mobile_view.set(MobileView::Queue);
        }
    };
    let open_add_url = move || {
        set_add_url_text.set(String::new());
        set_add_url_open.set(true);
        set_menu_open.set(false);
    };

    // Clear the web queue and reset its playback state.
    let clear_pane = move || {
        backend.send(SessionCommand::Clear);
        set_list_name.set(String::new());
        set_menu_open.set(false);
    };

    let open_preferences = move || {
        set_menu_open.set(false);
        set_settings_open.set(true);
    };

    let open_about = move || {
        set_menu_open.set(false);
        set_about_open.set(true);
    };

    // The dedicated sidebar toggle: on the desktop it hides the inline tree;
    // on a phone it switches to the Library view.
    let toggle_sidebar = move || {
        let mobile = web_sys::window()
            .and_then(|window| {
                window
                    .match_media("(max-width: 820px), (pointer: coarse) and (max-width: 1200px)")
                    .ok()
                    .flatten()
            })
            .map(|media| media.matches())
            .unwrap_or(false);
        if mobile {
            set_mobile_view.set(if mobile_view.get_untracked() == MobileView::Library {
                MobileView::Queue
            } else {
                MobileView::Library
            });
            return;
        }
        let open = sidebar_visible.get_untracked();
        let next = !open;
        set_sidebar_visible.set(next);
        set_sidebar_open.set(next);
    };
    let sidebar_shown = move || {
        let mobile = web_sys::window()
            .and_then(|window| {
                window
                    .match_media("(max-width: 820px), (pointer: coarse) and (max-width: 1200px)")
                    .ok()
                    .flatten()
            })
            .map(|media| media.matches())
            .unwrap_or(false);
        if mobile {
            mobile_view.get() == MobileView::Library
        } else {
            sidebar_visible.get()
        }
    };

    // Add one tree row: a file expands to its playable tracks, and a folder
    // contributes playable files throughout its subtree, including folders
    // inside archives.
    let add_row_to_playlist = move |row: TreeRow| {
        let destination = playlist_workspace.snapshot();
        if !destination.actions.append { return; }
        let scope = base();
        let entries = vec![serde_json::json!({"kind":row.kind,"path":row.path,"entry":row.entry,"fragment":row.fragment,"name":row.name})];
        if destination.active == "queue" {
            if row.is_dir {
                backend.send(SessionCommand::Collect {
                    scope, path: row.path, query: tree_search.get_untracked(),
                    root: tree_root.get_untracked(), action: QueueAction::AddToQueue,
                });
            } else {
                backend.send(SessionCommand::AppendToTab {key:destination.active.clone(), scope, entries});
            }
        } else {
            let header = auth().header();
            let query = tree_search.get_untracked();
            let root = tree_root.get_untracked();
            leptos::task::spawn_local(async move {
                let resolved = if row.is_dir {
                    session::request("GET", format!("{scope}/api/library/collect?path={}&q={}&root={}",
                        url_encode(&row.path), url_encode(&query), url_encode(&root)), header, None)
                        .await.map(|value| value["tracks"].as_array()
                            .map(|items| items.iter().map(browse_file_entry).collect()).unwrap_or_default())
                } else {
                    session::expand(&scope, header, entries).await
                };
                match resolved {
                    Ok(entries) if base() == scope => backend.send(SessionCommand::AppendToTab {
                        key:destination.active.clone(), scope,
                        entries:entries.iter().map(|entry| serde_json::to_value(entry).expect("serializable entry")).collect(),
                    }),
                    Ok(_) => set_message.set("Connect to the original playlist server".into()),
                    Err(error) => set_message.set(error),
                }
            });
        }
    };

    // The queue's drag-and-drop is bound by hand on the pane container rather
    // than through the view's on: directives: dragstart, dragover, and drop
    // listeners attached that way never saw their events here, which left
    // mouse dragging unable to reorder. dragstart bubbles from whichever row
    // was grabbed, so one container listener covers every row; the row's
    // queue index travels in its data-index attribute. The pane element only
    // exists once the view has rendered, hence the effect.
    {
        let drag_bound = Rc::new(std::cell::Cell::new(false));
        let interval_cell: Rc<std::cell::Cell<Option<i32>>> = Rc::new(std::cell::Cell::new(None));
        let bind = {
            let drag_bound = drag_bound.clone();
            let interval_cell = interval_cell.clone();
            Closure::<dyn FnMut()>::new(move || {
                if drag_bound.get() {
                    if let Some(id) = interval_cell.get() {
                        if let Some(window) = web_sys::window() {
                            window.clear_interval_with_handle(id);
                        }
                    }
                    return;
                }
                let Some(rows) = web_sys::window()
                    .and_then(|window| window.document())
                    .and_then(|document| document.get_element_by_id("playlist-rows"))
                else {
                    return; // The pane has not rendered yet; poll again.
                };
                if let Some(id) = interval_cell.get() {
                    if let Some(window) = web_sys::window() {
                        window.clear_interval_with_handle(id);
                    }
                }
                drag_bound.set(true);
                let cleanup_drag = {
                    let set_dragging_track = set_dragging_track.clone();
                    let set_reorder_to = set_reorder_to.clone();
                    move || {
                        set_dragging_track.set(None);
                        set_reorder_to.set(None);
                        clear_track_drop_marker();
                    }
                };

                let on_dragstart = {
                    let set_dragging_track = set_dragging_track.clone();
                    let set_reorder_to = set_reorder_to.clone();
                    Closure::<dyn FnMut(web_sys::DragEvent)>::new(move |ev: web_sys::DragEvent| {
                        let row = ev
                            .target()
                            .and_then(|target| target.dyn_into::<web_sys::Element>().ok())
                            .and_then(|target| target.closest(".track").ok().flatten());
                        let Some(row) = row else {
                            return;
                        };
                        let Some(index) = row
                            .get_attribute("data-index")
                            .and_then(|value| value.parse::<usize>().ok())
                        else {
                            return;
                        };
                        if !playlist_workspace.snapshot().actions.append { ev.prevent_default(); return; }
                        if !pane_selected.get_untracked().contains(&index) { select_row(index, false, false); }
                        set_dragging_track.set(Some(index));
                        set_reorder_to.set(None);
                        if let Some(transfer) = ev.data_transfer() {
                            let _ = transfer.set_data("text/plain", &index.to_string());
                            let _ = transfer.set_effect_allowed("move");
                        }
                    })
                };
                let _ = rows.add_event_listener_with_callback(
                    "dragstart",
                    on_dragstart.as_ref().unchecked_ref(),
                );
                on_dragstart.forget();

                let on_dragover = {
                    let dragging_track = dragging_track.clone();
                    let set_reorder_to = set_reorder_to.clone();
                    let set_playlist_drop_active = set_playlist_drop_active.clone();
                    Closure::<dyn FnMut(web_sys::DragEvent)>::new(move |ev: web_sys::DragEvent| {
                        let current = dragging_track.get_untracked();
                        ev.prevent_default();
                        if current.is_some() {
                            set_reorder_to.set(track_drop_target(
                                ev.client_x(),
                                ev.client_y(),
                                pane_entries.get_untracked().len(),
                            ));
                            if let Some(transfer) = ev.data_transfer() {
                                transfer.set_drop_effect("move");
                            }
                            return;
                        }
                        if let Some(transfer) = ev.data_transfer() {
                            let _ = transfer.set_drop_effect("copy");
                        }
                        set_playlist_drop_active.set(true);
                    })
                };
                let _ = rows.add_event_listener_with_callback(
                    "dragover",
                    on_dragover.as_ref().unchecked_ref(),
                );
                on_dragover.forget();

                let on_dragleave = {
                    let set_playlist_drop_active = set_playlist_drop_active.clone();
                    Closure::<dyn FnMut(web_sys::DragEvent)>::new(move |ev: web_sys::DragEvent| {
                        set_playlist_drop_active.set(false);
                        if dragging_track.get_untracked().is_some() {
                            set_reorder_to.set(track_drop_target(
                                ev.client_x(),
                                ev.client_y(),
                                pane_entries.get_untracked().len(),
                            ));
                        }
                    })
                };
                let _ = rows.add_event_listener_with_callback(
                    "dragleave",
                    on_dragleave.as_ref().unchecked_ref(),
                );
                on_dragleave.forget();

                let on_drop = {
                    let dragging_tree = dragging_tree.clone();
                    let dragging_playlist = dragging_playlist.clone();
                    let dragging_track = dragging_track.clone();
                    let reorder_to = reorder_to.clone();
                    let set_playlist_drop_active = set_playlist_drop_active.clone();
                    let add_row_to_playlist = add_row_to_playlist.clone();
                    let append_playlist = append_playlist.clone();
                    let move_track = move_track.clone();
                    Closure::<dyn FnMut(web_sys::DragEvent)>::new(move |ev: web_sys::DragEvent| {
                        ev.prevent_default();
                        set_playlist_drop_active.set(false);
                        if let Some(row) = dragging_tree.get_untracked() {
                            add_row_to_playlist(row);
                        } else if let Some(id) = dragging_playlist.get_untracked() {
                            append_playlist(id);
                        } else if let Some(from) = dragging_track.get_untracked()
                            && let Some(to) = reorder_to.get_untracked()
                        {
                            move_track(from, to);
                        }
                        set_dragging_tree.set(None);
                        set_dragging_playlist.set(None);
                        set_dragging_track.set(None);
                        set_reorder_to.set(None);
                        clear_track_drop_marker();
                    })
                };
                let _ =
                    rows.add_event_listener_with_callback("drop", on_drop.as_ref().unchecked_ref());
                on_drop.forget();

                let on_dragend = {
                    let cleanup_drag = cleanup_drag.clone();
                    Closure::<dyn FnMut(web_sys::DragEvent)>::new(move |_| {
                        cleanup_drag();
                    })
                };
                let _ = rows.add_event_listener_with_callback(
                    "dragend",
                    on_dragend.as_ref().unchecked_ref(),
                );
                on_dragend.forget();
            })
        };
        if let Ok(handle) = web_sys::window()
            .expect("window for queue drag bindings")
            .set_interval_with_callback_and_timeout_and_arguments_0(
                bind.as_ref().unchecked_ref(),
                200,
            )
        {
            interval_cell.set(Some(handle));
        }
        bind.forget();
    }

    let apply_width = move |id: ColumnId, width: f64| {
        set_columns.update(|columns| {
            if let Some(column) = columns.iter_mut().find(|column| column.id == id) {
                column.width = width.clamp(id.min_width(), 1024.0);
            }
        });
    };

    let auto_fit_column = {
        move |id: ColumnId| {
            let rows = view_rows();
            let cache = metadata.get();
            let starred = stars.get();
            let mut texts = vec![id.label().to_owned()];
            for (index, entry) in &rows {
                texts.push(column_text(
                    id,
                    *index,
                    entry,
                    meta_for(&cache, entry).as_ref(),
                    None,
                    starred.contains(&entry_star_locator(entry)),
                    "",
                ));
            }
            let width = content_width(id, &texts);
            apply_width(id, width);
            persist_columns(&columns.get_untracked());
        }
    };

    let auto_fit_all = {
        let auto_fit_column = auto_fit_column.clone();
        move || {
            for id in ColumnId::ALL {
                if columns
                    .get_untracked()
                    .iter()
                    .any(|column| column.id == id && column.visible)
                {
                    auto_fit_column(id);
                }
            }
        }
    };

    let toggle_column = move |id: ColumnId| {
        let mut next = columns.get_untracked();
        let shown = next.iter().filter(|column| column.visible).count();
        if let Some(column) = next.iter_mut().find(|column| column.id == id) {
            if column.visible && shown <= 1 {
                return;
            }
            column.visible = !column.visible;
        }
        persist_columns(&next);
        set_columns.set(next);
    };

    // The column the header menu acts on, and the Qt Move Column Left/Right
    // actions. Moving swaps the target with its visible neighbour, so hidden
    // columns never shift the visible order.
    let menu_column = move || {
        column_menu
            .get()
            .map(|(_, _, id)| id)
            .unwrap_or(ColumnId::Index)
    };

    let can_move_column = move |direction: i64| -> bool {
        let visible = columns.get();
        let visible: Vec<ColumnId> = visible
            .iter()
            .filter(|column| column.visible)
            .map(|column| column.id)
            .collect();
        match visible
            .iter()
            .position(|candidate| *candidate == menu_column())
        {
            Some(position) => {
                let target = position as i64 + direction;
                target >= 0 && (target as usize) < visible.len()
            }
            None => false,
        }
    };

    let move_column = move |direction: i64| {
        let id = menu_column();
        set_columns.update(|columns| {
            let visible: Vec<ColumnId> = columns
                .iter()
                .filter(|column| column.visible)
                .map(|column| column.id)
                .collect();
            let Some(position) = visible.iter().position(|candidate| *candidate == id) else {
                return;
            };
            let target = position as i64 + direction;
            if target < 0 || target as usize >= visible.len() {
                return;
            }
            let neighbor = visible[target as usize];
            if let (Some(source), Some(other)) = (
                columns.iter().position(|column| column.id == id),
                columns.iter().position(|column| column.id == neighbor),
            ) {
                columns.swap(source, other);
            }
        });
        persist_columns(&columns.get_untracked());
        set_column_menu.set(None);
    };

    let reset_columns = move || {
        let next = default_columns();
        persist_columns(&next);
        set_columns.set(next);
    };

    // Leave the transport title empty until its metadata lookup completes.
    // A finished lookup without a title falls back to the filename.
    let now_title = move || match current_entry() {
        Some(entry) => {
            display_title(&metadata.get(), &metadata_failed.get(), &entry).unwrap_or_default()
        }
        None => "Kog".to_owned(),
    };
    let now_subtitle = move || {
        let entry = current_entry();
        let cache = metadata.get();
        if entry
            .as_ref()
            .is_some_and(|entry| !metadata_ready(&cache, &metadata_failed.get(), entry))
        {
            return String::new();
        }
        let meta = entry.as_ref().and_then(|entry| meta_for(&cache, entry));
        let artist = meta
            .as_ref()
            .and_then(|meta| meta.artist.clone())
            .unwrap_or_default();
        let album = meta
            .as_ref()
            .and_then(|meta| meta.album.clone())
            .unwrap_or_default();
        match (artist.is_empty(), album.is_empty()) {
            (false, false) => format!("{artist}  •  {album}"),
            (false, true) => artist,
            (true, false) => album,
            (true, true) => entry
                .as_ref()
                .map(|entry| entry.location.clone())
                .unwrap_or_else(|| "Ready to play".to_owned()),
        }
    };
    // A phone player has very little room for a path. When tags are absent,
    // show the containing folder as context; the full location remains in
    // Track Details and the desktop subtitle.
    let now_mobile_subtitle = move || {
        let Some(entry) = current_entry() else {
            return "Ready to play".to_owned();
        };
        let cache = metadata.get();
        if !metadata_ready(&cache, &metadata_failed.get(), &entry) {
            return String::new();
        }
        let meta = meta_for(&cache, &entry);
        let artist = meta
            .as_ref()
            .and_then(|row| row.artist.as_deref())
            .unwrap_or("");
        let album = meta
            .as_ref()
            .and_then(|row| row.album.as_deref())
            .unwrap_or("");
        if !artist.trim().is_empty() && !album.trim().is_empty() {
            return format!("{artist} · {album}");
        }
        if !artist.trim().is_empty() {
            return artist.to_owned();
        }
        if !album.trim().is_empty() {
            return album.to_owned();
        }
        let location = if entry.kind == "archive" && !entry.entry.is_empty() {
            entry.entry.as_str()
        } else {
            entry.path.as_str()
        };
        std::path::Path::new(location)
            .parent()
            .and_then(|parent| parent.file_name())
            .and_then(|name| name.to_str())
            .filter(|name| !name.is_empty())
            .unwrap_or("Unknown artist")
            .to_owned()
    };
    let repeat_badge = move || match repeat_mode.get() {
        Repeat::Off => "",
        Repeat::One => "1",
        Repeat::Album => "A",
        Repeat::All => "∞",
    };
    let repeat_tip = move || match repeat_mode.get() {
        Repeat::Off => "Repeat off — click for one track",
        Repeat::One => "Repeat one track — click for album",
        Repeat::Album => "Repeat album — click for all",
        Repeat::All => "Repeat all tracks — click to turn off",
    };
    let shuffle_tip = move || match shuffle.get() {
        ShuffleMode::Off => "Shuffle off — click for albums",
        ShuffleMode::Albums => "Shuffle albums — click for all tracks",
        ShuffleMode::All => "Shuffle all tracks — click to turn off",
    };

    view! {
            <div
                class="app"
                class:transport-compact=move || transport_compact.get()
                class:mobile-library=move || mobile_view.get() == MobileView::Library
                class:mobile-queue=move || mobile_view.get() == MobileView::Queue
                class:mobile-playlists=move || mobile_view.get() == MobileView::Playlists
                on:pointermove=move |ev: web_sys::PointerEvent| {
                    if let Some((id, start_x, start_width)) = resizing.get_untracked() {
                        apply_width(id, start_width + (ev.client_x() as f64 - start_x));
                    }
                }
                on:pointerup=move |_| {
                    if resizing.get_untracked().is_some() {
                        set_resizing.set(None);
                        persist_columns(&columns.get_untracked());
                    }
                }
            >
                <header class="toolbar">
                    <img class="logo" src="/icons/kog.svg" alt="Kog" />
                    <button
                        class="flat icon-button"
                        title="Kog menu"
                        id="app-menu-button"
                        aria-haspopup="menu"
                        aria-expanded=move || menu_open.get().to_string()
                        on:keydown=move |event: web_sys::KeyboardEvent| {
                            if event.key() == "ArrowDown" { event.prevent_default(); set_menu_open.set(true); }
                        }
                        on:click=move |_| set_menu_open.update(|open| *open = !*open)
                        inner_html=icons::MENU
                    ></button>
                    <button
                        class="flat icon-button sidebar-toggle"
                        class:active=move || sidebar_shown()
                        title=move || {
                            if sidebar_shown() { "Hide File Tree" } else { "Show File Tree" }
                        }
                        on:click=move |_| toggle_sidebar()
                        inner_html=icons::VIEW_LIST_TREE
                    ></button>
                    <span class="mobile-title">
                        {move || match mobile_view.get() {
                            MobileView::Library => "Library",
                            MobileView::Queue => "Queue",
                            MobileView::Playlists => "Playlists",
                        }}
                    </span>
                    <div class="search">
                        <span class="pill-icon" aria-hidden="true" inner_html=icons::FIND></span>
                        <input
                            type="search"
                            placeholder="Search"
                            aria-label="Search playlist"
                            prop:value=move || pane_filter.get()
                            on:input=move |event| filter_pane(event_target_value(&event))
                        />
                        <Show when=move || !pane_filter.get().is_empty() fallback=|| ()>
                            <button
                                class="flat search-clear"
                                title="Clear playlist search"
                                on:click=move |_| filter_pane(String::new())
                            >"×"</button>
                        </Show>
                    </div>
                    <button
                        class="flat icon-button mobile-sort"
                        type="button"
                        title="Sort queue"
                        aria-label="Sort queue"
                        on:click=move |_| set_mobile_sort_open.set(true)
                    >"↕"</button>
                    <button
                        class="flat icon-button mobile-create-playlist"
                        type="button"
                        title="Create a playlist"
                        aria-label="Create a playlist"
                        on:click=move |_| open_create_playlist_dialog()
                    >"+"</button>
                    <select
                        class="codec"
                        title="Stream format"
                        prop:value=move || codec.get()
                        on:change=move |event| {
                            let value = event_target_value(&event);
                            set_codec.set(value);
                        }
                    >
                        <option value="aac">"AAC"</option>
                        <option value="opus">"Opus"</option>
                        <option value="flac">"FLAC"</option>
                    </select>
                    <button
                        class="flat server"
                        title=move || if connected.get() { "Connected" } else { "Not connected" }
                        aria-label=move || if connected.get() { "Server settings, connected" } else { "Server settings, disconnected" }
                        on:click=move |_| set_settings_open.update(|open| *open = !*open)
                    >
                        <span class="mobile-server-icon" aria-hidden="true" inner_html=icons::GEAR></span>
                        <span class=move || {
                            if connected.get() { "server-dot online" } else { "server-dot offline" }
                        }>{move || if connected.get() { "●" } else { "○" }}</span>
                        <span class="server-label">" Server"</span>
                    </button>
                </header>

                <div
                    class:sidebar-open=move || sidebar_open.get()
                    class:sidebar-hidden=move || !sidebar_visible.get()
                    class:resizing=move || resizing_sidebar.get()
                    class="workspace"
                    style=move || format!("--sidebar-width: {:.0}px", sidebar_width.get())
                >
                    <div
                        class="drawer-scrim"
                        on:click=move |_| set_sidebar_open.set(false)
                    ></div>

                    <div
                        class="pane-resizer"
                        title="Drag to resize the file tree"
                        on:pointerdown=move |ev: web_sys::PointerEvent| {
                            ev.target().and_then(|target| {
                                target.dyn_into::<web_sys::Element>().ok()
                            })
                            .and_then(|element| element.set_pointer_capture(ev.pointer_id()).ok());
                            set_resizing_sidebar.set(true);
                        }
                        on:pointermove=move |ev: web_sys::PointerEvent| {
                            if resizing_sidebar.get_untracked() {
                                let width = (ev.client_x() as f64).clamp(180.0, 600.0);
                                set_sidebar_width.set(width);
                            }
                        }
                        on:pointerup=move |_| {
                            set_resizing_sidebar.set(false);
                            store(
                                "kog.sidebar-width",
                                &format!("{:.0}", sidebar_width.get_untracked()),
                            );
                        }
                        on:pointercancel=move |_| set_resizing_sidebar.set(false)
                    ></div>

                    <aside class="sidebar">
                        <section class="section files">
                            <div
                                class="section-header"
                                role="button"
                                tabindex="0"
                                on:click=move |_| set_files_expanded.update(|open| *open = !*open)
                            >
                                <span class="expander">
                                    {move || if files_expanded.get() { "▾" } else { "▸" }}
                                </span>
                                <span class="section-title">"Files"</span>
                            </div>
                            <Show when=move || files_expanded.get() fallback=|| ()>
                                <div class="section-body">
                                    <Show
                                        when=move || connected.get()
                                        fallback=|| view! {
                                            <p class="empty">"Connect to a server to browse."</p>
                                        }
                                    >
                                        <div class="tree-root">
                                            <button
                                                class="icon-button"
                                                title="Choose a folder on the server to root the tree at"
                                                on:click=open_folder_picker
                                                inner_html=icons::FOLDER_OPEN
                                            ></button>
                                            <button
                                                class="icon-button"
                                                title="Refresh this folder"
                                                on:click={
                                                    let load_dir = load_dir.clone();
                                                    move |_| {
                                                        let root = tree_root.get();
                                                        load_dir(root, None);
                                                    }
                                                }
                                            >"↻"</button>
                                            <button
                                                class="root-path"
                                                title=move || tree_location_name(&current_root()).to_owned()
                                                on:click=move |_| goto_root(String::new())
                                            >{move || current_root()}</button>
                                        </div>
                                        <div class="tree-search">
                                            <span class="pill-icon" aria-hidden="true" inner_html=icons::FIND></span>
                                            <input
                                                type="search"
                                                placeholder="Search files and folders…"
                                                prop:value=move || tree_search.get()
                                                on:input=move |event| {
                                                    let query = event_target_value(&event);
                                                    set_tree_search.set(query.clone());
                                                    run_tree_search(query);
                                                }
                                            />
                                            {/* The desktop's busy indicator: the
                                                walk runs server-side in slices and
                                                the pane says so until it is done. */}
                                            <Show
                                                when=move || tree_search_pending.get()
                                                fallback=|| ()
                                            >
                                                <span
                                                    class="tree-search-spinner"
                                                    class:paused=search_paused
                                                    title=move || {
                                                        if search_paused.get() {
                                                            "Search paused. Click to resume."
                                                        } else {
                                                            "Searching files and archives. Click to pause."
                                                        }
                                                    }
                                                    on:click=move |_| toggle_search_paused()
                                                >
                                                    <span class="gear" inner_html=icons::GEAR></span>
                                                    <span class="pause-badge">
                                                        <span class="pause-bar"></span>
                                                        <span class="pause-bar"></span>
                                                    </span>
                                                </span>
                                            </Show>
                                        </div>
                                        {/* Progress line under the box, where the
                                            desktop shows its search status: counts
                                            while the walk runs, the total (or a
                                            "narrow your search" notice at the pull
                                            cap) once it is done. */}
                                        <Show
                                            when=move || !tree_search.get().trim().is_empty()
                                            fallback=|| ()
                                        >
                                            <p class="search-status">
                                                {move || {
                                                    let count = search_count.get();
                                                    let progress = search_progress.get();
                                                    let mut text = if search_paused.get() {
                                                        format!("Paused · {count} matches")
                                                    } else if tree_search_pending.get() {
                                                        if progress.scanning_archives {
                                                            format!(
                                                                "{count} matches · Archives {} of {}",
                                                                progress.archives_scanned,
                                                                progress.archive_count
                                                            )
                                                        } else {
                                                            format!(
                                                                "{count} matches · Searching folders ({} items)",
                                                                progress.scanned
                                                            )
                                                        }
                                                    } else if count == 0 {
                                                        "No matching files or folders".to_owned()
                                                    } else if search_capped.get() {
                                                        format!("{count} matches — narrow your search for more")
                                                    } else {
                                                        format!("{count} matching files or folders")
                                                    };
                                                    if progress.unreadable > 0 {
                                                        text.push_str(&format!(
                                                            " — {} archive(s) could not be searched",
                                                            progress.unreadable
                                                        ));
                                                    }
                                                    text
                                                }}
                                            </p>
                                        </Show>
                                        <div class="tree-list">
                                                <Show
                                                    when=move || {
                                                        // The ".." row climbs toward the root the
                                                        // visitor chose; the library root itself is
                                                        // the ceiling, so it lists its folders with
                                                        // no climb row above them.
                                                        tree_search.get().trim().is_empty()
                                                            && !tree_root.get().is_empty()
                                                    }
                                                    fallback=|| ()
                                                >
                                                    <button
                                                        class="tree-row parent-row"
                                                        title=move || tree_location_name(&parent_path(&tree_root.get())).to_owned()
                                                        on:click={
                                                            let go_up = go_up.clone();
                                                            move |_| go_up()
                                                        }
                                                        on:contextmenu=move |ev: web_sys::MouseEvent| {
                                                            ev.prevent_default();
                                                            let root = tree_root.get_untracked();
                                                            let row = TreeRow {
                                                                name: "..".to_owned(),
                                                                path: parent_path(&root),
                                                                parent: root,
                                                                is_dir: true,
                                                                depth: 0,
                                                                expanded: false,
                                                                kind: "dir".to_owned(),
                                                                entry: String::new(),
                                                                fragment: None,
                                                            };
                                                            set_tree_menu.set(Some((
                                                                ev.client_x() as f64,
                                                                ev.client_y() as f64,
                                                                row,
                                                            )));
                                                        }
                                                    >
                                                        <span class="twisty"></span>
                                                        <span class="tree-icon up" inner_html=icons::GO_UP></span>
                                                        <span class="label">".."</span>
                                                    </button>
                                                </Show>
    <For
                                                each=tree_rows
                                                key=|row| format!(
                                                    "{}#{}#{}#{}#{}",
                                                    row.kind,
                                                    row.path,
                                                    row.entry,
                                                    row.fragment.clone().unwrap_or_default(),
                                                    row.depth,
                                                )
                                                let:row
                                            >
                                                {
                                                    let row_click = row.clone();
                                                    let row_menu = row.clone();
                                                    let row_drag = row.clone();
                                                    let row_add = row.clone();
                                                    let row_name = row.name.clone();
                                                    let row_tooltip = row_name.clone();
                                                    let tree_toggle = tree_toggle.clone();
                                                    let add_row_to_playlist = add_row_to_playlist.clone();
                                                    let selected = row.path.clone();
                                                    // The arrow reads `expanded` reactively: the
                                                    // `For` key is the path, so a programmatic
                                                    // expand (a restore) reuses the row and a
                                                    // captured string would go stale.
                                                    let twisty_path = row.path.clone();
                                                    let is_dir = row.is_dir;
                                                    let twisty = move || {
                                                        if !is_dir {
                                                            ""
                                                        } else {
                                                            // Whichever mode owns the pane
                                                            // owns the expansion state.
                                                            let open =
                                                                if tree_search.get_untracked()
                                                                    .trim()
                                                                    .is_empty()
                                                                {
                                                                    expanded.get()
                                                                } else {
                                                                    search_expanded.get()
                                                                };
                                                            if open.contains(&twisty_path) {
                                                                "▾"
                                                            } else {
                                                                "▸"
                                                            }
                                                        }
                                                    };
                                                    let indent = 6 + row.depth * 16;
                                                    // Per-format mark, the same
                                                    // art the desktop tree shows.
                                                    let (icon_svg, icon_badge): (
                                                        Option<&'static str>,
                                                        Option<String>,
                                                    ) = if row.is_dir {
                                                        (None, None)
                                                    } else {
                                                        match file_icon(&row.path, &row.entry) {
                                                            FileIcon::Svg(svg) => (Some(svg), None),
                                                            FileIcon::Badge(ext) => (None, Some(ext)),
                                                            FileIcon::None => (None, None),
                                                        }
                                                    };
                                                    let icon_html: Option<String> = match (
                                                        icon_svg, &icon_badge,
                                                    ) {
                                                        (Some(svg), _) => Some(svg.to_owned()),
                                                        (None, Some(ext)) => Some(format!(
                                                            "{}<span class=\"ext\">{ext}</span>",
                                                            icons::FMT_PAPER
                                                        )),
                                                        (None, None) => None,
                                                    };
                                                    let has_icon = icon_html.is_some();
                                                    view! {
                                                        <button
                                                            class="tree-row"
                                                            class:directory=row.is_dir
                                                            class:selected=move || {
                                                                tree_selected.get() == selected
                                                            }
                                                            style=format!("--tree-indent: {indent}px")
                                                            title=row_tooltip
                                                            draggable="true"
                                                            on:click=move |ev: web_sys::MouseEvent| {
                                                                set_tree_selected.set(row_click.path.clone());
                                                                set_tree_selected_dir.set(row_click.is_dir);
                                                                if !touch_mode && ev.detail() >= 2 {
                                                                    add_row_to_playlist(row_click.clone());
                                                                } else if row_click.is_dir {
                                                                    tree_toggle(row_click.path.clone());
                                                                } else if touch_mode {
                                                                    // Touch: tapping a file
                                                                    // queues it, like the
                                                                    // desktop's double click.
                                                                    add_row_to_playlist(row_click.clone());
                                                                    set_sidebar_open.set(false);
                                                                    set_mobile_view.set(MobileView::Queue);
                                                                }
                                                            }
                                                            on:contextmenu=move |ev: web_sys::MouseEvent| {
                                                                ev.prevent_default();
                                                                ev.stop_propagation();
                                                                set_tree_selected.set(row_menu.path.clone());
                                                                set_tree_selected_dir.set(row_menu.is_dir);
                                                                set_tree_menu.set(Some((
                                                                    ev.client_x() as f64,
                                                                    ev.client_y() as f64,
                                                                    row_menu.clone(),
                                                                )));
                                                            }
                                                            on:dragstart=move |ev: web_sys::DragEvent| {
                                                                set_dragging_tree.set(Some(row_drag.clone()));
                                                                if let Some(transfer) = ev.data_transfer() {
                                                                    let _ = transfer.set_data(
                                                                        "text/plain",
                                                                        &row_drag.path,
                                                                    );
                                                                    transfer.set_effect_allowed("copy");
                                                                }
                                                            }
                                                            on:dragend=move |_| set_dragging_tree.set(None)
                                                        >
                                                            <span class="twisty">{move || twisty()}</span>
                                                            <span
                                                                class=if row.is_dir {
                                                                    "tree-icon dir"
                                                                } else if has_icon {
                                                                    "tree-icon fmt"
                                                                } else {
                                                                    "tree-icon file"
                                                                }
                                                                inner_html=icon_html
                                                            ></span>
                                                            <span class="label">
                                                                {move || {
                                                                    highlight_label(
                                                                        row_name.clone(),
                                                                        tree_search.get(),
                                                                    )
                                                                }}
                                                            </span>
                                                            // Touch's visible add: enqueues
                                                            // the folder or file without a
                                                            // double click, drag, or hold.
                                                            // Hidden on fine pointers by the
                                                            // stylesheet, like the drag grip.
                                                            <span
                                                                class="tree-add"
                                                                title=if touch_mode { "Add to queue" } else { "Add to playlist" }
                                                                on:click={
                                                                    let add_row_to_playlist =
                                                                        add_row_to_playlist.clone();
                                                                    let row_add = row_add.clone();
                                                                    move |ev: web_sys::MouseEvent| {
                                                                        ev.stop_propagation();
                                                                        add_row_to_playlist(
                                                                            row_add.clone(),
                                                                        );
                                                                        set_sidebar_open.set(false);
                                                                        if touch_mode { set_mobile_view.set(MobileView::Queue); }
                                                                    }
                                                                }
                                                            >"+"
                                                            </span>
                                                        </button>
                                                    }
                                                }
                                            </For>
                                            <Show
                                                when=move || {
                                                    !tree_search.get().trim().is_empty()
                                                        && tree_rows().is_empty()
                                                }
                                                fallback=|| ()
                                            >
                                                {/* While the walk runs the status line
                                                    above carries the progress; this
                                                    only speaks once it has finished. */}
                                                <Show when=move || !tree_search_pending.get() fallback=|| ()>
                                                    <p class="empty">"No matching files or folders"</p>
                                                </Show>
                                            </Show>
                                        </div>

                                        <Show
                                            when=move || {
                                                tree_rows().is_empty()
                                                    && library_root.get().is_empty()
                                                    && tree_search.get().trim().is_empty()
                                            }
                                            fallback=|| ()
                                        >
                                            <p class="empty">
                                                "No music folder is configured on the server."
                                            </p>
                                        </Show>
                                    </Show>
                                </div>
                            </Show>
                        </section>

                        <section class="section playlists">
                            <div
                                class="section-header"
                                role="button"
                                tabindex="0"
                                on:click=move |_| {
                                    set_playlists_expanded.update(|open| *open = !*open)
                                }
                            >
                                <span class="expander">
                                    {move || if playlists_expanded.get() { "▾" } else { "▸" }}
                                </span>
                                <span class="section-title">"Playlists"</span>
                                <button
                                    class="section-add"
                                    title="Create a playlist"
                                    on:click=move |event| {
                                        event.stop_propagation();
                                        open_create_playlist_dialog();
                                    }
                                >"+"</button>
                            </div>
                            <Show when=move || playlists_expanded.get() fallback=|| ()>
                                <div
                                    class="section-body"
                                    on:dragover=move |ev: web_sys::DragEvent| {
                                        ev.prevent_default();
                                        if dragging_playlist.get_untracked().is_none() {
                                            return;
                                        }
                                        let client_y = ev.client_y();
                                        let rows = js_sys::eval(&format!(
                                            "(() => {{ const rows = [...document.querySelectorAll('.playlist-row')]; const y = {client_y}; let index = rows.length; for (let i = 0; i < rows.length; i++) {{ const r = rows[i].getBoundingClientRect(); if (y < r.top + r.height / 2) {{ index = i; break; }} }} return index; }})()"
                                        ));
                                        if let Ok(value) = rows
                                            && let Some(index) = value.as_f64()
                                        {
                                            set_playlist_reorder_to.set(Some(index as usize));
                                        }
                                    }
                                    on:drop=move |ev: web_sys::DragEvent| {
                                        ev.prevent_default();
                                        if let Some(id) = dragging_playlist.get_untracked()
                                            && let Some(to) = playlist_reorder_to.get_untracked()
                                        {
                                            let url = format!("{}/api/playlists/{id}/move", base());
                                            let header = auth().header();
                                            leptos::task::spawn_local(async move {
                                                let _ = post_json(
                                                    url,
                                                    header,
                                                    serde_json::json!({ "to": to }),
                                                )
                                                .await;
                                                load_playlists();
                                            });
                                        }
                                        set_dragging_playlist.set(None);
                                        set_playlist_reorder_to.set(None);
                                    }
                                >
                                    <Show when=move || connected.get() fallback=|| ()>
                                        {
                                            view! {
                                            <button
                                                class="tree-row favorite-row"
                                                class:selected=move || pane_key.get() == format!("{}:0", base())
                                                title="Open Favorites in a tab"
                                                draggable="true"
                                                on:click=move |_| {
                                                    // Touch: a tap opens the list in
                                                    // the pane; the + button appends.
                                                    {
                                                        replace_pane_with_playlist(
                                                            0,
                                                            "Favorites".to_owned(),
                                                        );
                                                        set_sidebar_open.set(false);
                                                        set_mobile_view.set(MobileView::Queue);
                                                    }
                                                }
                                                on:dragstart=move |ev: web_sys::DragEvent| {
                                                    set_dragging_playlist.set(Some(0));
                                                    if let Some(transfer) = ev.data_transfer() {
                                                        let _ = transfer.set_data("text/plain", "Favorites");
                                                        transfer.set_effect_allowed("copy");
                                                    }
                                                }
                                                on:dragend=move |_| set_dragging_playlist.set(None)
                                                on:dblclick=move |_| {
                                                    playlist_workspace.open(0, "Favorites".into());
                                                }
                                            >
                                                <span class="twisty"></span>
                                                <span class="favorite-star">"★"</span>
                                                <span class="label">"Favorites"</span>
                                                <span
                                                    class="mobile-playlist-more"
                                                    role="button"
                                                    tabindex="0"
                                                    aria-label="Favorites actions"
                                                    on:click=move |ev: web_sys::MouseEvent| {
                                                        ev.stop_propagation();
                                                        set_playlist_menu.set(Some((ev.client_x() as f64, ev.client_y() as f64, 0, "Favorites".to_owned())));
                                                    }
                                                    on:keydown=move |ev: web_sys::KeyboardEvent| {
                                                        if ev.key() == "Enter" || ev.key() == " " {
                                                            ev.prevent_default();
                                                            ev.stop_propagation();
                                                            set_playlist_menu.set(Some((0.0, 0.0, 0, "Favorites".to_owned())));
                                                        }
                                                    }
                                                >"⋯"</span>
                                                <span
                                                    class="tree-add playlist-add"
                                                    role="button"
                                                    tabindex="0"
                                                    aria-label=playlist_append_title
                                                    aria-disabled=move || !playlist_workspace.snapshot().actions.append
                                                    on:keydown=move |ev: web_sys::KeyboardEvent| {
                                                        if ev.key() == "Enter" || ev.key() == " " {
                                                            ev.prevent_default();
                                                            ev.stop_propagation();
                                                            append_playlist(0);
                                                        }
                                                    }
                                                    title=playlist_append_title
                                                    on:click={
                                                        let append_playlist = append_playlist.clone();
                                                        move |ev: web_sys::MouseEvent| {
                                                            ev.stop_propagation();
                                                            append_playlist(0);
                                                            set_sidebar_open.set(false);
                                                            if touch_mode { set_mobile_view.set(MobileView::Queue); }
                                                        }
                                                    }
                                                >"+"
                                                </span>
                                            </button>
                                            }
                                        }
                                        // Re-render labels and counts after a rename or save.
                                        <For each=move || playlists.get() key=|item| format!("{}#{}#{}", item.0, item.1, item.2) let:item>
                                            {
                                                let id = item.0;
                                                let count = item.2;
                                                let label = item.1.clone();
                                                let drag_id = id;
                                                let drag_label = label.clone();
                                                let menu_label = label.clone();
                                                let more_click_label = label.clone();
                                                let more_key_label = label.clone();
                                                let more_aria_label = format!("Actions for {label}");
                                                let open_label = label.clone();
                                                view! {
                                                    <button
                                                        class="tree-row playlist-row"
                                                        class:selected=move || pane_key.get() == format!("{}:{drag_id}", base())
                                                        class:renaming=move || renaming_playlist.get() == Some(drag_id)
                                                        title="Open playlist in a tab"
                                                        draggable="true"
                                                        on:click=move |_| {
                                                            // Touch: a tap opens the
                                                            // playlist in the pane; the
                                                            // + button appends.
                                                            {
                                                                replace_pane_with_playlist(
                                                                    drag_id,
                                                                    open_label.clone(),
                                                                );
                                                                set_sidebar_open.set(false);
                                                                set_mobile_view.set(MobileView::Queue);
                                                            }
                                                        }
                                                        on:dragstart=move |ev: web_sys::DragEvent| {
                                                            set_dragging_playlist.set(Some(drag_id));
                                                            if let Some(transfer) = ev.data_transfer() {
                                                                let _ = transfer.set_data("text/plain", &drag_label);
                                                                transfer.set_effect_allowed("copy");
                                                            }
                                                        }
                                                        on:dragend=move |_| set_dragging_playlist.set(None)
                                                        on:dblclick=move |_| {
                                                            // Single click already focuses the editor tab.
                                                        }
                                                        on:contextmenu=move |ev: web_sys::MouseEvent| {
                                                            ev.prevent_default();
                                                            ev.stop_propagation();
                                                            set_playlist_menu.set(Some((
                                                                ev.client_x() as f64,
                                                                ev.client_y() as f64,
                                                                drag_id,
                                                                menu_label.clone(),
                                                           )));
                                                        }
                                                    >
                                                        <span class="twisty"></span>
                                                        <span class="playlist-entry-icon" aria-hidden="true" inner_html=icons::FMT_PLAYLIST></span>
                                                        <Show
                                                            when=move || renaming_playlist.get() == Some(drag_id)
                                                            fallback=move || view! {
                                                                <span class="label">{label.clone()}</span>
                                                            }
                                                        >
                                                            <input
                                                                class="playlist-rename"
                                                                node_ref=rename_input
                                                                prop:value=move || rename_text.get()
                                                                on:input=move |event| set_rename_text.set(event_target_value(&event))
                                                                on:keydown=move |event: web_sys::KeyboardEvent| {
                                                                    match event.key().as_str() {
                                                                        "Enter" => commit_rename(),
                                                                        "Escape" => set_renaming_playlist.set(None),
                                                                        _ => {}
                                                                    }
                                                                }
                                                                on:blur=move |_| commit_rename()
                                                            />
                                                        </Show>
                                                        <span class="count">{count}</span>
                                                        <span
                                                            class="mobile-playlist-more"
                                                            role="button"
                                                            tabindex="0"
                                                            aria-label=more_aria_label
                                                            on:click=move |ev: web_sys::MouseEvent| {
                                                                ev.stop_propagation();
                                                                set_playlist_menu.set(Some((ev.client_x() as f64, ev.client_y() as f64, drag_id, more_click_label.clone())));
                                                            }
                                                            on:keydown=move |ev: web_sys::KeyboardEvent| {
                                                                if ev.key() == "Enter" || ev.key() == " " {
                                                                    ev.prevent_default();
                                                                    ev.stop_propagation();
                                                                    set_playlist_menu.set(Some((0.0, 0.0, drag_id, more_key_label.clone())));
                                                                }
                                                            }
                                                        >"⋯"</span>
                                                        <span
                                                            class="tree-add playlist-add"
                                                            role="button"
                                                            tabindex="0"
                                                            aria-label=playlist_append_title
                                                            aria-disabled=move || count == 0 || !playlist_workspace.snapshot().actions.append
                                                            on:keydown=move |ev: web_sys::KeyboardEvent| {
                                                                if ev.key() == "Enter" || ev.key() == " " {
                                                                    ev.prevent_default();
                                                                    ev.stop_propagation();
                                                                    if count > 0 { append_playlist(drag_id); }
                                                                }
                                                            }
                                                            title=playlist_append_title
                                                            on:click={
                                                                let append_playlist =
                                                                    append_playlist.clone();
                                                                move |ev: web_sys::MouseEvent| {
                                                                    ev.stop_propagation();
                                                                    if count > 0 { append_playlist(drag_id); }
                                                                    set_sidebar_open.set(false);
                                                                    if touch_mode { set_mobile_view.set(MobileView::Queue); }
                                                                }
                                                            }
                                                        >"+"
                                                        </span>
                                                    </button>
                                                }
                                            }
                                        </For>
                                        <Show when=move || !connected.get() fallback=|| ()>
                                            <p class="empty">"Not connected."</p>
                                        </Show>
                                    </Show>
                                </div>
                            </Show>
                        </section>
                    </aside>

                    <main class="playlist">
                        <Show when=move || !backend.persistence.warning.get().is_empty() || !persist.warning.get().is_empty()>
                            <div class="persistence-warning" role="alert">
                                {move || {
                                    let session = backend.persistence.warning.get();
                                    let ui = persist.warning.get();
                                    if session.is_empty() { ui }
                                    else if ui.is_empty() || ui == session { session }
                                    else { format!("{session} {ui}") }
                                }}
                            </div>
                        </Show>
                        <workspace::Tabs controller=playlist_workspace />
                        <Show when=move || playlist_workspace.snapshot().error.is_some()>
                            <p class="empty" role="alert">{move || playlist_workspace.snapshot().error.clone().unwrap_or_default()}</p>
                        </Show>
                        <div
                            class="rows"
                            id="playlist-rows"
                            class:drop-active=move || playlist_drop_active.get()
                            on:wheel=move |event: web_sys::WheelEvent| {
                                if touch_mode {
                                    return;
                                }
                                // The wide column set must be reachable with the
                                // plain wheel: when the rows cannot scroll
                                // vertically, or the wheel hits the top/bottom of
                                // a long list, the delta scrolls horizontally.
                                let element = event
                                    .current_target()
                                    .expect("wheel target exists")
                                    .unchecked_into::<web_sys::Element>();
                                let vertical =
                                    element.scroll_height() > element.client_height();
                                let delta = event.delta_y();
                                if delta == 0.0 {
                                    return;
                                }
                                let redirect = !vertical || {
                                    let at_bottom = element.scroll_top()
                                        + element.client_height()
                                        >= element.scroll_height();
                                    let at_top = element.scroll_top() <= 0;
                                    (delta > 0.0 && at_bottom)
                                        || (delta < 0.0 && at_top)
                                };
                                if redirect {
                                    element.set_scroll_left(
                                        element.scroll_left() + delta as i32,
                                    );
                                    event.prevent_default();
                                }
                            }
                            style=move || {
                                format!(
                                    "--cols:{}; --table-width:{:.0}px",
                                    grid_template(),
                                    table_width()
                                )
                            }
                            on:dragleave=move |_| set_playlist_drop_active.set(false)
                        >
                            <div
                                class="columns"
                                on:contextmenu=move |ev: web_sys::MouseEvent| {
                                ev.prevent_default();
                                // Empty header space targets the first column so
                                // the Move items still have something to act on.
                                let target = visible_columns()
                                    .first()
                                    .map(|column| column.id)
                                    .unwrap_or(ColumnId::Index);
                                set_column_menu.set(Some((
                                    ev.client_x() as f64,
                                    ev.client_y() as f64,
                                    target,
                                )));
                            }
                        >
                            <For each=visible_columns key=|column| column.id.key() let:column>
                                {
                                    let id = column.id;
                                    let sort = id.sort_key();
                                    let align = if id.align_right() {
                                        "right"
                                    } else if id.align_center() {
                                        "center"
                                    } else {
                                        "left"
                                    };
                                    let start_column = id;
                                    view! {
                                        <div
                                            class=format!("cell col-head {}", id.class())
                                            on:contextmenu=move |ev: web_sys::MouseEvent| {
                                                ev.prevent_default();
                                                ev.stop_propagation();
                                                set_column_menu.set(Some((
                                                    ev.client_x() as f64,
                                                    ev.client_y() as f64,
                                                    id,
                                                )));
                                            }
                                        >
                                            <button
                                                class="col-sort"
                                                style=format!("text-align: {align}")
                                                on:click=move |_| toggle_sort(sort)
                                            >
                                                {move || format!("{}{}", id.label(), sort_arrow(sort))}
                                            </button>
                                            <span
                                                class="col-resize"
                                                title="Drag to resize, double-click to auto-fit"
                                                on:pointerdown=move |ev: web_sys::PointerEvent| {
                                                    ev.prevent_default();
                                                    ev.stop_propagation();
                                                    if let Some(target) = ev.current_target() {
                                                        if let Ok(element) =
                                                            target.dyn_into::<web_sys::Element>()
                                                        {
                                                            let _ = element
                                                                .set_pointer_capture(ev.pointer_id());
                                                        }
                                                    }
                                                    let width = columns
                                                        .get_untracked()
                                                        .iter()
                                                        .find(|column| column.id == start_column)
                                                        .map(|column| column.width)
                                                        .unwrap_or(0.0);
                                                    set_resizing.set(Some((
                                                        start_column,
                                                        ev.client_x() as f64,
                                                        width,
                                                    )));
                                                }
                                                on:dblclick={
                                                    let auto_fit_column = auto_fit_column.clone();
                                                    move |ev: web_sys::MouseEvent| {
                                                        ev.stop_propagation();
                                                        auto_fit_column(start_column);
                                                    }
                                                }
                                            ></span>
                                        </div>
                                    }
                                }
                            </For>
                        </div>

                            <Show
                                when=move || !view_rows().is_empty()
                                fallback=move || view! {
                                    <Show when=move || !connected.get() || pane_entries.get().is_empty()>
                                        <p class="empty">
                                            {move || if connected.get() {
                                                if pane_key.get() == "queue" { "Pick a folder in the file tree, or a playlist." }
                                                else { "This playlist is empty." }
                                            } else {
                                                "Open the server settings to connect."
                                            }}
                                        </p>
                                        <div class="mobile-queue-empty">
                                            <span class="mobile-empty-icon" inner_html=icons::FMT_PLAYLIST></span>
                                            <h2>{move || if !connected.get() { "Connect to Kog" } else if pane_key.get() == "queue" { "Your queue is empty" } else { "This playlist is empty" }}</h2>
                                            <p>{move || if connected.get() { "Choose music from your Library or Playlists." } else { "Connect to your Kog server to browse and play music." }}</p>
                                            <button
                                                type="button"
                                                class="primary"
                                                on:click=move |_| {
                                                    if connected.get_untracked() {
                                                        set_mobile_view.set(MobileView::Library);
                                                    } else {
                                                        set_settings_open.set(true);
                                                    }
                                                }
                                            >{move || if connected.get() { "Browse Library" } else { "Connect to Server" }}</button>
                                        </div>
                                    </Show>
                                }
                            >
                                <For
                                    each=view_rows
                                    key=move |(index, entry)| format!("{}:{index}:{}", pane_key.get(), meta_key(entry))
                                    let:row
                                >
                                    {
                                        let (index, entry) = row;
                                        let menu_entry = entry.clone();
                                        let mobile_entry = entry.clone();
                                        view! {
                                            <button
                                                class="track"
                                                draggable=move || if playlist_workspace.snapshot().actions.append { "true" } else { "false" }
                                                style="position: relative"
                                                data-index=move || index.to_string()
                                                class:current=move || pane_current.get() == Some(index)
                                                class:selected=move || pane_selected.read().contains(&index)
                                                on:contextmenu=move |ev: web_sys::MouseEvent| {
                                                    ev.prevent_default();
                                                    if !pane_selected.get_untracked().contains(&index) {
                                                        select_row(index,false,false);
                                                    }
                                                    set_song_menu.set(Some((
                                                        ev.client_x() as f64,
                                                        ev.client_y() as f64,
                                                        index,
                                                        menu_entry.clone(),
                                                    )));
                                                }
                                                on:click=move |ev: web_sys::MouseEvent| {
                                                    if touch_mode && !ev.shift_key() && !ev.ctrl_key() && !ev.meta_key() {
                                                        activate_row(index);
                                                        return;
                                                    }
                                                    // A single click only selects,
                                                    // like the desktop: a song starts
                                                    // on double click.
                                                    select_row(index,ev.shift_key(),ev.ctrl_key()||ev.meta_key());
                                                }
                                                on:keydown=move |ev: web_sys::KeyboardEvent| {
                                                    if ev.key() == "Enter" && !ev.ctrl_key() && !ev.meta_key() && !ev.alt_key() {
                                                        ev.prevent_default(); ev.stop_propagation();
                                                        activate_row(index);
                                                    }
                                                }
                                                on:dblclick=move |ev: web_sys::MouseEvent| {
                                                    ev.prevent_default();
                                                    // Touch handled the activation on
                                                    // the tap; the second tap of a
                                                    // double must not toggle again.
                                                    if touch_mode {
                                                        return;
                                                    }
                                                    activate_row(index);
                                                }
                                            >
                                                <For
                                                    each=visible_columns
                                                    key=|column| column.id.key()
                                                    let:column
                                                >
                                                    {
                                                        let id = column.id;
                                                        let entry = entry.clone();
                                                        let star_entry = entry.clone();
                                                        let toggle_star = toggle_star.clone();
                                                        // The title column carries
                                                        // the track's format icon,
                                                        // the same art the file
                                                        // tree shows.
                                                        let title_icon: Option<String> =
                                                            if id == ColumnId::Title {
                                                                match file_icon(
                                                                    &entry.path,
                                                                    &entry.entry,
                                                                ) {
                                                                    FileIcon::Svg(svg) => {
                                                                        Some(svg.to_owned())
                                                                    }
                                                                    FileIcon::Badge(ext) => {
                                                                        Some(format!(
                                                                            "{}<span class=\"ext\">{ext}</span>",
                                                                            icons::FMT_PAPER
                                                                        ))
                                                                    }
                                                                    FileIcon::None => {
                                                                        Some(icons::FMT_AUDIO.to_owned())
                                                                    }
                                                                }
                                                            } else {
                                                                None
                                                            };
                                                        // Split for the view: the
                                                        // visibility test borrows
                                                        // for the row's lifetime
                                                        // while the markup moves.
                                                        let title_icon_show =
                                                            title_icon.is_some();
                                                        let text = move || {
                                                            let cache = metadata.read();
                                                            let meta = meta_for(&cache, &entry);
                                                            let live = if pane_current.get() == Some(index)
                                                                && duration.get() > 0.0
                                                            {
                                                                Some(duration.get())
                                                            } else {
                                                                None
                                                            };
                                                            let starred = stars
                                                                .read()
                                                                .contains(&entry_star_locator(&entry));
                                                            let status = if pane_current.get() == Some(index) {
                                                                if playing.get() {
                                                                    "▶"
                                                                } else if !stopped.get() {
                                                                    "Ⅱ"
                                                                } else {
                                                                    ""
                                                                }
                                                            } else {
                                                                ""
                                                            };
                                                            if id == ColumnId::Status {
                                                                if pane_key.get() != "queue" { return status.to_owned(); }
                                                                policy_revision.track();
                                                                let policy = session_model.read_value();
                                                                let queued = policy.order().queue_position(index).map(|position| format!(" {}", position + 1)).unwrap_or_default();
                                                                let stop = if policy.order().should_stop_after(index) { " ■" } else { "" };
                                                                return format!("{status}{queued}{stop}");
                                                            }
                                                            if id == ColumnId::Title {
                                                                display_title(&cache, &metadata_failed.read(), &entry)
                                                                    .unwrap_or_default()
                                                            } else {
                                                                column_text(
                                                                    id,
                                                                    index,
                                                                    &entry,
                                                                    meta.as_ref(),
                                                                    live,
                                                                    starred,
                                                                    status,
                                                                )
                                                            }
                                                        };
                                                        // The searchable text columns paint
                                                        // matched tokens while the playlist
                                                        // filter is active; the other columns
                                                        // keep their plain text. The tooltip
                                                        // always shows the raw text.
                                                        let tip = text.clone();
                                                        let text = move || {
                                                            let raw = text();
                                                            if matches!(
                                                                id,
                                                                ColumnId::Title
                                                                    | ColumnId::Artist
                                                                    | ColumnId::Album
                                                            ) {
                                                                highlight_label(raw, pane_filter.get())
                                                            } else {
                                                                raw.into_any()
                                                            }
                                                        };
                                                        view! {
                                                            <span
                                                                class=format!("cell {}", id.class())
                                                                class:visualizer-trigger=move || {
                                                                    id == ColumnId::Status
                                                                        && pane_current.get() == Some(index)
                                                                        && !stopped.get()
                                                                }
                                                                title=move || {
                                                                    if id == ColumnId::Status
                                                                        && pane_current.get() == Some(index)
                                                                        && !stopped.get()
                                                                    {
                                                                        "Open audio visualizer".to_owned()
                                                                    } else {
                                                                        tip()
                                                                    }
                                                                }
                                                                on:click=move |ev: web_sys::MouseEvent| {
                                                                    if id == ColumnId::Status
                                                                        && pane_current.get_untracked() == Some(index)
                                                                        && !stopped.get_untracked()
                                                                    {
                                                                        ev.stop_propagation();
                                                                        set_visualizer_spectrum_mode.set(false);
                                                                        set_visualizer_open.set(true);
                                                                        return;
                                                                    }
                                                                    // The star cell toggles without
                                                                    // selecting or playing the row.
                                                                    if id == ColumnId::Star {
                                                                        ev.stop_propagation();
                                                                        let entry = star_entry.clone();
                                                                        let starred = stars
                                                                            .get_untracked()
                                                                            .contains(&entry_star_locator(&entry));
                                                                        toggle_star(entry, starred);
                                                                    }
                                                                }
                                                                on:dblclick=move |ev: web_sys::MouseEvent| {
                                                                    if id == ColumnId::Status
                                                                        && pane_current.get_untracked() == Some(index)
                                                                        && !stopped.get_untracked()
                                                                    {
                                                                        ev.stop_propagation();
                                                                    }
                                                                }
                                                            >
                                                                <Show
                                                                    when=move || title_icon_show
                                                                    fallback=|| ()
                                                                >
                                                                    <span
                                                                        class="cell-icon"
                                                                        inner_html=title_icon.clone().unwrap_or_default()
                                                                    ></span>
                                                                </Show>
                                                                {text}
                                                                <Show
                                                                    when=move || {
                                                                        id == ColumnId::Status
                                                                            && pane_current.get() == Some(index)
                                                                            && playing.get()
                                                                    }
                                                                    fallback=|| ()
                                                                >
                                                                    // The desktop's status cell:
                                                                    // the play/pause glyph with
                                                                    // the five-band waveform
                                                                    // beside it, centered in the
                                                                    // cell. Paint styles are
                                                                    // inline so a stale cached
                                                                    // stylesheet cannot leave the
                                                                    // bars unstyled (invisible).
                                                                    <span
                                                                        class="row-meter"
                                                                        style="display: inline-flex; align-items: flex-end; gap: 1px; width: 16px; height: 14px; padding: 1px; box-sizing: border-box; border-radius: 4px; vertical-align: middle; margin-left: 3px; pointer-events: none; background: rgba(5, 20, 28, 0.78); border: 1px solid rgba(255, 255, 255, 0.18);"
                                                                    >
                                                                        <For
                                                                            each=|| [0usize, 1, 2, 3, 4]
                                                                            key=|band| *band
                                                                            let:band
                                                                        >
                                                                            <span
                                                                                class="row-meter-bar"
                                                                                style=move || {
                                                                                    let levels = audio_levels.get();
                                                                                    let level = levels
                                                                                        .get(band)
                                                                                        .copied()
                                                                                        .unwrap_or(0.0)
                                                                                        .clamp(0.0, 1.0);
                                                                                    // The desktop's
                                                                                    // selected-row set:
                                                                                    // the current row
                                                                                    // is highlight
                                                                                    // blue, and the
                                                                                    // normal colors
                                                                                    // vanish on it.
                                                                                    const COLORS: [&str; 5] = [
                                                                                        "#8cbcff",
                                                                                        "#64d8ff",
                                                                                        "#47eee7",
                                                                                        "#53edb4",
                                                                                        "#82ef99",
                                                                                    ];
                                                                                    format!(
                                                                                        "width: 2px; flex: none; min-height: 2px; border-radius: 1px; background: {}; height: {}px;",
                                                                                        COLORS[band],
                                                                                        2.0 + 10.0 * level
                                                                                    )
                                                                                }
                                                                            ></span>
                                                                        </For>
                                                                    </span>
                                                                </Show>
                                                            </span>
                                                        }
                                                    }
                                                </For>
                                                {
                                                    let mobile_title_entry = mobile_entry.clone();
                                                    let mobile_detail_entry = mobile_entry.clone();
                                                    let mobile_duration_entry = mobile_entry.clone();
                                                    let mobile_star_label_entry = mobile_entry.clone();
                                                    let mobile_icon = match file_icon(&mobile_entry.path, &mobile_entry.entry) {
                                                        FileIcon::Svg(svg) => svg.to_owned(),
                                                        FileIcon::Badge(ext) => format!("{}<span class=\"ext\">{ext}</span>", icons::FMT_PAPER),
                                                        FileIcon::None => icons::FMT_AUDIO.to_owned(),
                                                    };
                                                    let mobile_star_entry = mobile_entry.clone();
                                                    let mobile_star_toggle = toggle_star.clone();
                                                    let mobile_star_key_entry = mobile_entry.clone();
                                                    let mobile_star_key_toggle = toggle_star.clone();
                                                    let mobile_menu_entry = mobile_entry.clone();
                                                    let mobile_menu_key_entry = mobile_entry.clone();
                                                    view! {
                                                        <span class="mobile-track-card">
                                                            <span class="mobile-track-icon" inner_html=mobile_icon></span>
                                                            <span class="mobile-track-copy">
                                                                <span class="mobile-track-head">
                                                                    <span
                                                                        class="mobile-track-status"
                                                                        role="button"
                                                                        tabindex="0"
                                                                        aria-label="Open audio visualizer"
                                                                        on:click=move |ev: web_sys::MouseEvent| {
                                                                            if pane_current.get_untracked() == Some(index) && !stopped.get_untracked() {
                                                                                ev.stop_propagation();
                                                                                set_visualizer_spectrum_mode.set(false);
                                                                                set_visualizer_open.set(true);
                                                                            }
                                                                        }
                                                                        on:keydown=move |ev: web_sys::KeyboardEvent| {
                                                                            if ev.key() == "Enter" || ev.key() == " " {
                                                                                ev.prevent_default();
                                                                                ev.stop_propagation();
                                                                                if pane_current.get_untracked() == Some(index) && !stopped.get_untracked() {
                                                                                    set_visualizer_spectrum_mode.set(false);
                                                                                    set_visualizer_open.set(true);
                                                                                }
                                                                            }
                                                                        }
                                                                    >{move || if pane_current.get() == Some(index) && !stopped.get() { if playing.get() { "▶" } else { "Ⅱ" } } else { "" }}</span>
                                                                    <span class="mobile-track-title">
                                                                        {move || display_title(&metadata.read(), &metadata_failed.read(), &mobile_title_entry).unwrap_or_default()}
                                                                    </span>
                                                                </span>
                                                                <span class="mobile-track-detail">
                                                                    {move || {
                                                                        let cache = metadata.read();
                                                                        let meta = meta_for(&cache, &mobile_detail_entry);
                                                                        let artist = meta.as_ref().and_then(|row| row.artist.as_ref().or(row.album_artist.as_ref())).cloned().unwrap_or_default();
                                                                        let album = meta.as_ref().and_then(|row| row.album.as_ref()).cloned().unwrap_or_default();
                                                                        if artist.is_empty() { album }
                                                                        else if album.is_empty() { artist }
                                                                        else { format!("{artist} · {album}") }
                                                                    }}
                                                                </span>
                                                            </span>
                                                            <span class="mobile-track-duration">
                                                                {move || meta_for(&metadata.read(), &mobile_duration_entry).and_then(|row| row.duration).map(clock).unwrap_or_default()}
                                                            </span>
                                                            <span
                                                                class="mobile-track-star"
                                                                role="button"
                                                                tabindex="0"
                                                                aria-label="Toggle favorite"
                                                                on:click=move |ev: web_sys::MouseEvent| {
                                                                    ev.stop_propagation();
                                                                    let item = mobile_star_entry.clone();
                                                                    let starred = stars.get_untracked().contains(&entry_star_locator(&item));
                                                                    mobile_star_toggle(item, starred);
                                                                }
                                                                on:keydown=move |ev: web_sys::KeyboardEvent| {
                                                                    if ev.key() == "Enter" || ev.key() == " " {
                                                                        ev.prevent_default();
                                                                        ev.stop_propagation();
                                                                        let item = mobile_star_key_entry.clone();
                                                                        let starred = stars.get_untracked().contains(&entry_star_locator(&item));
                                                                        mobile_star_key_toggle(item, starred);
                                                                    }
                                                                }
                                                            >{move || if stars.read().contains(&entry_star_locator(&mobile_star_label_entry)) { "★" } else { "☆" }}</span>
                                                            <span
                                                                class="mobile-track-more"
                                                                role="button"
                                                                tabindex="0"
                                                                aria-label="Track actions"
                                                                on:click=move |ev: web_sys::MouseEvent| {
                                                                    ev.stop_propagation();
                                                                    select_row(index,false,false);
                                                                    set_song_menu.set(Some((ev.client_x() as f64, ev.client_y() as f64, index, mobile_menu_entry.clone())));
                                                                }
                                                                on:keydown=move |ev: web_sys::KeyboardEvent| {
                                                                    if ev.key() == "Enter" || ev.key() == " " {
                                                                        ev.prevent_default();
                                                                        ev.stop_propagation();
                                                                        select_row(index,false,false);
                                                                        set_song_menu.set(Some((0.0, 0.0, index, mobile_menu_key_entry.clone())));
                                                                    }
                                                                }
                                                            >"⋯"</span>
                                                        </span>
                                                    }
                                                }
                                            <span
                                                class="drag-grip"
                                                title="Drag to reorder"
                                                on:pointerdown=move |ev: web_sys::PointerEvent| {
                                                    if let Some(target) = ev.target().and_then(|t| t.dyn_into::<web_sys::Element>().ok()) {
                                                        let _ = target.set_pointer_capture(ev.pointer_id());
                                                    }
                                                    set_dragging_track.set(Some(index));
                                                    set_reorder_to.set(None);
                                                }
                                                on:pointermove=move |ev: web_sys::PointerEvent| {
                                                    if dragging_track.get_untracked() != Some(index) {
                                                        return;
                                                    }
                                                    set_reorder_to.set(track_drop_target(
                                                        ev.client_x(), ev.client_y(), pane_entries.get_untracked().len()));
                                                }
                                                on:pointerup=move |ev: web_sys::PointerEvent| {
                                                    if dragging_track.get_untracked() == Some(index)
                                                        && let Some(to) = reorder_to.get_untracked()
                                                    {
                                                        move_track(index, to);
                                                    }
                                                    set_dragging_track.set(None);
                                                    set_reorder_to.set(None);
                                                    clear_track_drop_marker();
                                                }
                                                on:pointercancel=move |_| {
                                                    set_dragging_track.set(None);
                                                    set_reorder_to.set(None);
                                                    clear_track_drop_marker();
                                                }
                                                on:click=move |ev: web_sys::MouseEvent| ev.stop_propagation()
                                            >"⠿"</span>
    </button>
                                        }
                                    }
                                </For>
                            </Show>
                        </div>
                    </main>
                </div>

                <nav class="mobile-tabs" aria-label="Main navigation">
                    <button
                        type="button"
                        class:active=move || mobile_view.get() == MobileView::Library
                        aria-current=move || if mobile_view.get() == MobileView::Library { "page" } else { "false" }
                        on:click=move |_| {
                            set_files_expanded.set(true);
                            set_mobile_view.set(MobileView::Library);
                        }
                    >
                        <span class="mobile-tab-icon" inner_html=icons::VIEW_LIST_TREE></span>
                        <span>"Library"</span>
                    </button>
                    <button
                        type="button"
                        class:active=move || mobile_view.get() == MobileView::Queue
                        aria-current=move || if mobile_view.get() == MobileView::Queue { "page" } else { "false" }
                        on:click=move |_| set_mobile_view.set(MobileView::Queue)
                    >
                        <span class="mobile-tab-icon" inner_html=icons::QUEUE></span>
                        <span>"Queue"</span>
                    </button>
                    <button
                        type="button"
                        class:active=move || mobile_view.get() == MobileView::Playlists
                        aria-current=move || if mobile_view.get() == MobileView::Playlists { "page" } else { "false" }
                        on:click=move |_| {
                            set_playlists_expanded.set(true);
                            set_mobile_view.set(MobileView::Playlists);
                        }
                    >
                        <span class="mobile-tab-icon" inner_html=icons::FMT_PLAYLIST></span>
                        <span>"Playlists"</span>
                    </button>
                </nav>

                <div class="player-scrim" on:click=move |_| set_transport_compact.set(true)></div>

                <footer class="transport">
                    <div
                        class="now"
                        on:click=move |_| {
                            if touch_mode && transport_compact.get_untracked() {
                                set_transport_compact.set(false);
                            }
                        }
                    >
                        <div
                            class="art"
                            title="Show album cover enlarged"
                            on:click=move |_| {
                                if touch_mode && transport_compact.get_untracked() {
                                    set_transport_compact.set(false);
                                } else {
                                    set_cover_open.set(true);
                                }
                            }
                        >
                            <img
                                src=move || art_src()
                                alt="Album cover"
                                on:error=move |event| {
                                    if let Some(img) = event
                                        .target()
                                        .and_then(|target| target.dyn_into::<web_sys::HtmlImageElement>().ok())
                                    {
                                        if !img.src().ends_with("/icons/cover-placeholder.svg") {
                                            let _ = img.set_src("/icons/cover-placeholder.svg");
                                        }
                                    }
                                }
                            />
                        </div>
                        <div class="now-text">
                            <div class="title">{move || now_title()}</div>
                            <div class="subtitle">{move || now_subtitle()}</div>
                            <div class="mobile-now-subtitle">{move || now_mobile_subtitle()}</div>
                        </div>
                    </div>

                    <div class="transport-center">
                        <div class="controls">
                            <button
                                class="toggle shuffle"
                                class:active=move || shuffle.get() != ShuffleMode::Off
                                title=move || shuffle_tip()
                                disabled=move || queue.get().len() <= 1
                                on:click=move |_| select_shuffle(shuffle.get_untracked().next())
                                inner_html=icons::SHUFFLE
                            ></button>
                            <button
                                title="Previous"
                                disabled=move || queue.get().is_empty()
                                on:click=move |_| step(-1)
                                inner_html=icons::SKIP_BACKWARD
                            ></button>
                            <button
                                class="play"
                                title=move || if radio_waiting.get() { "Preparing next radio track — click to cancel" } else { "Play or pause" }
                                aria-busy=move || radio_waiting.get().to_string()
                                disabled=move || queue.get().is_empty() && !radio_on.get()
                                on:click=move |_| toggle_play()
                            >
                                <span
                                    class="glyph-icon"
                                    inner_html=move || if playing.get() || radio_waiting.get() { icons::PAUSE } else { icons::PLAY }
                                ></span>
                            </button>
                            <button
                                title="Stop"
                                disabled=move || queue.get().is_empty() && !radio_waiting.get()
                                on:click=move |_| {
                                    stop_playback();
                                }
                                inner_html=icons::STOP
                            ></button>
                            <button
                                title="Next"
                                disabled=move || queue.get().is_empty() && !radio_on.get()
                                on:click=move |_| step(1)
                                inner_html=icons::SKIP_FORWARD
                            ></button>
                            <button
                                class="toggle repeat"
                                class:active=move || repeat_mode.get() != Repeat::Off
                                title=move || repeat_tip()
                                disabled=move || queue.get().is_empty()
                                on:click=move |_| {
                                    select_repeat(repeat_mode.get_untracked().next());
                                }
                            >
                                <span class="glyph-icon" inner_html=icons::REPEAT></span>
                                <Show when=move || !repeat_badge().is_empty() fallback=|| ()>
                                    <span class="badge">{move || repeat_badge()}</span>
                                </Show>
                            </button>
                            <button
                                class="toggle radio"
                                class:active=move || radio_on.get()
                                title=move || {
                                    if radio_on.get() {
                                        "Random Radio on — click to turn off"
                                    } else {
                                        "Random Radio off — click to turn on"
                                    }
                                }
                                disabled=move || !connected.get()
                                on:click=move |_| {
                                    let next = !radio_on.get_untracked();
                                    set_radio(next);
                                }
                            >"⚄\u{FE0E}"</button>
                        </div>
                        <div class="seek-row">
                            <span class="time elapsed">{move || clock(position.get())}</span>
                            <input
                                class="seek"
                                type="range"
                                min="0"
                                max=move || {
                                    let value = duration.get();
                                    if value.is_finite() && value > 0.0 { value } else { 1.0 }
                                }
                                step="0.5"
                                prop:value=move || position.get()
                                on:input=move |event| {
                                    let requested: f64 =
                                        event_target_value(&event).parse().unwrap_or(0.0);
                                    // The element's own end beats the tag fallback:
                                    // a wrong tag must never send a seek past the
                                    // real audio.
                                    let mut end = duration.get_untracked();
                                    if let Some(audio) = audio_ref.get() {
                                        if let Some(real) = finite_duration(audio.duration()) {
                                            end = real;
                                        } else {
                                            let seekable = audio.seekable();
                                            if seekable.length() > 0 {
                                                if let Some(real) =
                                                    seekable.end(0).ok().and_then(finite_duration)
                                                {
                                                    end = real;
                                                }
                                            }
                                        }
                                    }
                                    let target = if end.is_finite() && end > 0.0 {
                                        requested.clamp(0.0, end)
                                    } else {
                                        requested
                                    };
                                    backend.send(SessionCommand::Seek {seconds:target});
                                }
                            />
                            <span class="time total">{move || clock(duration.get())}</span>
                        </div>
                    </div>

                    <div class="transport-right">
                        <div class="volume-row">
                            <button
                                class="flat clear-list"
                                title=if touch_mode { "Clear queue" } else { "Clear Play Queue" }
                                disabled=move || queue.get().is_empty()
                                on:click=move |_| clear_pane()
                            >
                                <span class="glyph-icon" inner_html=icons::CLEAR_LIST></span>
                            </button>
                            <button
                                class="flat mute"
                                title=move || if volume.get() <= 0.0 { "Unmute" } else { "Mute" }
                                on:click=toggle_mute
                            >
                                <span
                                    class="glyph-icon"
                                    inner_html=move || {
                                        if volume.get() <= 0.0 {
                                            icons::VOLUME_MUTED
                                        } else {
                                            icons::VOLUME
                                        }
                                    }
                                ></span>
                            </button>
                            <div class="volume-wrap">
                                <span
                                    class="volume-tip"
                                    style=move || {
                                        format!("left: {}%", (volume.get() * 100.0).round())
                                    }
                                >
                                    {move || format!("{}%", (volume.get() * 100.0).round())}
                                </span>
                                <input
                                    class="volume"
                                    type="range"
                                    min="0"
                                    max="1"
                                    step="0.01"
                                    title=move || {
                                        format!("Volume {}%", (volume.get() * 100.0).round())
                                    }
                                    prop:value=move || volume.get()
                                    on:input=move |event| {
                                        backend.send(SessionCommand::Volume {value:event_target_value(&event).parse().unwrap_or(0.9)})
                                    }
                                />
                            </div>
                        </div>
                        <div class="transport-status">
                            {move || {
                                let note = status_note.get();
                                if note.is_empty() {
                                    status_line()
                                } else {
                                    note
                                }
                            }}
                        </div>
                    </div>

                    <button
                        class="transport-expand"
                        title="Show playback controls"
                        aria-label="Show playback controls"
                        on:click=move |_| set_transport_compact.set(false)
                    >"⌃"</button>

                    <button
                        class="transport-collapse"
                        type="button"
                        title="Minimize player"
                        aria-label="Minimize player"
                        on:click=move |_| set_transport_compact.set(true)
                    >"⌄"</button>

                    <For each={move || output_token.get().into_iter().collect::<Vec<_>>()} key=|token| (token.incarnation,token.serial) let:request_token>
                        {
                            let pause_token=request_token.clone();let playing_token=request_token.clone();let progress_token=request_token.clone();
                            let failure_token=request_token.clone();let ended_token=request_token.clone();
                            let ready_token=request_token.clone();let canplay_token=request_token.clone();
                            let ready=resume_when_ready.clone();let canplay=resume_when_ready.clone();let source_changing=source_changing.clone();
                            view! { <audio class="audio" node_ref=audio_ref preload="auto" data-output=request_token.serial.to_string()
                                on:pause=move |_| {
                                    if !source_changing.get_untracked() && session_model.with_value(|s|s.snapshot().output_token==Some(&pause_token)) {
                                        if let Some(audio)=audio_ref.get() {if audio.paused() && !audio.ended() {backend.send(SessionCommand::Pause);}}
                                    }
                                }
                                on:playing=move |_| {
                                    if session_model.with_value(|s|s.snapshot().output_token==Some(&playing_token)) {
                                        if session_model.with_value(|s|s.snapshot().transport==Transport::Paused) {backend.send(SessionCommand::Resume);}
                                        backend.send(SessionCommand::Output {token:playing_token.clone(),event:OutputEvent::Started});
                                    }
                                }
                                on:timeupdate=move |_| {if let Some(audio)=audio_ref.get(){backend.send(SessionCommand::Output {token:progress_token.clone(),event:OutputEvent::Progress {seconds:audio.current_time(),duration:duration.get_untracked()}});}}
                                on:loadedmetadata=move |event|{if session_model.with_value(|s|s.snapshot().output_token==Some(&ready_token)){ready(event);}}
                                on:durationchange=refresh_media_duration
                                on:canplay=move |event|{if session_model.with_value(|s|s.snapshot().output_token==Some(&canplay_token)){canplay(event);}}
                                on:error=move |_|backend.send(SessionCommand::Output {token:failure_token.clone(),event:OutputEvent::Failed {error:"The browser could not play this stream".into()}})
                                on:ended=move |_|backend.send(SessionCommand::Output {token:ended_token.clone(),event:OutputEvent::Ended})
                            ></audio> }
                        }
                    </For>
                </footer>

                <Show when=move || visualizer_open.get() fallback=|| ()>
                    <div class="scrim" on:click=move |_| set_visualizer_open.set(false)></div>
                    <section class="audio-visualizer" role="dialog" aria-modal="true" aria-label="Audio visualizer">
                        <header class="audio-visualizer-header">
                            <h2>"Audio visualizer"</h2>
                            <button type="button" aria-label="Close visualizer" on:click=move |_| set_visualizer_open.set(false)>"×"</button>
                        </header>
                        <div class="audio-visualizer-modes" role="group" aria-label="Visualization mode">
                            <button
                                type="button"
                                class:active=move || !visualizer_spectrum_mode.get()
                                aria-pressed=move || (!visualizer_spectrum_mode.get()).to_string()
                                on:click=move |_| set_visualizer_spectrum_mode.set(false)
                            >"Waveform"</button>
                            <button
                                type="button"
                                class:active=move || visualizer_spectrum_mode.get()
                                aria-pressed=move || visualizer_spectrum_mode.get().to_string()
                                on:click=move |_| set_visualizer_spectrum_mode.set(true)
                            >"Spectrum"</button>
                        </div>
                        <canvas
                            node_ref=visualizer_canvas
                            width="720"
                            height="360"
                            role="img"
                            aria-label="Live audio visualization"
                        ></canvas>
                        <p class="audio-visualizer-title">{move || now_title()}</p>
                    </section>
                </Show>

                <Show when=move || add_url_open.get() fallback=|| ()>
                    <div class="scrim" on:click=move |_| set_add_url_open.set(false)></div>
                    <div class="settings add-url-dialog" role="dialog" aria-label="Add stream URL">
                        <h2>"Add Stream URL"</h2>
                        <label>
                            "URL"
                            <input
                                type="url"
                                placeholder="https://example.com/stream"
                                prop:value=move || add_url_text.get()
                                on:input=move |event| set_add_url_text.set(event_target_value(&event))
                                on:keydown=move |event: web_sys::KeyboardEvent| {
                                    if event.key() == "Enter" {
                                        event.prevent_default();
                                        add_url();
                                    }
                                }
                            />
                        </label>
                        <div class="settings-actions">
                            <button on:click=move |_| set_add_url_open.set(false)>"Cancel"</button>
                            <button
                                class="primary"
                                disabled=move || add_url_text.get().trim().is_empty()
                                on:click=move |_| add_url()
                            >"Add to Queue"</button>
                        </div>
                    </div>
                </Show>

                <Show when=move || settings_open.get() fallback=|| ()>
                    <div class="scrim" on:click=move |_| set_settings_open.set(false)></div>
                    <div class="settings" role="dialog">
                        <h2>"Server"</h2>
                        <label class="mobile-stream-format">
                            "Stream format"
                            <select
                                prop:value=move || codec.get()
                                on:change=move |event| {
                                    let value = event_target_value(&event);
                                            set_codec.set(value);
                                }
                            >
                                <option value="aac">"AAC"</option>
                                <option value="opus">"Opus"</option>
                                <option value="flac">"FLAC"</option>
                            </select>
                        </label>
                        <label>
                            "Address"
                            <input
                                placeholder="https://my-desktop:8420"
                                prop:value=move || server.get()
                                on:input=move |event| set_server.set(event_target_value(&event))
                            />
                        </label>
                        <label>
                            <input
                                type="checkbox"
                                prop:checked=move || use_basic.get()
                                on:change=move |event| set_use_basic.set(event_target_checked(&event))
                            />
                            "Use a username and password"
                        </label>
                        <Show
                            when=move || use_basic.get()
                            fallback=move || view! {
                                <label>
                                    "API token"
                                    <input
                                        type="password"
                                        prop:value=move || token.get()
                                        on:input=move |event| set_token.set(event_target_value(&event))
                                    />
                                </label>
                            }
                        >
                            <label>
                                "Username"
                                <input
                                    prop:value=move || user.get()
                                    on:input=move |event| set_user.set(event_target_value(&event))
                                />
                            </label>
                            <label>
                                "Password"
                                <input
                                    type="password"
                                    prop:value=move || password.get()
                                    on:input=move |event| set_password.set(event_target_value(&event))
                                />
                            </label>
                        </Show>
                        <Show when=move || connected.get() fallback=|| ()>
                            <label>
                                "MIDI synth"
                                <select
                                    prop:value=move || {
                                        midi_options.track();
                                        midi_engine.get()
                                    }
                                    on:change=move |event| {
                                        let value = event_target_value(&event);
                                        let header = auth().header();
                                        let url = format!("{}/api/settings/midi", base());
                                        leptos::task::spawn_local(async move {
                                            match post_json(url, header, serde_json::json!({ "engine": value })).await {
                                                Ok(reply) => {
                                                    set_midi_engine.set(reply["engine"].as_str().unwrap_or_default().to_owned());
                                                    set_message.set(String::new());
                                                }
                                                Err(error) => set_message.set(error),
                                            }
                                        });
                                    }
                                >
                                    <For each=move || midi_options.get() key=|option| option.0.clone() let:option>
                                        <option value={option.0.clone()}>{option.1.clone()}</option>
                                    </For>
                                </select>
                            </label>
                            <p class="hint">"Changing this reloads the current MIDI track. The SF2 and ROM engines need the assets the desktop's Preferences sets."</p>
                        </Show>
                        <label class="check">
                            <input
                                type="checkbox"
                                prop:checked=move || track_notifications.get()
                                on:change=move |event| {
                                    let checked = event
                                        .target()
                                        .and_then(|target| target.dyn_into::<web_sys::HtmlInputElement>().ok())
                                        .map(|input| input.checked())
                                        .unwrap_or(false);
                                    set_track_notifications_pref(
                                        checked,
                                        set_track_notifications,
                                        set_message,
                                    );
                                }
                            />
                            "Show a notification when the next song plays"
                        </label>
                        <p class="hint">
                            "The address and token are shown in Kog's Preferences → Server on the machine serving the library."
                        </p>
                        <div class="settings-actions">
                            <button class="primary" on:click=move |_| connect()>
                                {move || if connected.get() { "Reconnect" } else { "Connect" }}
                            </button>
                            <button on:click=move |_| set_settings_open.set(false)>"Close"</button>
                        </div>
                        <Show when=move || !message.get().is_empty() fallback=|| ()>
                            <p class="hint error">{move || message.get()}</p>
                        </Show>
                    </div>
                </Show>

                <Show when=move || about_open.get() fallback=|| ()>
                    <div class="scrim" on:click=move |_| set_about_open.set(false)></div>
                    <div class="settings about" role="dialog">
                        <h2>"About Kog"</h2>
                        <p class="hint">"Kog web player, served by the local Kog server."</p>
                        <p class="hint">
                            {move || if version.get().is_empty() {
                                "Server version unavailable".to_owned()
                            } else {
                                format!("Server version {}", version.get())
                            }}
                        </p>
                        <div class="settings-actions">
                            <button class="primary" on:click=move |_| set_about_open.set(false)>"Close"</button>
                        </div>
                    </div>
                </Show>

                <Show when=move || cover_open.get() fallback=|| ()>
                    {/* The desktop's cover dialog: the artwork enlarged, titled
                        with the track, subtitied with artist and album; click
                        anywhere or Escape closes. */}
                    <div class="scrim" on:click=move |_| set_cover_open.set(false)></div>
                    <div class="cover-dialog" role="dialog">
                        <img
                            class="cover-image"
                            src=move || art_src()
                            alt="Album cover enlarged"
                            on:click=move |_| set_cover_open.set(false)
                            on:error=move |event| {
                                if let Some(img) = event
                                    .target()
                                    .and_then(|target| target.dyn_into::<web_sys::HtmlImageElement>().ok())
                                {
                                    if !img.src().ends_with("/icons/cover-placeholder.svg") {
                                        let _ = img.set_src("/icons/cover-placeholder.svg");
                                    }
                                }
                            }
                        />
                        <p class="cover-title">{move || now_title()}</p>
                        <p class="cover-subtitle">{move || now_subtitle()}</p>
                    </div>
                </Show>

                <Show when=move || picker_open.get() fallback=|| ()>
                    <div class="scrim" on:click=move |_| set_picker_open.set(false)></div>
                    <div class="settings folder-picker" role="dialog">
                        <h2>"Choose a Folder"</h2>
                        <p class="folder-picker-path">{move || picker_dir.get()}</p>
                        <div class="folder-picker-list">
                            <Show
                                when=move || {
                                    let dir = picker_dir.get();
                                    !dir.is_empty()
                                        && dir != library_root.get_untracked()
                                        && !parent_path(&dir).is_empty()
                                }
                                fallback=|| ()
                            >
                                <button
                                    class="folder-picker-row parent"
                                    on:click=move |_| set_picker_dir.set(parent_path(&picker_dir.get()))
                                >
                                    <span class="tree-icon up" inner_html=icons::GO_UP></span>
                                    ".."
                                </button>
                            </Show>
                            <For
                                each=move || picker_entries.get()
                                key=|entry| entry.1.clone()
                                let:entry
                            >
                                {
                                    let name = entry.0.clone();
                                    let browse_path = entry.1.clone();
                                    let root_path = entry.1.clone();
                                    view! {
                                        <div class="folder-picker-row">
                                            <button
                                                class="folder-picker-open"
                                                title="Browse this folder"
                                                on:click=move |_| set_picker_dir.set(browse_path.clone())
                                            >
                                                <span class="tree-icon dir"></span>
                                                {name.clone()}
                                            </button>
                                            <button
                                                class="icon-button folder-picker-root"
                                                title={format!("Use {name} as Tree Root")}
                                                on:click=move |_| root_at_folder(root_path.clone())
                                                inner_html=icons::FOLDER_OPEN
                                            ></button>
                                        </div>
                                    }
                                }
                            </For>
                        </div>
                        <div class="settings-actions">
                            <button on:click=move |_| set_picker_open.set(false)>"Cancel"</button>
                            <button class="primary" on:click=use_picked_folder>"Use as Tree Root"</button>
                        </div>
                    </div>
                </Show>

                <Show when=move || tree_menu.get().is_some() fallback=|| ()>
                    {
                        let row_is_dir = move || {
                            tree_menu.get().map(|(_, _, row)| row.is_dir).unwrap_or(false)
                        };
                        view! {
                            <div class="menu-scrim" on:click=move |_| set_tree_menu.set(None)></div>
                            <div
                                class="context-menu"
                                style=move || match tree_menu.get() {
                                    Some((x, y, _)) => format!("left:{x}px; top:{y}px;"),
                                    None => String::new(),
                                }
                            >
                                <button
                                    class="menu-item"
                                    disabled=move || !row_is_dir()
                                    on:click={
                                        let goto_root = goto_root.clone();
                                        move |_| {
                                            if let Some((_, _, row)) = tree_menu.get() {
                                                goto_root(row.path.clone());
                                            }
                                            set_tree_menu.set(None);
                                        }
                                    }
                                >
                                    "Use as Tree Root"
                                </button>
                                <button
                                    class="menu-item"
                                    disabled=move || tree_root.get().is_empty()
                                    on:click={
                                        let go_up = go_up.clone();
                                        move |_| {
                                            go_up();
                                            set_tree_menu.set(None);
                                        }
                                    }
                                >
                                    "Go Up"
                                </button>
                                <div class="menu-separator"></div>
                                <button
                                    class="menu-item"
                                    on:click={
                                        let add_row_to_playlist = add_row_to_playlist.clone();
                                        move |_| {
                                            if let Some((_, _, row)) = tree_menu.get() {
                                                add_row_to_playlist(row);
                                                if touch_mode { set_mobile_view.set(MobileView::Queue); }
                                            }
                                            set_tree_menu.set(None);
                                        }
                                    }
                                >
                                    {if touch_mode { "Add to Queue" } else { "Add to Playlist" }}
                                </button>
                                <Show
                                    when=move || {
                                        tree_menu.get()
                                            .map(|(_, _, row)| {
                                                !row.is_dir && row.kind != "remote"
                                            })
                                            .unwrap_or(false)
                                    }
                                    fallback=|| ()
                                >
                                    <button
                                        class="menu-item"
                                        on:click=move |_| {
                                            if let Some((_, _, row)) = tree_menu.get() {
                                                set_tree_menu.set(None);
                                                let url = format!(
                                                    "{}/api/media/download?kind={}&path={}&token={}",
                                                    base(),
                                                    url_encode(&row.kind),
                                                    url_encode(&row.path),
                                                    url_encode(&token.get()),
                                                );
                                                trigger_browser_download(&url, "");
                                            }
                                        }
                                    >
                                        "Download"
                                    </button>
                                </Show>
                            </div>
                        }
                    }
                </Show>

                <Show when=move || song_menu.get().is_some() fallback=|| ()>
                    <div class="menu-scrim" on:click=move |_| set_song_menu.set(None)></div>
                    <div
                        class="context-menu"
                        style=move || match song_menu.get() {
                            Some((x, y, ..)) => format!("left:{x}px; top:{y}px;"),
                            None => String::new(),
                        }
                    >
                        <button
                            class="menu-item"
                            on:click={
                                let reveal_in_tree = reveal_in_tree.clone();
                                move |_| {
                                    if let Some((_, _, index, entry)) = song_menu.get_untracked() {
                                        set_song_menu.set(None);
                                        play_row(index);
                                    }
                                }
                            }
                        >
                            "Play"
                        </button>
                        <Show when=move || pane_key.get() == "queue">
                        <button class="menu-item" on:click=move |_| {
                            if let Some((_, _, index, _)) = song_menu.get_untracked() {
                                let indices = if selected.get_untracked().contains(&index) { let mut indices: Vec<_> = selected.get_untracked().into_iter().collect(); indices.sort_unstable(); indices } else { vec![index] };
                                backend.send(SessionCommand::ToggleQueued {indices});
                                set_policy_revision.update(|value| *value = value.wrapping_add(1));
                                set_song_menu.set(None);
                            }
                        }>"Toggle Play Next"</button>
                        <button class="menu-item" on:click=move |_| {
                            if let Some((_, _, index, _)) = song_menu.get_untracked() {
                                let indices = if selected.get_untracked().contains(&index) { let mut indices: Vec<_> = selected.get_untracked().into_iter().collect(); indices.sort_unstable(); indices } else { vec![index] };
                                backend.send(SessionCommand::ToggleStopAfter {indices});
                                set_policy_revision.update(|value| *value = value.wrapping_add(1));
                                set_song_menu.set(None);
                            }
                        }>"Toggle Stop After"</button>
                        </Show>
                        <Show when=move || pane_key.get() != "queue">
                            <button class="menu-item" disabled=move || !playlist_workspace.snapshot().actions.queue on:click=move |_| {
                                playlist_workspace.send(kog_playback_policy::workspace::Command::Queue { action: QueueAction::PlayNext });
                                set_song_menu.set(None);
                            }>"Play Next"</button>
                            <button class="menu-item" disabled=move || !playlist_workspace.snapshot().actions.queue on:click=move |_| {
                                playlist_workspace.send(kog_playback_policy::workspace::Command::Queue { action: QueueAction::AddToQueue });
                                set_song_menu.set(None);
                            }>"Add to Queue"</button>
                        </Show>
                        <button
                            class="menu-item"
                            on:click=move |_| {
                                if let Some((_, _, index, entry)) = song_menu.get_untracked() {
                                    set_track_details.set(Some((index, entry)));
                                    set_song_menu.set(None);
                                }
                            }
                        >"Track Details"</button>
                        <div class="menu-separator"></div>
                        <Show when=move || radio_on.get() fallback=|| ()>
                            <button
                                class="menu-item"
                                on:click=move |_| {
                                    set_song_menu.set(None);
                                    reshuffle_radio();
                                }
                            >
                                "Reshuffle Radio"
                            </button>
                        </Show>
                        <button
                            class="menu-item"
                            disabled=move || !playlist_workspace.snapshot().actions.remove
                            on:click=move |_| {
                                set_song_menu.set(None);
                                remove_selected();
                            }
                        >
                            "Remove Selected"
                        </button>
                        <button
                            class="menu-item"
                            on:click=move |_| {
                                set_song_menu.set(None);
                                select_all_results();
                            }
                        >
                            "Select All"
                        </button>
                        <button
                            class="menu-item"
                            disabled=move || !playlist_workspace.snapshot().actions.clear
                            on:click=move |_| {
                                set_song_menu.set(None);
                                playlist_workspace.send(kog_playback_policy::workspace::Command::Clear);
                            }
                        >
                            {move || if pane_key.get() == "queue" { "Clear Play Queue" } else { "Clear Playlist" }}
                        </button>
                        <div class="menu-separator"></div>
                        <button
                            class="menu-item"
                            on:click={
                                let reveal_in_tree = reveal_in_tree.clone();
                                move |_| {
                                    if let Some((_, _, _, entry)) = song_menu.get_untracked() {
                                        set_song_menu.set(None);
                                        reveal_in_tree(entry);
                                        if touch_mode {
                                            set_mobile_view.set(MobileView::Library);
                                        }
                                    }
                                }
                            }
                        >
                            "Show in File Tree"
                        </button>
                        <Show
                            when=move || {
                                song_menu
                                    .get_untracked()
                                    .map(|(_, _, _, entry)| entry.kind != "remote")
                                    .unwrap_or(false)
                            }
                            fallback=|| ()
                        >
                            <button
                                class="menu-item"
                                on:click=move |_| {
                                    if let Some((_, _, _, entry)) = song_menu.get_untracked() {
                                        set_song_menu.set(None);
                                        let member = if entry.kind == "archive" {
                                            last_segment(&entry.entry)
                                        } else {
                                            last_segment(&entry.path)
                                        };
                                        let url = format!(
                                            "{}/api/media/download?kind={}&path={}&entry={}&token={}",
                                            base(),
                                            url_encode(&entry.kind),
                                            url_encode(&entry.path),
                                            url_encode(&entry.entry),
                                            url_encode(&token.get()),
                                        );
                                        trigger_browser_download(&url, "");
                                    }
                                }
                            >
                                "Download"
                            </button>
                        </Show>
                    </div>
                </Show>

                <Show when=move || playlist_menu.get().is_some() fallback=|| ()>
                    <div class="menu-scrim" on:click=move |_| set_playlist_menu.set(None)></div>
                    <div
                        class="context-menu"
                        style=move || match playlist_menu.get() {
                            Some((x, y, ..)) => format!("left:{x}px; top:{y}px;"),
                            None => String::new(),
                        }
                    >
                        {/* The desktop's playlist context menu, in its order. */}
                        <button
                            class="menu-item"
                            on:click=move |_| {
                                if let Some((_, _, id, _)) = playlist_menu.get_untracked() {
                                    set_playlist_menu.set(None);
                                    append_playlist(id);
                                    if touch_mode { set_mobile_view.set(MobileView::Queue); }
                                }
                            }
                        >
                            {if touch_mode { "Add to Queue" } else { "Add to Pane" }}
                        </button>
                        <button
                            class="menu-item"
                            on:click=move |_| {
                                if let Some((_, _, id, _)) = playlist_menu.get_untracked() {
                                    set_playlist_menu.set(None);
                                    play_playlist(id);
                                    if touch_mode { set_mobile_view.set(MobileView::Queue); }
                                }
                            }
                        >
                            "Play"
                        </button>
                        <button
                            class="menu-item"
                            on:click=move |_| {
                                if let Some((_, _, id, name)) = playlist_menu.get_untracked() {
                                    set_playlist_menu.set(None);
                                    replace_pane_with_playlist(id, name);
                                    if touch_mode { set_mobile_view.set(MobileView::Queue); }
                                }
                            }
                        >
                            {if touch_mode { "Replace Queue" } else { "Replace Pane" }}
                        </button>
                        <Show
                            when=move || {
                                playlist_menu
                                    .get()
                                    .map(|(_, _, id, _)| id > 0)
                                    .unwrap_or(false)
                            }
                            fallback=|| ()
                        >
                            <button
                                class="menu-item"
                                on:click=move |_| {
                                    if let Some((_, _, id, _)) = playlist_menu.get_untracked() {
                                        set_playlist_menu.set(None);
                                        prune_playlist_missing(id);
                                    }
                                }
                            >
                                "Remove Missing Files"
                            </button>
                        </Show>
                        <div class="menu-separator"></div>
                        <Show
                            when=move || {
                                playlist_menu
                                    .get()
                                    .map(|(_, _, id, _)| id > 0)
                                    .unwrap_or(false)
                            }
                            fallback=|| ()
                        >
                            <button
                                class="menu-item"
                                on:click=move |_| {
                                    if let Some((_, _, id, name)) = playlist_menu.get_untracked() {
                                        set_playlist_menu.set(None);
                                        set_rename_text.set(name);
                                        set_renaming_playlist.set(Some(id));
                                    }
                                }
                            >
                                "Rename"
                            </button>
                            <button
                                class="menu-item"
                                on:click=move |_| {
                                    if let Some((_, _, id, name)) = playlist_menu.get_untracked() {
                                        set_playlist_menu.set(None);
                                        set_playlist_dialog_opened.update(|epoch| *epoch += 1);
                                        set_playlist_dialog.set(Some(PlaylistDialog {
                                            mode: PlaylistDialogMode::Duplicate,
                                            title: "Duplicate Playlist",
                                            value: format!("{name} copy"),
                                            id,
                                            name,
                                        }));
                                    }
                                }
                            >
                                "Duplicate"
                            </button>
                            <button
                                class="menu-item"
                                on:click=move |_| {
                                    if let Some((_, _, id, name)) = playlist_menu.get_untracked() {
                                        set_playlist_menu.set(None);
                                        export_playlist_m3u(id, name);
                                    }
                                }
                            >
                                "Export as m3u…"
                            </button>
                            <div class="menu-separator"></div>
                            <button
                                class="menu-item"
                                on:click=move |_| {
                                    if let Some((_, _, id, name)) = playlist_menu.get_untracked() {
                                        set_playlist_menu.set(None);
                                        set_playlist_dialog_opened.update(|epoch| *epoch += 1);
                                        set_playlist_dialog.set(Some(PlaylistDialog {
                                            mode: PlaylistDialogMode::ConfirmDelete,
                                            title: "Delete Playlists",
                                            value: String::new(),
                                            id,
                                            name,
                                        }));
                                    }
                                }
                            >
                                "Delete…"
                            </button>
                        </Show>
                    </div>
                </Show>

                <Show when=move || playlist_dialog.get().is_some() fallback=|| ()>
                    <div class="scrim" on:click=move |_| set_playlist_dialog.set(None)></div>
                    <div class="settings playlist-dialog" role="dialog">
                        <h2>{move || {
                            playlist_dialog.get().map(|dialog| dialog.title.to_owned())
                                .unwrap_or_default()
                        }}</h2>
                        <p class="hint">
                            {move || match playlist_dialog.get() {
                                Some(dialog) => match dialog.mode {
                                    PlaylistDialogMode::CreateFromPane { selected_only } => {
                                        let name = dialog.value.trim();
                                        if !name.is_empty()
                                            && playlist_name_exists(name.to_owned())
                                        {
                                            format!(
                                                "A playlist named \u{201c}{name}\u{201d} already exists. Saving will replace its tracks."
                                            )
                                        } else {
                                            match selected_only {
                                                Some(true) => "Name the new playlist. Only selected tracks are saved.",
                                                Some(false) => "Name the new playlist. The entire play queue is saved.",
                                                None => "Name the new playlist. The selection, or the full queue when nothing is selected, is saved.",
                                            }.to_owned()
                                        }
                                    }
                                    PlaylistDialogMode::Duplicate => {
                                        "Name the copy.".to_owned()
                                    }
                                    PlaylistDialogMode::ConfirmDelete => {
                                        "Delete this playlist? Its songs stay in your library."
                                            .to_owned()
                                    }
                                },
                                None => String::new(),
                            }}
                        </p>
                        {/* Name field and confirm label stay mounted: recreating
                            the input per keystroke re-fired focus and re-selected
                            the text, so typing overwrote itself. */}
                        <input
                            class="playlist-dialog-name"
                            class:hidden=confirm_delete_mode
                            node_ref=playlist_dialog_input
                            prop:value=move || {
                                playlist_dialog.get().map(|d| d.value).unwrap_or_default()
                            }
                            on:input=move |event| {
                                set_playlist_dialog.update(|dialog| {
                                    if let Some(dialog) = dialog {
                                        dialog.value = event_target_value(&event);
                                    }
                                });
                            }
                            on:keydown=move |event: web_sys::KeyboardEvent| {
                                if event.key() == "Enter" {
                                    accept_playlist_dialog();
                                }
                            }
                        />
                        <p
                            class="playlist-dialog-confirm"
                            class:hidden=move || !confirm_delete_mode()
                        >
                            {move || {
                                playlist_dialog.get().map(|d| d.name).unwrap_or_default()
                            }}
                        </p>
                        <div class="settings-actions">
                            <button on:click=move |_| set_playlist_dialog.set(None)>"Cancel"</button>
                            <button
                                class="primary"
                                disabled=move || {
                                    playlist_dialog
                                        .get()
                                        .map(|dialog| {
                                            dialog.mode != PlaylistDialogMode::ConfirmDelete
                                                && dialog.value.trim().is_empty()
                                        })
                                        .unwrap_or(true)
                                }
                                on:click=move |_| accept_playlist_dialog()
                            >
                                {move || {
                                    playlist_dialog
                                        .get()
                                        .map(|dialog| match dialog.mode {
                                            PlaylistDialogMode::CreateFromPane { .. } => {
                                                let name = dialog.value.trim();
                                                if !name.is_empty()
                                                    && playlist_name_exists(name.to_owned())
                                                {
                                                    "Overwrite"
                                                } else {
                                                    "Save"
                                                }
                                            }
                                            PlaylistDialogMode::Duplicate => "Duplicate",
                                            PlaylistDialogMode::ConfirmDelete => "Delete",
                                        })
                                        .unwrap_or("OK")
                                        .to_owned()
                                }}
                            </button>
                        </div>
                    </div>
                </Show>

                <Show when=move || column_menu.get().is_some() fallback=|| ()>
                    <div class="menu-scrim" on:click=move |_| set_column_menu.set(None)></div>
                    <div
                        class="context-menu column-menu"
                        style=move || match column_menu.get() {
                            Some((x, y, _)) => format!("left:{x}px; top:{y}px;"),
                            None => String::new(),
                        }
                    >
                        <button
                            class="menu-item"
                            disabled=move || !can_move_column(-1)
                            on:click=move |_| move_column(-1)
                        >
                            "Move Column Left"
                        </button>
                        <button
                            class="menu-item"
                            disabled=move || !can_move_column(1)
                            on:click=move |_| move_column(1)
                        >
                            "Move Column Right"
                        </button>
                        <button
                            class="menu-item"
                            on:click={
                                let auto_fit_column = auto_fit_column.clone();
                                move |_| {
                                    auto_fit_column(menu_column());
                                    set_column_menu.set(None);
                                }
                            }
                        >
                            "Auto-Fit Column"
                        </button>
                        <button
                            class="menu-item"
                            on:click=move |_| {
                                auto_fit_all();
                                set_column_menu.set(None);
                            }
                        >
                            "Auto-Fit All Columns"
                        </button>
                        <div class="menu-separator"></div>
                        <For each=move || ColumnId::MENU_ORDER key=|id| id.key() let:id>
                            {
                                let visible = move || {
                                    columns
                                        .get()
                                        .iter()
                                        .any(|column| column.id == id && column.visible)
                                };
                                let only_visible = move || {
                                    let shown =
                                        columns.get().iter().filter(|column| column.visible).count();
                                    visible() && shown <= 1
                                };
                                view! {
                                    <button
                                        class="menu-item"
                                        disabled=only_visible
                                        on:click=move |_| toggle_column(id)
                                    >
                                        <span class="menu-check">
                                            {move || if visible() { "✓" } else { "" }}
                                        </span>
                                        {id.menu_label()}
                                    </button>
                                }
                            }
                        </For>
                        <div class="menu-separator"></div>
                        <button
                            class="menu-item"
                            on:click=move |_| {
                                reset_columns();
                                set_column_menu.set(None);
                            }
                        >
                            "Reset Columns"
                        </button>
                    </div>
                </Show>

                <Show when=move || mobile_sort_open.get() fallback=|| ()>
                    <div class="menu-scrim" on:click=move |_| set_mobile_sort_open.set(false)></div>
                    <div class="context-menu mobile-sort-menu" role="menu" aria-label="Sort queue">
                        <div class="menu-group">"Sort queue"</div>
                        <For
                            each=|| [
                                ColumnId::Index, ColumnId::Title, ColumnId::Artist,
                                ColumnId::AlbumArtist, ColumnId::Album, ColumnId::Composer,
                                ColumnId::Length, ColumnId::Year, ColumnId::Genre,
                                ColumnId::Track, ColumnId::FileSize, ColumnId::Path,
                                ColumnId::Filename, ColumnId::Codec, ColumnId::SampleRate,
                                ColumnId::BitsPerSample, ColumnId::Bitrate, ColumnId::Star,
                            ]
                            key=|id| id.key()
                            let:id
                        >
                            <button
                                class="menu-item"
                                on:click=move |_| {
                                    toggle_sort(id.sort_key());
                                    set_mobile_sort_open.set(false);
                                }
                            >
                                <span class="menu-check">{move || if pane_sort.get().0 == id.sort_key() { "✓" } else { "" }}</span>
                                {id.menu_label()}
                                <span class="mobile-sort-direction">{move || sort_arrow(id.sort_key())}</span>
                            </button>
                        </For>
                    </div>
                </Show>

                <Show when=move || track_details.get().is_some() fallback=|| ()>
                    <div class="scrim" on:click=move |_| set_track_details.set(None)></div>
                    <div class="settings track-details" role="dialog" aria-label="Track details">
                        <h2>"Track Details"</h2>
                        <dl class="track-details-list">
                            <For
                                each=move || {
                                    let Some((index, entry)) = track_details.get() else { return Vec::new(); };
                                    let cache = metadata.get();
                                    let meta = meta_for(&cache, &entry);
                                    ColumnId::MENU_ORDER.into_iter().filter_map(|id| {
                                        if matches!(id, ColumnId::Status | ColumnId::Rating | ColumnId::PlayCount) {
                                            return None;
                                        }
                                        let value = if id == ColumnId::Title {
                                            display_title(&cache, &metadata_failed.get(), &entry).unwrap_or_default()
                                        } else {
                                            column_text(id, index, &entry, meta.as_ref(), None, false, "")
                                        };
                                        (!value.trim().is_empty()).then(|| (id.menu_label().to_owned(), value))
                                    }).collect::<Vec<_>>()
                                }
                                key=|field| field.0.clone()
                                let:field
                            >
                                <div class="track-detail">
                                    <dt>{field.0}</dt>
                                    <dd>{field.1}</dd>
                                </div>
                            </For>
                        </dl>
                        <div class="settings-actions">
                            <button class="primary" on:click=move |_| set_track_details.set(None)>"Done"</button>
                        </div>
                    </div>
                </Show>

                <Show when=move || update_ready.get() fallback=|| ()>
                    <div class="update-banner">
                        <span>"A new Kog build is ready — reloading shortly."</span>
                        <button
                            class="update-reload"
                            on:click=move |_| {
                                if let Some(window) = web_sys::window() {
                                    let _ = window.location().reload();
                                }
                            }
                        >
                            "Reload now"
                        </button>
                    </div>
                </Show>

                <Show when=move || menu_open.get() fallback=|| ()>
                    <menu::Menu on_close=Callback::new(move |_| set_menu_open.set(false))>
                        <button class="menu-item" role="menuitem" on:click=move |_| open_add_url()>"Add URL…"</button>
                        <button class="menu-item" role="menuitem" on:click=move |_| open_preferences()>"Connect to Server…"</button>
                        <button class="menu-item" role="menuitem" disabled=move || !connected.get() on:click=move |event| {
                            set_menu_open.set(false); open_folder_picker(event);
                        }>"Choose Music Folder…"</button>
                        <div class="menu-separator" role="separator"></div>
                        <button class="menu-item" role="menuitem"
                            disabled=move || playlist_workspace.snapshot().active != "queue" || queue.get().is_empty()
                            on:click=move |_| { set_menu_open.set(false); open_save_playlist_dialog(false); }>"Save As…"</button>
                        <button class="menu-item" role="menuitem"
                            disabled=move || playlist_workspace.snapshot().active != "queue" || selected.get().is_empty()
                            on:click=move |_| { set_menu_open.set(false); open_save_playlist_dialog(true); }>"Save Selection As…"</button>
                        <div class="menu-separator" role="separator"></div>
                        <menu::Submenu label="Edit" depth=1>
                            <workspace::EditMenu controller=playlist_workspace on_action=Callback::new(move |_| set_menu_open.set(false)) on_select_all=Callback::new(move |_| select_all_results()) />
                        </menu::Submenu>
                        <menu::Submenu label="View" depth=1>
                            <button class="menu-item" role="menuitemcheckbox" aria-checked=move || sidebar_shown().to_string() on:click=move |_| {
                                if touch_mode {
                                    set_files_expanded.set(true);
                                    set_mobile_view.set(MobileView::Library);
                                } else { toggle_sidebar(); }
                                set_menu_open.set(false);
                            }>
                                <span class="menu-check" aria-hidden="true">{move || if sidebar_shown() { "✓" } else { "" }}</span>"Show File Tree"
                            </button>
                            <button class="menu-item" role="menuitem" disabled=move || pane_entries.get().is_empty()
                                on:click=move |_| {
                                    let index = pane_selected.get_untracked().iter().copied().min().unwrap_or_else(|| if pane_key.get_untracked() == "queue" { current.get_untracked() } else { 0 });
                                    if let Some(entry) = pane_entries.get_untracked().get(index).cloned() {
                                        set_track_details.set(Some((index, entry)));
                                    }
                                    set_menu_open.set(false);
                                }>"Show Info Inspector"</button>
                            <button class="menu-item" role="menuitem" on:click=move |_| {
                                set_menu_open.set(false); set_visualizer_open.set(true);
                            }>"Visualizer"</button>
                        </menu::Submenu>
                        <menu::Submenu label="Playback" depth=1>
                            <button class="menu-item" role="menuitem" disabled=move || queue.get().is_empty() && !radio_on.get()
                                on:click=move |_| { toggle_play(); set_menu_open.set(false); }>"Play/Pause"</button>
                            <button class="menu-item" role="menuitem" disabled=move || queue.get().is_empty() && !radio_waiting.get()
                                on:click=move |_| { stop_playback(); set_menu_open.set(false); }>"Stop"</button>
                            <div class="menu-separator" role="separator"></div>
                            <button class="menu-item" role="menuitem" disabled=move || queue.get().is_empty()
                                on:click=move |_| { step(-1); set_menu_open.set(false); }>"Previous"</button>
                            <button class="menu-item" role="menuitem" disabled=move || queue.get().is_empty() && !radio_on.get()
                                on:click=move |_| { step(1); set_menu_open.set(false); }>"Next"</button>
                            <div class="menu-separator" role="separator"></div>
                            <menu::Submenu label="Shuffle" depth=2>
                                <button class="menu-item" role="menuitemradio" aria-checked=move || (shuffle.get() == ShuffleMode::Off).to_string()
                                    on:click=move |_| { select_shuffle(ShuffleMode::Off); set_menu_open.set(false); }>
                                    <span class="menu-check" aria-hidden="true">{move || if shuffle.get() == ShuffleMode::Off { "●" } else { "" }}</span>"Off"
                                </button>
                                <button class="menu-item" role="menuitemradio" aria-checked=move || (shuffle.get() == ShuffleMode::Albums).to_string()
                                    on:click=move |_| { select_shuffle(ShuffleMode::Albums); set_menu_open.set(false); }>
                                    <span class="menu-check" aria-hidden="true">{move || if shuffle.get() == ShuffleMode::Albums { "●" } else { "" }}</span>"Albums"
                                </button>
                                <button class="menu-item" role="menuitemradio" aria-checked=move || (shuffle.get() == ShuffleMode::All).to_string()
                                    on:click=move |_| { select_shuffle(ShuffleMode::All); set_menu_open.set(false); }>
                                    <span class="menu-check" aria-hidden="true">{move || if shuffle.get() == ShuffleMode::All { "●" } else { "" }}</span>"All Tracks"
                                </button>
                            </menu::Submenu>
                            <menu::Submenu label="Repeat" depth=2>
                                <button class="menu-item" role="menuitemradio" aria-checked=move || (repeat_mode.get() == Repeat::Off).to_string()
                                    on:click=move |_| { select_repeat(Repeat::Off); set_menu_open.set(false); }>
                                    <span class="menu-check" aria-hidden="true">{move || if repeat_mode.get() == Repeat::Off { "●" } else { "" }}</span>"Off"
                                </button>
                                <button class="menu-item" role="menuitemradio" aria-checked=move || (repeat_mode.get() == Repeat::One).to_string()
                                    on:click=move |_| { select_repeat(Repeat::One); set_menu_open.set(false); }>
                                    <span class="menu-check" aria-hidden="true">{move || if repeat_mode.get() == Repeat::One { "●" } else { "" }}</span>"One Track"
                                </button>
                                <button class="menu-item" role="menuitemradio" aria-checked=move || (repeat_mode.get() == Repeat::Album).to_string()
                                    on:click=move |_| { select_repeat(Repeat::Album); set_menu_open.set(false); }>
                                    <span class="menu-check" aria-hidden="true">{move || if repeat_mode.get() == Repeat::Album { "●" } else { "" }}</span>"Album"
                                </button>
                                <button class="menu-item" role="menuitemradio" aria-checked=move || (repeat_mode.get() == Repeat::All).to_string()
                                    on:click=move |_| { select_repeat(Repeat::All); set_menu_open.set(false); }>
                                    <span class="menu-check" aria-hidden="true">{move || if repeat_mode.get() == Repeat::All { "●" } else { "" }}</span>"All Tracks"
                                </button>
                            </menu::Submenu>
                            <button class="menu-item" role="menuitem" disabled=move || !radio_on.get()
                                on:click=move |_| { reshuffle_radio(); set_menu_open.set(false); }>"Reshuffle Radio"</button>
                            <div class="menu-separator" role="separator"></div>
                            <Show when=move || pane_key.get() == "queue">
                                <button class="menu-item" role="menuitem" disabled=move || selected.get().is_empty()
                                    on:click=move |_| {
                                        backend.send(SessionCommand::ToggleQueued { indices: selected.get_untracked().into_iter().collect() });
                                        set_menu_open.set(false);
                                    }>{move || {
                                        policy_revision.track();
                                        let indices: Vec<_> = selected.get().into_iter().collect();
                                        match session_model.with_value(|model| model.order().queue_selection_state(&indices)) {
                                            kog_playback_policy::SelectionState::All => "Remove from Queue",
                                            kog_playback_policy::SelectionState::Mixed => "Toggle Queue",
                                            _ => "Add to Queue",
                                        }
                                    }}</button>
                                <button class="menu-item" role="menuitem" disabled=move || selected.get().is_empty()
                                    on:click=move |_| {
                                        backend.send(SessionCommand::ToggleStopAfter { indices: selected.get_untracked().into_iter().collect() });
                                        set_menu_open.set(false);
                                    }>{move || {
                                        policy_revision.track();
                                        let indices: Vec<_> = selected.get().into_iter().collect();
                                        match session_model.with_value(|model| model.order().stop_after_selection_state(&indices)) {
                                            kog_playback_policy::SelectionState::All => "Clear Stop After",
                                            kog_playback_policy::SelectionState::Mixed => "Toggle Stop After",
                                            _ => "Stop After Selection",
                                        }
                                    }}</button>
                            </Show>
                            <button class="menu-item" role="menuitem" disabled=move || {
                                policy_revision.track(); session_model.with_value(|model| model.order().queue_count() == 0)
                            } on:click=move |_| { backend.send(SessionCommand::ClearQueued); set_menu_open.set(false); }>"Clear Queue"</button>
                            <div class="menu-separator" role="separator"></div>
                            <workspace::PlaybackMenu controller=playlist_workspace on_action=Callback::new(move |_| set_menu_open.set(false)) />
                            <button class="menu-item" role="menuitemcheckbox" aria-checked=move || radio_on.get().to_string()
                                disabled=move || !connected.get() on:click=move |_| {
                                    set_radio(!radio_on.get_untracked()); set_menu_open.set(false);
                                }>
                                <span class="menu-check" aria-hidden="true">{move || if radio_on.get() { "✓" } else { "" }}</span>"Random Radio"
                            </button>
                        </menu::Submenu>
                        <div class="menu-separator" role="separator"></div>
                        <button class="menu-item" role="menuitem" on:click=move |_| open_preferences()>"Preferences…"</button>
                        <button class="menu-item" role="menuitem" on:click=move |_| open_about()>"About Kog…"</button>
                    </menu::Menu>
                </Show>
            </div>
        }
}
