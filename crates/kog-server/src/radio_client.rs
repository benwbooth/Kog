//! Native scheduling adapter around the same radio service exposed over HTTP.
//! Picking, deadlines, blacklists, persistence, and round rotation belong to
//! `Radio`; buffering, cancellation, and deferred play belong to `RadioBuffer`.

use crate::radio::{Radio, RadioEntry};
use kog_audio::playback_order::radio::RadioBuffer;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, mpsc};

#[derive(Clone, Copy)]
enum Command {
    Enable(bool),
    Reshuffle,
    Advance,
}

struct Request {
    generation: u64,
    command: Command,
    root: Option<PathBuf>,
    scope: Option<PathBuf>,
}

struct Response<T> {
    generation: u64,
    result: Result<(Vec<T>, bool), String>,
}

pub struct RadioClient<T> {
    buffer: RadioBuffer<T>,
    root: Option<PathBuf>,
    scope: Option<PathBuf>,
    requests: mpsc::Sender<Request>,
    responses: mpsc::Receiver<Response<T>>,
    generation: Arc<AtomicU64>,
    error: Option<String>,
}

impl<T: Send + 'static> RadioClient<T> {
    pub fn new(
        radio: Radio,
        mut present: impl FnMut(RadioEntry) -> Result<T, String> + Send + 'static,
    ) -> Result<Self, String> {
        let (requests, jobs) = mpsc::channel::<Request>();
        let (results, responses) = mpsc::channel();
        let generation = Arc::new(AtomicU64::new(0));
        let worker_generation = generation.clone();
        std::thread::Builder::new()
            .name("kog-radio-client".to_owned())
            .spawn(move || {
                while let Ok(request) = jobs.recv() {
                    if request.generation != worker_generation.load(Ordering::Acquire) {
                        continue;
                    }
                    let root = request.root.as_deref();
                    let scope = request.scope.as_deref();
                    let (entries, exhausted) = match request.command {
                        Command::Enable(enabled) => {
                            let status = radio.set_enabled_incremental(enabled, root, scope);
                            let exhausted = status.entries.is_empty();
                            (status.entries, exhausted)
                        }
                        Command::Reshuffle => {
                            let status = radio.reshuffle_incremental(root, scope);
                            let exhausted = status.entries.is_empty();
                            (status.entries, exhausted)
                        }
                        Command::Advance => {
                            let advance = radio.advance_incremental(root, scope);
                            (advance.entries, advance.exhausted)
                        }
                    };
                    if request.generation != worker_generation.load(Ordering::Acquire) {
                        continue;
                    }
                    let result = entries
                        .into_iter()
                        .map(&mut present)
                        .collect::<Result<Vec<_>, _>>()
                        .map(|entries| (entries, exhausted));
                    if results
                        .send(Response {
                            generation: request.generation,
                            result,
                        })
                        .is_err()
                    {
                        break;
                    }
                }
            })
            .map_err(|error| format!("Starting radio: {error}"))?;
        Ok(Self {
            buffer: RadioBuffer::default(),
            root: None,
            scope: None,
            requests,
            responses,
            generation,
            error: None,
        })
    }

    pub fn set_enabled(&mut self, enabled: bool, root: Option<PathBuf>, scope: Option<PathBuf>) {
        self.reset(enabled, root, scope);
        self.send(Command::Enable(enabled));
    }

    pub fn reshuffle(&mut self, root: Option<PathBuf>, scope: Option<PathBuf>) {
        self.reset(true, root, scope);
        self.send(Command::Reshuffle);
    }

    /// Discard ready entries after a blacklist edit while preserving the
    /// shared round's position. The next request reloads the blacklist.
    pub fn refresh(&mut self) {
        if self.buffer.enabled() {
            self.reset(true, self.root.clone(), self.scope.clone());
            self.send(Command::Advance);
        }
    }

    fn reset(&mut self, enabled: bool, root: Option<PathBuf>, scope: Option<PathBuf>) {
        let generation = self.buffer.reset(enabled);
        self.generation.store(generation, Ordering::Release);
        self.root = root;
        self.scope = scope;
        self.error = None;
    }

    fn send(&mut self, command: Command) {
        let generation = self.buffer.begin_request();
        let request = Request {
            generation,
            command,
            root: self.root.clone(),
            scope: self.scope.clone(),
        };
        if self.requests.send(request).is_err() {
            self.buffer.fail(generation);
            self.error = Some("Random Radio worker stopped".to_owned());
        }
    }

    fn refill(&mut self) {
        if self.buffer.needs_refill() {
            self.send(Command::Advance);
        }
    }

    pub fn poll(&mut self) {
        loop {
            match self.responses.try_recv() {
                Ok(response) => match response.result {
                    Ok((entries, exhausted)) => {
                        self.buffer.accept(response.generation, entries, exhausted);
                    }
                    Err(error) => {
                        if self.buffer.fail(response.generation) {
                            self.error = Some(error);
                        }
                    }
                },
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => {
                    if !self.buffer.exhausted() {
                        self.buffer.fail(self.buffer.generation());
                        self.error = Some("Random Radio worker stopped".to_owned());
                    }
                    break;
                }
            }
        }
        self.refill();
    }

    pub fn request_next(&mut self) -> Option<T> {
        let entry = self.buffer.request_next();
        self.refill();
        entry
    }

    pub fn take_pending(&mut self) -> Option<T> {
        let entry = self.buffer.take_pending();
        self.refill();
        entry
    }

    pub fn cancel_waiting(&mut self) {
        self.buffer.cancel_waiting();
    }
    pub fn waiting(&self) -> bool {
        self.buffer.waiting()
    }
    pub fn ready_len(&self) -> usize {
        self.buffer.ready_len()
    }
    pub fn exhausted(&self) -> bool {
        self.buffer.exhausted()
    }
    pub fn take_error(&mut self) -> Option<String> {
        self.error.take()
    }
}

