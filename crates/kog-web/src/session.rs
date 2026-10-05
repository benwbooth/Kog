//! Browser ports for the same session used by native and mobile frontends.
use super::*;
use kog_playback_policy::session::{Item, Session};
use serde_json::{Value, json};
impl Item for Entry {
    fn entry(&self) -> Value {
        serde_json::to_value(self).expect("serializable entry")
    }
    fn metadata(&self) -> SortRow {
        entry_sort_row(0, self, &HashMap::new(), &HashSet::new())
    }
}
#[derive(Clone, Copy)]
pub struct Controller {
    pub model: StoredValue<Session<Entry>>,
    pub revision: RwSignal<u64>,
    pub changed: Callback<(bool, bool)>,
    pub output: StoredValue<Option<Callback<SessionEffect>>>,
    pub base: Callback<(), String>,
    pub auth: Callback<(), Option<String>>,
    pub saved: StoredValue<Option<Callback<()>>>,
}
impl Controller {
    pub fn send(self, command: SessionCommand<Entry>) {
        let scope = self.base.run(());
        let progress = matches!(
            &command,
            SessionCommand::Output {
                event: OutputEvent::Progress { .. },
                ..
            }
        );
        let updated = matches!(&command, SessionCommand::UpdateItem { .. });
        let effects = {
            let mut model = self.model.write_value();
            let _ = model.dispatch(SessionCommand::Scopes {
                scopes: vec![scope],
            });
            model.dispatch(command)
        };
        let changed = updated
            || effects
                .iter()
                .any(|e| matches!(e, SessionEffect::QueueChanged { .. }));
        self.changed.run((changed, progress));
        if !progress {
            self.revision.update(|revision| *revision += 1);
        }
        for effect in effects {
            match effect {
                SessionEffect::Persist { value } => {
                    let id = self.model.with_value(|s| s.id().to_owned());
                    store(&format!("kog.backend-session.{id}"), &value.to_string());
                }
                SessionEffect::QueueChanged { .. } => {}
                SessionEffect::Load { .. }
                | SessionEffect::Save { .. }
                | SessionEffect::Expand { .. }
                | SessionEffect::Collect { .. }
                | SessionEffect::Radio { .. } => self.io(effect),
                effect => {
                    if let Some(port) = self.output.get_value() {
                        port.run(effect);
                    }
                }
            }
        }
    }
    fn io(self, effect: SessionEffect) {
        let header = self.auth.run(());
        let base = self.base.run(());
        let session_id = self.model.with_value(|s| s.id().to_owned());
        leptos::task::spawn_local(async move {
            let (token, result) = match effect {
                SessionEffect::Load {
                    token,
                    scope,
                    playlist_id,
                } => {
                    let result = if scope != base {
                        Err("Connect to the original playlist server".into())
                    } else {
                        request(
                            "GET",
                            format!("{scope}/api/playlists/{playlist_id}"),
                            header,
                            None,
                        )
                        .await
                    };
                    (
                        token,
                        result.map(|v| IoResult::Loaded {
                            entries: v["entries"].as_array().cloned().unwrap_or_default(),
                        }),
                    )
                }
                SessionEffect::Save {
                    token,
                    scope,
                    playlist_id,
                    mut entries,
                    expected_entries,
                } => {
                    for entry in &mut entries {
                        if entry["fragment"].is_null() {
                            entry["fragment"] = json!("");
                        }
                    }
                    let result = if scope != base {
                        Err("Connect to the original playlist server".into())
                    } else {
                        request(
                            "PUT",
                            format!("{scope}/api/playlists/{playlist_id}/entries"),
                            header,
                            Some(json!({"entries":entries,"expected_entries":expected_entries})),
                        )
                        .await
                    };
                    (
                        token,
                        result.map(|_| {
                            if let Some(saved) = self.saved.get_value() {
                                saved.run(());
                            }
                            IoResult::Saved
                        }),
                    )
                }
                SessionEffect::Expand {
                    token,
                    scope,
                    entries,
                } => {
                    let result = if scope != base {
                        Err("Connect to the original source server".into())
                    } else {
                        expand(&scope, header, entries).await
                    };
                    (token, result.map(|tracks| IoResult::Expanded { tracks }))
                }
                SessionEffect::Collect {
                    token,
                    scope,
                    path,
                    query,
                    root,
                } => {
                    let result = if scope != base {
                        Err("Connect to the original source server".into())
                    } else {
                        request(
                            "GET",
                            format!(
                                "{scope}/api/library/collect?path={}&q={}&root={}",
                                url_encode(&path),
                                url_encode(&query),
                                url_encode(&root)
                            ),
                            header,
                            None,
                        )
                        .await
                    };
                    (
                        token,
                        result.map(|v| IoResult::Expanded {
                            tracks: v["tracks"]
                                .as_array()
                                .map(|a| a.iter().map(browse_file_entry).collect())
                                .unwrap_or_default(),
                        }),
                    )
                }
                SessionEffect::Radio {
                    token,
                    scope,
                    root,
                    enabled,
                    reshuffle,
                    reset,
                } => {
                    let endpoint = if reshuffle {
                        "reshuffle"
                    } else if reset {
                        "enabled"
                    } else {
                        "advance"
                    };
                    let root_query = if root.is_empty() {
                        String::new()
                    } else {
                        format!("&root={}", url_encode(&root))
                    };
                    let result = if scope != base {
                        Err("Connect to the original radio server".into())
                    } else {
                        post_json(format!("{scope}/api/radio/{endpoint}?incremental=true{root_query}&session={}&incarnation={}&serial={}",url_encode(&session_id),token.incarnation,token.serial),header,json!({"enabled":enabled})).await
                    };
                    (
                        token,
                        result.map(|v| {
                            let tracks = radio_entries(&v);
                            let exhausted = v["exhausted"].as_bool().unwrap_or(tracks.is_empty());
                            IoResult::Radio { tracks, exhausted }
                        }),
                    )
                }
                _ => return,
            };
            self.send(SessionCommand::Complete {
                token,
                result: result.unwrap_or_else(|error| IoResult::Failed { error }),
            });
        });
    }
}
async fn expand(
    scope: &str,
    header: Option<String>,
    entries: Vec<Value>,
) -> Result<Vec<Entry>, String> {
    let mut resolved = Vec::new();
    for entry in entries {
        if let Some(id) = entry["playlist_id"].as_i64() {
            let value = request(
                "GET",
                format!("{scope}/api/playlists/{id}"),
                header.clone(),
                None,
            )
            .await?;
            resolved.extend(value["entries"].as_array().cloned().unwrap_or_default());
        } else {
            resolved.push(entry);
        }
    }
    let mut tracks = Vec::new();
    for chunk in resolved.chunks(200) {
        let value = post_json(format!("{scope}/api/expand"), header.clone(), json!(chunk)).await?;
        if let Some(lists) = value["tracks"].as_array() {
            for list in lists {
                if let Some(items) = list.as_array() {
                    tracks.extend(items.iter().map(browse_file_entry));
                }
            }
        }
    }
    Ok(tracks)
}
async fn request(
    method: &str,
    url: String,
    header: Option<String>,
    body: Option<Value>,
) -> Result<Value, String> {
    let mut request = if method == "PUT" {
        Request::put(&url)
    } else {
        Request::get(&url)
    };
    if let Some(header) = header {
        request = request.header("Authorization", &header);
    }
    request = request.header("X-Kog-Device", &device_id());
    let response = match body {
        Some(body) => request.json(&body).map_err(|e| e.to_string())?.send().await,
        None => request.send().await,
    }
    .map_err(|e| e.to_string())?;
    if !response.ok() {
        return Err(error_text(response).await);
    }
    response.json::<Value>().await.map_err(|e| e.to_string())
}
