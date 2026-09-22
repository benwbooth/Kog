//! Kog's web player.
//!
//! A client, not a second server: with per-client streams the browser is the
//! player, so this app owns its queue and plays the API's stream URLs through
//! an `<audio>` element.
//!
//! The layout mirrors the desktop window: a 48px toolbar, a sidebar holding the
//! file tree and the playlists, the playlist pane with a column header, and a
//! 92px transport bar. The shell is a fixed-height grid, so only the pane
//! scrolls, never the page. Below 820px the sidebar becomes a drawer and the
//! rows go compact for phones.
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
use std::sync::Arc;

use gloo_net::http::Request;
use leptos::prelude::*;
use serde::{Deserialize, Serialize};
use wasm_bindgen::closure::Closure;
use wasm_bindgen::prelude::wasm_bindgen;
use wasm_bindgen::JsCast;

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
    CreateFromPane,
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
    duration: Option<f64>,
    sample_rate: Option<u32>,
    channels: Option<u16>,
    bits_per_sample: Option<u8>,
    codec: Option<String>,
    bitrate: Option<u32>,
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
    const ALL: [ColumnId; 20] = [
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
    const MENU_ORDER: [ColumnId; 19] = [
        ColumnId::Album,
        ColumnId::AlbumArtist,
        ColumnId::Artist,
        ColumnId::Bitrate,
        ColumnId::BitsPerSample,
        ColumnId::Codec,
        ColumnId::Composer,
        ColumnId::Filename,
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
            Self::Index => 54.0,
            Self::Star => 40.0,
            Self::Status => 38.0,
            Self::Rating => 78.0,
            Self::Title => 220.0,
            Self::AlbumArtist => 150.0,
            Self::Artist => 190.0,
            Self::Composer => 151.0,
            Self::Album => 220.0,
            Self::Length => 70.0,
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
            Self::Index => 28.0,
            Self::Star => 28.0,
            Self::Status => 38.0,
            Self::Rating => 48.0,
            Self::Title => 96.0,
            Self::AlbumArtist => 96.0,
            Self::Artist => 96.0,
            Self::Composer => 96.0,
            Self::Album => 96.0,
            Self::Length => 44.0,
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
            Self::Index | Self::Star | Self::Status | Self::Title | Self::Artist | Self::Album | Self::Length | Self::Track
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
    let mut widest = dom_text_width(".columns .col-sort", &head_text)
        .unwrap_or_else(|| {
            let head_font = styled_font(".columns .col-sort", "11px system-ui, sans-serif");
            context.set_font(&head_font);
            measured(&context, &head_text)
        })
        + 16.0
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
        ColumnId::Artist => meta.and_then(|meta| meta.artist.clone()).unwrap_or_default(),
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

/// Repeat policy, cycled by the transport's repeat toggle.
#[derive(Clone, Copy, PartialEq, Eq, Default)]
enum Repeat {
    #[default]
    Off,
    One,
    All,
}

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
        let b = [chunk[0], *chunk.get(1).unwrap_or(&0), *chunk.get(2).unwrap_or(&0)];
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
        let _ = web_sys::window().expect("window")
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

/// Everything the web player remembers across a reload, in one localStorage
/// value. Deliberately client-side: the desktop's session.json is shared by
/// every client, so a phone must never overwrite the desktop's pane.
#[derive(Default)]
struct RestoredSession {
    queue: Vec<Entry>,
    current: usize,
    list_name: String,
    tree_root: String,
    expanded: Vec<String>,
    volume: Option<f64>,
    shuffle: bool,
    repeat: Repeat,
    radio_on: bool,
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
    session.current = value["current"].as_u64().unwrap_or(0) as usize;
    // A remembered row that no longer exists falls back to the first row.
    if session.queue.is_empty() {
        session.current = 0;
    } else {
        session.current = session.current.min(session.queue.len() - 1);
    }
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
    session.shuffle = value["shuffle"].as_bool().unwrap_or(false);
    session.repeat = match value["repeat"].as_str() {
        Some("one") => Repeat::One,
        Some("all") => Repeat::All,
        _ => Repeat::Off,
    };
    session.radio_on = value["radioOn"].as_bool().unwrap_or(false);
    session
}

/// Encode a session the way `decode_session` reads it. Only the locator fields
/// of an entry are stored; the display name and location are derived on load.
fn encode_session(
    queue: &[Entry],
    current: usize,
    list_name: &str,
    tree_root: &str,
    expanded: &HashSet<String>,
    volume: f64,
    shuffle: bool,
    repeat: Repeat,
    radio_on: bool,
) -> String {
    let entries: Vec<serde_json::Value> = queue
        .iter()
        .map(|entry| {
            serde_json::json!({
                "kind": entry.kind,
                "path": entry.path,
                "entry": entry.entry,
                "fragment": entry.fragment,
            })
        })
        .collect();
    // A stable order keeps the value identical between saves, so an unchanged
    // session never rewrites localStorage.
    let mut expanded: Vec<&String> = expanded.iter().collect();
    expanded.sort();
    let repeat = match repeat {
        Repeat::Off => "off",
        Repeat::One => "one",
        Repeat::All => "all",
    };
    serde_json::json!({
        "queue": entries,
        "current": current,
        "listName": list_name,
        "treeRoot": tree_root,
        "expanded": expanded,
        "volume": volume,
        "shuffle": shuffle,
        "repeat": repeat,
        "radioOn": radio_on,
    })
    .to_string()
}

/// The session's writer. Writes are deduplicated against the last value, so
/// restoring the pane and the auto-refresh reload can never fight over the
/// stored value, and a change that does not alter the snapshot never rewrites
/// localStorage.
#[derive(Clone, Default)]
struct SessionPersist {
    last: Rc<RefCell<Option<String>>>,
}

impl SessionPersist {
    fn save(&self, snapshot: String) {
        let mut last = self.last.borrow_mut();
        if last.as_deref() == Some(snapshot.as_str()) {
            return;
        }
        *last = Some(snapshot.clone());
        store("kog.session", &snapshot);
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
        store("kog.track_notifications", "0");
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
            store("kog.track_notifications", "1");
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
            let smoothing = if target > self.smoothed[index] { 0.72 } else { 0.16 };
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
fn meta_for(cache: &HashMap<String, Option<MetaRow>>, entry: &Entry) -> Option<MetaRow> {
    cache.get(&meta_key(entry)).and_then(|row| row.clone())
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
/// A random queue position other than `current`, for shuffle playback.
fn random_other(current: usize, len: usize) -> usize {
    if len <= 1 {
        return current;
    }
    let mut candidate = current;
    for _ in 0..8 {
        let roll = (js_sys::Math::random() * len as f64).floor() as usize;
        candidate = roll.min(len - 1);
        if candidate != current {
            break;
        }
    }
    candidate
}

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
        items.extend(files.iter().map(|file| {
            let path = file["path"].as_str().unwrap_or_default().to_owned();
            let name = file["name"]
                .as_str()
                .map(str::to_owned)
                .unwrap_or_else(|| last_segment(&path));
            let where_ = file["relative"]
                .as_str()
                .map(str::to_owned)
                .unwrap_or_else(|| name.clone());
            // The server expands folder playlists into `kind`/`entry`/
            // `fragment` locators; older responses (and plain files) fall back
            // to a bare local entry.
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
        }));
    }
    items
}

/// The playable files in a library response.
fn library_files(value: &serde_json::Value) -> Vec<Entry> {
    library_entries(value)
        .into_iter()
        .filter(|entry| !entry.is_dir())
        .collect()
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
    // Rows the pane actions act on. A plain click selects a row (and plays it),
    // Ctrl/Cmd-click adds to the selection, matching the desktop's multi-select.
    let (selected, set_selected) = signal(HashSet::<usize>::new());
    // Live-reload: the content hash of the running `/kog_web.js`, and whether a
    // newer build has appeared. A change reloads on the next pause or idle.
    let (asset_etag, set_asset_etag) = signal(Option::<String>::None);
    let (update_ready, set_update_ready) = signal(false);
    // When the pending build first appeared. A deferred reload (music
    // playing) force-applies after a grace period, or a tab with hours of
    // playing never reaches the new build at all.
    let (update_since, set_update_since) = signal(Option::<f64>::None);
    // Desktop shows the sidebar inline; phones open it as a drawer. The
    // desktop choice is persisted, the drawer state is not.
    let (sidebar_open, set_sidebar_open) = signal(false);
    // Draggable tree-pane width (the splitter between the tree and the pane).
    let (sidebar_width, set_sidebar_width) = signal(
        load("kog.sidebar-width")
            .and_then(|value| value.trim().parse::<f64>().ok())
            .map(|value| value.clamp(180.0, 600.0))
            .unwrap_or(260.0),
    );
    let (resizing_sidebar, set_resizing_sidebar) = signal(false);
    let (sidebar_visible, set_sidebar_visible) =
        signal(load("kog.sidebar").map(|value| value != "0").unwrap_or(true));
    let (files_expanded, set_files_expanded) = signal(true);
    let (playlists_expanded, set_playlists_expanded) = signal(true);

    // Library tree, loaded one directory at a time as it is expanded.
    let (children, set_children) = signal(HashMap::<String, Vec<Entry>>::new());
    let (expanded, set_expanded) = signal(HashSet::<String>::new());
    let (library_root, set_library_root) = signal(String::new());
    // The directory the tree is rooted at: "" means the server's library root.
    let (tree_root, set_tree_root) = signal(String::new());
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
    let (search_children, set_search_children) =
        signal(HashMap::<String, Vec<Entry>>::new());
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
    let (playlist_menu, set_playlist_menu) =
        signal(Option::<(f64, f64, i64, String)>::None);
    let (renaming_playlist, set_renaming_playlist) = signal(Option::<i64>::None);
    let (rename_text, set_rename_text) = signal(String::new());
    let rename_input = NodeRef::<leptos::html::Input>::new();
    // The enlarged cover view, opened by clicking the transport art like
    // the desktop's cover dialog.
    let (cover_open, set_cover_open) = signal(false);
    // Modal prompts where the desktop opens dialogs: naming a new playlist
    // (the + button), naming a duplicate, and confirming a delete.
    let (playlist_dialog, set_playlist_dialog) =
        signal(Option::<PlaylistDialog>::None);
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
    // (column, pointer start x, width at pointer-down) while a divider drags.
    let (resizing, set_resizing) = signal(Option::<(ColumnId, f64, f64)>::None);

    let (playlists, set_playlists) = signal(Vec::<(i64, String, i64)>::new());
    // The remembered session (pane, tree, transport modes) is restored here,
    // before any signal is read by the view, so the first paint already shows
    // the saved pane. `restoring` suppresses persistence until the initial
    // restore has settled, so the empty defaults never overwrite the saved
    // session.
    let restored = load("kog.session")
        .map(|raw| decode_session(&raw))
        .unwrap_or_default();
    // While this is true the session effect ignores signal changes, so the
    // empty defaults never overwrite the saved session before the restore has
    // run.
    let restoring = Rc::new(RefCell::new(true));
    let persist = SessionPersist::default();
    let (queue, set_queue) = signal(restored.queue.clone());
    let (list_name, set_list_name) = signal(restored.list_name.clone());
    let (current, set_current) = signal(restored.current);
    let (playing, set_playing) = signal(false);
    // Band levels of the currently streaming track, for the playing row's
    // meter: five 0..1 values polled from the server while it decodes.
    let (audio_levels, set_audio_levels) = signal([0.0_f32; 5]);
    // Stopped is stricter than paused: nothing was played and nothing is held
    // mid-song. A fresh page load starts stopped, and Stop returns here, so
    // the current row shows no playing or paused glyph.
    let (stopped, set_stopped) = signal(true);
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
    // Random Radio: the server owns the shuffled round; the web shows its
    // window and plays it. `radio_busy` guards the async window top-up.
    let (radio_on, set_radio_on) = signal(restored.radio_on);
    let (radio_busy, set_radio_busy) = signal(false);
    // The desktop stages radio picks into a hidden buffer and moves one onto
    // the playlist at a time; the pool is that buffer. Only shifted tracks
    // become rows.
    let (radio_pool, set_radio_pool) = signal(Vec::<Entry>::new());
    // Tag cache keyed by locator. An `Rc` so reading it clones a pointer, not
    // the map, on every cell render.
    let (metadata, set_metadata) =
        signal_local(Rc::new(HashMap::<String, Option<MetaRow>>::new()));
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

    // One place that talks to the API, so every call carries the same auth and
    // reports the same failures.
    let get_json = move |route: String| {
        let url = format!("{}{route}", base());
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
                            let kind = if item["is_dir"].as_bool().unwrap_or(false) {
                                "dir".to_owned()
                            } else {
                                item["kind"]
                                    .as_str()
                                    .unwrap_or("local")
                                    .to_owned()
                            };
                            let member = item["entry"].as_str().unwrap_or_default();
                            rows.push(Entry {
                                name,
                                path,
                                kind,
                                entry: member.to_owned(),
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
                        archives_scanned: value["archives_scanned"]
                            .as_u64()
                            .unwrap_or(0),
                        unreadable: value["unreadable_archives"].as_u64().unwrap_or(0),
                        scanning_archives: value["scanning_archives"]
                            .as_bool()
                            .unwrap_or(false),
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
                match get_json(format!(
                    "/api/library/search?q={}",
                    url_encode(&trimmed)
                ))
                .await
                {
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

    // ---------------------------------------------------------------- radio
    // Random Radio is server-owned: the server shuffles the library (sharing
    // the desktop's radio-round.json) and returns a window of tracks. Like the
    // desktop, the window is a hidden staging buffer: the visible playlist
    // only ever grows by one shifted track at a time, appended at the end.
    // Toggling or reshuffling never disturbs the queue that is playing.
    let apply_radio = move |value: &serde_json::Value| {
        let enabled = value["enabled"].as_bool().unwrap_or(false);
        set_radio_on.set(enabled);
        if !enabled {
            set_radio_pool.set(Vec::new());
            return;
        }
        let entries = radio_entries(value);
        if entries.is_empty() {
            return;
        }
        // Radio is a playback mode: repeat and shuffle would fight the round,
        // just as the desktop forces repeat off when radio is enabled.
        set_repeat_mode.set(Repeat::Off);
        set_shuffle.set(false);
        set_list_name.set("Random Radio".to_owned());
        set_radio_pool.set(entries);
    };

    let load_radio = {
        let get_json = get_json;
        let apply_radio = apply_radio.clone();
        move || {
            leptos::task::spawn_local(async move {
                if let Ok(value) = get_json("/api/radio".to_owned()).await {
                    apply_radio(&value);
                }
            });
        }
    };

    let set_radio = {
        let apply_radio = apply_radio.clone();
        let radio_on = radio_on.clone();
        move |enabled: bool| {
            // Flip at once: building the first round can keep the server busy
            // for a long while on a huge library, and a toggle that waits for
            // that reads as broken. The response still lands here and wins.
            set_radio_on.set(enabled);
            let url = format!("{}/api/radio/enabled", base());
            let header = auth().header();
            let apply_radio = apply_radio.clone();
            leptos::task::spawn_local(async move {
                let body = serde_json::json!({ "enabled": enabled });
                match post_json(url, header, body).await {
                    Ok(value) => apply_radio(&value),
                    Err(error) => set_message.set(error),
                }
            });
        }
    };

    let reshuffle_radio = {
        let apply_radio = apply_radio.clone();
        move || {
            let url = format!("{}/api/radio/reshuffle", base());
            let header = auth().header();
            let apply_radio = apply_radio.clone();
            leptos::task::spawn_local(async move {
                match post_json(url, header, serde_json::json!({})).await {
                    Ok(value) => apply_radio(&value),
                    Err(error) => set_message.set(error),
                }
            });
        }
    };

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
                            let relative =
                                value["path"].as_str().unwrap_or_default().to_owned();
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
            let target = if parent.is_empty()
                || parent == library
                || !is_under(&parent, &library)
            {
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
                    set_picker_parent.set(
                        value["parent"]
                            .as_str()
                            .map(str::to_owned),
                    );
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
        let root = restored.tree_root.clone();
        let targets = restored.expanded.clone();
        let restore_flag = restoring.clone();
        let done = Rc::new(RefCell::new(false));
        Effect::new(move |_| {
            if *done.borrow() {
                return;
            }
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
            *restore_flag.borrow_mut() = false;
            *done.borrow_mut() = true;
        });
        // A stored root that the server will not browse must not leave the
        // session permanently unpersisted: after a grace period the restore is
        // declared finished so changes start being saved again.
        let restoring = restoring.clone();
        if let Some(window) = web_sys::window() {
            let callback = Closure::<dyn FnMut()>::new(move || {
                *restoring.borrow_mut() = false;
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
    // session, and `SessionPersist` drops writes that do not change the value.
    {
        let restoring = restoring.clone();
        let persist = persist.clone();
        Effect::new(move |_| {
            let queue = queue.get();
            let current = current.get();
            let list_name = list_name.get();
            let tree_root = tree_root.get();
            let expanded = expanded.get();
            let volume = volume.get();
            let shuffle = shuffle.get();
            let repeat = repeat_mode.get();
            let radio_on = radio_on.get();
            if *restoring.borrow() {
                return;
            }
            persist.save(encode_session(
                &queue,
                current,
                &list_name,
                &tree_root,
                &expanded,
                volume,
                shuffle,
                repeat,
                radio_on,
            ));
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

    // Move one queued row to another position, keeping the current track
    // playing and its index pointing at the same song.
    let move_track = move |from: usize, to: usize| {
        let total = queue.get_untracked().len();
        if from >= total {
            return;
        }
        let to = to.min(total);
        // A row dropped on itself or on its lower edge is a no-op.
        if to == from || to == from + 1 {
            return;
        }
        let insert_at = if to < from { to } else { to - 1 };
        let current_index = current.get_untracked();
        set_queue.update(|items| {
            let moved = items.remove(from);
            items.insert(insert_at.min(items.len()), moved);
        });
        let new_current = if from == current_index {
            insert_at
        } else {
            let adjusted = if from < current_index {
                current_index - 1
            } else {
                current_index
            };
            if insert_at <= adjusted {
                adjusted + 1
            } else {
                adjusted
            }
        };
        set_current.set(new_current.min(total - 1));
    };

    // The playlists header's "+", as the desktop sidebar offers.
    // The + button opens the desktop's Save Playlist dialog: the visitor
    // names it, and the pane (or just the selection) is saved into it.
    let open_create_playlist_dialog = move || {
        set_playlist_dialog_opened.update(|epoch| *epoch += 1);
        set_playlist_dialog.set(Some(PlaylistDialog {
            mode: PlaylistDialogMode::CreateFromPane,
            title: "Save Playlist",
            value: String::new(),
            id: 0,
            name: String::new(),
        }));
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
                        store("kog.columns", &layout);
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
        let load_dir = load_dir.clone();
        let load_playlists = load_playlists.clone();
        let load_radio = load_radio.clone();
        let load_stars = load_stars.clone();
        let load_midi = load_midi.clone();
        let load_shared_columns = load_shared_columns.clone();
        move || {
            let header = auth().header();
            // A protected endpoint: the version call is open to everyone, so it
            // cannot tell a missing token from a working connection — and with
            // token auth on, that silence left the tree quietly empty.
            let url = format!("{}/api/codecs", base());
            let basic = use_basic.get();
            store("kog.server", &server.get());
            if !basic {
                store("kog.token", &token.get());
            }
            let load_dir = load_dir.clone();
            let load_playlists = load_playlists.clone();
            let load_radio = load_radio.clone();
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
                                if let Some(version) =
                                    value["version"].as_str().map(str::to_owned)
                                {
                                    set_version.set(version);
                                }
                            }
                        }
                        load_dir(String::new(), None);
                        load_playlists();
                        load_radio();
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
                let _ = js_sys::eval(
                    "document.querySelectorAll('.track.reorder-above').forEach(t => t.classList.remove('reorder-above'))",
                );
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
                if let TouchPress::Pending { row, x, y, is_track, .. } = taken {
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
                        target.closest(".tree-row, .track, .col-head").ok().flatten()
                    });
                let Some(row) = row else {
                    cancel_touch_press(&press, &window, &|| cleanup_drag());
                    return;
                };
                cancel_touch_press(&press, &window, &|| cleanup_drag());
                let is_track = row.class_list().contains("track");
                let timer = window
                    .set_timeout_with_callback_and_timeout_and_arguments_0(
                        &timer_callback,
                        550,
                    )
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
            .add_event_listener_with_callback(
                "touchstart",
                on_touch_start.as_ref().unchecked_ref(),
            )
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
                    TouchPress::Pending { row, x, y, is_track, timer } => {
                        let moved = (touch.client_x() - x).abs() + (touch.client_y() - y).abs()
                            > 10;
                        if moved {
                            window.clear_timeout_with_handle(timer);
                        } else {
                            *press.borrow_mut() =
                                TouchPress::Pending { row, x, y, is_track, timer };
                        }
                    }
                    TouchPress::Armed { row, x, y } => {
                        event.prevent_default();
                        let moved = (touch.client_x() - x).abs() + (touch.client_y() - y).abs()
                            > 6;
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
                        // The same midpoint rule the mouse drag uses: drop
                        // above a row's centre line, with the marker drawn.
                        let y = touch.client_y();
                        let marker = js_sys::eval(&format!(
                            "(() => {{ const rows = [...document.querySelectorAll('.track')]; const y = {y}; let index = rows.length; for (let i = 0; i < rows.length; i++) {{ const r = rows[i].getBoundingClientRect(); if (y < r.top + r.height / 2) {{ index = i; break; }} }} rows.forEach(r => r.classList.remove('reorder-above')); if (index < rows.length) rows[index].classList.add('reorder-above'); return index; }})()",
                        ));
                        let to = marker.ok().and_then(|value| value.as_f64()).map(|v| v as usize);
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
                        let _ = js_sys::eval(
                            "document.querySelectorAll('.track.reorder-above').forEach(t => t.classList.remove('reorder-above'))",
                        );
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


    // Global keys, from anywhere on the page. Escape dismisses any open menu
    // or dialog; Ctrl/Cmd+A selects the whole pane like the desktop's
    // Select All — except inside a text field, where it keeps selecting text.
    let key_handle = window_event_listener(leptos::ev::keydown, move |ev: web_sys::KeyboardEvent| {
        if (ev.ctrl_key() || ev.meta_key())
            && !ev.alt_key()
            && ev.key().eq_ignore_ascii_case("a")
        {
            let in_text = ev
                .target()
                .and_then(|target| target.dyn_into::<web_sys::HtmlElement>().ok())
                .map(|element| {
                    let tag = element.tag_name().to_ascii_lowercase();
                    tag == "input" || tag == "textarea" || tag == "select"
                        || element.is_content_editable()
                })
                .unwrap_or(false);
            if !in_text && !queue.get_untracked().is_empty() {
                ev.prevent_default();
                let total = queue.get_untracked().len();
                set_selected.update(|set| {
                    set.clear();
                    for index in 0..total {
                        set.insert(index);
                    }
                });
            }
            return;
        }
        if ev.key() == "Escape" {
            set_cover_open.set(false);
            set_menu_open.set(false);
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
        if update_ready.get() && !playing.get() {
            if let Some(window) = web_sys::window() {
                let _ = window.location().reload();
            }
        }
    });

    let stream_url = move |entry: &Entry| {
        format!(
            "{}/api/stream?kind={}&path={}&entry={}&codec={}{}&device={}&token={}",
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
            url_encode(&device_id()),
            // The audio element cannot send the Authorization header.
            url_encode(&token.get()),
        )
    };

    let audio_ref = NodeRef::<leptos::html::Audio>::new();
    let current_entry = move || queue.get().get(current.get()).cloned();
    let audio_src = move || current_entry().map(|entry| stream_url(&entry)).unwrap_or_default();

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

    // Keep the element in step with the transport button, and roll on when a
    // track ends.
    Effect::new(move |_| {
        let index = current.get();
        let playing = playing.get();
        if let Some(audio) = audio_ref.get() {
            if playing && index < queue.get().len() {
                let _ = audio.play();
            } else {
                let _ = audio.pause();
            }
        }
    });

    // The element's src is written only when it actually changes. Assigning
    // even the same URL restarts the media load algorithm, and the URL is
    // derived from the whole queue — so an append (dragging a song in),
    // a reorder, or a removal would reload the element mid-song and restart
    // it from zero. Only a change of the row at `current`, or of the token
    // baked into the stream URL, may reload.
    let applied_src = Rc::new(std::cell::RefCell::new(String::new()));
    {
        let applied_src = applied_src.clone();
        Effect::new(move |_| {
            let desired = audio_src();
            if applied_src.borrow().as_str() == desired {
                return;
            }
            if let Some(audio) = audio_ref.get() {
                audio.set_src(&desired);
                *applied_src.borrow_mut() = desired;
            }
        });
    }

    // Starting a row sets `current` and `playing` together. The src effect
    // then rewrites the element's source, which aborts a `play()`
    // issued in the same tick and leaves the track paused. Re-issue play once
    // the new source is actually ready, so a row click always starts playback.
    // The transport button is unaffected: it does not change the source, so the
    // effect above is the only thing that runs.
    let resume_when_ready = {
        let refresh_media_duration = refresh_media_duration.clone();
        move |event: web_sys::Event| {
            refresh_media_duration(event);
            if playing.get_untracked() {
                if let Some(audio) = audio_ref.get() {
                    let _ = audio.play();
                }
            }
        }
    };

    Effect::new(move |_| {
        if let Some(audio) = audio_ref.get() {
            audio.set_volume(volume.get());
        }
    });

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
        let watchdog = Closure::<dyn FnMut()>::new(move || {
            let Some(audio) = audio_ref.get() else {
                return;
            };
            if !playing.get_untracked() || current.get_untracked() >= queue.get_untracked().len()
            {
                return;
            }
            if audio.error().is_some() {
                // Re-assigning the source restarts the load; the ready
                // handlers take over and resume playback from zero.
                let src = audio.src();
                let _ = audio.set_src(&src);
                return;
            }
            if audio.paused() && !audio.ended() {
                let _ = audio.play();
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

    // The playing row's level meter, tapping the audio element through the
    // Web Audio API (the desktop meters its local playback the same way).
    // The graph is built exactly once, synchronously inside the visitor's
    // first pointer gesture — the one moment a context legally starts
    // Running — and only when the stream shares the page's origin: a
    // cross-origin element routed through Web Audio outputs silence, and a
    // routed element can never be un-routed. Every connect is verified; if
    // one fails after routing, the source is wired straight to the
    // destination so audio keeps flowing while the meter idles.
    {
        let audio_ref = audio_ref.clone();
        let playing = playing.clone();
        let base = base.clone();
        let graph: Rc<
            RefCell<Option<(web_sys::AudioContext, web_sys::AnalyserNode)>>,
        > = Rc::new(RefCell::new(None));
        // Band analysis state, mirroring the desktop's per-stream state.
        let analysis = Rc::new(RefCell::new(BandState::default()));

        // The graph builder. Runs in-gesture; `Ok(())` once the element is
        // safely routed (metered or bypassed), `Err(reason)` only when the
        // element was never routed (direct playback continues).
        let build_graph = {
            let audio_ref = audio_ref.clone();
            let base = base.clone();
            let graph = graph.clone();
            Rc::new(move || -> Result<(), String> {
                // Only tap streams that share the page's origin; base()
                // builds every stream URL, so it is the origin to check.
                let page_origin = web_sys::window()
                    .and_then(|window| window.location().origin().ok())
                    .unwrap_or_default();
                let server_origin = web_sys::Url::new(&format!(
                    "{}/",
                    base().trim_end_matches('/')
                ))
                .ok()
                .map(|url| url.origin())
                .unwrap_or_default();
                if page_origin.is_empty() || server_origin != page_origin {
                    return Err("stream origin differs from the page".to_owned());
                }
                let Some(audio) = audio_ref.get() else {
                    return Err("no audio element".to_owned());
                };
                let context = web_sys::AudioContext::new()
                    .map_err(|error| format!("AudioContext: {error:?}"))?;
                let _ = context.resume();
                if context.state() != web_sys::AudioContextState::Running {
                    return Err("context did not start running".to_owned());
                }
                let analyser = context
                    .create_analyser()
                    .map_err(|error| format!("analyser: {error:?}"))?;
                analyser.set_fft_size(2048);
                // From here the element is routed: audio must flow through
                // the graph whatever happens next.
                let source = context.create_media_element_source(&audio).map_err(
                    |error| format!("media source: {error:?}"),
                )?;
                if source.connect_with_audio_node(&analyser).is_err()
                    || analyser
                        .connect_with_audio_node(&context.destination())
                        .is_err()
                {
                    // Meter path failed: bypass so playback survives.
                    let _ = source.connect_with_audio_node(&context.destination());
                }
                *graph.borrow_mut() = Some((context, analyser));
                Ok(())
            })
        };

        // Build on the first pointer press — before any click-driven
        // playback — and only once. Failures are retried on later presses
        // (the element is only routed on success or after a bypass).
        {
            let build_graph = build_graph.clone();
            let built = Rc::new(std::cell::Cell::new(false));
            let press = Closure::<dyn FnMut()>::new(move || {
                if built.get() {
                    return;
                }
                if build_graph().is_ok() {
                    built.set(true);
                }
            });
            let document = web_sys::window()
                .and_then(|window| window.document())
                .expect("document for the meter gesture");
            let _ = document.add_event_listener_with_callback_and_bool(
                "pointerdown",
                press.as_ref().unchecked_ref(),
                true,
            );
            press.forget();
        }

        let levels_poll = Closure::<dyn FnMut()>::new(move || {
            if !playing.get_untracked() {
                set_audio_levels.set([0.0; 5]);
                return;
            }
            let graph = graph.borrow();
            let Some((context, analyser)) = graph.as_ref() else {
                return;
            };
            let Some(audio) = audio_ref.get() else {
                return;
            };
            if audio.paused() || audio.muted() || audio.volume() <= 0.0 {
                set_audio_levels.set([0.0; 5]);
                return;
            }
            let _ = context.resume();
            let mut samples = vec![0.0_f32; analyser.fft_size() as usize];
            analyser.get_float_time_domain_data(&mut samples);
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

    // One batched tag lookup for everything the pane currently shows. The cache
    // is read untracked so a successful fetch does not immediately schedule the
    // same request again; a re-render of the rows is the only effect.
    Effect::new(move |_| {
        if !connected.get() {
            return;
        }
        let entries = queue.get();
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
                let Ok(value) = post_json(url, header, body).await else {
                    return;
                };
                let Ok(rows) = serde_json::from_value::<Vec<Option<MetaRow>>>(value) else {
                    return;
                };
                let updates: Vec<(String, Option<MetaRow>)> = keys
                    .into_iter()
                    .enumerate()
                    .map(|(index, key)| (key, rows.get(index).cloned().flatten()))
                    .collect();
                set_metadata.update(|map| {
                    let map = Rc::make_mut(map);
                    for (key, row) in updates {
                        map.insert(key, row);
                    }
                });
            });
        }
    });

    let jump = move |index: usize| {
        set_current.set(index);
        set_position.set(0.0);
        set_media_duration.set(None);
        set_stopped.set(false);
        set_playing.set(true);
    };

    // Album art for the transport thumbnail: the current track's embedded
    // or sibling cover through the server, falling back to the logo when a
    // track has none (the img swaps itself back on a load error, and the
    // src changes with the track so the fallback does not stick).
    let art_src = move || {
        match queue.with_untracked(|q| q.get(current.get()).cloned()) {
            Some(entry) if !entry.is_dir() => format!(
                "{}/api/art?kind={}&path={}&token={}",
                base(),
                url_encode(&entry.kind),
                url_encode(&entry.path),
                url_encode(&token.get()),
            ),
            _ => "/icons/kog.svg".to_owned(),
        }
    };

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
        let detail = [meta.as_ref().and_then(|meta| meta.artist.clone()),
            meta.as_ref().and_then(|meta| meta.album.clone())]
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
        current.track();
        if queue.get_untracked().is_empty() {
            return;
        }
        leptos::task::spawn_local(async move {
            for _ in 0..10 {
                sleep_ms(50).await;
                let scrolled = js_sys::eval(
                    "(() => { const row = document.querySelector('.track.current');\
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

    // The desktop's shift_radio_track: move one staged radio track to the end
    // of the playlist and play it. Skips picks already queued so a window
    // refetched after a reload never duplicates restored rows.
    let shift_radio = {
        let jump = jump.clone();
        move || -> bool {
            let mut pool = radio_pool.get_untracked();
            let Some(pos) = pool
                .iter()
                .position(|entry| {
                    !queue
                        .get_untracked()
                        .iter()
                        .any(|queued| entry_star_locator(queued) == entry_star_locator(entry))
                })
            else {
                set_radio_pool.set(pool);
                return false;
            };
            let entry = pool.remove(pos);
            set_radio_pool.set(pool);
            let index = queue.get_untracked().len();
            set_queue.update(|items| items.push(entry));
            jump(index);
            true
        }
    };

    // Radio advances one track at a time: shift from the staged pool, and
    // only when it runs dry ask the server for the next window. A barren
    // round reshuffles so radio never stalls silently.

    // Pull a fresh window in the background: building one takes the server a
    // while on a huge library, and pressing next must never wait on that.
    // Runs only when radio is on and the staged pool is running low.
    let refill_radio_pool = {
        let radio_busy = radio_busy.clone();
        let set_radio_pool = set_radio_pool.clone();
        let set_radio_busy = set_radio_busy.clone();
        let reshuffle_radio = reshuffle_radio.clone();
        move || {
            if radio_busy.get_untracked() {
                return;
            }
            set_radio_busy.set(true);
            let url = format!("{}/api/radio/advance", base());
            let header = auth().header();
            let set_radio_pool = set_radio_pool.clone();
            let set_radio_busy = set_radio_busy.clone();
            let reshuffle_radio = reshuffle_radio.clone();
            leptos::task::spawn_local(async move {
                let result = post_json(url, header, serde_json::json!({})).await;
                set_radio_busy.set(false);
                match result {
                    Ok(value) => {
                        let entries = radio_entries(&value);
                        if !entries.is_empty() {
                            set_radio_pool.set(entries);
                        } else if value["exhausted"].as_bool().unwrap_or(true) {
                            reshuffle_radio();
                        }
                    }
                    Err(_) => {}
                }
            });
        }
    };

    let advance_radio = {
        let shift_radio = shift_radio.clone();
        let reshuffle_radio = reshuffle_radio.clone();
        let refill_radio_pool = refill_radio_pool.clone();
        let radio_pool = radio_pool.clone();
        move || {
            // The staged pool serves the press at once; a fresh window is
            // fetched in the background once it starts running low.
            if shift_radio() {
                if radio_on.get() && radio_pool.get_untracked().len() < 30 {
                    refill_radio_pool();
                }
                return;
            }
            if radio_busy.get_untracked() {
                return;
            }
            set_radio_busy.set(true);
            let url = format!("{}/api/radio/advance", base());
            let header = auth().header();
            let shift_radio = shift_radio.clone();
            let reshuffle_radio = reshuffle_radio.clone();
            let set_radio_busy = set_radio_busy.clone();
            let set_radio_pool = set_radio_pool.clone();
            let refill_radio_pool = refill_radio_pool.clone();
            let radio_on = radio_on.clone();
            leptos::task::spawn_local(async move {
                let result = post_json(url, header, serde_json::json!({})).await;
                set_radio_busy.set(false);
                match result {
                    Ok(value) => {
                        let entries = radio_entries(&value);
                        if entries.is_empty() {
                            if value["exhausted"].as_bool().unwrap_or(true) {
                                reshuffle_radio();
                            }
                            return;
                        }
                        set_radio_pool.set(entries);
                        if !shift_radio() {
                            set_radio_pool.set(Vec::new());
                            reshuffle_radio();
                        }
                        if radio_on.get()
                            && radio_pool.get_untracked().len() < 30
                        {
                            refill_radio_pool();
                        }
                    }
                    Err(error) => set_message.set(error),
                }
            });
        }
    };

    // End-of-track advance: radio shifts in one staged track when the queue
    // runs out, shuffle picks any other row, otherwise step the queue and
    // wrap only when repeat is on.
    let advance_after_track = {
        let advance_radio = advance_radio.clone();
        move || -> Option<usize> {
            let len = queue.get().len();
            if len == 0 {
                return None;
            }
            if radio_on.get() && current.get() + 1 >= len {
                advance_radio();
                return None;
            }
            if shuffle.get() {
                return Some(random_other(current.get(), len));
            }
            if current.get() + 1 < len {
                return Some(current.get() + 1);
            }
            if repeat_mode.get() == Repeat::All {
                return Some(0);
            }
            None
        }
    };

    let step = {
        let advance_radio = advance_radio.clone();
        move |delta: i64| {
            let len = queue.get().len();
            if len == 0 {
                // Radio kickstart: with nothing queued, play/next shifts the
                // first staged track in and plays it, as the desktop does.
                if radio_on.get() && delta > 0 {
                    advance_radio();
                }
                return;
            }
            // Next at the end of a radio round pulls the following window
            // instead of stopping.
            if delta > 0
                && radio_on.get()
                && current.get() + 1 >= len
                && repeat_mode.get() != Repeat::All
                && !shuffle.get()
            {
                advance_radio();
                return;
            }
            let next = if shuffle.get() {
                random_other(current.get(), len)
            } else if delta < 0 {
                if current.get() == 0 {
                    if repeat_mode.get() == Repeat::All {
                        len - 1
                    } else {
                        0
                    }
                } else {
                    current.get() - 1
                }
            } else if current.get() + 1 < len {
                current.get() + 1
            } else if repeat_mode.get() == Repeat::All {
                0
            } else {
                current.get()
            };
            if next != current.get() {
                jump(next);
            }
        }
    };

    let toggle_mute = move |_| {
        if volume.get() > 0.0 {
            set_volume_before_mute.set(volume.get());
            set_volume.set(0.0);
        } else {
            let restored = volume_before_mute.get();
            set_volume.set(if restored > 0.0 { restored } else { 0.75 });
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

    // The visible rows carry their index into the queue, so filtering and
    // sorting never change what plays next.
    let view_rows = move || {
        // Desktop search semantics: the query is whitespace-separated words,
        // and a row matches when every word appears somewhere in its name or
        // tags — order-independent, so longer queries keep finding rows.
        let tokens: Vec<String> = filter
            .get()
            .split_whitespace()
            .map(|word| word.to_lowercase())
            .collect();
        let cache = metadata.get();
        let key = sort_key.get();
        let mut rows: Vec<(usize, Entry)> = queue
            .get()
            .into_iter()
            .enumerate()
            .filter(|(_, entry)| {
                if tokens.is_empty() {
                    return true;
                }
                let mut fields = vec![entry.name.to_lowercase(), entry.location.to_lowercase()];
                if let Some(meta) = meta_for(&cache, entry) {
                    fields.extend(
                        [meta.title, meta.artist, meta.album]
                            .into_iter()
                            .flatten()
                            .map(|value| value.to_lowercase()),
                    );
                }
                tokens.iter().all(|token| {
                    fields
                        .iter()
                        .any(|field| field.contains(token.as_str()))
                })
            })
            .collect();
        if key != SortKey::Index {
            let value = |entry: &Entry| -> String {
                let meta = meta_for(&cache, entry);
                match key {
                    SortKey::Index => String::new(),
                    SortKey::Star => {
                        if stars.get().contains(&entry_star_locator(entry)) {
                            "1".to_owned()
                        } else {
                            "0".to_owned()
                        }
                    }
                    // No server-side rating/status value to order by; keeps the
                    // header toggle from reordering the pane.
                    SortKey::Status | SortKey::Rating | SortKey::PlayCount => String::new(),
                    SortKey::Title => meta
                        .and_then(|meta| meta.title)
                        .unwrap_or_else(|| entry.name.clone()),
                    SortKey::AlbumArtist => meta
                        .and_then(|meta| meta.album_artist)
                        .unwrap_or_default(),
                    SortKey::Composer => meta.and_then(|meta| meta.composer).unwrap_or_default(),
                    SortKey::Artist => meta.and_then(|meta| meta.artist).unwrap_or_default(),
                    SortKey::Album => meta.and_then(|meta| meta.album).unwrap_or_default(),
                    SortKey::Genre => meta.and_then(|meta| meta.genre).unwrap_or_default(),
                    SortKey::Year => meta
                        .and_then(|meta| meta.year)
                        .map(|year| format!("{year:010}"))
                        .unwrap_or_default(),
                    SortKey::Length => meta
                        .and_then(|meta| meta.duration)
                        .map(|seconds| format!("{seconds:010.3}"))
                        .unwrap_or_default(),
                    SortKey::Track => meta
                        .and_then(|meta| meta.track_number)
                        .map(|number| format!("{number:06}"))
                        .unwrap_or_default(),
                    SortKey::Path => entry_path(entry),
                    SortKey::Filename => entry_filename(entry),
                    SortKey::Codec => meta.and_then(|meta| meta.codec).unwrap_or_default(),
                    SortKey::SampleRate => meta
                        .and_then(|meta| meta.sample_rate)
                        .map(|rate| format!("{rate:010}"))
                        .unwrap_or_default(),
                    SortKey::BitsPerSample => meta
                        .and_then(|meta| meta.bits_per_sample)
                        .map(|bits| format!("{bits:06}"))
                        .unwrap_or_default(),
                    SortKey::Bitrate => meta
                        .and_then(|meta| meta.bitrate)
                        .map(|bitrate| format!("{bitrate:010}"))
                        .unwrap_or_default(),
                }
            };
            rows.sort_by_key(|(_, entry)| value(entry).to_lowercase());
            if !sort_asc.get() {
                rows.reverse();
            }
        }
        rows
    };

    // The pane's status line, shared by the header and the transport: how many
    // tracks the pane shows (all of the queue, or the filter's matches) and
    // their total probed duration, in the desktop's footer spirit.
    let status_line = move || -> String {
        let total = queue.get().len();
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
        let cache = metadata.get();
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
            .map(|items| items.iter().filter(|item| !item.is_dir()).cloned().collect())
            .unwrap_or_default()
    };

    let toggle_sort = move |key: SortKey| {
        if sort_key.get() == key {
            set_sort_asc.update(|asc| *asc = !*asc);
        } else {
            set_sort_key.set(key);
            set_sort_asc.set(true);
        }
    };
    let sort_arrow = move |key: SortKey| -> &'static str {
        if sort_key.get() == key {
            if sort_asc.get() { " ▲" } else { " ▼" }
        } else {
            ""
        }
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
        store("kog.columns", &layout);
        // Share the layout so every client - and the desktop - agrees.
        let url = format!("{}/api/columns", base());
        let header = auth().header();
        leptos::task::spawn_local(async move {
            let _ = post_json(url, header, serde_json::json!({ "layout": layout })).await;
        });
    };

    // Append to the pane without ever touching the transport: the current song
    // keeps playing, exactly as the desktop tree's "Add to Playlist" does. The
    // same song may appear twice — adds always append, and the count that
    // landed is reported so the outcome is never a silent nothing.
    let append_entries = move |entries: Vec<Entry>| -> usize {
        if entries.is_empty() {
            return 0;
        }
        let mut added = 0_usize;
        set_queue.update(|items| {
            for entry in entries {
                items.push(entry);
                added += 1;
            }
        });
        added
    };

    // Opening a playlist appends its tracks to the pane - like the desktop's
    // tree, where adding a playlist never throws away what is queued.
    let append_playlist = {
        let get_json = get_json;
        let append_entries = append_entries;
        move |id: i64| {
            leptos::task::spawn_local(async move {
                match get_json(format!("/api/playlists/{id}")).await {
                    Ok(value) => {
                        let entries: Vec<Entry> = value["entries"]
                            .as_array()
                            .map(|items| items.iter().map(entry_from_json).collect())
                            .unwrap_or_default();
                        append_entries(entries);
                    }
                    Err(error) => set_message.set(error),
                }
            });
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
                    if let Err(error) =
                        post_json(url, header, serde_json::json!({ "name": name })).await
                    {
                        set_message.set(error);
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
                    _ => load_playlists(),
                }
            });
        }
    };

    // The desktop sidebar's Play: append the playlist's tracks and start
    // playing the first of them.
    let play_playlist = {
        let get_json = get_json;
        let jump = jump.clone();
        move |id: i64| {
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
                        let start = queue.get_untracked().len();
                        set_queue.update(|items| items.extend(entries));
                        jump(start);
                    }
                    Err(error) => set_message.set(error),
                }
            });
        }
    };

    // Replace Pane, the desktop's load_playlist_into_pane: clear the pane,
    // load the playlist, and play it from the top.
    let replace_pane_with_playlist = {
        let get_json = get_json;
        let jump = jump.clone();
        move |id: i64, name: String| {
            leptos::task::spawn_local(async move {
                match get_json(format!("/api/playlists/{id}")).await {
                    Ok(value) => {
                        let entries: Vec<Entry> = value["entries"]
                            .as_array()
                            .map(|items| items.iter().map(entry_from_json).collect())
                            .unwrap_or_default();
                        set_queue.set(entries);
                        set_selected.set(HashSet::new());
                        set_list_name.set(name);
                        jump(0);
                    }
                    Err(error) => set_message.set(error),
                }
            });
        }
    };

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
                            let duration = meta
                                .as_ref()
                                .and_then(|meta| meta.duration)
                                .unwrap_or(0.0) as i64;
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
                                let staged =
                                    web_sys::Url::create_object_url_with_blob(&blob);
                                match staged {
                                    Ok(url) => {
                                        trigger_browser_download(
                                            &url,
                                            &format!("{name}.m3u"),
                                        );
                                        set_status_note.set(format!(
                                            "Exported {} tracks to {}.m3u",
                                            entries.len(),
                                            name
                                        ));
                                    }
                                    Err(_) => {
                                        set_message.set(
                                            "could not stage the m3u file".to_owned(),
                                        )
                                    }
                                }
                            }
                            Err(_) => {
                                set_message.set("could not build the m3u file".to_owned())
                            }
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
        move |name: String| {
            let url = format!("{}/api/playlists", base());
            let header = auth().header();
            let selected: HashSet<usize> = selected.get_untracked();
            let entries: Vec<Entry> = queue
                .get_untracked()
                .into_iter()
                .enumerate()
                .filter(|(index, _)| selected.is_empty() || selected.contains(index))
                .map(|(_, entry)| entry)
                .collect();
            leptos::task::spawn_local(async move {
                match post_json(url, header.clone(), serde_json::json!({ "name": name }))
                    .await
                {
                    Ok(value) => {
                        let id = value["id"].as_i64().unwrap_or_default();
                        if !entries.is_empty() {
                            let payload = serde_json::json!({
                                "entries": entries
                                    .iter()
                                    .map(|entry| {
                                        serde_json::json!({
                                            "kind": entry.kind,
                                            "path": entry.path,
                                            "entry": entry.entry,
                                            "fragment": entry
                                                .fragment
                                                .clone()
                                                .unwrap_or_default(),
                                        })
                                    })
                                    .collect::<Vec<_>>(),
                            });
                            if let Err(error) = post_json(
                                format!("{}/api/playlists/{id}/entries", base()),
                                header,
                                payload,
                            )
                            .await
                            {
                                set_message.set(error);
                            }
                        }
                        set_status_note
                            .set(format!("Saved {name} as a new playlist"));
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
        move |id: i64, name: String| {
            let url = format!("{}/api/playlists/{id}/entries", base());
            let header = auth().header();
            let selected: HashSet<usize> = selected.get_untracked();
            let entries: Vec<Entry> = queue
                .get_untracked()
                .into_iter()
                .enumerate()
                .filter(|(index, _)| selected.is_empty() || selected.contains(index))
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
                        set_status_note
                            .set(format!("Overwrote {name} with {count} tracks"));
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
        let url = format!("{}/api/library/search/pause", base());
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
        let url = format!("{}/api/library/search/pause", base());
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
            PlaylistDialogMode::CreateFromPane => {
                if !name.is_empty() {
                    set_playlist_dialog.set(None);
                    if let Some(id) = existing_playlist_id(&name) {
                        overwrite_playlist_with_pane(id, name);
                    } else {
                        create_playlist_from_pane(name);
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
    // The application (`☰`) menu's playlist/queue actions. The web pane is the
    // queue, so Clear Playlist and Clear Queue both empty it: the desktop's
    // separate up-next queue has no web counterpart.
    let open_add_url = move || {
        let Some(window) = web_sys::window() else {
            return;
        };
        let Ok(Some(url)) = window.prompt_with_message("Stream URL") else {
            return;
        };
        let url = url.trim().to_owned();
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
        set_queue.update(|items| items.push(entry));
        set_menu_open.set(false);
    };

    // Save the current pane as a new server playlist, then append its entries,
    // the same two calls the desktop makes.
    let save_playlist_as = {
        let load_playlists = load_playlists.clone();
        move || {
            let entries = queue.get_untracked();
            if entries.is_empty() {
                set_message.set("There is nothing to save".to_owned());
                set_menu_open.set(false);
                return;
            }
            let Some(window) = web_sys::window() else {
                return;
            };
            let Ok(Some(name)) = window.prompt_with_message("Save playlist as") else {
                return;
            };
            let name = name.trim().to_owned();
            if name.is_empty() {
                return;
            }
            set_menu_open.set(false);
            let root = base();
            let header = auth().header();
            let load_playlists = load_playlists.clone();
            leptos::task::spawn_local(async move {
                let body = serde_json::json!({ "name": name.clone() });
                let id = match post_json(format!("{root}/api/playlists"), header.clone(), body).await
                {
                    Ok(value) => value["id"].as_i64(),
                    Err(error) => {
                        set_message.set(error);
                        return;
                    }
                };
                let Some(id) = id else {
                    return;
                };
                let body = serde_json::json!({
                    "entries": entries
                        .iter()
                        .map(|entry| serde_json::json!({
                            "kind": entry.kind,
                            "path": entry.path,
                            "entry": entry.entry,
                            "fragment": entry.fragment.clone().unwrap_or_default(),
                        }))
                        .collect::<Vec<_>>(),
                });
                match post_json(format!("{root}/api/playlists/{id}/entries"), header, body).await {
                    Ok(_) => {
                        set_list_name.set(name);
                        load_playlists();
                    }
                    Err(error) => set_message.set(error),
                }
            });
        }
    };

    // Remove the selected rows from the pane. A row is selected by clicking it;
    // there is no server call, the pane is client-owned like the desktop queue.
    let remove_selected = move || {
        let mut indices: Vec<usize> = selected.get_untracked().into_iter().collect();
        if indices.is_empty() {
            return;
        }
        indices.sort_unstable();
        let current_index = current.get_untracked();
        let removed_before_current = indices.iter().filter(|index| **index < current_index).count();
        set_queue.update(|items| {
            for index in indices.iter().rev() {
                if *index < items.len() {
                    items.remove(*index);
                }
            }
        });
        let len = queue.get_untracked().len();
        let next = if len == 0 {
            0
        } else {
            current_index.saturating_sub(removed_before_current).min(len - 1)
        };
        if next != current_index {
            set_current.set(next);
            set_position.set(0.0);
            set_media_duration.set(None);
        }
        set_selected.set(HashSet::new());
        set_menu_open.set(false);
    };

    // Clear Playlist / Clear Queue: the web pane is the queue, so both empty it.
    let clear_pane = move || {
        set_queue.set(Vec::new());
        set_current.set(0);
        set_position.set(0.0);
        set_media_duration.set(None);
        set_stopped.set(true);
        set_playing.set(false);
        set_selected.set(HashSet::new());
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

    // The dedicated sidebar toggle: on the desktop it hides the inline tree, on
    // a phone it drives the drawer.
    let toggle_sidebar = move || {
        let mobile = web_sys::window()
            .map(|window| {
                window.inner_width().ok().and_then(|width| width.as_f64()).unwrap_or(1024.0)
                    <= 820.0
            })
            .unwrap_or(false);
        let open = if mobile {
            sidebar_open.get_untracked()
        } else {
            sidebar_visible.get_untracked()
        };
        let next = !open;
        set_sidebar_visible.set(next);
        set_sidebar_open.set(next);
        store("kog.sidebar", if next { "1" } else { "0" });
    };
    let sidebar_shown = move || {
        let mobile = web_sys::window()
            .map(|window| {
                window.inner_width().ok().and_then(|width| width.as_f64()).unwrap_or(1024.0)
                    <= 820.0
            })
            .unwrap_or(false);
        if mobile {
            sidebar_open.get()
        } else {
            sidebar_visible.get()
        }
    };

    // Add one tree row: a file is appended directly, a folder contributes its
    // immediate files (fetched if that level was never expanded).
    let add_row_to_playlist = {
        let get_json = get_json;
        let queue_files = queue_files.clone();
        let append_entries = append_entries;
        // Mirror the desktop's add-path status so an add is never a silent
        // no-op from the visitor's point of view.
        let report_add = {
            let set_status_note = set_status_note.clone();
            move |added: usize| {
                set_status_note.set(match added {
                    0 => "No tracks added".to_owned(),
                    1 => "Added to playlist".to_owned(),
                    count => format!("Added {count} tracks to playlist"),
                });
                if let Some(window) = web_sys::window() {
                    let note = set_status_note.clone();
                    let clear =
                        Closure::<dyn FnMut()>::new(move || note.set(String::new()));
                    let _ = window.set_timeout_with_callback_and_timeout_and_arguments_0(
                        clear.as_ref().unchecked_ref(),
                        4000,
                    );
                    clear.forget();
                }
            }
        };
        move |row: TreeRow| {
            if !row.is_dir {
                // Match the whole locator, not just the path: a subsong
                // playlist lists several rows for one file. Rows the loaded
                // tree does not know (search results, a stale listing) still
                // append, built straight from the row.
                let known = queue_files(&row.parent).into_iter().find(|item| {
                    item.kind == row.kind
                        && item.path == row.path
                        && item.entry == row.entry
                        && item.fragment == row.fragment
                });
                let entry = known.unwrap_or_else(|| Entry {
                    kind: row.kind.clone(),
                    path: row.path.clone(),
                    entry: row.entry.clone(),
                    fragment: row.fragment.clone(),
                    name: row.name.clone(),
                    location: row.path.clone(),
                });
                report_add(append_entries(vec![entry]));
                return;
            }
            let route = format!("/api/library?path={}", url_encode(&row.path));
            let append_entries = append_entries;
            let report_add = report_add.clone();
            leptos::task::spawn_local(async move {
                if let Ok(value) = get_json(route).await {
                    report_add(append_entries(library_files(&value)));
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
        let interval_cell: Rc<std::cell::Cell<Option<i32>>> =
            Rc::new(std::cell::Cell::new(None));
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
                let _ = js_sys::eval(
                    "document.querySelectorAll('.track.reorder-above').forEach(t => t.classList.remove('reorder-above'))",
                );
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
                if let Some(from) = current {
                    // Reordering a queue row: highlight the insertion point
                    // under the cursor.
                    let client_y = ev.client_y();
                    let marker = js_sys::eval(&format!(
                        "(() => {{
                            const rows = document.querySelector('.rows');
                            const tracks = [...rows.querySelectorAll('.track')];
                            tracks.forEach(t => t.classList.remove('reorder-above'));
                            const y = {client_y};
                            const from = {from};
                            let index = tracks.length;
                            for (let i = 0; i < tracks.length; i++) {{
                                const r = tracks[i].getBoundingClientRect();
                                if (y < r.top + r.height / 2) {{
                                    index = i;
                                    if (i !== from && i !== from + 1)
                                        tracks[i].classList.add('reorder-above');
                                    break;
                                }}
                            }}
                            return index;
                        }})()"
                    ));
                    let marker_result = match marker {
                        Ok(value) => match value.as_f64() {
                            Some(index) => {
                                set_reorder_to.set(Some(index as usize));
                                String::new()
                            }
                            None => "eval returned non-number".to_owned(),
                        },
                        Err(error) => format!("eval threw: {error:?}"),
                    };
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
            Closure::<dyn FnMut(web_sys::DragEvent)>::new(move |_| {
                set_playlist_drop_active.set(false);
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
                let _ = js_sys::eval(
                    "document.querySelectorAll('.track.reorder-above').forEach(t => t.classList.remove('reorder-above'))",
                );
            })
        };
        let _ = rows.add_event_listener_with_callback("drop", on_drop.as_ref().unchecked_ref());
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
                if columns.get_untracked().iter().any(|column| column.id == id && column.visible) {
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
        match visible.iter().position(|candidate| *candidate == menu_column()) {
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

    // The transport shows the current row's tags, with the file name as the
    // title fallback and artist • album as the subtitle, like the desktop bar.
    let now_title = move || {
        let entry = current_entry();
        let meta = entry.as_ref().and_then(|entry| meta_for(&metadata.get(), entry));
        meta.and_then(|meta| meta.title)
            .or_else(|| entry.as_ref().map(|entry| entry.name.clone()))
            .unwrap_or_else(|| "Kog".to_owned())
    };
    let now_subtitle = move || {
        let entry = current_entry();
        let meta = entry.as_ref().and_then(|entry| meta_for(&metadata.get(), entry));
        let artist = meta.as_ref().and_then(|meta| meta.artist.clone()).unwrap_or_default();
        let album = meta.as_ref().and_then(|meta| meta.album.clone()).unwrap_or_default();
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
    let repeat_badge = move || match repeat_mode.get() {
        Repeat::Off => "",
        Repeat::One => "1",
        Repeat::All => "∞",
    };
    let repeat_tip = move || match repeat_mode.get() {
        Repeat::Off => "Repeat off — click for one track",
        Repeat::One => "Repeat one track — click for all",
        Repeat::All => "Repeat all tracks — click to turn off",
    };
    let shuffle_tip = move || {
        if shuffle.get() {
            "Shuffle on — click to turn off"
        } else {
            "Shuffle off — click to turn on"
        }
    };

    view! {
        <div
            class="app"
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
                <div class="search">
                    <span class="pill-icon" aria-hidden="true" inner_html=icons::FIND></span>
                    <input
                        type="search"
                        placeholder="Search playlist"
                        prop:value=move || filter.get()
                        on:input=move |event| set_filter.set(event_target_value(&event))
                    />
                    <Show when=move || !filter.get().is_empty() fallback=|| ()>
                        <button
                            class="flat search-clear"
                            title="Clear playlist search"
                            on:click=move |_| set_filter.set(String::new())
                        >"×"</button>
                    </Show>
                </div>
                <select
                    class="codec"
                    title="Stream format"
                    prop:value=move || codec.get()
                    on:change=move |event| {
                        let value = event_target_value(&event);
                        store("kog.codec", &value);
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
                    on:click=move |_| set_settings_open.update(|open| *open = !*open)
                >
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
                                            title=move || current_root()
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
                                                    title=move || format!("Go to {}", parent_path(&tree_root.get()))
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
                                                let row_dbl = row.clone();
                                                let row_menu = row.clone();
                                                let row_drag = row.clone();
                                                let row_name = row.name.clone();
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
                                                view! {
                                                    <button
                                                        class="tree-row"
                                                        class:directory=row.is_dir
                                                        class:selected=move || {
                                                            tree_selected.get() == selected
                                                        }
                                                        style=format!("padding-left: {indent}px")
                                                        title=row.path.clone()
                                                        draggable="true"
                                                        on:click=move |_| {
                                                            set_tree_selected.set(row_click.path.clone());
                                                            set_tree_selected_dir.set(row_click.is_dir);
                                                            if row_click.is_dir {
                                                                tree_toggle(row_click.path.clone());
                                                            }
                                                        }
                                                        on:dblclick=move |_| {
                                                            // The desktop's
                                                            // activate: a double
                                                            // click queues the
                                                            // row — a file
                                                            // queues itself, a
                                                            // folder queues its
                                                            // files. Expansion
                                                            // stays the single
                                                            // click's job (the
                                                            // pair of clicks in
                                                            // a double click
                                                            // opens and closes
                                                            // the folder, as on
                                                            // the desktop).
                                                            add_row_to_playlist(row_dbl.clone());
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
                                                        <span class=if row.is_dir {
                                                            "tree-icon dir"
                                                        } else {
                                                            "tree-icon file"
                                                        }></span>
                                                        <span class="label">
                                                            {move || {
                                                                highlight_label(
                                                                    row_name.clone(),
                                                                    tree_search.get(),
                                                                )
                                                            }}
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
                                            title="Double-click to add to the playlist, or drag it there"
                                            draggable="true"
                                            on:dragstart=move |ev: web_sys::DragEvent| {
                                                set_dragging_playlist.set(Some(0));
                                                if let Some(transfer) = ev.data_transfer() {
                                                    let _ = transfer.set_data("text/plain", "Favorites");
                                                    transfer.set_effect_allowed("copy");
                                                }
                                            }
                                            on:dragend=move |_| set_dragging_playlist.set(None)
                                            on:dblclick=move |_| append_playlist(0)
                                        >
                                            <span class="twisty"></span>
                                            <span class="favorite-star">"★"</span>
                                            <span class="label">"Favorites"</span>
                                        </button>
                                        }
                                    }
                                    // The key carries the name: a rename must
                                    // re-render the row's label, not reuse it.
                                    <For each=move || playlists.get() key=|item| format!("{}#{}", item.0, item.1) let:item>
                                        {
                                            let id = item.0;
                                            let count = item.2;
                                            let label = item.1.clone();
                                            let drag_id = id;
                                            let drag_label = label.clone();
                                            let menu_label = label.clone();
                                            view! {
                                                <button
                                                    class="tree-row playlist-row"
                                                    title="Double-click to add to the playlist, or drag it there"
                                                    draggable="true"
                                                    on:dragstart=move |ev: web_sys::DragEvent| {
                                                        set_dragging_playlist.set(Some(drag_id));
                                                        if let Some(transfer) = ev.data_transfer() {
                                                            let _ = transfer.set_data("text/plain", &drag_label);
                                                            transfer.set_effect_allowed("copy");
                                                        }
                                                    }
                                                    on:dragend=move |_| set_dragging_playlist.set(None)
                                                    on:dblclick=move |_| append_playlist(drag_id)
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
                    <div
                        class="rows"
                        id="playlist-rows"
                        class:drop-active=move || playlist_drop_active.get()
                        on:wheel=move |event: web_sys::WheelEvent| {
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
                                <p class="empty">
                                    {move || if connected.get() {
                                        "Pick a folder in the file tree, or a playlist."
                                    } else {
                                        "Open the server settings to connect."
                                    }}
                                </p>
                            }
                        >
                            <For
                                each=view_rows
                                key=|(index, entry)| format!("{index}:{}::{}", entry.path, entry.entry)
                                let:row
                            >
                                {
                                    let (index, entry) = row;
                                    let menu_entry = entry.clone();
                                    view! {
                                        <button
                                            class="track"
                                            draggable="true"
                                            style="position: relative"
                                            data-index=move || index.to_string()
                                            class:current=move || current.get() == index
                                            class:selected=move || selected.get().contains(&index)
                                            on:contextmenu=move |ev: web_sys::MouseEvent| {
                                                ev.prevent_default();
                                                set_selected.update(|set| {
                                                    set.clear();
                                                    set.insert(index);
                                                });
                                                set_song_menu.set(Some((
                                                    ev.client_x() as f64,
                                                    ev.client_y() as f64,
                                                    index,
                                                    menu_entry.clone(),
                                                )));
                                            }
                                            on:click=move |ev: web_sys::MouseEvent| {
                                                // A single click only selects,
                                                // like the desktop: a song starts
                                                // on double click.
                                                if ev.ctrl_key()
                                                    || ev.meta_key()
                                                    || ev.shift_key()
                                                {
                                                    set_selected.update(|set| {
                                                        if !set.remove(&index) {
                                                            set.insert(index);
                                                        }
                                                    });
                                                    return;
                                                }
                                                set_selected.set(HashSet::from([index]));
                                            }
                                            on:dblclick=move |ev: web_sys::MouseEvent| {
                                                ev.prevent_default();
                                                // The desktop's activate: the
                                                // playing row toggles pause, any
                                                // other row starts playing.
                                                if current.get_untracked() == index {
                                                    set_playing.update(|value| *value = !*value);
                                                    set_stopped.set(false);
                                                    return;
                                                }
                                                set_selected.set(HashSet::from([index]));
                                                set_current.set(index);
                                                set_position.set(0.0);
                                                set_media_duration.set(None);
                                                set_stopped.set(false);
                                                set_playing.set(true);
                                            }
                                        >
                                            {/* The playing row's level
                                                meter, the desktop's
                                                five-band waveform at the
                                                row's right edge. */}
                                            <Show when=move || current.get() == index fallback=|| ()>
                                                <span class="row-meter">
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
                                                                format!(
                                                                    "height: {}px",
                                                                    2.0 + 10.0 * level
                                                                )
                                                            }
                                                        ></span>
                                                    </For>
                                                </span>
                                            </Show>
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
                                                    let text = move || {
                                                        let meta = meta_for(&metadata.get(), &entry);
                                                        let live = if current.get() == index
                                                            && duration.get() > 0.0
                                                        {
                                                            Some(duration.get())
                                                        } else {
                                                            None
                                                        };
                                                        let starred = stars
                                                            .get()
                                                            .contains(&entry_star_locator(&entry));
                                                        let status = if current.get() == index {
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
                                                        column_text(
                                                            id,
                                                            index,
                                                            &entry,
                                                            meta.as_ref(),
                                                            live,
                                                            starred,
                                                            status,
                                                        )
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
                                                            highlight_label(raw, filter.get())
                                                        } else {
                                                            raw.into_any()
                                                        }
                                                    };
                                                    view! {
                                                        <span
                                                            class=format!("cell {}", id.class())
                                                            title=move || tip()
                                                            on:click=move |ev: web_sys::MouseEvent| {
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
                                                        >
                                                            {text}
                                                        </span>
                                                    }
                                                }
                                            </For>
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
                                                let client_y = ev.client_y();
                                                let marker = js_sys::eval(&format!(
                                                    "(() => {{ const rows = [...document.querySelectorAll('.track')]; const y = {client_y}; let index = rows.length; for (let i = 0; i < rows.length; i++) {{ const r = rows[i].getBoundingClientRect(); if (y < r.top + r.height / 2) {{ index = i; break; }} }} return index; }})()",
                                                ));
                                                if let Ok(value) = marker && let Some(index) = value.as_f64() {
                                                    set_reorder_to.set(Some(index as usize));
                                                }
                                            }
                                            on:pointerup=move |ev: web_sys::PointerEvent| {
                                                if dragging_track.get_untracked() == Some(index)
                                                    && let Some(to) = reorder_to.get_untracked()
                                                {
                                                    move_track(index, to);
                                                }
                                                set_dragging_track.set(None);
                                                set_reorder_to.set(None);
                                                let _ = js_sys::eval(
                                                    "document.querySelectorAll('.track.reorder-above').forEach(t => t.classList.remove('reorder-above'))",
                                                );
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

            <footer class="transport">
                <div class="now">
                    <div
                        class="art"
                        title="Show album cover enlarged"
                        on:click=move |_| set_cover_open.set(true)
                    >
                        <img
                            src=move || art_src()
                            alt="Album cover"
                            on:error=move |event| {
                                if let Some(img) = event
                                    .target()
                                    .and_then(|target| target.dyn_into::<web_sys::HtmlImageElement>().ok())
                                {
                                    let _ = img.set_src("/icons/kog.svg");
                                }
                            }
                        />
                    </div>
                    <div class="now-text">
                        <div class="title">{move || now_title()}</div>
                        <div class="subtitle">{move || now_subtitle()}</div>
                    </div>
                </div>

                <div class="transport-center">
                    <div class="controls">
                        <button
                            class="toggle shuffle"
                            class:active=move || shuffle.get()
                            title=move || shuffle_tip()
                            disabled=move || queue.get().len() <= 1
                            on:click=move |_| set_shuffle.update(|value| *value = !*value)
                            inner_html=icons::SHUFFLE
                        ></button>
                        <button
                            title="Previous"
                            disabled=move || queue.get().is_empty() || (!shuffle.get() && current.get() == 0 && repeat_mode.get() != Repeat::All)
                            on:click=move |_| step(-1)
                            inner_html=icons::SKIP_BACKWARD
                        ></button>
                        <button
                            class="play"
                            title="Play or pause"
                            disabled=move || queue.get().is_empty() && !radio_on.get()
                            on:click=move |_| {
                                if queue.get().is_empty() {
                                    // Radio kickstart: nothing queued yet.
                                    if radio_on.get() {
                                        advance_radio();
                                    }
                                } else {
                                    set_playing.update(|value| *value = !*value);
                                    set_stopped.set(false);
                                }
                            }
                        >
                            <span
                                class="glyph-icon"
                                inner_html=move || if playing.get() { icons::PAUSE } else { icons::PLAY }
                            ></span>
                        </button>
                        <button
                            title="Stop"
                            disabled=move || queue.get().is_empty()
                            on:click=move |_| {
                                set_playing.set(false);
                                set_stopped.set(true);
                                set_position.set(0.0);
                                if let Some(audio) = audio_ref.get() {
                                    let _ = audio.set_current_time(0.0);
                                }
                            }
                            inner_html=icons::STOP
                        ></button>
                        <button
                            title="Next"
                            disabled=move || {
                                (queue.get().is_empty() && !radio_on.get())
                                    || (current.get() + 1 >= queue.get().len()
                                        && repeat_mode.get() != Repeat::All
                                        && !shuffle.get()
                                        && !radio_on.get())
                            }
                            on:click=move |_| step(1)
                            inner_html=icons::SKIP_FORWARD
                        ></button>
                        <button
                            class="toggle repeat"
                            class:active=move || repeat_mode.get() != Repeat::Off
                            title=move || repeat_tip()
                            disabled=move || queue.get().is_empty()
                            on:click=move |_| {
                                set_repeat_mode.update(|mode| {
                                    *mode = match mode {
                                        Repeat::Off => Repeat::One,
                                        Repeat::One => Repeat::All,
                                        Repeat::All => Repeat::Off,
                                    };
                                });
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
                                if let Some(audio) = audio_ref.get() {
                                    audio.set_current_time(target);
                                }
                                set_position.set(target);
                            }
                        />
                        <span class="time total">{move || clock(duration.get())}</span>
                    </div>
                </div>

                <div class="transport-right">
                    <div class="volume-row">
                        <button
                            class="flat clear-list"
                            title="Clear Playlist"
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
                                    set_volume
                                        .set(event_target_value(&event).parse().unwrap_or(0.9))
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

                <audio
                    class="audio"
                    node_ref=audio_ref
                    preload="auto"
                    on:timeupdate=move |_| {
                        if let Some(audio) = audio_ref.get() {
                            set_position.set(audio.current_time());
                        }
                    }
                    on:loadedmetadata=resume_when_ready
                    on:durationchange=refresh_media_duration
                    on:canplay=resume_when_ready
                    on:ended=move |_| {
                        if repeat_mode.get() == Repeat::One {
                            if let Some(audio) = audio_ref.get() {
                                let _ = audio.set_current_time(0.0);
                                let _ = audio.play();
                            }
                            set_position.set(0.0);
                            set_stopped.set(false);
                            set_playing.set(true);
                        } else if let Some(next) = advance_after_track() {
                            jump(next);
                        } else {
                            // The queue ran out: the desktop calls this
                            // Stopped, not Paused.
                            set_stopped.set(true);
                            set_playing.set(false);
                        }
                    }
                ></audio>
            </footer>

            <Show when=move || settings_open.get() fallback=|| ()>
                <div class="scrim" on:click=move |_| set_settings_open.set(false)></div>
                <div class="settings" role="dialog">
                    <h2>"Server"</h2>
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
                        <p class="hint">"Used the next time a MIDI file streams. The SF2 and ROM engines need the assets the desktop's Preferences sets."</p>
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
                                let _ = img.set_src("/icons/kog.svg");
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
                                        }
                                        set_tree_menu.set(None);
                                    }
                                }
                            >
                                "Add to Playlist"
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
                                    jump(index);
                                }
                            }
                        }
                    >
                        "Play"
                    </button>
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
                            let total = queue.get_untracked().len();
                            set_selected.update(|set| {
                                set.clear();
                                for index in 0..total {
                                    set.insert(index);
                                }
                            });
                        }
                    >
                        "Select All"
                    </button>
                    <button
                        class="menu-item"
                        on:click=move |_| {
                            set_song_menu.set(None);
                            clear_pane();
                        }
                    >
                        "Clear Playlist"
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
                            }
                        }
                    >
                        "Add to Pane"
                    </button>
                    <button
                        class="menu-item"
                        on:click=move |_| {
                            if let Some((_, _, id, _)) = playlist_menu.get_untracked() {
                                set_playlist_menu.set(None);
                                play_playlist(id);
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
                            }
                        }
                    >
                        "Replace Pane"
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
                                PlaylistDialogMode::CreateFromPane => {
                                    let name = dialog.value.trim();
                                    if !name.is_empty()
                                        && playlist_name_exists(name.to_owned())
                                    {
                                        format!(
                                            "A playlist named \u{201c}{name}\u{201d} already exists. Saving will replace its tracks."
                                        )
                                    } else {
                                        "Name the new playlist. The pane is saved into it."
                                            .to_owned()
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
                                        PlaylistDialogMode::CreateFromPane => {
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
                <div class="menu-scrim" on:click=move |_| set_menu_open.set(false)></div>
                <div class="context-menu app-menu" role="menu">
                    <button class="menu-item" on:click=move |_| open_add_url()>
                        "Add URL…"
                    </button>
                    <div class="menu-separator"></div>
                    <button
                        class="menu-item"
                        disabled=move || queue.get().is_empty()
                        on:click=move |_| save_playlist_as()
                    >
                        "Save Playlist…"
                    </button>
                    <div class="menu-separator"></div>
                    <button
                        class="menu-item"
                        disabled=move || selected.get().is_empty()
                        on:click=move |_| remove_selected()
                    >
                        "Remove Selected"
                    </button>
                    <button
                        class="menu-item"
                        disabled=move || queue.get().is_empty()
                        on:click=move |_| clear_pane()
                    >
                        "Clear Playlist"
                    </button>
                    <div class="menu-separator"></div>
                    <div class="menu-group">"View"</div>
                    <button class="menu-item" on:click=move |_| toggle_sidebar()>
                        <span class="menu-check">
                            {move || if sidebar_shown() { "✓" } else { "" }}
                        </span>
                        "Show File Tree"
                    </button>
                    <div class="menu-separator"></div>
                    <div class="menu-group">"Playback"</div>
                    <button
                        class="menu-item"
                        disabled=move || queue.get().is_empty()
                        on:click=move |_| {
                            set_playing.update(|playing| *playing = !*playing);
                            set_stopped.set(false);
                            set_menu_open.set(false);
                        }
                    >
                        "Play/Pause"
                    </button>
                    <button
                        class="menu-item"
                        disabled=move || queue.get().is_empty()
                        on:click=move |_| {
                            set_playing.set(false);
                            set_stopped.set(true);
                            set_position.set(0.0);
                            if let Some(audio) = audio_ref.get() {
                                let _ = audio.set_current_time(0.0);
                            }
                            set_menu_open.set(false);
                        }
                    >
                        "Stop"
                    </button>
                    <button
                        class="menu-item"
                        disabled=move || queue.get().is_empty() || (!shuffle.get() && current.get() == 0 && repeat_mode.get() != Repeat::All)
                        on:click=move |_| {
                            step(-1);
                            set_menu_open.set(false);
                        }
                    >
                        "Previous"
                    </button>
                    <button
                        class="menu-item"
                        disabled=move || queue.get().is_empty() || (current.get() + 1 >= queue.get().len() && repeat_mode.get() != Repeat::All && !shuffle.get() && !radio_on.get())
                        on:click=move |_| {
                            step(1);
                            set_menu_open.set(false);
                        }
                    >
                        "Next"
                    </button>
                    <div class="menu-group">"Shuffle"</div>
                    <button
                        class="menu-item"
                        on:click=move |_| {
                            set_shuffle.set(false);
                            set_menu_open.set(false);
                        }
                    >
                        <span class="menu-check">{move || if !shuffle.get() { "●" } else { "" }}</span>
                        "Off"
                    </button>
                    <button
                        class="menu-item"
                        on:click=move |_| {
                            set_shuffle.set(true);
                            set_menu_open.set(false);
                        }
                    >
                        <span class="menu-check">{move || if shuffle.get() { "●" } else { "" }}</span>
                        "All Tracks"
                    </button>
                    <div class="menu-group">"Repeat"</div>
                    <button
                        class="menu-item"
                        on:click=move |_| {
                            set_repeat_mode.set(Repeat::Off);
                            set_menu_open.set(false);
                        }
                    >
                        <span class="menu-check">{move || if repeat_mode.get() == Repeat::Off { "●" } else { "" }}</span>
                        "Off"
                    </button>
                    <button
                        class="menu-item"
                        on:click=move |_| {
                            set_repeat_mode.set(Repeat::One);
                            set_menu_open.set(false);
                        }
                    >
                        <span class="menu-check">{move || if repeat_mode.get() == Repeat::One { "●" } else { "" }}</span>
                        "One Track"
                    </button>
                    <button
                        class="menu-item"
                        on:click=move |_| {
                            set_repeat_mode.set(Repeat::All);
                            set_menu_open.set(false);
                        }
                    >
                        <span class="menu-check">{move || if repeat_mode.get() == Repeat::All { "●" } else { "" }}</span>
                        "All Tracks"
                    </button>
                    <button
                        class="menu-item"
                        disabled=move || !radio_on.get()
                        on:click=move |_| {
                            reshuffle_radio();
                            set_menu_open.set(false);
                        }
                    >
                        "Reshuffle Radio"
                    </button>
                    <div class="menu-separator"></div>
                    <button
                        class="menu-item"
                        disabled=move || queue.get().is_empty()
                        on:click=move |_| clear_pane()
                    >
                        "Clear Queue"
                    </button>
                    <div class="menu-separator"></div>
                    <button class="menu-item" on:click=move |_| open_preferences()>
                        "Preferences…"
                    </button>
                    <button class="menu-item" on:click=move |_| open_about()>
                        "About Kog…"
                    </button>
                </div>
            </Show>
        </div>
    }
}
