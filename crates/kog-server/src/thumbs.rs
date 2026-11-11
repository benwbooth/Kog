//! Small covers for the phone's song rows. A row asks
//! `GET /api/art/thumb` which image shows its song's cover and loads that
//! image from `GET /api/art/thumbs/{image}`. The image's name is its
//! fingerprint, so every song of an album shares one image the browser keeps
//! for good. When the row knows the song's album (from its tags), the answer
//! comes straight from the album's entry in the cover cache, without reading
//! the song again (which for chiptunes means playing it through); otherwise
//! the ordinary cover lookup runs, and the row asks again while it is busy.
//! Thumbnails are made once per cover and kept on disk beside the cache.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

use axum::extract::{Path as UrlPath, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};

use crate::covers::CoverResult;
use crate::routes::AppState;

/// The thumbnail's side in pixels: sharp at three device pixels per point
/// for a row's 40-point cover.
const SIZE: u32 = 120;
const DIRECTORY: &str = "cover-thumbs";
/// Answers remembered in memory, before starting over.
const REMEMBERED: usize = 8192;
/// How long a lookup may run before the row is told to ask again.
const LOOKUP_WAIT: std::time::Duration = std::time::Duration::from_millis(400);

pub(crate) struct Thumbs {
    dir: PathBuf,
    /// Album cover keys and song paths to the image showing their cover
    /// (empty: the song has none).
    names: Mutex<HashMap<String, String>>,
    /// The cover service's change count the names were found under.
    generation: AtomicU64,
}

#[derive(serde::Deserialize)]
pub struct ThumbQuery {
    pub kind: Option<String>,
    pub path: Option<String>,
    /// The song's album tag, when the client knows it (empty for none).
    pub album: Option<String>,
}

fn fnv(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x0100_0000_01b3)
    })
}

fn valid_name(name: &str) -> bool {
    name.len() == 16 && name.bytes().all(|byte| byte.is_ascii_hexdigit())
}

/// A square JPEG of the cover's middle.
fn shrink(bytes: &[u8]) -> Option<Vec<u8>> {
    let image = image::load_from_memory(bytes).ok()?;
    let small = image
        .resize_to_fill(SIZE, SIZE, image::imageops::FilterType::Triangle)
        .to_rgb8();
    let mut out = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, 82)
        .encode_image(&small)
        .ok()?;
    Some(out)
}

impl Thumbs {
    pub(crate) fn beside(cover_cache: &Path) -> Self {
        Self {
            dir: cover_cache
                .parent()
                .map(|parent| parent.join(DIRECTORY))
                .unwrap_or_else(|| std::env::temp_dir().join("kog-cover-thumbs")),
            names: Mutex::default(),
            generation: AtomicU64::new(0),
        }
    }

    /// Forget the names when covers were changed by hand since (see
    /// `CoverService::follow_changes`).
    pub(crate) fn follow(&self, generation: u64) {
        if self.generation.swap(generation, Ordering::Relaxed) != generation {
            self.forget();
        }
    }

    fn remembered(&self, key: &str) -> Option<String> {
        self.names
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .get(key)
            .cloned()
    }

    fn remember(&self, keys: &[&str], name: &str) {
        let mut names = self.names.lock().unwrap_or_else(|error| error.into_inner());
        if names.len() >= REMEMBERED {
            names.clear();
        }
        for key in keys {
            names.insert((*key).to_owned(), name.to_owned());
        }
    }

    /// A cover was rejected: its songs look again.
    pub(crate) fn forget(&self) {
        self.names
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .clear();
    }

    /// The thumbnail's name for these cover bytes, made on first use.
    fn thumbnail(&self, cover: &[u8]) -> Option<String> {
        let name = format!("{:016x}", fnv(cover));
        let path = self.dir.join(&name);
        if !path.is_file() {
            let small = shrink(cover)?;
            std::fs::create_dir_all(&self.dir).ok()?;
            let temporary = path.with_extension(format!("{}.tmp", std::process::id()));
            std::fs::write(&temporary, small).ok()?;
            std::fs::rename(&temporary, &path).ok()?;
        }
        Some(name)
    }
}

/// Which image shows a cover may change (a cover chosen or rejected), so the
/// browser asks again each time a page asks; the image itself never changes.
const ANSWER_CACHING: (&str, &str) = ("cache-control", "no-cache");

fn found(name: String) -> Response {
    if name.is_empty() {
        return (StatusCode::NOT_FOUND, [ANSWER_CACHING]).into_response();
    }
    (
        [ANSWER_CACHING],
        axum::Json(serde_json::json!({ "image": name })),
    )
        .into_response()
}

