//! Kog's web player.
//!
//! A client, not a second server: with per-client streams the browser is the
//! player, so this app owns its queue and plays the API's stream URLs through
//! an `<audio>` element.
//!
//! The layout mirrors the desktop window: a toolbar, a sidebar holding the file
//! tree and the playlists, the playlist pane with a column header, and the
//! transport bar. The shell is a fixed-height grid, so only the pane scrolls,
//! never the page. Below 820px the sidebar becomes a drawer and the rows go
//! compact for phones.

use std::collections::{HashMap, HashSet};

use gloo_net::http::Request;
use leptos::prelude::*;
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::wasm_bindgen;

/// One playable entry, addressed the way the whole API addresses tracks.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct Entry {
    kind: String,
    path: String,
    entry: String,
    fragment: Option<String>,
    /// Display name; "dir" entries carry one, playlist entries derive it.
    name: String,
    /// Location as shown in the second column (relative where known).
    location: String,
}

impl Entry {
    fn is_dir(&self) -> bool {
        self.kind == "dir"
    }

    /// A minimal local entry for a path the API only named.
    fn local(path: &str, name: &str, location: &str) -> Self {
        Self {
            kind: "local".to_owned(),
            path: path.to_owned(),
            entry: String::new(),
            fragment: None,
            name: name.to_owned(),
            location: location.to_owned(),
        }
    }
}

