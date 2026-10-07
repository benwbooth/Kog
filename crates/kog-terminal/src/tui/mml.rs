//! Kog MML view: the whole song as text, following playback.
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::sync::mpsc::{self, Receiver};

use kog_audio::inspection::score::Progress;

use super::{Surface, paint};
use kog_audio::inspection::mml::{Document, STYLES};

/// A document and whether recording has finished, or why it failed.
type Result = std::result::Result<(Document, bool), String>;

#[derive(Default)]
pub(super) struct Mml {
    key: String,
    pending: Option<Receiver<Result>>,
    progress: Arc<Progress>,
    document: Option<std::result::Result<Document, String>>,
    /// Wrapped rows as byte ranges of the text, for the width they fit.
    rows: Vec<(usize, usize)>,
    wrapped_for: usize,
    pub scroll: usize,
    pub follow: bool,
}

impl Mml {
    /// Start recording a new song's score unless this one is already loaded.
    pub fn request(&mut self, key: &str, start: impl FnOnce(Arc<Progress>) -> Receiver<Result>) {
        if self.key == key {
            return;
        }
        self.progress.cancel.store(true, Ordering::Relaxed);
        self.progress = Arc::new(Progress::default());
        self.key = key.to_owned();
        self.document = None;
        self.rows.clear();
        self.scroll = 0;
        self.follow = true;
        self.pending = Some(start(Arc::clone(&self.progress)));
    }

    fn poll(&mut self) {
        while let Some(receiver) = &self.pending {
            match receiver.try_recv() {
                Ok(Ok((document, done))) => {
                    self.document = Some(Ok(document));
                    self.wrapped_for = 0;
                    if done {
                        self.pending = None;
                    }
                }
                Ok(Err(error)) => {
                    self.document = Some(Err(error));
                    self.pending = None;
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    if self.document.is_none() {
                        self.document = Some(Err("Score recording stopped".into()));
                    }
                    self.pending = None;
                }
                Err(mpsc::TryRecvError::Empty) => break,
            }
        }
    }

    fn wrap(&mut self, width: usize) {
        let Some(Ok(document)) = &self.document else {
            return;
        };
        if self.wrapped_for == width {
            return;
        }
        self.wrapped_for = width;
        self.rows.clear();
        let width = width.max(8);
        let mut start = 0;
        for line in document.text.split_inclusive('\n') {
            let end = start + line.trim_end_matches('\n').len();
            let mut from = start;
            loop {
                if end - from <= width {
                    self.rows.push((from, end));
                    break;
                }
                // Break after a space so tokens stay whole where possible.
                let limit = from + width;
                let cut = document.text[from..limit]
                    .rfind(' ')
                    .filter(|at| *at > 0)
                    .map_or(limit, |at| from + at + 1);
                self.rows.push((from, cut));
                from = cut;
            }
            start += line.len();
        }
    }

    pub fn draw(&mut self, out: &mut String, area: (usize, usize, usize, usize), seconds: f64) {
        let (x, y, width, height) = area;
        self.poll();
        let document = match &self.document {
            None => {
                let text = if self.pending.is_some() {
                    let seconds = |ms: u64| format!("{}:{:02}", ms / 60_000, ms / 1000 % 60);
                    format!(
                        "Recording every channel of this song for its MML score… {} of {}",
                        seconds(self.progress.recorded_ms.load(Ordering::Relaxed)),
                        seconds(self.progress.total_ms.load(Ordering::Relaxed))
                    )
                } else {
                    "Play a song to see its MML score.".into()
                };
                paint(out, y, x, &text, width, Surface::Muted, false);
                return;
            }
            Some(Err(error)) => {
                paint(out, y, x, error, width, Surface::Muted, false);
                return;
            }
            Some(Ok(_)) => {
                self.wrap(width);
                let Some(Ok(document)) = &self.document else { unreachable!() };
                document
            }
        };
        let (active, bar) = document.active(seconds);
        let height = if self.pending.is_some() {
            let ms = |ms: u64| format!("{}:{:02}", ms / 60_000, ms / 1000 % 60);
            paint(
                out,
                y + height.saturating_sub(1),
                x,
                &format!(
                    "Still recording… {} of {}",
                    ms(self.progress.recorded_ms.load(Ordering::Relaxed)),
                    ms(self.progress.total_ms.load(Ordering::Relaxed))
                ),
                width,
                Surface::Muted,
                false,
            );
            height.saturating_sub(1)
        } else {
            height
        };
        if self.follow {
            if let Some(bar) = bar {
                let first = self.rows.partition_point(|row| row.1 <= bar.from);
                let last = self.rows.partition_point(|row| row.0 < bar.to);
                let keep = height.saturating_sub(1);
                if first < self.scroll || last > self.scroll + keep {
                    self.scroll = first.saturating_sub(1);
                }
            }
        }
        self.scroll = self.scroll.min(self.rows.len().saturating_sub(height));
        for (n, &(from, to)) in self.rows.iter().skip(self.scroll).take(height).enumerate() {
            let in_bar = bar.is_some_and(|bar| from >= bar.from && to <= bar.to);
            let background = if in_bar { "23;46;56" } else { "25;27;29" };
            // Paint the row as colour runs, then the sounding notes over them.
            let mut line = format!("\x1b[{};{}H\x1b[48;2;{background}m", y + n, x);
            let mut cursor = from;
            let colour = |class: u8| rgb(&document.palette[usize::from(class).min(document.palette.len() - 1)]);
            for &(start, end, class) in document.styles_in(from, to) {
                let (start, end) = ((start as usize).max(from), (end as usize).min(to));
                if start > cursor {
                    line.push_str(&format!("\x1b[22;38;2;220;224;228m{}", &document.text[cursor..start]));
                }
                let bold = if matches!(STYLES[usize::from(class)].0, "note" | "label") { "1" } else { "22" };
                line.push_str(&format!("\x1b[{bold};38;2;{}m{}", colour(class), &document.text[start..end]));
                cursor = end;
            }
            if cursor < to {
                line.push_str(&format!("\x1b[22;38;2;220;224;228m{}", &document.text[cursor..to]));
            }
            line.push_str(&" ".repeat(width.saturating_sub(to - from)));
            line.push_str("\x1b[0m");
            out.push_str(&line);
            for span in active.iter().filter(|span| span.from < to && span.to > from) {
                let start = span.from.max(from);
                let end = span.to.min(to);
                out.push_str(&format!(
                    "\x1b[{};{}H\x1b[1;38;2;11;16;22;48;2;80;200;239m{}\x1b[0m",
                    y + n,
                    x + start - from,
                    &document.text[start..end]
                ));
            }
        }
    }
}

/// `#rrggbb` as an ANSI `r;g;b` triple.
fn rgb(hex: &str) -> String {
    let channel = |at: usize| u8::from_str_radix(hex.get(at..at + 2).unwrap_or("cc"), 16).unwrap_or(204);
    format!("{};{};{}", channel(1), channel(3), channel(5))
}
