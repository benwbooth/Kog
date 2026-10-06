//! Bound the table's DOM work to the viewport, while selection and playback
//! continue to address the complete playlist by source index.
use super::*;

const OVERSCAN: usize = 12;

#[derive(Clone, Copy, PartialEq)]
struct Metrics {
    top: f64,
    height: f64,
    row: f64,
}

#[derive(Clone, Copy)]
pub struct RowWindow {
    pub node: NodeRef<leptos::html::Div>,
    metrics: RwSignal<Metrics>,
}

impl RowWindow {
    pub fn new() -> Self {
        let window = Self {
            node: NodeRef::new(),
            metrics: RwSignal::new(Metrics {
                top: 0.0,
                height: 900.0,
                row: 26.0,
            }),
        };
        Effect::new(move |_| {
            window.node.track();
            window.refresh();
        });
        let resize = window_event_listener(leptos::ev::resize, move |_| window.refresh());
        on_cleanup(move || resize.remove());
        window
    }

    pub fn refresh(self) {
        let Some(node) = self.node.get_untracked() else {
            return;
        };
        let browser = window();
        let compact = browser
            .match_media("(max-width: 820px), (pointer: coarse) and (max-width: 1200px)")
            .ok()
            .flatten()
            .is_some_and(|query| query.matches());
        // Reserve the full window height so changes to tabs, toolbars or the
        // mobile navigation cannot leave the bottom of the viewport unpainted.
        let height = browser
            .inner_height()
            .ok()
            .and_then(|value| value.as_f64())
            .unwrap_or(900.0);
        let next = Metrics {
            top: node.scroll_top() as f64,
            height,
            row: if compact { 60.0 } else { 26.0 },
        };
        if self.metrics.get_untracked() != next {
            self.metrics.set(next);
        }
    }

    pub fn range(self, count: usize) -> std::ops::Range<usize> {
        let metrics = self.metrics.get();
        let length = (metrics.height / metrics.row).ceil() as usize + 2 * OVERSCAN;
        let start = (metrics.top / metrics.row).floor() as usize;
        let start = start
            .saturating_sub(OVERSCAN)
            .min(count.saturating_sub(length));
        start..(start + length).min(count)
    }

    pub fn height(self, rows: usize) -> f64 {
        self.metrics.get().row * rows as f64
    }

    /// Scroll an offscreen row into the mounted window before looking up its
    /// DOM node. Used by the existing follow-playing-track behavior.
    pub fn reveal(self, position: usize) {
        let Some(node) = self.node.get_untracked() else {
            return;
        };
        let metrics = self.metrics.get_untracked();
        let header = node
            .query_selector(".columns")
            .ok()
            .flatten()
            .map(|header| header.get_bounding_client_rect().height())
            .unwrap_or(0.0);
        let top = position as f64 * metrics.row;
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
