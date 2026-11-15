//! Bound the table's DOM work to the viewport, while selection and playback
//! continue to address the complete playlist by source index. The rows view
//! is laid out the same way with taller rows, and the cover grid as lines of
//! several tiles: the window then starts and ends on whole lines.
use super::*;

const OVERSCAN: usize = 12;
/// The cover grid's tiles are about this wide, stretched to fill the line.
const TILE_WIDTH: f64 = 180.0;
/// A tile's height beyond its square cover: the title and artist lines.
const TILE_TEXT: f64 = 62.0;
/// The shortest rows-view row; lines beyond three make it taller (see
/// `card_row_height`).
const CARD_ROW: f64 = 64.0;

/// A rows-view row's height for the lines it shows: their text (13, 12, 11
/// and 10px at a 1.3 line height) with room around them.
pub fn card_row_height(lines: &[Vec<String>]) -> f64 {
    const HEIGHTS: [f64; 4] = [16.9, 15.6, 14.3, 13.0];
    let (total, shown) = lines
        .iter()
        .zip(HEIGHTS)
        .filter(|(fields, _)| !fields.is_empty())
        .fold((0.0, 0.0), |(total, shown), (_, height)| (total + height, shown + 1.0));
    (14.0 + total + (shown - 1.0_f64).max(0.0)).max(CARD_ROW).round()
}

/// How the desktop shows songs: the column table, two-line rows with album
/// art, or a grid of cover tiles. Phones keep their own cards.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ViewMode {
    List,
    Rows,
    Grid,
}

impl ViewMode {
    pub fn key(self) -> &'static str {
        match self {
            Self::List => "list",
            Self::Rows => "rows",
            Self::Grid => "grid",
        }
    }

    pub fn from_key(key: &str) -> Self {
        match key {
            "rows" => Self::Rows,
            "grid" => Self::Grid,
            _ => Self::List,
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
struct Metrics {
    top: f64,
    height: f64,
    row: f64,
    /// Songs side by side: more than one only in the cover grid.
    columns: usize,
}

#[derive(Clone, Copy)]
pub struct RowWindow {
    pub node: NodeRef<leptos::html::Div>,
    metrics: RwSignal<Metrics>,
    mode: RwSignal<ViewMode>,
    /// The rows view's row height, from the lines it shows.
    card_row: Signal<f64>,
}

impl RowWindow {
    pub fn new(mode: RwSignal<ViewMode>, card_row: Signal<f64>) -> Self {
        let window = Self {
            node: NodeRef::new(),
            metrics: RwSignal::new(Metrics {
                top: 0.0,
                height: 900.0,
                row: 26.0,
                columns: 1,
            }),
            mode,
            card_row,
        };
        Effect::new(move |_| {
            window.node.track();
            window.mode.track();
            window.card_row.track();
            window.refresh();
        });
        let resize = window_event_listener(leptos::ev::resize, move |_| window.refresh());
        on_cleanup(move || resize.remove());
        window
    }

    /// Whether the phone layout shows: its cards, whatever the view mode.
    pub fn compact() -> bool {
        window()
            .match_media("(max-width: 820px), (pointer: coarse) and (max-width: 1200px)")
            .ok()
            .flatten()
            .is_some_and(|query| query.matches())
    }

    pub fn refresh(self) {
        let Some(node) = self.node.get_untracked() else {
            return;
        };
        let browser = window();
        let compact = Self::compact();
        // Reserve the full window height so changes to tabs, toolbars or the
        // mobile navigation cannot leave the bottom of the viewport unpainted.
        let height = browser
            .inner_height()
            .ok()
            .and_then(|value| value.as_f64())
            .unwrap_or(900.0);
        let mode = if compact { ViewMode::List } else { self.mode.get_untracked() };
        let (row, columns) = match mode {
            // Phones: room for a title on two lines over its detail line.
            ViewMode::List if compact => (68.0, 1),
            ViewMode::List => (26.0, 1),
            ViewMode::Rows => (self.card_row.get_untracked(), 1),
            ViewMode::Grid => {
                let width = f64::from(node.client_width()).max(TILE_WIDTH);
                let columns = ((width / TILE_WIDTH).floor() as usize).max(1);
                let tile = (width / columns as f64).floor();
                ((tile - 20.0).max(48.0) + TILE_TEXT, columns)
            }
        };
        let next = Metrics {
            top: node.scroll_top() as f64,
            height,
            row: row.round(),
            columns,
        };
        if self.metrics.get_untracked() != next {
            self.metrics.set(next);
        }
    }

    pub fn range(self, count: usize) -> std::ops::Range<usize> {
        let metrics = self.metrics.get();
        let columns = metrics.columns.max(1);
        let lines = (metrics.height / metrics.row).ceil() as usize + 2 * OVERSCAN;
        let total_lines = count.div_ceil(columns);
        let first = (metrics.top / metrics.row).floor() as usize;
        let first = first
            .saturating_sub(OVERSCAN)
            .min(total_lines.saturating_sub(lines));
        let start = first * columns;
        start..((first + lines) * columns).min(count)
    }

    /// The height of `rows` songs laid out as the view shows them.
    pub fn height(self, rows: usize) -> f64 {
        let metrics = self.metrics.get();
        metrics.row * rows.div_ceil(metrics.columns.max(1)) as f64
    }

    /// Songs side by side (the cover grid's columns) and a line's height.
    pub fn columns(self) -> usize {
        self.metrics.get().columns
    }

    pub fn row_height(self) -> f64 {
        self.metrics.get().row
    }

    /// Scroll an offscreen row into the mounted window before looking up its
    /// DOM node. Used by the existing follow-playing-track behavior.
    pub fn reveal(self, position: usize) {
        let Some(node) = self.node.get_untracked() else {
            return;
        };
        let metrics = self.metrics.get_untracked();
        let header = node
            .query_selector(".columns, .sort-bar")
            .ok()
            .flatten()
            .map(|header| header.get_bounding_client_rect().height())
            .unwrap_or(0.0);
        let top = (position / metrics.columns.max(1)) as f64 * metrics.row;
        let visible = (node.client_height() as f64 - header).max(metrics.row);
        let scroll = node.scroll_top() as f64;
        if top < scroll {
            node.set_scroll_top(top as i32);
        } else if top + metrics.row > scroll + visible {
            node.set_scroll_top((top + metrics.row - visible) as i32);
        }
        self.refresh();
    }
}