impl<T> Drop for RadioClient<T> {
    fn drop(&mut self) {
        self.generation.fetch_add(1, Ordering::Release);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kog_audio::decoder::DecoderRegistry;
    use std::time::{Duration, Instant};

    fn wait_for<T: Send + 'static>(client: &mut RadioClient<T>, count: usize) {
        let deadline = Instant::now() + Duration::from_secs(45);
        while client.ready_len() < count {
            client.poll();
            assert!(client.take_error().is_none());
            assert!(!client.exhausted(), "fixture unexpectedly exhausted");
            assert!(Instant::now() < deadline, "radio refill timed out");
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    #[test]
    fn native_client_stages_one_song_per_pick_without_autoplay() {
        let directory = tempfile::tempdir().unwrap();
        let file = directory.path().join("game.nsf");
        let mut bytes = vec![0; 128];
        bytes[..5].copy_from_slice(b"NESM\x1a");
        bytes[5] = 1;
        bytes[6] = 12;
        bytes[7] = 1;
        bytes[8..10].copy_from_slice(&0x8000_u16.to_le_bytes());
        bytes[10..12].copy_from_slice(&0x8000_u16.to_le_bytes());
        bytes[12..14].copy_from_slice(&0x8001_u16.to_le_bytes());
        bytes.extend_from_slice(&[0x60, 0x60]);
        std::fs::write(&file, bytes).unwrap();
        let decoders = DecoderRegistry::default();
        assert_eq!(decoders.expand(file.clone()).unwrap().len(), 12);
        let radio = Radio::new(Some(directory.path().to_owned()), None, false);
        let mut client =
            RadioClient::new(radio, move |entry| entry.audio_track(&decoders)).unwrap();
        client.set_enabled(true, Some(directory.path().to_owned()), None);
        wait_for(&mut client, 10);
        assert_eq!(client.ready_len(), 10);
        assert!(client.take_pending().is_none());
        for _ in 0..10 {
            let track = client.request_next().unwrap();
            assert_eq!(track.source.path, file);
            assert!(track.source.subsong.is_some_and(|song| song < 12));
        }
        assert!(client.request_next().is_none());
        assert!(client.waiting());
        client.cancel_waiting();
        wait_for(&mut client, 1);
        assert!(client.take_pending().is_none());
        client.set_enabled(false, Some(directory.path().to_owned()), None);
        assert!(client.request_next().is_none());
    }

    #[test]
    #[ignore = "requires KOG_RADIO_TEST_FILE pointing to a real multi-song file"]
    fn native_client_real_multisong_file() {
        let source =
            PathBuf::from(std::env::var_os("KOG_RADIO_TEST_FILE").expect("KOG_RADIO_TEST_FILE"));
        let directory = tempfile::tempdir().unwrap();
        let file = directory.path().join(source.file_name().unwrap());
        std::fs::copy(source, &file).unwrap();
        let decoders = DecoderRegistry::default();
        let count = decoders.expand(file.clone()).unwrap().len();
        assert!(count > 1);
        let radio = Radio::new(Some(directory.path().to_owned()), None, false);
        let mut client =
            RadioClient::new(radio, move |entry| entry.audio_track(&decoders)).unwrap();
        client.set_enabled(true, Some(directory.path().to_owned()), None);
        wait_for(&mut client, 1);
        assert!(client.take_pending().is_none());
        let track = client.request_next().unwrap();
        assert_eq!(track.source.path, file);
        assert!(
            track
                .source
                .subsong
                .is_some_and(|song| (song as usize) < count)
        );
        eprintln!(
            "Shared radio selected one of {count} subsongs: {:?}",
            track.source.subsong
        );
    }
}
