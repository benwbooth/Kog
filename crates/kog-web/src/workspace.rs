//! Browser I/O and presentation for the shared playlist workspace.
use super::*;
use kog_playback_policy::workspace::{
    CloseChoice, Command, Effect as WorkspaceEffect, QueueAction, Workspace,
};

#[derive(Default)]
pub struct QueueJobs {
    issued: u64,
    next: u64,
    ready: std::collections::BTreeMap<u64, Option<(u64, String, QueueAction, Vec<Entry>)>>,
}

#[derive(Clone, Copy)]
pub struct Controller {
    pub model: StoredValue<Workspace>,
    pub revision: RwSignal<u64>,
    pub queue_generation: StoredValue<u64>,
    pub queue_jobs: StoredValue<QueueJobs>,
    pub base: Callback<(), String>,
    pub auth: Callback<(), Option<String>>,
    pub queued: Callback<(QueueAction, Vec<Entry>)>,
    pub saved: Callback<()>,
    pub error: WriteSignal<String>,
}
impl Controller {
    pub fn snapshot(self) -> kog_playback_policy::workspace::Snapshot {
        self.revision.get();
        self.model.with_value(Workspace::snapshot)
    }
    pub fn open(self, id: i64, name: String) {
        let scope = self.base.run(());
        self.send(Command::Open {
            key: format!("{scope}:{id}"),
            scope,
            playlist_id: id,
            name,
            readonly: id == 0,
        });
    }
    fn queue_finished(self, ticket: u64, result: Option<(u64, String, QueueAction, Vec<Entry>)>) {
        let mut ready = Vec::new();
        {
            let mut jobs = self.queue_jobs.write_value();
            jobs.ready.insert(ticket, result);
            loop {
                let next = jobs.next;
                let Some(result) = jobs.ready.remove(&next) else {
                    break;
                };
                jobs.next += 1;
                if let Some(result) = result {
                    ready.push(result);
                }
            }
        }
        for (generation, source, action, entries) in ready {
            if self.queue_generation.get_value() == generation && self.base.run(()) == source {
                self.queued.run((action, entries));
            }
        }
    }
    pub fn send(self, command: Command) {
        let effect = self.model.write_value().apply(command);
        if let Ok(value) = self.model.with_value(serde_json::to_string) {
            store("kog.playlist-tabs", &value);
        }
        self.revision.update(|value| *value += 1);
        let effect = match effect {
            Ok(effect) => effect,
            Err(error) => {
                self.error.set(error);
                return;
            }
        };
        let ticket = if matches!(effect, WorkspaceEffect::Queue { .. }) {
            let mut jobs = self.queue_jobs.write_value();
            let ticket = jobs.issued;
            jobs.issued += 1;
            ticket
        } else {
            0
        };
        let header = self.auth.run(());
        let base = self.base.run(());
        let generation = self.queue_generation.get_value();
        leptos::task::spawn_local(async move {
            match effect {
                WorkspaceEffect::None => {}
                WorkspaceEffect::Load {
                    key,
                    scope,
                    playlist_id,
                    generation,
                } => {
                    let result = if scope != base {
                        Err("Connect to this playlist's server to reload it".into())
                    } else {
                        request(
                            "GET",
                            format!("{scope}/api/playlists/{playlist_id}"),
                            header,
                            None,
                        )
                        .await
                    };
                    self.send(match result {
                        Ok(value) => Command::Loaded {
                            key,
                            generation,
                            entries: value["entries"].as_array().cloned().unwrap_or_default(),
                        },
                        Err(error) => Command::LoadFailed {
                            key,
                            generation,
                            error,
                        },
                    });
                }
                WorkspaceEffect::Save {
                    key,
                    scope,
                    playlist_id,
                    revision,
                    mut entries,
                    expected_entries,
                } => {
                    // The existing append API accepts an empty string for no fragment.
                    for entry in &mut entries {
                        if entry["fragment"].is_null() {
                            entry["fragment"] = serde_json::json!("");
                        }
                    }
                    let result = if scope != base {
                        Err("Connect to this playlist's server to save it".into())
                    } else {
                        request("PUT", format!("{scope}/api/playlists/{playlist_id}/entries"), header,
                            Some(serde_json::json!({"entries":entries,"expected_entries":expected_entries}))).await
                    };
                    self.send(match result {
                        Ok(_) => {
                            self.saved.run(());
                            Command::Saved { key, revision }
                        }
                        Err(error) => Command::SaveFailed {
                            key,
                            revision,
                            error,
                        },
                    });
                }
                WorkspaceEffect::Queue {
                    mode,
                    entries,
                    scope,
                } => {
                    if scope != base {
                        self.error
                            .set("Connect to this playlist's server to queue it".into());
                        self.queue_finished(ticket, None);
                        return;
                    }
                    let mut tracks = Vec::new();
                    for chunk in entries.chunks(200) {
                        match post_json(
                            format!("{scope}/api/expand"),
                            header.clone(),
                            serde_json::json!(chunk),
                        )
                        .await
                        {
                            Ok(value) => {
                                if let Some(lists) = value["tracks"].as_array() {
                                    for list in lists {
                                        if let Some(items) = list.as_array() {
                                            tracks.extend(items.iter().map(browse_file_entry));
                                        }
                                    }
                                }
                            }
                            Err(error) => {
                                self.error.set(error);
                                self.queue_finished(ticket, None);
                                return;
                            }
                        }
                    }
                    self.queue_finished(ticket, Some((generation, base, mode, tracks)));
                }
            }
        });
    }
}
async fn request(
    method: &str,
    url: String,
    header: Option<String>,
    body: Option<serde_json::Value>,
) -> Result<serde_json::Value, String> {
    let mut builder = if method == "PUT" {
        Request::put(&url)
    } else {
        Request::get(&url)
    };
    if let Some(header) = header {
        builder = builder.header("Authorization", &header);
    }
    builder = builder.header("X-Kog-Device", &device_id());
    let result = match body {
        Some(body) => builder.json(&body).map_err(|e| e.to_string())?.send().await,
        None => builder.send().await,
    };
    let response = result.map_err(|e| e.to_string())?;
    if !response.ok() {
        return Err(error_text(response).await);
    }
    response.json().await.map_err(|e| e.to_string())
}