/// `GET /api/art/thumb` — which thumbnail shows a song's cover:
/// `{"image": name}`, 404 for a song without one, or 202 while its cover is
/// still being looked for (ask again in a second).
pub async fn thumb(State(state): State<AppState>, Query(query): Query<ThumbQuery>) -> Response {
    if !matches!(query.kind.as_deref().unwrap_or("local"), "local" | "archive") {
        return StatusCode::NOT_FOUND.into_response();
    }
    let Some(song) = query.path.clone() else {
        return StatusCode::NOT_FOUND.into_response();
    };
    state.thumbs.follow(state.covers.follow_changes());
    if let Some(name) = state.thumbs.remembered(&song) {
        return found(name);
    }
    // The album's cover, straight from the cache: no reading the song.
    let library = state.library.clone();
    let thumbs = state.thumbs.clone();
    let cache_dir = state.covers.cache_dir().to_path_buf();
    let album = query.album.clone();
    let path = song.clone();
    let early = tokio::task::spawn_blocking(move || {
        let file = library.resolve(Some(&path)).ok().filter(|file| file.is_file())?;
        let Some(album) = album else {
            return Some((file, None));
        };
        let key = crate::covers::album_cover_key(&file, &album);
        if let Some(name) = thumbs.remembered(&key) {
            thumbs.remember(&[&path], &name);
            return Some((file, Some(name)));
        }
        let name = crate::covers::album_cover(&cache_dir, &key)
            .and_then(|cover| thumbs.thumbnail(&cover));
        if let Some(name) = &name {
            thumbs.remember(&[&key, &path], name);
        }
        Some((file, name))
    })
    .await
    .ok()
    .flatten();
    let Some((file, name)) = early else {
        return StatusCode::NOT_FOUND.into_response();
    };
    if let Some(name) = name {
        return found(name);
    }
    let result = state.covers.lookup(
        file,
        state.streams.clone(),
        kog_audio::settings::AppSettings::load().download_cover_art,
    );
    let result = tokio::time::timeout(LOOKUP_WAIT, crate::covers::wait(result.clone()))
        .await
        .unwrap_or_else(|_| result.borrow().clone());
    match result {
        CoverResult::Pending => (
            StatusCode::ACCEPTED,
            [("cache-control", "no-store"), ("retry-after", "1")],
        )
            .into_response(),
        CoverResult::Missing => {
            state.thumbs.remember(&[&song], "");
            found(String::new())
        }
        CoverResult::Ready(cover) => {
            let thumbs = state.thumbs.clone();
            let name = tokio::task::spawn_blocking(move || {
                let name = thumbs.thumbnail(&cover).unwrap_or_default();
                thumbs.remember(&[&song], &name);
                name
            })
            .await
            .unwrap_or_default();
            found(name)
        }
    }
}

/// `GET /api/art/thumbs/{image}` — a thumbnail by the name
/// `/api/art/thumb` gave; it never changes.
pub async fn thumb_image(State(state): State<AppState>, UrlPath(image): UrlPath<String>) -> Response {
    if !valid_name(&image) {
        return StatusCode::NOT_FOUND.into_response();
    }
    let path = state.thumbs.dir.join(image);
    let Ok(Ok(bytes)) = tokio::task::spawn_blocking(move || std::fs::read(path)).await else {
        return StatusCode::NOT_FOUND.into_response();
    };
    (
        [
            ("content-type", "image/jpeg"),
            ("cache-control", "private, max-age=31536000, immutable"),
        ],
        bytes,
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thumbnails_are_small_squares_named_by_their_cover() {
        let directory = tempfile::tempdir().unwrap();
        let thumbs = Thumbs::beside(&directory.path().join("covers"));
        let mut cover = Vec::new();
        image::RgbImage::from_pixel(600, 400, image::Rgb([200, 30, 30]))
            .write_to(&mut std::io::Cursor::new(&mut cover), image::ImageFormat::Png)
            .unwrap();
        let name = thumbs.thumbnail(&cover).unwrap();
        assert!(valid_name(&name));
        assert_eq!(thumbs.thumbnail(&cover).as_deref(), Some(name.as_str()));
        let small = image::load_from_memory(&std::fs::read(thumbs.dir.join(&name)).unwrap()).unwrap();
        assert_eq!((small.width(), small.height()), (SIZE, SIZE));
        assert!(thumbs.thumbnail(b"not an image").is_none());
    }
}
