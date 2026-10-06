//! One cancellable cover lookup for the current track. The resulting blob is
//! shared by both images and Media Session, without more server requests.

use super::*;

const PLACEHOLDER: &str = "/icons/cover-placeholder.svg";

#[derive(Default)]
struct Loading {
    generation: u64,
    controller: Option<web_sys::AbortController>,
    object_url: Option<String>,
}

impl Drop for Loading {
    fn drop(&mut self) {
        if let Some(controller) = &self.controller {
            controller.abort();
        }
        if let Some(url) = &self.object_url {
            let _ = web_sys::Url::revoke_object_url(url);
        }
    }
}

pub fn source(request_url: Memo<Option<String>>) -> Memo<String> {
    let loading = StoredValue::new_local(Loading::default());
    let resolved = RwSignal::new(None::<(String, String)>);
    Effect::new(move |_| {
        let url = request_url.get();
        loading.update_value(|state| {
            *state = Loading {
                generation: state.generation.wrapping_add(1),
                controller: web_sys::AbortController::new().ok(),
                object_url: None,
            };
        });
        resolved.set(None);
        let Some(url) = url else { return };
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
                loading.update_value(|state| state.object_url = Some(object_url.clone()));
                resolved.set(Some((url, object_url)));
                return;
            }
        });
    });
    Memo::new(move |_| {
        let url = request_url.get();
        resolved.with(|resolved| {
            resolved
                .as_ref()
                .filter(|(request, _)| url.as_ref() == Some(request))
                .map(|(_, object_url)| object_url.clone())
                .unwrap_or_else(|| PLACEHOLDER.to_owned())
        })
    })
}
