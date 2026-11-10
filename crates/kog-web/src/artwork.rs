//! One cancellable cover lookup for the current track. The resulting blob is
//! shared by both images and Media Session, without more server requests,
//! and cached for the page's lifetime by song and by album.

use super::*;

const PLACEHOLDER: &str = "/icons/cover-placeholder.svg";

#[derive(Default)]
struct Loading {
    generation: u64,
    controller: Option<web_sys::AbortController>,
}

impl Drop for Loading {
    fn drop(&mut self) {
        if let Some(controller) = &self.controller {
            controller.abort();
        }
    }
}

/// Covers already fetched, by request URL and by album, kept as object URLs
/// for the page's lifetime (bounded): the next song of an album shows its
/// cover at once instead of asking the server again.
#[derive(Default)]
struct Covers {
    by_url: HashMap<String, String>,
    by_album: HashMap<String, String>,
    order: Vec<String>,
}

const CACHED_COVERS: usize = 64;

impl Covers {
    fn insert(&mut self, url: &str, album: &str, object_url: &str) {
        if !self.by_url.contains_key(url) {
            self.order.push(object_url.to_owned());
        }
        self.by_url.insert(url.to_owned(), object_url.to_owned());
        if !album.is_empty() {
            self.by_album.insert(album.to_owned(), object_url.to_owned());
        }
        while self.order.len() > CACHED_COVERS {
            let old = self.order.remove(0);
            self.by_url.retain(|_, cached| cached != &old);
            self.by_album.retain(|_, cached| cached != &old);
            let _ = web_sys::Url::revoke_object_url(&old);
        }
    }

    fn lookup(&self, url: &str, album: &str) -> Option<String> {
        self.by_url
            .get(url)
            .or_else(|| (!album.is_empty()).then(|| self.by_album.get(album)).flatten())
            .cloned()
    }
}

/// `request_url` is the cover request for the current song, with a key for
/// its album (artist and album title; empty when unknown).
pub fn source(request_url: Memo<Option<(String, String)>>) -> Memo<String> {
    let loading = StoredValue::new_local(Loading::default());
    let covers = StoredValue::new_local(Covers::default());
    let resolved = RwSignal::new(None::<(String, String)>);
    Effect::new(move |_| {
        let request = request_url.get();
        loading.update_value(|state| {
            if let Some(controller) = &state.controller {
                controller.abort();
            }
            state.generation = state.generation.wrapping_add(1);
            state.controller = web_sys::AbortController::new().ok();
        });
        let Some((url, album)) = request else {
            resolved.set(None);
            return;
        };
        if let Some(object_url) = covers.with_value(|covers| covers.lookup(&url, &album)) {
            resolved.set(Some((url, object_url)));
            return;
        }
        let (generation, signal) = loading.with_value(|state| {
            (
                state.generation,
                state
                    .controller
                    .as_ref()
                    .map(web_sys::AbortController::signal),
            )
        });
        leptos::task::spawn_local(async move {
            let is_current = move || {
                loading
                    .try_with_value(|state| state.generation == generation)
                    .unwrap_or(false)
            };
            while is_current() {
                let response = Request::get(&url)
                    .header("X-Kog-Device", &device_id())
                    .abort_signal(signal.as_ref())
                    .send()
                    .await;
                if !is_current() {
                    return;
                }
                let Ok(response) = response else { return };
                if response.status() == 202 {
                    let _ = sleep_ms(1000).await;
                    continue;
                }
                if response.status() != 200 {
                    // No cover for this song: the placeholder, not the last
                    // song's cover.
                    resolved.set(Some((url, PLACEHOLDER.to_owned())));
                    return;
                }
                let mime = response
                    .headers()
                    .get("content-type")
                    .unwrap_or_else(|| "image/jpeg".to_owned());
                let Ok(bytes) = response.binary().await else {
                    return;
                };
                if !is_current() {
                    return;
                }
                let parts = js_sys::Array::new();
                parts.push(&js_sys::Uint8Array::from(bytes.as_slice()));
                let options = web_sys::BlobPropertyBag::new();
                options.set_type(&mime);
                let Ok(blob) =
                    web_sys::Blob::new_with_u8_slice_sequence_and_options(&parts, &options)
                else {
                    return;
                };
                let Ok(object_url) = web_sys::Url::create_object_url_with_blob(&blob) else {
                    return;
                };
                covers.update_value(|covers| covers.insert(&url, &album, &object_url));
                resolved.set(Some((url, object_url)));
                return;
            }
        });
    });
    // While the next cover loads, the last one stays up rather than flashing
    // the placeholder; a new album's cover replaces it as soon as it arrives.
    Memo::new(move |previous: Option<&String>| {
        let current = request_url.get();
        resolved.with(|resolved| match (resolved, &current) {
            (Some((request, object_url)), Some((url, _))) if request == url => object_url.clone(),
            (_, None) => PLACEHOLDER.to_owned(),
            _ => previous.cloned().unwrap_or_else(|| PLACEHOLDER.to_owned()),
        })
    })
}

