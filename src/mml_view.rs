//! Kog MML score for the Qt channel inspector. The song is recorded on a
//! worker with the same decoder settings as playback; QML asks for the bar
//! that is playing as highlighted rich text and caches the others.
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::sync::mpsc::{self, Receiver};

use kog_audio::decoder::{DecoderSettings, PlaybackSource};
use kog_audio::inspection::mml::{Document, encode};
use kog_audio::inspection::score::{Progress, analyze_progressively};

type Update = Result<(Document, bool), String>;

#[derive(Default)]
pub struct MmlView {
    key: String,
    progress: Arc<Progress>,
    receiver: Option<Receiver<Update>>,
    document: Option<Document>,
    error: Option<String>,
    revision: u64,
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

impl MmlView {
    /// Record the playing source unless its score is already loaded.
    pub fn follow(&mut self, source: Option<&PlaybackSource>, title: &str, settings: &DecoderSettings) {
        let Some(source) = source else { return };
        let key = format!("{}\0{:?}\0{:?}", source.path.display(), source.subsong, source.remote_url);
        if key == self.key {
            return;
        }
        self.progress.cancel.store(true, Ordering::Relaxed);
        self.progress = Arc::new(Progress::default());
        self.key = key;
        self.document = None;
        self.error = None;
        self.revision += 1;
        if source.remote_url.is_some() {
            self.receiver = None;
            self.error = Some("Open the server's web player to see MML scores for server tracks.".into());
            return;
        }
        let (sender, receiver) = mpsc::channel();
        let (source, title, settings, progress) =
            (source.clone(), title.to_owned(), settings.clone(), Arc::clone(&self.progress));
        let _ = std::thread::Builder::new().name("kog-mml-score".into()).spawn(move || {
            let result = analyze_progressively(source, settings, &title, &progress, &mut |score| {
                let _ = sender.send(Ok((encode(&score), false)));
            });
            let _ = sender.send(result.map(|score| (encode(&score), true)));
        });
        self.receiver = Some(receiver);
    }

    fn poll(&mut self) {
        while let Some(receiver) = &self.receiver {
            match receiver.try_recv() {
                Ok(Ok((document, done))) => {
                    self.document = Some(document);
                    self.revision += 1;
                    if done {
                        self.receiver = None;
                    }
                }
                Ok(Err(error)) => {
                    self.error = Some(error);
                    self.revision += 1;
                    self.receiver = None;
                }
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => self.receiver = None,
            }
        }
    }

    /// The view state at the audible position. `current_html` is the playing
    /// bar with its sounding notes highlighted.
    pub fn state(&mut self, position: f64) -> serde_json::Value {
        self.poll();
        let time = |ms: u64| format!("{}:{:02}", ms / 60_000, ms / 1000 % 60);
        let message = if let Some(error) = &self.error {
            error.clone()
        } else if self.receiver.is_some() {
            format!(
                "Recording every channel of this song… {} of {}",
                time(self.progress.recorded_ms.load(Ordering::Relaxed)),
                time(self.progress.total_ms.load(Ordering::Relaxed))
            )
        } else if self.document.is_none() {
            "Play a song to see its MML score.".into()
        } else {
            String::new()
        };
        let Some(document) = &self.document else {
            return serde_json::json!({"revision": self.revision, "message": message, "bars": 0, "current": -1});
        };
        let (active, bar) = document.active_indices(position);
        let current = bar.map(|bar| bar.index);
        serde_json::json!({
            "revision": self.revision,
            "message": message,
            "bars": document.bars.len(),
            "current": current.map_or(-1, |index| index as i64),
            "header": escape(&document.text[..document.bars.first().map_or(0, |bar| bar.from)]),
            "currentHtml": current.map(|index| self.bar_html(index, &active)).unwrap_or_default(),
        })
    }

    pub fn bar(&self, index: usize) -> String {
        self.bar_html(index, &[])
    }

    fn bar_html(&self, index: usize, active: &[usize]) -> String {
        let Some(document) = &self.document else { return String::new() };
        let Some(bar) = document.bars.get(index) else { return String::new() };
        let text = &document.text;
        let mut html = String::new();
        let mut cursor = bar.from;
        // Spans are in text order, so only this bar's slice is visited.
        let first = document.spans.partition_point(|span| span.from < bar.from);
        let last = document.spans.partition_point(|span| span.from < bar.to);
        for (span_index, span) in document.spans.iter().enumerate().take(last).skip(first) {
            let style = if active.contains(&span_index) {
                "background-color:#50c8ef;color:#0b1016;font-weight:bold"
            } else if span.kind == "rest" {
                "color:#6f8794"
            } else if span.kind == "command" {
                "color:#83d4bb"
            } else {
                continue;
            };
            html.push_str(&escape(&text[cursor..span.from]));
            html.push_str(&format!("<span style=\"{style}\">{}</span>", escape(&text[span.from..span.to])));
            cursor = span.to;
        }
        html.push_str(&escape(&text[cursor..bar.to]));
        format!("<p style=\"white-space:pre-wrap\">{}</p>", html.trim_end().replace('\n', "<br/>"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mml_view_records_a_song_and_highlights_the_playing_bar() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("native/game-music-emu/test.nsf");
        let source = PlaybackSource { path, ..PlaybackSource::default() };
        let mut view = MmlView::default();
        let settings = DecoderSettings::default();
        view.follow(Some(&source), "test", &settings);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(120);
        let mut state = view.state(3.0);
        while state["bars"].as_u64().unwrap_or(0) == 0 && std::time::Instant::now() < deadline {
            std::thread::sleep(std::time::Duration::from_millis(50));
            state = view.state(3.0);
        }
        assert!(state["bars"].as_u64().unwrap() > 1, "{state}");
        assert!(state["current"].as_i64().unwrap() >= 0, "{state}");
        let current = state["currentHtml"].as_str().unwrap();
        assert!(current.contains("; bar") && current.contains("background-color"), "{current}");
        assert!(!view.bar(0).contains("background-color"));
        // The same source keeps its score; a different one starts over.
        let revision = state["revision"].clone();
        view.follow(Some(&source), "test", &settings);
        assert!(view.state(3.0)["revision"].as_u64() >= revision.as_u64());
        view.follow(None, "", &settings);
        assert!(view.state(3.0)["bars"].as_u64().unwrap() > 0);
    }
}