/// One rendered tree line: a flattened view of the expanded directories.
#[derive(Clone, Debug, PartialEq)]
struct TreeRow {
    name: String,
    path: String,
    parent: String,
    is_dir: bool,
    depth: usize,
    expanded: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum SortKey {
    Index,
    Name,
    Location,
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

/// Minimal base64 so the app needs no dependency for one header.
fn base64(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for chunk in bytes.chunks(3) {
        let b = [chunk[0], *chunk.get(1).unwrap_or(&0), *chunk.get(2).unwrap_or(&0)];
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

/// `m:ss`, or `-:--` when the length is not known yet.
fn clock(seconds: f64) -> String {
    if !seconds.is_finite() || seconds < 0.0 {
        return "-:--".to_owned();
    }
    let total = seconds.round() as u64;
    format!("{}:{:02}", total / 60, total % 60)
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

fn last_segment(path: &str) -> String {
    path.rsplit('/').next().unwrap_or(path).to_owned()
}

fn entry_from_json(value: &serde_json::Value) -> Entry {
    let path = value["path"].as_str().unwrap_or_default().to_owned();
    let entry = value["entry"].as_str().unwrap_or_default().to_owned();
    let fragment = value["fragment"].as_str().map(str::to_owned);
    let location = value["relative"]
        .as_str()
        .map(str::to_owned)
        .filter(|relative| !relative.is_empty())
        .unwrap_or_else(|| {
            if entry.is_empty() {
                path.clone()
            } else {
                format!("{path}::{entry}")
            }
        });
    Entry {
        kind: value["kind"].as_str().unwrap_or("local").to_owned(),
        name: last_segment(&path),
        path,
        entry,
        fragment,
        location,
    }
}

/// Walk the expanded directories into a flat list of tree lines.
fn flatten(
    children: &HashMap<String, Vec<Entry>>,
    expanded: &HashSet<String>,
    parent: &str,
    depth: usize,
    out: &mut Vec<TreeRow>,
) {
    let Some(items) = children.get(parent) else {
        return;
    };
    for item in items {
        let is_expanded = expanded.contains(&item.path);
        out.push(TreeRow {
            name: item.name.clone(),
            path: item.path.clone(),
            parent: parent.to_owned(),
            is_dir: item.is_dir(),
            depth,
            expanded: is_expanded,
        });
        if item.is_dir() && is_expanded {
            flatten(children, expanded, &item.path, depth + 1, out);
        }
    }
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
    let (settings_open, set_settings_open) = signal(false);
    // Desktop shows the sidebar inline; phones open it as a drawer.
    let (sidebar_open, set_sidebar_open) = signal(false);

    // Library tree, loaded one directory at a time as it is expanded.
    let (children, set_children) = signal(HashMap::<String, Vec<Entry>>::new());
    let (expanded, set_expanded) = signal(HashSet::<String>::new());
    let (library_root, set_library_root) = signal(String::new());

    let (playlists, set_playlists) = signal(Vec::<(i64, String, i64)>::new());
    let (queue, set_queue) = signal(Vec::<Entry>::new());
    let (list_name, set_list_name) = signal(String::new());
    let (current, set_current) = signal(0_usize);
    let (playing, set_playing) = signal(false);
    let (filter, set_filter) = signal(String::new());
    let (sort_key, set_sort_key) = signal(SortKey::Index);
    let (sort_asc, set_sort_asc) = signal(true);
    let (volume, set_volume) = signal(0.9_f64);
    let (position, set_position) = signal(0.0_f64);
    let (duration, set_duration) = signal(0.0_f64);

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

    // Load one directory level. The library root is keyed as "".
    let load_dir = {
        let get_json = get_json;
        move |directory: String| {
            let route = if directory.is_empty() {
                "/api/library".to_owned()
            } else {
                format!("/api/library?path={}", url_encode(&directory))
            };
            leptos::task::spawn_local(async move {
                match get_json(route).await {
                    Ok(value) => {
                        let relative = value["path"].as_str().unwrap_or_default().to_owned();
                        if directory.is_empty() {
                            set_library_root.set(relative.clone());
                        }
                        let mut items: Vec<Entry> = Vec::new();
                        if let Some(dirs) = value["directories"].as_array() {
                            for dir in dirs {
                                let path =
                                    dir["path"].as_str().unwrap_or_default().to_owned();
                                let name = dir["name"]
                                    .as_str()
                                    .map(str::to_owned)
                                    .unwrap_or_else(|| last_segment(&path));
                                let where_ = dir["relative"]
                                    .as_str()
                                    .map(str::to_owned)
                                    .unwrap_or_else(|| name.clone());
                                items.push(Entry {
                                    kind: "dir".to_owned(),
                                    path,
                                    entry: String::new(),
                                    fragment: None,
                                    name,
                                    location: where_,
                                });
                            }
                        }
                        if let Some(files) = value["files"].as_array() {
                            items.extend(files.iter().map(|file| {
                                let path =
                                    file["path"].as_str().unwrap_or_default().to_owned();
                                let name = file["name"]
                                    .as_str()
                                    .map(str::to_owned)
                                    .unwrap_or_else(|| last_segment(&path));
                                let where_ = file["relative"]
                                    .as_str()
                                    .map(str::to_owned)
                                    .unwrap_or_else(|| name.clone());
                                Entry::local(&path, &name, &where_)
                            }));
                        }
                        set_children.update(|map| {
                            map.insert(directory.clone(), items);
                        });
                    }
                    Err(error) => set_message.set(error),
                }
            });
        }
    };

    let toggle_dir = {
        let load_dir = load_dir.clone();
        move |path: String| {
            let mut now_open = false;
            set_expanded.update(|set| {
                if set.remove(&path) {
                    now_open = false;
                } else {
                    set.insert(path.clone());
                    now_open = true;
                }
            });
            if now_open {
                let loaded = children.get().contains_key(&path);
                if !loaded {
                    load_dir(path);
                }
            }
        }
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

    // Selecting a playlist loads its entries into the pane, the way the desktop
    // window swaps the playlist pane when you pick one in the sidebar.
    let load_playlist = {
        let get_json = get_json;
        move |id: i64, name: String| {
            leptos::task::spawn_local(async move {
                match get_json(format!("/api/playlists/{id}")).await {
                    Ok(value) => {
                        let entries: Vec<Entry> = value["entries"]
                            .as_array()
                            .map(|items| items.iter().map(entry_from_json).collect())
                            .unwrap_or_default();
                        set_list_name.set(name);
                        set_queue.set(entries);
                        set_current.set(0);
                        set_position.set(0.0);
                        set_duration.set(0.0);
                        set_playing.set(false);
                    }
                    Err(error) => set_message.set(error),
                }
            });
        }
    };

    let connect = {
        let load_dir = load_dir.clone();
        let load_playlists = load_playlists.clone();
        move |_| {
            let header = auth().header();
            let url = format!("{}/api/version", base());
            let basic = use_basic.get();
            store("kog.server", &server.get());
            if !basic {
                store("kog.token", &token.get());
            }
            let load_dir = load_dir.clone();
            let load_playlists = load_playlists.clone();
            leptos::task::spawn_local(async move {
                let mut request = Request::get(&url);
                if let Some(header) = header {
                    request = request.header("Authorization", &header);
                }
                match request.send().await {
                    Ok(response) if response.ok() => {
                        set_connected.set(true);
                        set_settings_open.set(false);
                        set_message.set(String::new());
                        load_dir(String::new());
                        load_playlists();
                    }
                    Ok(response) if response.status() == 401 => {
                        set_message.set("That token or password was rejected".to_owned())
                    }
                    Ok(response) => {
                        set_message.set(format!("Server returned {}", response.status()))
                    }
                    Err(error) => set_message.set(format!("Could not reach the server: {error}")),
                }
            });
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

    // Playing a list: the pane's rows are the queue, like the desktop playlist.
    let play_list = move |entries: Vec<Entry>, name: String, start: usize| {
        if entries.is_empty() {
            return;
        }
        let len = entries.len();
        set_list_name.set(name);
        set_queue.set(entries);
        set_current.set(start.min(len.saturating_sub(1)));
        set_position.set(0.0);
        set_duration.set(0.0);
        set_playing.set(true);
    };

    let audio_ref = NodeRef::<leptos::html::Audio>::new();
    let current_entry = move || queue.get().get(current.get()).cloned();
    let audio_src = move || current_entry().map(|entry| stream_url(&entry)).unwrap_or_default();

    // Keep the element in step with the transport button, and roll on when a
    // track ends.
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

    Effect::new(move |_| {
        if let Some(audio) = audio_ref.get() {
            audio.set_volume(volume.get());
        }
    });

    let step = move |delta: i64| {
        let len = queue.get().len();
        if len == 0 {
            return;
        }
        let next = (current.get() as i64 + delta).clamp(0, len as i64 - 1) as usize;
        if next != current.get() {
            set_current.set(next);
            set_position.set(0.0);
            set_duration.set(0.0);
            set_playing.set(true);
        }
    };

    let tree_rows = move || {
        let children = children.get();
        let expanded = expanded.get();
        let mut out = Vec::new();
        flatten(&children, &expanded, "", 0, &mut out);
        out
    };

    // The visible rows carry their index into the queue, so filtering and
    // sorting never change what plays next.
    let view_rows = move || {
        let needle = filter.get().trim().to_lowercase();
        let mut rows: Vec<(usize, Entry)> = queue
            .get()
            .into_iter()
            .enumerate()
            .filter(|(_, entry)| {
                needle.is_empty()
                    || entry.name.to_lowercase().contains(&needle)
                    || entry.location.to_lowercase().contains(&needle)
            })
            .collect();
        match sort_key.get() {
            SortKey::Index => {}
            SortKey::Name => rows.sort_by_key(|(_, entry)| entry.name.to_lowercase()),
            SortKey::Location => rows.sort_by_key(|(_, entry)| entry.location.clone()),
        }
        if !sort_asc.get() && sort_key.get() != SortKey::Index {
            rows.reverse();
        }
        rows
    };

    let queue_files = move |directory: &str| -> Vec<Entry> {
        children
            .get()
            .get(directory)
            .map(|items| items.iter().filter(|item| !item.is_dir()).cloned().collect())
            .unwrap_or_default()
    };

    let toggle_sort = move |key: SortKey| {
        if sort_key.get() == key {
            set_sort_asc.update(|asc| *asc = !*asc);
        } else {
            set_sort_key.set(key);
            set_sort_asc.set(true);
        }
    };

    view! {
        <div class="app">
            <header class="toolbar">
                <button
                    class="flat"
                    title="Show or hide the sidebar"
                    on:click=move |_| set_sidebar_open.update(|open| *open = !*open)
                >"☰"</button>
                <img class="logo" src="/icons/kog.svg" alt="" />
                <span class="brand">"Kog"</span>
                <div class="search">
                    <input
                        type="search"
                        placeholder="Filter the playlist"
                        prop:value=move || filter.get()
                        on:input=move |event| set_filter.set(event_target_value(&event))
                    />
                </div>
                <select
                    class="codec"
                    title="Stream format"
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
                <button
                    class="flat"
                    title=move || if connected.get() { "Connected" } else { "Not connected" }
                    on:click=move |_| set_settings_open.update(|open| *open = !*open)
                >{move || if connected.get() { "● Server" } else { "○ Server" }}</button>
            </header>

            <div class:sidebar-open=move || sidebar_open.get() class="workspace">
                <div
                    class="drawer-scrim"
                    on:click=move |_| set_sidebar_open.set(false)
                ></div>

                <aside class="sidebar">
                    <section class="section files">
                        <div class="section-header">
                            <span>"File Tree"</span>
                        </div>
                        <div class="section-body">
                            <Show when=move || connected.get() fallback=|| view! {
                                <p class="empty">"Connect to a server to browse."</p>
                            }>
                                <Show when=move || tree_rows().is_empty() fallback=|| ()>
                                    <p class="empty">"No music folder is configured on the server."</p>
                                </Show>
                                <For each=tree_rows key=|row| format!("{}#{}", row.path, row.depth) let:row>
                                    {
                                        let row_for_click = row.clone();
                                        let toggle_dir = toggle_dir.clone();
                                        let play_list = play_list.clone();
                                        let queue_files = queue_files.clone();
                                        let label = if row.is_dir {
                                            format!("{} {}", if row.expanded { "▾" } else { "▸" }, row.name)
                                        } else {
                                            format!("  {}", row.name)
                                        };
                                        view! {
                                            <button
                                                class="tree-row"
                                                class:directory=row.is_dir
                                                style=format!("padding-left: {}px", 8 + row.depth * 12)
                                                title=row.path.clone()
                                                on:click=move |_| {
                                                    if row_for_click.is_dir {
                                                        toggle_dir(row_for_click.path.clone());
                                                    } else {
                                                        let files = queue_files(&row_for_click.parent);
                                                        let index = files
                                                            .iter()
                                                            .position(|item| item.path == row_for_click.path)
                                                            .unwrap_or(0);
                                                        play_list(files, last_segment(&row_for_click.parent), index);
                                                    }
                                                }
                                            >
                                                <span class="twisty"></span>
                                                <span
                                                    class=if row_for_click.is_dir {
                                                        "tree-icon dir"
                                                    } else {
                                                        "tree-icon file"
                                                    }
                                                ></span>
                                                <span class="label">{label}</span>
                                            </button>
                                        }
                                    }
                                </For>
                            </Show>
                        </div>
                    </section>

                    <section class="section">
                        <div class="section-header"><span>"Playlists"</span></div>
                        <div class="section-body">
                            <Show when=move || connected.get() fallback=|| ()>
                                {
                                    let load_favorites = load_playlist.clone();
                                    view! {
                                        <button class="tree-row" on:click=move |_| load_favorites(0, "Favorites".to_owned())>
                                            <span class="twisty"></span>
                                            <span class="label">"★ Favorites"</span>
                                        </button>
                                    }
                                }
                                <For each=move || playlists.get() key=|item| item.0 let:item>
                                    {
                                        let load_one = load_playlist.clone();
                                        let id = item.0;
                                        let name = item.1.clone();
                                        let label = name.clone();
                                        view! {
                                            <button class="tree-row" on:click=move |_| load_one(id, name.clone())>
                                                <span class="twisty"></span>
                                                <span class="label">{label}</span>
                                                <span class="twisty">{item.2}</span>
                                            </button>
                                        }
                                    }
                                </For>
                                <Show when=move || !connected.get() fallback=|| ()>
                                    <p class="empty">"Not connected."</p>
                                </Show>
                            </Show>
                        </div>
                    </section>
                </aside>

                <main class="playlist">
                    <div class="playlist-head">
                        <span class="name">
                            {move || if list_name.get().is_empty() { "Playlist".to_owned() } else { list_name.get() }}
                        </span>
                        <span class="count">
                            {move || {
                                let total = queue.get().len();
                                let shown = view_rows().len();
                                if shown == total {
                                    format!("{total} tracks")
                                } else {
                                    format!("{shown} of {total}")
                                }
                            }}
                        </span>
                    </div>

                    <div class="columns">
                        <button on:click=move |_| toggle_sort(SortKey::Index)>"#"</button>
                        <button on:click=move |_| toggle_sort(SortKey::Name)>"Title"</button>
                        <button class="location-cell" on:click=move |_| toggle_sort(SortKey::Location)>"Location"</button>
                        <span class="length-cell cell">"Length"</span>
                    </div>

                    <div class="rows">
                        <Show
                            when=move || !view_rows().is_empty()
                            fallback=move || view! {
                                <p class="empty">
                                    {move || if connected.get() {
                                        "Pick a folder in the file tree, or a playlist."
                                    } else {
                                        "Open the server settings to connect."
                                    }}
                                </p>
                            }
                        >
                            <For each=view_rows key=|(index, entry)| format!("{index}:{}", entry.path) let:row>
                                {
                                    let (index, entry) = row;
                                    let length = move || {
                                        if current.get() == index && duration.get() > 0.0 {
                                            clock(duration.get())
                                        } else {
                                            "-:--".to_owned()
                                        }
                                    };
                                    view! {
                                        <button
                                            class="track"
                                            class:current=move || current.get() == index
                                            on:click=move |_| {
                                                set_current.set(index);
                                                set_position.set(0.0);
                                                set_duration.set(0.0);
                                                set_playing.set(true);
                                            }
                                        >
                                            <span class="index cell">{index + 1}</span>
                                            <span class="cell name-cell">{entry.name.clone()}</span>
                                            <span class="cell location">{entry.location.clone()}</span>
                                            <span class="length cell">{length}</span>
                                        </button>
                                    }
                                }
                            </For>
                        </Show>
                    </div>
                </main>
            </div>

            <footer class="transport">
                <div class="now">
                    <div class="art">"♫"</div>
                    <div class="now-text">
                        <div class="title">
                            {move || current_entry().map(|entry| entry.name).unwrap_or_else(|| "Kog".to_owned())}
                        </div>
                        <div class="subtitle">
                            {move || current_entry().map(|entry| entry.location).unwrap_or_else(|| "Ready to play".to_owned())}
                        </div>
                    </div>
                </div>

                <div class="controls">
                    <button
                        title="Previous"
                        disabled=move || (queue.get().is_empty() || current.get() == 0)
                        on:click=move |_| step(-1)
                    >"⏮"</button>
                    <button
                        class="play"
                        title="Play or pause"
                        disabled=move || queue.get().is_empty()
                        on:click=move |_| set_playing.update(|value| *value = !*value)
                    >{move || if playing.get() { "⏸" } else { "▶" }}</button>
                    <button
                        title="Next"
                        disabled=move || (queue.get().is_empty() || current.get() + 1 >= queue.get().len())
                        on:click=move |_| step(1)
                    >"⏭"</button>
                </div>

                <div class="right">
                    <div class="seek">
                        <span>{move || clock(position.get())}</span>
                        <input
                            type="range"
                            min="0"
                            max=move || (if duration.get() > 0.0 { duration.get() } else { 1.0 })
                            step="0.5"
                            prop:value=move || position.get()
                            on:input=move |event| {
                                let target: f64 = event_target_value(&event).parse().unwrap_or(0.0);
                                if let Some(audio) = audio_ref.get() {
                                    audio.set_current_time(target);
                                }
                                set_position.set(target);
                            }
                        />
                        <span>{move || clock(duration.get())}</span>
                    </div>
                    <input
                        class="volume"
                        type="range"
                        min="0"
                        max="1"
                        step="0.01"
                        title="Volume"
                        prop:value=move || volume.get()
                        on:input=move |event| {
                            set_volume.set(event_target_value(&event).parse().unwrap_or(0.9))
                        }
                    />
                </div>

                <audio
                    class="audio"
                    node_ref=audio_ref
                    preload="auto"
                    prop:src=audio_src
                    on:timeupdate=move |_| {
                        if let Some(audio) = audio_ref.get() {
                            set_position.set(audio.current_time());
                        }
                    }
                    on:loadedmetadata=move |_| {
                        if let Some(audio) = audio_ref.get() {
                            set_duration.set(audio.duration());
                        }
                    }
                    on:ended=move |_| {
                        if current.get() + 1 < queue.get().len() {
                            set_current.update(|index| *index += 1);
                            set_position.set(0.0);
                            set_duration.set(0.0);
                            set_playing.set(true);
                        } else {
                            set_playing.set(false);
                        }
                    }
                ></audio>
            </footer>

            <Show when=move || settings_open.get() fallback=|| ()>
                <div class="scrim" on:click=move |_| set_settings_open.set(false)></div>
                <div class="settings" role="dialog">
                    <h2>"Server"</h2>
                    <label>
                        "Address"
                        <input
                            placeholder="https://my-desktop:8420"
                            prop:value=move || server.get()
                            on:input=move |event| set_server.set(event_target_value(&event))
                        />
                    </label>
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
                            <label>
                                "API token"
                                <input
                                    type="password"
                                    prop:value=move || token.get()
                                    on:input=move |event| set_token.set(event_target_value(&event))
                                />
                            </label>
                        }
                    >
                        <label>
                            "Username"
                            <input
                                prop:value=move || user.get()
                                on:input=move |event| set_user.set(event_target_value(&event))
                            />
                        </label>
                        <label>
                            "Password"
                            <input
                                type="password"
                                prop:value=move || password.get()
                                on:input=move |event| set_password.set(event_target_value(&event))
                            />
                        </label>
                    </Show>
                    <p class="hint">
                        "The address and token are shown in Kog's Preferences → Server on the machine serving the library."
                    </p>
                    <div class="settings-actions">
                        <button class="primary" on:click=connect>
                            {move || if connected.get() { "Reconnect" } else { "Connect" }}
                        </button>
                        <button on:click=move |_| set_settings_open.set(false)>"Close"</button>
                    </div>
                    <Show when=move || !message.get().is_empty() fallback=|| ()>
                        <p class="hint error">{move || message.get()}</p>
                    </Show>
                </div>
            </Show>
        </div>
    }
}
