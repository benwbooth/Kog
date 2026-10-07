//! Kog MML view: the whole song as text, following playback.
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::sync::mpsc::{self, Receiver};

use kog_audio::inspection::score::Progress;

use super::{Surface, paint};
use kog_audio::inspection::mml::{BARS_PER_LINE, Document, STYLES, Score, encode_lines};

/// A score and whether recording has finished, or why it failed.
type Result = std::result::Result<(Score, bool), String>;

#[derive(Default)]
pub(super) struct Mml {
    key: String,
    pending: Option<Receiver<Result>>,
    progress: Arc<Progress>,
    score: Option<Score>,
    document: Option<std::result::Result<Document, String>>,
    /// Bars written on each track's line; `+` and `-` change it.
    pub bars: usize,
    /// Wrapped rows as byte ranges of the text, for the width they fit.
    rows: Vec<(usize, usize)>,
    wrapped_for: usize,
    pub scroll: usize,
    pub follow: bool,
    /// What the last copy or export did.
    pub notice: String,
    /// Text to hand to the terminal's clipboard on the next draw.
    clipboard: Option<String>,
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
        self.score = None;
        self.rows.clear();
        self.scroll = 0;
        self.follow = true;
        self.notice.clear();
        self.pending = Some(start(Arc::clone(&self.progress)));
    }

    fn bars(&self) -> usize {
        if self.bars == 0 { BARS_PER_LINE } else { self.bars }
    }

    /// Re-wrap the recorded score with more or fewer bars on each line.
    pub fn change_bars(&mut self, more: bool) {
        let bars = self.bars();
        self.bars = if more { (bars + 1).min(16) } else { bars.saturating_sub(1).max(1) };
        if let Some(score) = &self.score {
            self.document = Some(Ok(encode_lines(score, self.bars)));
            self.wrapped_for = 0;
        }
    }

    /// Copy the whole score through the terminal (OSC 52).
    pub fn copy(&mut self) {
        let Some(Ok(document)) = &self.document else {
            self.notice = "No score to copy yet".into();
            return;
        };
        self.clipboard = Some(document.text.clone());
        self.notice = "Copied the MML score (needs a terminal with OSC 52 clipboard support)".into();
    }

    /// Save the whole score as `<title>.mml` in the working directory.
    pub fn export(&mut self) {
        let Some(Ok(document)) = &self.document else {
            self.notice = "No score to export yet".into();
            return;
        };
        let title = document
            .text
            .lines()
            .find_map(|line| line.strip_prefix("#TITLE "))
            .map(|title| title.trim().trim_matches('"').to_owned())
            .filter(|title| !title.is_empty())
            .unwrap_or_else(|| "score".into());
        let name: String = title
            .chars()
            .map(|c| if c.is_control() || "\\/:*?\"<>|".contains(c) { '_' } else { c })
            .collect();
        let path = std::env::current_dir().unwrap_or_default().join(format!("{name}.mml"));
        self.notice = match std::fs::write(&path, &document.text) {
            Ok(()) => format!("Saved {}", path.display()),
            Err(error) => format!("Could not save {}: {error}", path.display()),
        };
    }

    pub fn bars_label(&self) -> usize {
        self.bars()
    }

    fn poll(&mut self) {
        while let Some(receiver) = &self.pending {
            match receiver.try_recv() {
                Ok(Ok((score, done))) => {
                    self.document = Some(Ok(encode_lines(&score, self.bars())));
                    self.score = Some(score);
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
        if let Some(text) = self.clipboard.take() {
            use base64::Engine;
            out.push_str("\x1b]52;c;");
            out.push_str(&base64::engine::general_purpose::STANDARD.encode(text));
            out.push_str("\x07");
        }
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
