//! Metadata for saved-playlist views, separate from the playback queue.
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, TryRecvError};

use kog_audio::decoder::DecoderRegistry;
use kog_audio::track::Track;
use kog_core::db::StoredEntry;
use serde_json::Value;

#[derive(Default)]
pub struct WorkspaceTracks {
    entries: Vec<Value>,
    pub rows: Vec<MetadataRow>,
    receiver: Option<Receiver<(usize, Result<Track, String>)>>,
    cancel: Arc<AtomicBool>,
}

pub struct MetadataRow {
    pub entry: Option<StoredEntry>,
    pub track: Option<Track>,
    pub error: Option<String>,
}

pub fn entry_key(entry: &StoredEntry) -> String {
    let base = if entry.kind == "archive" {
        format!("{}::{}", entry.path, entry.entry)
    } else {
        entry.path.clone()
    };
    match entry.fragment.as_deref().filter(|s| !s.is_empty()) {
        Some(fragment) => format!("{base}#{fragment}"),
        None => base,
    }
}

impl WorkspaceTracks {
    pub fn refresh(
        &mut self,
        entries: Vec<Value>,
        known: &[Track],
        worker: impl FnOnce() -> DecoderRegistry,
    ) {
        if entries == self.entries {
            return;
        }
        self.cancel.store(true, Ordering::Relaxed);
        self.cancel = Arc::new(AtomicBool::new(false));
        self.receiver = None;
        let mut cache: HashMap<String, Track> = self
            .rows
            .iter()
            .filter_map(|row| Some((entry_key(row.entry.as_ref()?), row.track.clone()?)))
            .collect();
        for track in known {
            if let Some(entry) = kog_audio::playback_order::stored_entry_for_track(track) {
                cache.insert(entry_key(&entry), track.clone());
            }
        }
        self.rows = entries
            .iter()
            .map(|entry| {
                match kog_server::api::stored_entries_from_json(std::slice::from_ref(entry)) {
                    Ok(mut entries) => {
                        let entry = entries.remove(0);
                        let track = cache.get(&entry_key(&entry)).cloned();
                        MetadataRow {
                            entry: Some(entry),
                            track,
                            error: None,
                        }
                    }
                    Err(error) => MetadataRow {
                        entry: None,
                        track: None,
                        error: Some(error),
                    },
                }
            })
            .collect();
        self.entries = entries;
        let missing: Vec<_> = self
            .rows
            .iter()
            .enumerate()
            .filter_map(|(index, row)| {
                row.track
                    .is_none()
                    .then(|| row.entry.clone().map(|entry| (index, entry)))
                    .flatten()
            })
            .collect();
        if missing.is_empty() {
            return;
        }
        let decoders = worker();
        let cancel = self.cancel.clone();
        let (sender, receiver) = std::sync::mpsc::channel();
        self.receiver = Some(receiver);
        std::thread::spawn(move || {
            for (index, entry) in missing {
                if cancel.load(Ordering::Relaxed) {
                    break;
                }
                let result = if let Some(track) = cache.get(&entry_key(&entry)) {
                    Ok(track.clone())
                } else {
                    decoders
                        .expand_queue_entry(&entry)
                        .into_iter()
                        .next()
                        .map(|source| Track::from_source(source, &decoders))
                        .ok_or_else(|| "Unable to read track metadata".to_owned())
                };
                if let Ok(track) = &result {
                    cache.insert(entry_key(&entry), track.clone());
                }
                if cancel.load(Ordering::Relaxed) || sender.send((index, result)).is_err() {
                    break;
                }
            }
        });
    }

    pub fn poll(&mut self) -> bool {
        let mut changed = false;
        for _ in 0..64 {
            let Some(receiver) = &self.receiver else {
                break;
            };
            match receiver.try_recv() {
                Ok((index, result)) => {
                    if let Some(row) = self.rows.get_mut(index) {
                        match result {
                            Ok(track) => row.track = Some(track),
                            Err(error) => row.error = Some(error),
                        }
                        changed = true;
                    }
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    self.receiver = None;
                    break;
                }
            }
        }
        changed
    }
}

impl Drop for WorkspaceTracks {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
}

impl MetadataRow {
    pub fn locator(&self) -> String {
        self.entry
            .as_ref()
            .map(|entry| {
                if entry.kind == "archive" {
                    format!("{}/{}", entry.path, entry.entry)
                } else {
                    entry.path.clone()
                }
            })
            .unwrap_or_default()
    }
    pub fn fallback_title(&self) -> String {
        let path = self.locator();
        let filename = path.rsplit(['/', '\\']).next().unwrap_or_default();
        let title = filename
            .rsplit_once('.')
            .map(|(stem, _)| stem)
            .unwrap_or(filename);
        match self
            .entry
            .as_ref()
            .and_then(|entry| entry.fragment.as_deref())
        {
            Some(fragment) => format!("{title} [{fragment}]"),
            None => title.to_owned(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kog_audio::decoder::PlaybackSource;
    use serde_json::json;

    fn track(path: &str, title: &str) -> Track {
        Track {
            source: PlaybackSource::from_path(path.into()),
            title: title.into(),
            ..Track::default()
        }
    }
    fn entry(path: &str) -> Value {
        json!({"kind":"local", "path":path, "entry":""})
    }

    #[test]
    fn draft_reordering_reuses_metadata_and_retains_duplicate_rows() {
        let mut view = WorkspaceTracks::default();
        view.refresh(
            vec![entry("/one.flac"), entry("/two.flac")],
            &[track("/one.flac", "One"), track("/two.flac", "Two")],
            || panic!("known queue metadata should be reused"),
        );
        view.refresh(
            vec![entry("/two.flac"), entry("/one.flac"), entry("/two.flac")],
            &[],
            || panic!("reordering should not probe files again"),
        );
        assert_eq!(
            view.rows
                .iter()
                .map(|row| row.track.as_ref().unwrap().title.as_str())
                .collect::<Vec<_>>(),
            vec!["Two", "One", "Two"]
        );
    }

    #[test]
    fn changing_tabs_discards_in_flight_metadata_from_the_previous_tab() {
        let mut view = WorkspaceTracks::default();
        view.refresh(
            vec![entry("/one.flac")],
            &[track("/one.flac", "One")],
            || unreachable!(),
        );
        let old_cancel = view.cancel.clone();
        let (sender, receiver) = std::sync::mpsc::channel();
        view.receiver = Some(receiver);
        view.refresh(
            vec![entry("/two.flac")],
            &[track("/two.flac", "Two")],
            || unreachable!(),
        );
        assert!(old_cancel.load(Ordering::Relaxed));
        assert!(
            sender
                .send((0, Ok(track("/one.flac", "Old result"))))
                .is_err()
        );
        assert!(!view.poll());
        assert_eq!(view.rows[0].track.as_ref().unwrap().title, "Two");
    }
}
