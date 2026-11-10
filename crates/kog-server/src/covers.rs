//! Cover lookups outlive individual HTTP requests. Web clients poll briefly
//! while local probes and remote searches run in bounded background jobs.

use std::collections::HashMap;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use axum::body::Bytes;
use kog_audio::{cover_art, playlist::PlaylistEntry};
use tokio::sync::{Semaphore, watch};

use crate::service::StreamService;

const RESULT_TTL: Duration = Duration::from_secs(5 * 60);
const MAX_JOBS: usize = 64;
const MAX_RESULT_BYTES: usize = 32 * 1024 * 1024;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum CoverResult {
    Pending,
    Ready(Bytes),
    Missing,
}

struct Job {
    result: watch::Receiver<CoverResult>,
    created: Instant,
    accessed: Instant,
}

#[derive(Default)]
struct Jobs(Mutex<HashMap<String, Job>>);

impl Jobs {
    fn prune(jobs: &mut HashMap<String, Job>) {
        jobs.retain(|_, job| {
            *job.result.borrow() == CoverResult::Pending || job.created.elapsed() < RESULT_TTL
        });
        let mut bytes: usize = jobs
            .values()
            .map(|job| match &*job.result.borrow() {
                CoverResult::Ready(bytes) => bytes.len(),
                _ => 0,
            })
            .sum();
        while jobs.len() >= MAX_JOBS || bytes > MAX_RESULT_BYTES {
            let oldest = jobs
                .iter()
                .filter(|(_, job)| *job.result.borrow() != CoverResult::Pending)
                .min_by_key(|(_, job)| job.accessed)
                .map(|(key, _)| key.clone());
            let Some(oldest) = oldest else { break };
            if let Some(job) = jobs.remove(&oldest)
                && let CoverResult::Ready(data) = &*job.result.borrow()
            {
                bytes = bytes.saturating_sub(data.len());
            }
        }
    }

    /// Only the first caller starts work. Pending and failed lookups are shared
    /// too, so missing artwork never triggers one provider search per client.
    fn get_or_start(
        self: &Arc<Self>,
        key: String,
        resolve: impl Future<Output = Option<Bytes>> + Send + 'static,
    ) -> watch::Receiver<CoverResult> {
        let mut jobs = self.0.lock().unwrap_or_else(|error| error.into_inner());
        Self::prune(&mut jobs);
        if let Some(job) = jobs.get_mut(&key) {
            job.accessed = Instant::now();
            return job.result.clone();
        }
        let (sender, receiver) = watch::channel(CoverResult::Pending);
        if jobs.len() >= MAX_JOBS {
            // No room while every job is running. A later short poll can try
            // again; do not create an unbounded queue of obsolete lookups.
            return receiver;
        }
        jobs.insert(
            key,
            Job {
                result: receiver.clone(),
                created: Instant::now(),
                accessed: Instant::now(),
            },
        );
        let cache = self.clone();
        tokio::spawn(async move {
            let result = resolve
                .await
                .map(CoverResult::Ready)
                .unwrap_or(CoverResult::Missing);
            sender.send_replace(result);
            Self::prune(&mut cache.0.lock().unwrap_or_else(|error| error.into_inner()));
        });
        receiver
    }
}

pub(crate) async fn wait(mut receiver: watch::Receiver<CoverResult>) -> CoverResult {
    loop {
        let result = receiver.borrow_and_update().clone();
        if result != CoverResult::Pending || receiver.changed().await.is_err() {
            return result;
        }
    }
}

pub(crate) struct CoverService {
    tracks: Arc<Jobs>,
    albums: Arc<Jobs>,
    probes: Arc<Semaphore>,
    downloads: Arc<Semaphore>,
    cache_dir: PathBuf,
}

impl Default for CoverService {
    fn default() -> Self {
        Self {
            tracks: Arc::new(Jobs::default()),
            albums: Arc::new(Jobs::default()),
            probes: Arc::new(Semaphore::new(2)),
            downloads: Arc::new(Semaphore::new(4)),
            cache_dir: directories::ProjectDirs::from("org", "Kog", "Kog")
                .map(|directories| cover_art::cache_directory(directories.cache_dir()))
                .unwrap_or_else(|| std::env::temp_dir().join("kog-covers")),
        }
    }
}

fn track_key(file: &Path, allow_download: bool) -> String {
    format!("{allow_download}:{file:?}")
}

