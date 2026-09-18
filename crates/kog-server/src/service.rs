//! Streaming service: turn a library entry into an encoded audio stream.
//!
//! Cache-first. A cached encode is served as a plain file with `Range`
//! support, so seeking and replays are instant. On a miss the track is decoded
//! and encoded on the fly, and the bytes are tee'd into the cache as they
//! stream to the client, so the next listener gets the fast path.
//!
//! This is what makes "faster than realtime" honest: the encoder is only part
//! of the story, because Kog's emulator-backed decoders (SC-55, vgmstream,
//! GME, PSF) can render slower than realtime. Encoding once into a per-track
//! file removes that cost from every later play or seek.

use std::io::Write;
use std::path::{Path, PathBuf};

use axum::body::Bytes;
use kog_audio::decoder::DecoderSettings;
use kog_audio::playlist::PlaylistEntry;
use kog_audio::streaming::{PcmReader, resolve_entry};

use crate::stream::{StreamCache, StreamKey, encode_to_writer, encoder_args};
use crate::StreamCodec;

/// Bytes buffered between the encoder thread and the HTTP body. Small enough
/// that a slow client does not stall the encode for long, large enough to keep
/// the encoder fed.
const CHUNK_BYTES: usize = 64 * 1024;
const CHANNEL_DEPTH: usize = 16;

/// Where a stream's bytes come from.
pub enum StreamSource {
    /// A finished encode on disk: serve with `Range` support.
    Cached(PathBuf),
    /// An encode in progress: bytes arrive as they are produced.
    Encoding {
        receiver: tokio::sync::mpsc::Receiver<Result<Bytes, std::io::Error>>,
    },
}

#[derive(Clone)]
pub struct StreamService {
    cache: StreamCache,
    decoder_settings: DecoderSettings,
    /// Encoder executable; `ffmpeg` by default.
    encoder: PathBuf,
    scratch: PathBuf,
}

impl StreamService {
    pub fn new(
        cache: StreamCache,
        decoder_settings: DecoderSettings,
        encoder: PathBuf,
        scratch: PathBuf,
    ) -> Self {
        Self {
            cache,
            decoder_settings,
            encoder,
            scratch,
        }
    }

    pub fn cache(&self) -> &StreamCache {
        &self.cache
    }

    /// Resolve the encoder: `KOG_FFMPEG`, then `ffmpeg` on `PATH`.
    pub fn default_encoder() -> PathBuf {
        std::env::var_os("KOG_FFMPEG")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("ffmpeg"))
    }

    /// Open a stream for one entry, encoding on a miss.
    pub fn open(
        &self,
        entry: PlaylistEntry,
        key: StreamKey,
    ) -> Result<StreamSource, String> {
        if let Some(path) = self.cache.lookup(&key) {
            return Ok(StreamSource::Cached(path));
        }
        self.start_encode(entry, key)
    }

    fn start_encode(
        &self,
        entry: PlaylistEntry,
        key: StreamKey,
    ) -> Result<StreamSource, String> {
        let (sender, receiver) = tokio::sync::mpsc::channel(CHANNEL_DEPTH);
        let partial = self.cache.create_partial(&key)?;
        let service = self.clone();
        // Decoding and encoding are blocking, CPU-bound work: keep them off
        // the async runtime's worker threads.
        std::thread::Builder::new()
            .name("kog-stream-encode".to_owned())
            .spawn(move || {
                let result = service.encode_into(entry, &key, partial, sender.clone());
                if let Err(error) = &result {
                    eprintln!("kog-server: streaming {}: {error}", key.locator);
                    let _ = sender.blocking_send(Err(std::io::Error::other(error.clone())));
                }
            })
            .map_err(|error| format!("starting the stream encoder: {error}"))?;
        Ok(StreamSource::Encoding { receiver })
    }

    fn encode_into(
        &self,
        entry: PlaylistEntry,
        key: &StreamKey,
        partial: std::fs::File,
        sender: tokio::sync::mpsc::Sender<Result<Bytes, std::io::Error>>,
    ) -> Result<(), String> {
        let decoders = kog_audio::decoder::DecoderRegistry::new(self.decoder_settings.clone());
        let source = resolve_entry(&entry, &decoders, &self.scratch)?;
        let pcm = PcmReader::open(source, self.decoder_settings.clone())?;
        let args = encoder_args(
            key.codec,
            key.bitrate_kbps,
            pcm.sample_rate(),
            pcm.channels(),
        );
        let tee = TeeWriter {
            file: partial,
            sender,
        };
        encode_to_writer(&self.encoder, &args, pcm, tee)?;
        self.cache.commit(
            key,
            &self
                .cache
                .partial_path(key)
                .to_path_buf(),
        )?;
        Ok(())
    }
}

