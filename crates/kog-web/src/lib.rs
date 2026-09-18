//! Kog's web frontend.
//!
//! Deliberately a *client*, not a second server: with per-client streams the
//! browser is the player, so this app owns its own queue and plays the API's
//! stream URLs through an `<audio>` element. It mirrors the desktop layout
//! where that makes sense (header, library, playlists, stars, transport) and
//! collapses to a bottom tab bar and a fixed transport bar on phones.

use gloo_net::http::Request;
use leptos::prelude::*;
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::wasm_bindgen;

/// One track queued for playback (or playing).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct Entry {
    kind: String,
    path: String,
    entry: String,
    fragment: Option<String>,
    /// Display name, filled in when queued from the library.
    name: String,
}

impl Entry {
    fn locator(&self) -> String {
        let base = if self.kind == "archive" && !self.entry.is_empty() {
            format!("{}::{}", self.path, self.entry)
        } else {
            self.path.clone()
        };
        match self.fragment.as_deref() {
            Some(fragment) if !fragment.is_empty() => format!("{base}#{fragment}"),
            _ => base,
        }
    }
}

#[derive(Clone, Debug)]
enum Auth {
    None,
    Token(String),
    Basic(String, String),
}

impl Auth {
    fn header(&self) -> Option<String> {
        match self {
            Self::None => None,
            Self::Token(token) => Some(format!("Bearer {token}")),
            Self::Basic(user, password) => {
                let raw = format!("{user}:{password}");
                Some(format!("Basic {}", base64(raw.as_bytes())))
            }
        }
    }
}

/// Minimal base64 so the app does not need a dependency for one header.
fn base64(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for chunk in bytes.chunks(3) {
        let b = [
            chunk[0],
            *chunk.get(1).unwrap_or(&0),
            *chunk.get(2).unwrap_or(&0),
        ];
        let n = ((b[0] as u32) << 16) | ((b[1] as u32) << 8) | b[2] as u32;
        out.push(TABLE[((n >> 18) & 63) as usize] as char);
        out.push(TABLE[((n >> 12) & 63) as usize] as char);
        out.push(if chunk.len() > 1 {
            TABLE[((n >> 6) & 63) as usize] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            TABLE[(n & 63) as usize] as char
        } else {
            '='
        });
    }
    out
}

fn storage() -> Option<web_sys::Storage> {
    web_sys::window()?.local_storage().ok()?
}

fn store(key: &str, value: &str) {
    if let Some(storage) = storage() {
        let _ = storage.set_item(key, value);
    }
}

fn load(key: &str) -> Option<String> {
    storage()?.get_item(key).ok()?
}

#[wasm_bindgen(start)]
pub fn start() {
    console_error_panic_hook::set_once();
    leptos::mount::mount_to_body(App);
}

