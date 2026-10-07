//! Whole-song MML scores for clients that play through the server.
//!
//! Recording a score renders the track once with the same decoder settings
//! as its stream. Jobs are shared by every client asking for the same track,
//! and a few recent ones are kept so reopening the view is immediate.
use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};

use kog_audio::decoder::{DecoderRegistry, DecoderSettings};
use kog_audio::inspection::mml::{Document, encode};
use kog_audio::inspection::score::{Progress, analyze_progressively};
use kog_audio::playlist::PlaylistEntry;
use kog_audio::streaming::resolve_entry;
use serde::Serialize;

const KEPT_JOBS: usize = 6;

#[derive(Default)]
struct State {
    /// Bumped whenever a newer document replaces the previous one.
    revision: u64,
    document: Option<Arc<Document>>,
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
    pub fn status(
        &self,
        key: String,
        title: String,
        entry: PlaylistEntry,
        settings: DecoderSettings,
        scratch: PathBuf,
        have: Option<u64>,
    ) -> Status {
        let job = self.job(key, title, entry, settings, scratch);
        let state = job.state.lock().unwrap_or_else(|e| e.into_inner());
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
            document: state
                .document
                .clone()
                .filter(|_| have != Some(state.revision)),
        }
    }

    fn job(
        &self,
        key: String,
        title: String,
        entry: PlaylistEntry,
        settings: DecoderSettings,
        scratch: PathBuf,
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
                let publish = |document: Document, done: bool| {
                    let mut state = worker.state.lock().unwrap_or_else(|e| e.into_inner());
                    state.revision += 1;
                    state.document = Some(Arc::new(document));
                    state.done = done;
                };
                let decoders = DecoderRegistry::new(settings.clone());
                let result = resolve_entry(&entry, &decoders, &scratch).and_then(|source| {
                    analyze_progressively(source, settings, &title, &worker.progress, &mut |score| {
                        publish(encode(&score), false)
                    })
                });
                match result {
                    Ok(score) => publish(encode(&score), true),
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
