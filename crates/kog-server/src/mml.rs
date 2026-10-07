//! Whole-song MML scores for clients that play through the server.
//!
//! Recording a score renders the track once with the same decoder settings
//! as its stream. Jobs are shared by every client asking for the same track,
//! and a few recent ones are kept so reopening the view is immediate.
use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};

use kog_audio::decoder::{DecoderRegistry, DecoderSettings, PlaybackSource};
use kog_audio::inspection::mml::{Document, Score, encode_lines};
use kog_audio::inspection::score::{Progress, analyze_progressively};
use kog_audio::playlist::PlaylistEntry;
use kog_audio::streaming::resolve_entry;
use serde::Serialize;

const KEPT_JOBS: usize = 6;

#[derive(Default)]
struct State {
    /// Bumped whenever a newer score replaces the previous one.
    revision: u64,
    score: Option<Arc<Score>>,
    /// The last document written, and its bars per line.
    document: Option<(usize, Arc<Document>)>,
    done: bool,
    error: Option<String>,
}

struct Job {
    key: String,
    progress: Arc<Progress>,
    state: Mutex<State>,
}

#[derive(Clone, Default)]
pub struct MmlJobs(Arc<Mutex<VecDeque<Arc<Job>>>>);

#[derive(Serialize)]
pub struct Status {
    /// recording, ready, or error
    pub status: &'static str,
    pub revision: u64,
    pub recorded_ms: u64,
    pub total_ms: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    /// Omitted when the client already holds this revision.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub document: Option<Arc<Document>>,
}

impl MmlJobs {
    /// Score a playlist entry, resolved like its audio stream.
    pub fn status(
        &self,
        key: String,
        title: String,
        entry: PlaylistEntry,
        settings: DecoderSettings,
        scratch: PathBuf,
        have: Option<u64>,
        bars: usize,
    ) -> Status {
        self.status_with(key, have, bars, move |progress, partial| {
            let decoders = DecoderRegistry::new(settings.clone());
            let source = resolve_entry(&entry, &decoders, &scratch)?;
            analyze_progressively(source, settings, &title, progress, partial)
        })
    }

    /// Score a source a native player has already resolved on the device.
    pub fn status_for_source(
        &self,
        key: String,
        title: String,
        source: PlaybackSource,
        settings: DecoderSettings,
        have: Option<u64>,
        bars: usize,
    ) -> Status {
        self.status_with(key, have, bars, move |progress, partial| {
            analyze_progressively(source, settings, &title, progress, partial)
        })
    }

    /// Score with a custom recorder, for native players that open sources
    /// their own way. Jobs with the same key are shared.
    pub fn status_with(
        &self,
        key: String,
        have: Option<u64>,
        bars: usize,
        record: impl FnOnce(&Progress, &mut dyn FnMut(Score)) -> Result<Score, String> + Send + 'static,
    ) -> Status {
        let job = self.job(key, record);
        let mut state = job.state.lock().unwrap_or_else(|e| e.into_inner());
        // Write the score with the caller's bars per line, reusing the last
        // document when nothing changed. Callers changing the setting send no
        // `have`, so they always receive the rewrapped text.
        let document = if have == Some(state.revision) {
            None
        } else {
            let cached = state.document.clone().filter(|(width, _)| *width == bars).map(|(_, d)| d);
            cached.or_else(|| {
                let document = Arc::new(encode_lines(state.score.as_deref()?, bars));
                state.document = Some((bars, Arc::clone(&document)));
                Some(document)
            })
        };
        Status {
            status: if state.error.is_some() {
                "error"
            } else if state.done {
                "ready"
            } else {
                "recording"
            },
            revision: state.revision,
            recorded_ms: job.progress.recorded_ms.load(Ordering::Relaxed),
            total_ms: job.progress.total_ms.load(Ordering::Relaxed),
            detail: state.error.clone(),
            document,
        }
    }

    fn job(
        &self,
        key: String,
        record: impl FnOnce(&Progress, &mut dyn FnMut(Score)) -> Result<Score, String> + Send + 'static,
    ) -> Arc<Job> {
        let mut jobs = self.0.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(index) = jobs.iter().position(|job| job.key == key) {
            let job = jobs.remove(index).expect("job index is in range");
            jobs.push_back(Arc::clone(&job));
            return job;
        }
        let job = Arc::new(Job {
            key,
            progress: Arc::new(Progress::default()),
            state: Mutex::new(State::default()),
        });
        jobs.push_back(Arc::clone(&job));
        while jobs.len() > KEPT_JOBS {
            if let Some(old) = jobs.pop_front() {
                old.progress.cancel.store(true, Ordering::Relaxed);
            }
        }
        let worker = Arc::clone(&job);
        let spawned = std::thread::Builder::new()
            .name("kog-mml-score".into())
            .spawn(move || {
                let publish = |score: Score, done: bool| {
                    let mut state = worker.state.lock().unwrap_or_else(|e| e.into_inner());
                    state.revision += 1;
                    state.score = Some(Arc::new(score));
                    state.document = None;
                    state.done = done;
                };
                let progress = Arc::clone(&worker.progress);
                match record(&progress, &mut |score| publish(score, false)) {
                    Ok(score) => publish(score, true),
                    Err(error) => {
                        let mut state = worker.state.lock().unwrap_or_else(|e| e.into_inner());
                        state.error = Some(error);
                    }
                }
            });
        if let Err(error) = spawned {
            job.state.lock().unwrap_or_else(|e| e.into_inner()).error =
                Some(format!("starting the score recorder: {error}"));
        }
        job
    }
}