#[component]
fn App() -> impl IntoView {
    // A phone opening the server's own address is already pointed at it.
    let origin = web_sys::window()
        .and_then(|window| window.location().origin().ok())
        .unwrap_or_default();
    let (server, set_server) = signal(load("kog.server").unwrap_or(origin));
    let (token, set_token) = signal(load("kog.token").unwrap_or_default());
    let (user, set_user) = signal(String::new());
    let (password, set_password) = signal(String::new());
    let (use_basic, set_use_basic) = signal(false);
    let (codec, set_codec) = signal(load("kog.codec").unwrap_or_else(|| "aac".to_owned()));
    let (connected, set_connected) = signal(false);
    let (message, set_message) = signal(String::new());

    let (path, set_path) = signal(String::new());
    let (directories, set_directories) = signal(Vec::<(String, String)>::new());
    let (files, set_files) = signal(Vec::<Entry>::new());
    let (stack, set_stack) = signal(Vec::<String>::new());
    let (search_query, set_search_query) = signal(String::new());
    let (searching, set_searching) = signal(false);
    let (new_playlist, set_new_playlist) = signal(String::new());

    let (playlists, set_playlists) = signal(Vec::<(i64, String, i64)>::new());
    let (stars, set_stars) = signal(Vec::<Entry>::new());
    let (queue, set_queue) = signal(Vec::<Entry>::new());
    let (current, set_current) = signal(0_usize);
    let (tab, set_tab) = signal("library".to_owned());
    let (playing, set_playing) = signal(false);

    let base = move || server.get().trim_end_matches('/').to_owned();
    let auth = move || {
        if use_basic.get() {
            Auth::Basic(user.get(), password.get())
        } else if token.get().trim().is_empty() {
            Auth::None
        } else {
            Auth::Token(token.get())
        }
    };

    // One place that talks to the API, so every call carries the same auth and
    // reports the same failures.
    let get_json = move |route: String| {
        let url = format!("{}{route}", base());
        let header = auth().header();
        async move {
            let mut request = Request::get(&url);
            if let Some(header) = header {
                request = request.header("Authorization", &header);
            }
            let response = request.send().await.map_err(|error| error.to_string())?;
            if response.status() == 401 {
                return Err("Sign in to continue".to_owned());
            }
            if !response.ok() {
                return Err(format!("Request failed ({})", response.status()));
            }
            response
                .json::<serde_json::Value>()
                .await
                .map_err(|error| error.to_string())
        }
    };

    // One place that writes JSON, mirroring `get_json`.
    let post_json = move |route: String, body: serde_json::Value| {
        let url = format!("{}{route}", base());
        let header = auth().header();
        async move {
            let mut request = Request::post(&url);
            if let Some(header) = header {
                request = request.header("Authorization", &header);
            }
            let request = request.json(&body).map_err(|error| error.to_string())?;
            let response = request.send().await.map_err(|error| error.to_string())?;
            if response.status() == 401 {
                return Err("Sign in to continue".to_owned());
            }
            if !response.ok() {
                return Err(format!("Request failed ({})", response.status()));
            }
            response
                .json::<serde_json::Value>()
                .await
                .map_err(|error| error.to_string())
        }
    };

    let stream_url = move |entry: &Entry| {
        format!(
            "{}/api/stream?kind={}&path={}&entry={}&codec={}{}",
            base(),
            url_encode(&entry.kind),
            url_encode(&entry.path),
            url_encode(&entry.entry),
            url_encode(&codec.get()),
            entry
                .fragment
                .as_deref()
                .filter(|fragment| !fragment.is_empty())
                .map(|fragment| format!("&fragment={}", url_encode(fragment)))
                .unwrap_or_default()
        )
    };

    let browse = {
        let get_json = get_json;
        move |next: String| {
            let route = if next.is_empty() {
                "/api/library".to_owned()
            } else {
                format!("/api/library?path={}", url_encode(&next))
            };
            leptos::task::spawn_local(async move {
                match get_json(route).await {
                    Ok(value) => {
                        set_path.set(
                            value["path"].as_str().unwrap_or_default().to_owned(),
                        );
                        let dirs = value["directories"]
                            .as_array()
                            .map(|items| {
                                items
                                    .iter()
                                    .map(|item| {
                                        (
                                            item["name"].as_str().unwrap_or_default().to_owned(),
                                            item["path"].as_str().unwrap_or_default().to_owned(),
                                        )
                                    })
                                    .collect()
                            })
                            .unwrap_or_default();
                        let entries = value["files"]
                            .as_array()
                            .map(|items| {
                                items
                                    .iter()
                                    .map(|item| Entry {
                                        kind: "local".to_owned(),
                                        path: item["path"].as_str().unwrap_or_default().to_owned(),
                                        entry: String::new(),
                                        fragment: None,
                                        name: item["name"].as_str().unwrap_or_default().to_owned(),
                                    })
                                    .collect()
                            })
                            .unwrap_or_default();
                        set_directories.set(dirs);
                        set_files.set(entries);
                        set_searching.set(false);
                    }
                    Err(error) => set_message.set(error),
                }
            });
        }
    };

    let browse_for_connect = browse.clone();
    let connect = move |_| {
        let header = auth().header();
        let url = format!("{}/api/version", base());
        let basic = use_basic.get();
        store("kog.server", &server.get());
        if !basic {
            store("kog.token", &token.get());
        }
        leptos::task::spawn_local({
            let browse = browse_for_connect.clone();
            async move {
                let mut request = Request::get(&url);
                if let Some(header) = header {
                    request = request.header("Authorization", &header);
                }
                match request.send().await {
                    Ok(response) if response.ok() => {
                        set_connected.set(true);
                        set_message.set("Connected".to_owned());
                        browse(String::new());
                    }
                    Ok(response) if response.status() == 401 => {
                        set_message.set("That token or password was rejected".to_owned())
                    }
                    Ok(response) => {
                        set_message.set(format!("Server returned {}", response.status()))
                    }
                    Err(error) => set_message.set(format!("Could not reach the server: {error}")),
                }
            }
        });
    };

    let load_playlists = {
        let get_json = get_json;
        move || {
            leptos::task::spawn_local(async move {
                if let Ok(value) = get_json("/api/playlists".to_owned()).await {
                    let lists = value["playlists"]
                        .as_array()
                        .map(|items| {
                            items
                                .iter()
                                .map(|item| {
                                    (
                                        item["id"].as_i64().unwrap_or_default(),
                                        item["name"].as_str().unwrap_or_default().to_owned(),
                                        item["entryCount"].as_i64().unwrap_or_default(),
                                    )
                                })
                                .collect()
                        })
                        .unwrap_or_default();
                    set_playlists.set(lists);
                }
            });
        }
    };

    let load_stars = {
        let get_json = get_json;
        move || {
            leptos::task::spawn_local(async move {
                if let Ok(value) = get_json("/api/stars".to_owned()).await {
                    let entries = value["entries"]
                        .as_array()
                        .map(|items| items.iter().map(entry_from_json).collect())
                        .unwrap_or_default();
                    set_stars.set(entries);
                }
            });
        }
    };

    // Queue a list of entries and start the first one.
    let play = move |entries: Vec<Entry>| {
        if entries.is_empty() {
            return;
        }
        set_queue.set(entries);
        set_current.set(0);
        set_playing.set(true);
    };

    let load_playlist_into_queue = {
        let get_json = get_json;
        move |id: i64| {
            leptos::task::spawn_local(async move {
                if let Ok(value) = get_json(format!("/api/playlists/{id}")).await {
                    let entries: Vec<Entry> = value["entries"]
                        .as_array()
                        .map(|items| items.iter().map(entry_from_json).collect())
                        .unwrap_or_default();
                    set_queue.set(entries);
                    set_current.set(0);
                    set_playing.set(true);
                }
            });
        }
    };

    // Name search across the library. Results replace the directory listing
    // and the "Up" trail until the user browses again.
    let load_search = {
        let get_json = get_json;
        move |query: String| {
            let trimmed = query.trim().to_owned();
            if trimmed.is_empty() {
                return;
            }
            let route = format!("/api/library/search?q={}", url_encode(&trimmed));
            leptos::task::spawn_local(async move {
                match get_json(route).await {
                    Ok(value) => {
                        let found: Vec<Entry> = value["results"]
                            .as_array()
                            .map(|items| items.iter().map(entry_from_json).collect())
                            .unwrap_or_default();
                        set_directories.set(Vec::new());
                        set_files.set(found);
                        set_searching.set(true);
                        set_stack.set(Vec::new());
                        set_path.set(format!("Search: {trimmed}"));
                    }
                    Err(error) => set_message.set(error),
                }
            });
        }
    };

    let create_playlist = {
        let post_json = post_json;
        let load_playlists = load_playlists.clone();
        move |name: String| {
            let trimmed = name.trim().to_owned();
            if trimmed.is_empty() {
                return;
            }
            let load = load_playlists.clone();
            leptos::task::spawn_local(async move {
                match post_json(
                    "/api/playlists".to_owned(),
                    serde_json::json!({ "name": trimmed }),
                )
                .await
                {
                    Ok(_) => {
                        set_message.set("Playlist created".to_owned());
                        load();
                    }
                    Err(error) => set_message.set(error),
                }
            });
        }
    };

    let add_to_playlist = {
        let post_json = post_json;
        move |id: i64, entry: Entry| {
            let body = serde_json::json!({
                "entries": [{
                    "kind": entry.kind,
                    "path": entry.path,
                    "entry": entry.entry,
                    "fragment": entry.fragment.clone().unwrap_or_default(),
                }]
            });
            leptos::task::spawn_local(async move {
                match post_json(format!("/api/playlists/{id}/entries"), body).await {
                    Ok(_) => set_message.set("Added to the playlist".to_owned()),
                    Err(error) => set_message.set(error),
                }
            });
        }
    };

    let toggle_star = move |entry: Entry, starred: bool| {
        let url = format!("{}/api/stars", base());
        let header = auth().header();
        leptos::task::spawn_local(async move {
            let mut request = Request::post(&url);
            if let Some(header) = header {
                request = request.header("Authorization", &header);
            }
            let body = serde_json::json!({
                "kind": entry.kind,
                "path": entry.path,
                "entry": entry.entry,
                "fragment": entry.fragment.clone().unwrap_or_default(),
                "starred": starred,
            });
            if let Ok(request) = request.json(&body) {
                let _ = request.send().await;
            }
        });
    };

    let current_entry = move || queue.get().get(current.get()).cloned();
    let audio_ref = NodeRef::<leptos::html::Audio>::new();

    // The transport button and the element have to stay in step, and a track
    // that reaches its end should roll on to the next one.
    Effect::new(move |_| {
        let index = current.get();
        let playing = playing.get();
        if let Some(audio) = audio_ref.get() {
            if playing && index < queue.get().len() {
                let _ = audio.play();
            } else {
                let _ = audio.pause();
            }
        }
    });
    let audio_src = move || current_entry().map(|entry| stream_url(&entry)).unwrap_or_default();

    view! {
        <div class="app">
            <header class="topbar">
                <span class="brand">"Kog"</span>
                <input
                    class="server"
                    placeholder="https://my-desktop:8420"
                    prop:value=move || server.get()
                    on:input=move |event| set_server.set(event_target_value(&event))
                />
                <button class="primary" on:click=connect>
                    {move || if connected.get() { "Reconnect" } else { "Connect" }}
                </button>
            </header>

            <Show when=move || !connected.get()>
                <section class="connect">
                    <h2>"Sign in"</h2>
                    <label>
                        <input
                            type="checkbox"
                            prop:checked=move || use_basic.get()
                            on:change=move |event| set_use_basic.set(event_target_checked(&event))
                        />
                        "Use a username and password"
                    </label>
                    <Show
                        when=move || use_basic.get()
                        fallback=move || view! {
                            <input
                                type="password"
                                placeholder="API token"
                                prop:value=move || token.get()
                                on:input=move |event| set_token.set(event_target_value(&event))
                            />
                        }
                    >
                        <input
                            placeholder="Username"
                            prop:value=move || user.get()
                            on:input=move |event| set_user.set(event_target_value(&event))
                        />
                        <input
                            type="password"
                            placeholder="Password"
                            prop:value=move || password.get()
                            on:input=move |event| set_password.set(event_target_value(&event))
                        />
                    </Show>
                    <p class="hint">
                        "Ask the machine running Kog for its address and token in Preferences → Server."
                    </p>
                </section>
            </Show>

            <Show when=move || connected.get()>
                <nav class="tabs">
                    <button
                        class:active=move || tab.get() == "library"
                        on:click=move |_| set_tab.set("library".to_owned())
                    >"Library"</button>
                    <button
                        class:active=move || tab.get() == "playlists"
                        on:click=move |_| {
                            set_tab.set("playlists".to_owned());
                            load_playlists();
                        }
                    >"Playlists"</button>
                    <button
                        class:active=move || tab.get() == "stars"
                        on:click=move |_| {
                            set_tab.set("stars".to_owned());
                            load_stars();
                        }
                    >"Favorites"</button>
                </nav>

                <main>
                    <Show when=move || tab.get() == "library">
                        <div class="crumbs">
                            <button
                                disabled=move || stack.get().is_empty()
                                on:click={
                                    let browse = browse.clone();
                                    move |_| {
                                        let mut history = stack.get();
                                        let previous = history.pop().unwrap_or_default();
                                        set_stack.set(history);
                                        browse(previous);
                                    }
                                }
                            >"Up"</button>
                            <span class="path">{move || path.get()}</span>
                            <form
                                class="search"
                                on:submit=move |event| {
                                    event.prevent_default();
                                    load_search(search_query.get());
                                }
                            >
                                <input
                                    type="search"
                                    placeholder="Search"
                                    prop:value=move || search_query.get()
                                    on:input=move |event| set_search_query.set(event_target_value(&event))
                                />
                            </form>
                            <button
                                class="clear"
                                disabled=move || !searching.get()
                                title="Leave the search results"
                                on:click={
                                    let browse = browse.clone();
                                    move |_| {
                                        set_searching.set(false);
                                        set_search_query.set(String::new());
                                        browse(String::new());
                                    }
                                }
                            >"✕"</button>
                        </div>
                        <ul class="rows">
                            <For each=move || directories.get() key=|item| item.1.clone() let:item>
                                {
                                    let browse = browse.clone();
                                    let name = item.0.clone();
                                    let target = item.1.clone();
                                    view! {
                                        <li>
                                            <button on:click={
                                                let browse = browse.clone();
                                                let target = target.clone();
                                                move |_| {
                                                    let history = stack.get()
                                                        .into_iter()
                                                        .chain([path.get()])
                                                        .collect();
                                                    set_stack.set(history);
                                                    browse(target.clone());
                                                }
                                            }>"▸ " {name}</button>
                                        </li>
                                    }
                                }
                            </For>
                            <For each=move || files.get() key=|item| item.path.clone() let:item>
                                {
                                    let play = play.clone();
                                    let files_now = item.clone();
                                    view! {
                                        <li class="row">
                                            <button class="grow" on:click={
                                                let play = play.clone();
                                                let entry = files_now.clone();
                                                move |_| play(vec![entry.clone()])
                                            }>{item.name.clone()}</button>
                                            <button
                                                class="icon"
                                                title="Favorite"
                                                on:click={
                                                    let toggle = toggle_star.clone();
                                                    let entry = files_now.clone();
                                                    move |_| toggle(entry.clone(), true)
                                                }
                                            >"★"</button>
                                        </li>
                                    }
                                }
                            </For>
                        </ul>
                    </Show>

                    <Show when=move || tab.get() == "playlists">
                        <form
                            class="new-playlist"
                            on:submit=move |event| {
                                event.prevent_default();
                                create_playlist(new_playlist.get());
                                set_new_playlist.set(String::new());
                            }
                        >
                            <input
                                type="text"
                                placeholder="New playlist"
                                prop:value=move || new_playlist.get()
                                on:input=move |event| set_new_playlist.set(event_target_value(&event))
                            />
                            <button type="submit">"Create"</button>
                        </form>
                        <ul class="rows">
                            <For each=move || playlists.get() key=|item| item.0 let:item>
                                {
                                    let load = load_playlist_into_queue.clone();
                                    let add = add_to_playlist.clone();
                                    let current_entry = current_entry.clone();
                                    let id = item.0;
                                    view! {
                                        <li class="row">
                                            <button class="grow" on:click=move |_| load(id)>
                                                {item.1.clone()}
                                            </button>
                                            <button
                                                class="icon"
                                                title="Add the current track"
                                                disabled=move || current_entry().is_none()
                                                on:click={
                                                    let add = add.clone();
                                                    let current_entry = current_entry.clone();
                                                    let id = id;
                                                    move |_| {
                                                        if let Some(entry) = current_entry() {
                                                            add(id, entry);
                                                        }
                                                    }
                                                }
                                            >"＋"</button>
                                            <span class="count">{item.2}</span>
                                        </li>
                                    }
                                }
                            </For>
                        </ul>
                    </Show>

                    <Show when=move || tab.get() == "stars">
                        <ul class="rows">
                            <For each=move || stars.get() key=|item| item.locator() let:item>
                                {
                                    let play = play.clone();
                                    let entry = item.clone();
                                    view! {
                                        <li class="row">
                                            <button class="grow" on:click={
                                                let play = play.clone();
                                                let entry = entry.clone();
                                                move |_| play(vec![entry.clone()])
                                            }>{display_name(&entry)}</button>
                                        </li>
                                    }
                                }
                            </For>
                        </ul>
                    </Show>
                </main>

                <footer class="transport">
                    <div class="now">
                        {move || current_entry().map(|entry| display_name(&entry)).unwrap_or_default()}
                    </div>
                    <div class="controls">
                        <button
                            disabled=move || current.get() == 0
                            on:click=move |_| set_current.update(|index| *index = index.saturating_sub(1))
                        >"⏮"</button>
                        <button
                            class="play"
                            disabled=move || queue.get().is_empty()
                            on:click=move |_| set_playing.update(|value| *value = !*value)
                        >{move || if playing.get() { "⏸" } else { "▶" }}</button>
                        <button
                            disabled=move || current.get() + 1 >= queue.get().len()
                            on:click=move |_| set_current.update(|index| *index += 1)
                        >"⏭"</button>
                        <select
                            prop:value=move || codec.get()
                            on:change=move |event| {
                                let value = event_target_value(&event);
                                store("kog.codec", &value);
                                set_codec.set(value);
                            }
                        >
                            <option value="aac">"AAC"</option>
                            <option value="opus">"Opus"</option>
                            <option value="flac">"FLAC"</option>
                        </select>
                    </div>
                    <audio
                        class="player"
                        controls
                        preload="auto"
                        node_ref=audio_ref
                        prop:src=audio_src
                        on:ended=move |_| {
                            if current.get() + 1 < queue.get().len() {
                                set_current.update(|index| *index += 1);
                                set_playing.set(true);
                            } else {
                                set_playing.set(false);
                            }
                        }
                    ></audio>
                </footer>
            </Show>

            <p class="message">{move || message.get()}</p>
        </div>
    }
}

fn display_name(entry: &Entry) -> String {
    if !entry.name.is_empty() {
        return entry.name.clone();
    }
    entry
        .path
        .rsplit('/')
        .next()
        .unwrap_or(&entry.path)
        .to_owned()
}

fn entry_from_json(value: &serde_json::Value) -> Entry {
    let path = value["path"].as_str().unwrap_or_default().to_owned();
    let entry = value["entry"].as_str().unwrap_or_default().to_owned();
    let name = path
        .rsplit('/')
        .next()
        .unwrap_or(&path)
        .to_owned();
    Entry {
        kind: value["kind"].as_str().unwrap_or("local").to_owned(),
        name,
        path,
        entry,
        fragment: value["fragment"].as_str().map(str::to_owned),
    }
}

fn url_encode(value: &str) -> String {
    value
        .bytes()
        .map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (byte as char).to_string()
            }
            other => format!("%{other:02X}"),
        })
        .collect()
}