/// Writes encoded bytes to the cache file and to the client at once.
///
/// A client that disconnects (or a filled channel) does not abort the encode:
/// finishing the cache entry is worth more than the abandoned response, since
/// every later play of that track then takes the cached path.
struct TeeWriter {
    file: std::fs::File,
    sender: tokio::sync::mpsc::Sender<Result<Bytes, std::io::Error>>,
}

impl Write for TeeWriter {
    fn write(&mut self, buffer: &[u8]) -> std::io::Result<usize> {
        self.file.write_all(buffer)?;
        let _ = self
            .sender
            .blocking_send(Ok(Bytes::copy_from_slice(buffer)));
        Ok(buffer.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.file.flush()
    }
}

/// Locate the cache root: `KOG_CACHE_DIR` when set (tests), else Kog's cache
/// directory beside cover art.
pub fn cache_root() -> PathBuf {
    if let Some(directory) = std::env::var_os("KOG_CACHE_DIR") {
        return PathBuf::from(directory).join("streams");
    }
    kog_audio::settings::setting_path("cache")
        .map(|path| path.join("streams"))
        .unwrap_or_else(|| PathBuf::from("/tmp/kog-streams"))
}

/// Scratch directory for the temporary one-line playlists used to resolve
/// entries. Kept beside the cache so it inherits the same lifetime and cleanup.
pub fn scratch_root() -> PathBuf {
    cache_root().join("scratch")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stream::StreamCache;
    use kog_audio::playlist::PlaylistLocation;

    fn cache() -> (tempfile::TempDir, StreamCache) {
        let directory = tempfile::tempdir().unwrap();
        let cache = StreamCache::new(directory.path().join("streams"), 1 << 20);
        (directory, cache)
    }

    #[test]
    fn a_cached_encode_is_served_without_re_encoding() {
        let (_dir, cache) = cache();
        let key = StreamKey::new("/music/song.flac", StreamCodec::Aac, 192);
        let entry_path = cache.entry_path(&key);
        std::fs::create_dir_all(entry_path.parent().unwrap()).unwrap();
        std::fs::write(&entry_path, b"cached bytes").unwrap();

        let service = StreamService::new(
            cache,
            DecoderSettings::default(),
            PathBuf::from("ffmpeg"),
            PathBuf::from("/tmp"),
        );
        let entry = PlaylistEntry {
            location: PlaylistLocation::Local(PathBuf::from("/music/song.flac")),
            fragment: None,
        };
        match service.open(entry, key).expect("cached stream") {
            StreamSource::Cached(path) => assert_eq!(path, entry_path),
            StreamSource::Encoding { .. } => panic!("a cached entry must not re-encode"),
        }
    }

    #[test]
    fn the_tee_writer_mirrors_bytes_and_survives_a_dead_client() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("out.bin");
        let (sender, mut receiver) = tokio::sync::mpsc::channel(2);
        let mut tee = TeeWriter {
            file: std::fs::File::create(&path).unwrap(),
            sender,
        };
        tee.write_all(b"first").unwrap();
        drop(receiver); // client went away
        // A disconnected client must not fail the encode: the cache matters.
        tee.write_all(b"second").unwrap();
        tee.flush().unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"firstsecond");

        // And with a live client the bytes are mirrored.
        let (sender, mut receiver) = tokio::sync::mpsc::channel(2);
        let mut tee = TeeWriter {
            file: std::fs::File::create(directory.path().join("out2.bin")).unwrap(),
            sender,
        };
        tee.write_all(b"streamed").unwrap();
        let received = receiver.try_recv().unwrap().unwrap();
        assert_eq!(received.as_ref(), b"streamed");
    }
}