enum Prepared {
    Local(Bytes),
    Album { artist: String, album: String, tagged: bool },
}

/// The album a song belongs to, for keeping its cover consistent: its folder
/// and album tag, or just the folder when it has no album tag. Every song of
/// one album then shows the cover first found for any of them, rather than
/// each its own embedded picture or search result.
fn album_cover_key(file: &Path, album: &str) -> String {
    let folder = file.parent().unwrap_or(file);
    cover_art::cache_key("album", &format!("{}\0{}", folder.display(), album.trim()))
}

/// The album's cover, if one of its songs already found it (and the listener
/// hasn't rejected it since).
fn album_cover(cache_dir: &Path, key: &str) -> Option<Bytes> {
    cover_art::cache_lookup(cache_dir, key)
        .and_then(|path| std::fs::read(path).ok())
        .filter(|bytes| cover_art::sniff_image_kind(bytes).is_some())
        .filter(|bytes| !cover_art::is_rejected(cache_dir, bytes))
        .map(Bytes::from)
}

/// Make `bytes` the album's cover, unless another of its songs got there
/// first; returns the album's cover either way.
fn adopt_album_cover(cache_dir: &Path, key: &str, bytes: Bytes) -> Bytes {
    if let Some(existing) = album_cover(cache_dir, key) {
        return existing;
    }
    let _ = cover_art::store_cache(cache_dir, key, &bytes);
    bytes
}

impl CoverService {
    /// The listener marked these bytes a wrong cover: never serve them again
    /// and forget finished lookups, so the next request searches afresh.
    pub(crate) fn reject(&self, bytes: &[u8]) -> bool {
        if !cover_art::reject_cover(&self.cache_dir, bytes) {
            return false;
        }
        for jobs in [&self.tracks, &self.albums] {
            jobs.0
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .retain(|_, job| *job.result.borrow() == CoverResult::Pending);
        }
        true
    }

    pub(crate) fn lookup(
        self: &Arc<Self>,
        file: PathBuf,
        streams: Arc<StreamService>,
        allow_download: bool,
    ) -> watch::Receiver<CoverResult> {
        let service = self.clone();
        self.tracks
            .get_or_start(track_key(&file, allow_download), async move {
                let permit = service.probes.clone().acquire_owned().await.ok()?;
                let probe_file = file.clone();
                let probe_cache_dir = service.cache_dir.clone();
                let prepared = tokio::task::spawn_blocking(move || {
                    // The album's tag (and the artist, for a search) from
                    // the song's metadata; a song that can't be read still
                    // shares its folder's cover.
                    let properties = PlaylistEntry::from_locator(
                        "local",
                        &probe_file.to_string_lossy(),
                        "",
                        None,
                    )
                    .ok()
                    .and_then(|entry| streams.probe_entry(entry).ok());
                    let tagged = properties.is_some();
                    let (artist, album) = properties
                        .map(|properties| {
                            (properties.artist.unwrap_or_default(), properties.album.unwrap_or_default())
                        })
                        .unwrap_or_default();
                    let album_key = album_cover_key(&probe_file, &album);
                    if let Some(bytes) = album_cover(&probe_cache_dir, &album_key) {
                        return Some((album_key, Prepared::Local(bytes)));
                    }
                    if let Some(bytes) = cover_art::embedded_cover_bytes(&probe_file)
                        .or_else(|| cover_art::sibling_cover_bytes(&probe_file))
                        .filter(|bytes| !cover_art::is_rejected(&probe_cache_dir, bytes))
                    {
                        let bytes = adopt_album_cover(&probe_cache_dir, &album_key, Bytes::from(bytes));
                        return Some((album_key, Prepared::Local(bytes)));
                    }
                    Some((album_key, Prepared::Album { artist, album, tagged }))
                })
                .await
                .ok()
                .flatten();
                drop(permit);
                let (album_key, prepared) = prepared?;
                match prepared {
                    Prepared::Local(bytes) => Some(bytes),
                    // Without readable metadata there is nothing to search for.
                    Prepared::Album { tagged: false, .. } => None,
                    Prepared::Album { artist, album, .. } => {
                        let lookup_album = cover_art::fallback_album(&file, &album);
                        let (key, _) =
                            cover_art::track_cache_key(&artist, &album, &lookup_album, &file);
                        let download_service = service.clone();
                        let result = service.albums.get_or_start(
                            format!("{allow_download}:{key}"),
                            async move {
                                let _permit = download_service
                                    .downloads
                                    .clone()
                                    .acquire_owned()
                                    .await
                                    .ok()?;
                                tokio::task::spawn_blocking(move || {
                                    cached_or_downloaded_cover(
                                        &file,
                                        &artist,
                                        &album,
                                        &download_service.cache_dir,
                                        allow_download,
                                        crate::cover_network::fetch,
                                    )
                                    .map(Bytes::from)
                                })
                                .await
                                .ok()
                                .flatten()
                            },
                        );
                        match wait(result).await {
                            CoverResult::Ready(bytes) => {
                                let cache_dir = service.cache_dir.clone();
                                tokio::task::spawn_blocking(move || {
                                    adopt_album_cover(&cache_dir, &album_key, bytes)
                                })
                                .await
                                .ok()
                            }
                            _ => None,
                        }
                    }
                }
            })
    }
}