#[component]
pub fn Tabs(controller: Controller) -> impl IntoView {
    view! {
        <div class="playlist-tabs" role="tablist" aria-label="Open playlists">
            <For each=move || controller.snapshot().tabs key=|tab| (tab.key.clone(), tab.name.clone(), tab.dirty) let:tab>
                { let focus = tab.key.clone(); let active = tab.key.clone(); let close = tab.key.clone(); let closable = tab.key != "queue";
                  view! { <div class="playlist-tab" class:active=move || controller.snapshot().active == active>
                    <button role="tab" aria-selected=move || controller.snapshot().active == focus
                        on:click={let key=tab.key.clone(); move |_| controller.send(Command::Focus { key:key.clone() })}>
                        {format!("{}{}",tab.name,if tab.dirty { " *" } else { "" })}
                    </button>
                    {closable.then(|| view! { <button title="Close playlist tab" on:click=move |_| controller.send(Command::Close { key:close.clone() })>"×"</button> })}
                  </div> }
                }
            </For>
        </div>
        <Show when=move || controller.snapshot().pending_close.is_some()>
            <div class="workspace-close" role="alertdialog" aria-label="Unsaved playlist changes">
                <span>"Save changes before closing?"</span>
                <button on:click=move |_| controller.send(Command::ResolveClose { choice:CloseChoice::Save })>"Save"</button>
                <button on:click=move |_| controller.send(Command::ResolveClose { choice:CloseChoice::Discard })>"Discard"</button>
                <button on:click=move |_| controller.send(Command::ResolveClose { choice:CloseChoice::Cancel })>"Cancel"</button>
            </div>
        </Show>
    }
}
#[component]
pub fn Editor(
    controller: Controller,
    queue: ReadSignal<Vec<Entry>>,
    selected: ReadSignal<HashSet<usize>>,
) -> impl IntoView {
    let readonly = move || {
        let state = controller.snapshot();
        state
            .tabs
            .iter()
            .find(|tab| tab.key == state.active)
            .is_none_or(|tab| tab.readonly || tab.loading)
    };
    view! {
        <div class="playlist-editor" on:keydown=move |event:web_sys::KeyboardEvent| {
            if event.ctrl_key() || event.meta_key() {
                let command = match event.key().as_str() { "s" => Some(Command::Save), "z" if event.shift_key()=>Some(Command::Redo), "z"=>Some(Command::Undo), "y"=>Some(Command::Redo), "a"=>Some(Command::Select {indices:(0..controller.snapshot().entries.len()).collect()}), _=>None };
                if let Some(command)=command { event.prevent_default(); event.stop_propagation(); controller.send(command); }
            } else if event.key()=="Delete" { event.prevent_default(); event.stop_propagation(); controller.send(Command::Remove); }
        }>
            <div class="workspace-actions">
                <button on:click=move |_| controller.send(Command::Queue { action:QueueAction::PlayNow })>"Play Now"</button>
                <button on:click=move |_| controller.send(Command::Queue { action:QueueAction::PlayNext })>"Play Next"</button>
                <button on:click=move |_| controller.send(Command::Queue { action:QueueAction::AddToQueue })>"Add to Queue"</button>
                <button disabled=move || readonly() || !controller.snapshot().tabs.iter().any(|tab|tab.key==controller.snapshot().active && tab.dirty && !tab.saving)
                    on:click=move |_| controller.send(Command::Save)>"Save"</button>
                <button on:click=move |_| controller.send(Command::Reload)>"Reload"</button>
            </div>
            <div class="workspace-actions">
                <button disabled=readonly on:click=move |_| controller.send(Command::Append { entries:queue.get_untracked().iter().map(|entry|serde_json::to_value(entry).unwrap()).collect() })>"Add Play Queue"</button>
                <button disabled=move || readonly() || selected.get().is_empty() on:click=move |_| controller.send(Command::Append { entries:queue.get_untracked().iter().enumerate().filter(|(i,_)|selected.get_untracked().contains(i)).map(|(_,entry)|serde_json::to_value(entry).unwrap()).collect() })>"Add Queue Selection"</button>
                <button disabled=readonly on:click=move |_| controller.send(Command::Remove)>"Remove"</button>
                <button disabled=readonly on:click=move |_| controller.send(Command::Nudge { delta:-1 })>"Move Up"</button>
                <button disabled=readonly on:click=move |_| controller.send(Command::Nudge { delta:1 })>"Move Down"</button>
                <button disabled=move || !controller.snapshot().can_undo on:click=move |_| controller.send(Command::Undo)>"Undo"</button>
                <button disabled=move || !controller.snapshot().can_redo on:click=move |_| controller.send(Command::Redo)>"Redo"</button>
                <button on:click=move |_| controller.send(Command::Select { indices:vec![] })>"Clear Selection"</button>
            </div>
            <p class="workspace-status">{move || {let state=controller.snapshot(); state.error.unwrap_or_else(||format!("{} tracks · {} selected · playback actions use {}",state.entries.len(),state.selected.len(),if state.selected.is_empty(){"the whole playlist"}else{"the selection"}))}}</p>
            <div class="workspace-entries" role="listbox" aria-multiselectable="true">
                <For each={move || controller.snapshot().entries.into_iter().enumerate().collect::<Vec<_>>()} key=|(i,entry)|format!("{i}:{entry}") let:row>
                    {let (index,entry)=row; let label=entry["title"].as_str().or_else(||entry["name"].as_str()).filter(|value|!value.is_empty()).map(str::to_owned).unwrap_or_else(||last_segment(entry["entry"].as_str().filter(|value|!value.is_empty()).or_else(||entry["path"].as_str()).unwrap_or_default()));
                     view! { <button role="option" class:selected=move || controller.snapshot().selected.contains(&index)
                       aria-selected=move || controller.snapshot().selected.contains(&index)
                       on:click=move |event:web_sys::MouseEvent| {
                         let mut indices=controller.snapshot().selected;
                         if event.ctrl_key() || event.meta_key() { if let Some(at)=indices.iter().position(|i|*i==index){indices.remove(at);}else{indices.push(index);} }
                         else if event.shift_key() && !indices.is_empty() {indices=((*indices.first().unwrap()).min(index)..=(*indices.first().unwrap()).max(index)).collect();}
                         else {indices=vec![index];}
                         controller.send(Command::Select { indices });
                       }>{format!("{}. {label}",index+1)}</button> }
                    }
                </For>
            </div>
        </div>
    }
}