/// Playlist tiles on phones. The server picks each tile's songs (different
/// images, in playlist order) and finds their covers in the background; the
/// page loads those images, kept for its lifetime by folder (the songs of
/// an album in one folder share a cover) with a fingerprint of the image,
/// so a grid never shows the same art twice.
type FolderCover = (u64, String);

thread_local! {
    static FOLDER_COVERS: RefCell<HashMap<String, FolderCover>> = RefCell::new(HashMap::new());
}

/// How soon to ask the server again while it is still finding tile covers,
/// doubling up to the maximum.
pub const TILE_POLL_MS: i32 = 2_000;
pub const TILE_POLL_MAX_MS: i32 = 15_000;

pub fn fingerprint(bytes: &[u8]) -> u64 {
    // FNV-1a: identical images from different folders compare equal.
    bytes.iter().fold(0xcbf2_9ce4_8422_2325_u64, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x0100_0000_01b3)
    })
}

/// The cover of the song `path`, addressed as the now-playing art is.
/// The server has usually found it already; a lookup still running is
/// polled for a while, then given up on.
async fn folder_cover(base: &str, token: &str, path: &str) -> Option<FolderCover> {
    let url = format!(
        "{base}/api/art?kind=local&path={}&token={}&background=true",
        url_encode(path),
        url_encode(token),
    );
    for _ in 0..20 {
        let response = Request::get(&url)
            .header("X-Kog-Device", &device_id())
            .send()
            .await
            .ok()?;
        if response.status() == 202 {
            let _ = sleep_ms(1000).await;
            continue;
        }
        if response.status() != 200 {
            return None;
        }
        let mime = response
            .headers()
            .get("content-type")
            .unwrap_or_else(|| "image/jpeg".to_owned());
        let bytes = response.binary().await.ok()?;
        let parts = js_sys::Array::new();
        parts.push(&js_sys::Uint8Array::from(bytes.as_slice()));
        let options = web_sys::BlobPropertyBag::new();
        options.set_type(&mime);
        let blob = web_sys::Blob::new_with_u8_slice_sequence_and_options(&parts, &options).ok()?;
        let object_url = web_sys::Url::create_object_url_with_blob(&blob).ok()?;
        return Some((fingerprint(&bytes), object_url));
    }
    None
}

/// The images of up to `want` of a tile's songs (`paths`, as the server
/// chose them), each a different cover.
pub async fn playlist_covers(base: String, token: String, paths: Vec<String>, want: usize) -> Vec<String> {
    let mut covers = Vec::<FolderCover>::new();
    for path in paths {
        if covers.len() >= want {
            break;
        }
        let folder = path.rsplit_once('/').map(|(folder, _)| folder).unwrap_or_default();
        let key = format!("{base}|{folder}");
        let cached = FOLDER_COVERS.with(|cache| cache.borrow().get(&key).cloned());
        let cover = match cached {
            Some(cover) => Some(cover),
            None => {
                // A miss isn't remembered: the server only names songs whose
                // covers it has, so a failure here is passing.
                let cover = folder_cover(&base, &token, &path).await;
                if let Some(cover) = &cover {
                    FOLDER_COVERS.with(|cache| cache.borrow_mut().insert(key, cover.clone()));
                }
                cover
            }
        };
        if let Some((print, object_url)) = cover
            && !covers.iter().any(|(seen, _)| *seen == print)
        {
            covers.push((print, object_url));
        }
    }
    covers.into_iter().map(|(_, object_url)| object_url).collect()
}