fn cached_or_downloaded_cover(
    file: &Path,
    artist: &str,
    tagged_album: &str,
    cache_dir: &Path,
    allow_download: bool,
    fetch: impl Fn(&str, u32) -> Option<Vec<u8>> + Sync,
) -> Option<Vec<u8>> {
    let album = cover_art::fallback_album(file, tagged_album);
    if album.is_empty() {
        return None;
    }
    let (key, may_download) = cover_art::track_cache_key(artist, tagged_album, &album, file);
    if let Some(bytes) = cover_art::cache_lookup(cache_dir, &key)
        .and_then(|path| std::fs::read(path).ok())
        .filter(|bytes| cover_art::sniff_image_kind(bytes).is_some())
    {
        return Some(bytes);
    }
    if !may_download || !allow_download || cover_art::recent_miss(cache_dir, &key) {
        return None;
    }
    let found = cover_art::download_cover(
        file,
        artist,
        &album,
        fetch,
        || false,
        |bytes| {
            let _ = cover_art::store_cache(cache_dir, &key, &bytes);
            Some(bytes)
        },
    );
    if found.is_none() {
        cover_art::record_miss(cache_dir, &key);
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[tokio::test]
    async fn concurrent_lookups_share_work_and_reuse_missing_results() {
        let jobs = Arc::new(Jobs::default());
        let calls = Arc::new(AtomicUsize::new(0));
        let gate = Arc::new(Semaphore::new(0));
        let mut results = Vec::new();
        for _ in 0..12 {
            let calls = calls.clone();
            let gate = gate.clone();
            results.push(jobs.get_or_start("same album".into(), async move {
                calls.fetch_add(1, Ordering::SeqCst);
                let _permit = gate.acquire().await.unwrap();
                None
            }));
        }
        tokio::task::yield_now().await;
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert!(
            results
                .iter()
                .all(|result| *result.borrow() == CoverResult::Pending)
        );
        gate.add_permits(1);
        for result in results {
            assert_eq!(wait(result).await, CoverResult::Missing);
        }
        let cached = jobs.get_or_start("same album".into(), async {
            panic!("a missing cover must not trigger another provider search")
        });
        assert_eq!(wait(cached).await, CoverResult::Missing);

        jobs.0
            .lock()
            .unwrap()
            .get_mut("same album")
            .unwrap()
            .created = Instant::now() - RESULT_TTL;
        let retried = jobs.get_or_start("same album".into(), async {
            Some(Bytes::from_static(b"new cover"))
        });
        assert_eq!(
            wait(retried).await,
            CoverResult::Ready(Bytes::from_static(b"new cover"))
        );
    }

    #[tokio::test]
    async fn pending_art_releases_http_requests_and_keeps_audio_available() {
        use axum::{
            body::Body,
            http::{Request, StatusCode},
        };
        use tower::ServiceExt;

        let directory = tempfile::tempdir().unwrap();
        let track = directory.path().join("track.wav");
        std::fs::write(&track, []).unwrap();
        let track = track.canonicalize().unwrap();
        let state = crate::routes::AppState::new(
            crate::ServerConfig::default(),
            "test",
            StreamService::new(
                crate::StreamCache::new(directory.path().join("streams"), 1 << 20),
                kog_audio::decoder::DecoderSettings::default(),
                directory.path().join("scratch"),
            ),
            crate::api::Library::new(
                Some(directory.path().to_path_buf()),
                kog_core::db::LibraryDb::open_in_memory().unwrap(),
            ),
        );
        let (release, pending) = tokio::sync::oneshot::channel();
        let jpeg = Bytes::from_static(&[0xff, 0xd8, 0xff, 0xe0]);
        let image = jpeg.clone();
        // Hold the actual endpoint's lookup indefinitely, like a slow provider.
        let job = state.covers.tracks.get_or_start(
            track_key(
                &track,
                kog_audio::settings::AppSettings::load().download_cover_art,
            ),
            async move {
                pending.await.unwrap();
                Some(image)
            },
        );
        let path: String =
            url::form_urlencoded::byte_serialize(track.to_str().unwrap().as_bytes()).collect();
        let uri = format!("/api/art?kind=local&path={path}&background=true");
        let stream_key =
            crate::StreamKey::new(track.to_string_lossy(), crate::StreamCodec::Aac, 192);
        let cached_audio = state.streams.cache().entry_path(&stream_key);
        std::fs::create_dir_all(cached_audio.parent().unwrap()).unwrap();
        std::fs::write(cached_audio, b"cached audio").unwrap();
        let app = crate::routes::router(state);
        tokio::time::timeout(Duration::from_secs(2), async {
            for _ in 0..8 {
                let response = app
                    .clone()
                    .oneshot(Request::builder().uri(&uri).body(Body::empty()).unwrap())
                    .await
                    .unwrap();
                assert_eq!(response.status(), StatusCode::ACCEPTED);
                assert_eq!(response.headers()["retry-after"], "1");
                assert_eq!(response.headers()["cache-control"], "no-store");
            }
            let response = app
                .clone()
                .oneshot(
                    Request::builder()
                        .uri(format!("/api/stream?kind=local&path={path}&codec=aac"))
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::OK);
            assert_eq!(
                axum::body::to_bytes(response.into_body(), 1024)
                    .await
                    .unwrap(),
                "cached audio"
            );
        })
        .await
        .expect("art requests must finish while the provider is still blocked");
        assert_eq!(*job.borrow(), CoverResult::Pending);

        // The legacy request still waits for an image, and the next web poll
        // receives that same completed image.
        let legacy = app.clone().oneshot(
            Request::builder()
                .uri(format!("/api/art?kind=local&path={path}"))
                .body(Body::empty())
                .unwrap(),
        );
        let legacy = tokio::spawn(legacy);
        release.send(()).unwrap();
        assert_eq!(wait(job).await, CoverResult::Ready(jpeg.clone()));
        for response in [
            legacy.await.unwrap().unwrap(),
            app.oneshot(Request::builder().uri(&uri).body(Body::empty()).unwrap())
                .await
                .unwrap(),
        ] {
            assert_eq!(response.status(), StatusCode::OK);
            assert_eq!(response.headers()["content-type"], "image/jpeg");
            assert_eq!(
                axum::body::to_bytes(response.into_body(), 1024)
                    .await
                    .unwrap(),
                jpeg
            );
        }
    }

    #[test]
    fn web_cover_uses_shared_search_then_cache_without_redownloading() {
        let directory = tempfile::tempdir().unwrap();
        let file = directory.path().join("song.mp3");
        let jpeg = vec![0xff, 0xd8, 0xff, 0xe0];
        let mut requested = Vec::new();
        let resolved = cached_or_downloaded_cover(
            &file,
            "Artist",
            "Album",
            directory.path(),
            true,
            |url, _| {
                requested.push(url.to_owned());
                if url.starts_with("https://api.deezer.com/") {
                    Some(br#"{"data":[{"title":"Album","artist":{"name":"Artist"},"cover_xl":"https://example.test/cover.jpg"}]}"#.to_vec())
                } else if url == "https://example.test/cover.jpg" {
                    Some(jpeg.clone())
                } else {
                    None
                }
            },
        );
        assert_eq!(resolved, Some(jpeg.clone()));
        assert_eq!(requested.len(), 2);
        assert_eq!(
            cached_or_downloaded_cover(
                &file,
                "Artist",
                "Album",
                directory.path(),
                false,
                |_, _| panic!("cached art should not request the network"),
            ),
            Some(jpeg),
        );
        assert_eq!(
            cached_or_downloaded_cover(&file, "", "", directory.path(), true, |_, _| panic!(
                "untagged files should not request the network"
            ),),
            None,
        );
    }
}
