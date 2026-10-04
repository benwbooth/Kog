//! In-process adapter to the same library API used by Web and TUI. No socket,
//! web view, child process, or second implementation of the library rules.
use crate::{api::Library, routes::AppState};
use axum::{
    Router,
    body::{Body, to_bytes},
    http::Request,
};
use serde_json::{Value, json};
use std::{
    path::PathBuf,
    sync::{Mutex, OnceLock},
};
use tower::ServiceExt;

struct Backend {
    root: PathBuf,
    storage: PathBuf,
    router: Router,
}
static BACKEND: Mutex<Option<Backend>> = Mutex::new(None);
static RUNTIME: OnceLock<Result<tokio::runtime::Runtime, String>> = OnceLock::new();

fn router(root: &std::path::Path, storage: &std::path::Path) -> Result<Router, String> {
    std::fs::create_dir_all(root).map_err(|e| e.to_string())?;
    let db = kog_core::db::LibraryDb::open_at(&storage.join("library.sqlite"))?;
    rebase_library(&db, root, storage)?;
    let library = Library::new(Some(root.to_owned()), db);
    let config = crate::ServerConfig::default();
    let streams = crate::StreamService::new(
        crate::StreamCache::new(storage.join("streams"), 16 * 1024 * 1024),
        kog_audio::decoder::DecoderSettings::default(),
        storage.join("scratch"),
    );
    let state = AppState::with_radio(
        config,
        env!("CARGO_PKG_VERSION"),
        streams,
        library,
        crate::radio::Radio::new(
            Some(root.to_owned()),
            Some(storage.join("radio.json")),
            false,
        ),
    );
    Ok(crate::api::router()
        .merge(crate::radio::router())
        .with_state(state))
}

// iOS assigns a new absolute sandbox path on some app updates. Entries keep
// their relative place under Imports while the shared database stays intact.
fn rebase_library(
    db: &kog_core::db::LibraryDb,
    root: &std::path::Path,
    storage: &std::path::Path,
) -> Result<(), String> {
    let marker = storage.join("imports-root.txt");
    if let Ok(previous) = std::fs::read_to_string(&marker) {
        let previous = std::path::Path::new(&previous);
        if previous != root {
            let rebase = |entry: &mut kog_core::db::StoredEntry| {
                if matches!(entry.kind.as_str(), "local" | "archive") {
                    if let Ok(relative) = std::path::Path::new(&entry.path).strip_prefix(previous) {
                        entry.path = root.join(relative).to_string_lossy().into_owned();
                    }
                }
            };
            for playlist in db.list_playlists()?.into_iter().filter(|p| p.id != 0) {
                let mut entries = db.playlist_entries(playlist.id)?;
                for entry in &mut entries {
                    rebase(entry);
                }
                db.replace_entries(playlist.id, &entries)?;
            }
            for mut entry in db.starred_entries()? {
                let old = crate::media_filter::locator_for(&entry)?;
                rebase(&mut entry);
                let new = crate::media_filter::locator_for(&entry)?;
                if old != new {
                    db.set_star(
                        &new,
                        &entry.kind,
                        &entry.path,
                        &entry.entry,
                        entry.fragment.as_deref(),
                        true,
                    )?;
                    db.set_star(
                        &old,
                        &entry.kind,
                        &entry.path,
                        &entry.entry,
                        entry.fragment.as_deref(),
                        false,
                    )?;
                }
            }
        }
    }
    std::fs::write(marker, root.to_string_lossy().as_bytes()).map_err(|e| e.to_string())
}

pub fn request(input: Value) -> Result<Value, String> {
    let root = PathBuf::from(input["root"].as_str().ok_or("Missing imports root")?);
    let storage = PathBuf::from(input["storage"].as_str().ok_or("Missing library storage")?);
    let router = {
        let mut backend = BACKEND.lock().map_err(|e| e.to_string())?;
        if backend
            .as_ref()
            .is_none_or(|state| state.root != root || state.storage != storage)
        {
            *backend = Some(Backend {
                router: router(&root, &storage)?,
                root,
                storage,
            });
        }
        backend.as_ref().unwrap().router.clone()
    };
    let runtime = RUNTIME
        .get_or_init(|| {
            tokio::runtime::Builder::new_multi_thread()
                .worker_threads(2)
                .enable_all()
                .build()
                .map_err(|e| e.to_string())
        })
        .as_ref()
        .map_err(Clone::clone)?;
    let method = input["method"].as_str().unwrap_or("GET");
    let uri = input["uri"].as_str().ok_or("Missing API path")?;
    // Only JSON library operations are exposed; audio stays in the native player.
    if ![
        "/api/library",
        "/api/metadata",
        "/api/expand",
        "/api/playlists",
        "/api/stars",
        "/api/radio",
    ]
    .iter()
    .any(|prefix| {
        uri == *prefix
            || uri.starts_with(&format!("{prefix}/"))
            || uri.starts_with(&format!("{prefix}?"))
    }) {
        return Err("Unsupported on-device operation".into());
    }
    let body = input
        .get("body")
        .filter(|b| !b.is_null())
        .map_or(String::new(), Value::to_string);
    let request = Request::builder()
        .method(method)
        .uri(uri)
        .header("content-type", "application/json")
        .body(Body::from(body))
        .map_err(|e| e.to_string())?;
    runtime.block_on(async {
        let response = router.oneshot(request).await.map_err(|e| e.to_string())?;
        let status = response.status().as_u16();
        let bytes = to_bytes(response.into_body(), 32 * 1024 * 1024)
            .await
            .map_err(|e| e.to_string())?;
        let body: Value = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
        Ok(json!({ "status": status, "body": body }))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn offline_library_round_trip_uses_shared_storage() {
        let temp = tempfile::tempdir().unwrap();
        let call = |method: &str, uri: &str, body: Value| {
            request(json!({
                "root": temp.path().join("music"), "storage": temp.path().join("state"),
                "method": method, "uri": uri, "body": body
            }))
            .unwrap()
        };
        assert_eq!(call("GET", "/api/library", Value::Null)["status"], 200);
        let created = call("POST", "/api/playlists", json!({"name":"Offline"}));
        let id = created["body"]["id"].as_i64().unwrap();
        let entry = json!({"kind":"local", "path": temp.path().join("music/song.wav"), "entry":"", "fragment":""});
        assert_eq!(
            call(
                "POST",
                &format!("/api/playlists/{id}/entries"),
                json!({"entries":[entry.clone()]})
            )["status"],
            200
        );
        assert_eq!(
            call("GET", &format!("/api/playlists/{id}"), Value::Null)["body"]["entries"][0]["path"],
            entry["path"]
        );
        let mut star = entry;
        star["starred"] = json!(true);
        assert_eq!(call("POST", "/api/stars", star)["status"], 200);
        assert_eq!(
            call("GET", "/api/stars", Value::Null)["body"]["entries"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
        let export = call("GET", &format!("/api/playlists/{id}/export"), Value::Null);
        assert!(
            export["body"]["text"]
                .as_str()
                .unwrap()
                .contains("song.wav")
        );
        assert_eq!(
            call(
                "PUT",
                &format!("/api/playlists/{id}/entries"),
                json!({"entries":[]})
            )["status"],
            200
        );
        assert_eq!(
            call("GET", &format!("/api/playlists/{id}"), Value::Null)["body"]["entries"],
            json!([])
        );
        assert_eq!(
            call("DELETE", &format!("/api/playlists/{id}"), Value::Null)["status"],
            200
        );
    }
}
