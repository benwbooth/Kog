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
