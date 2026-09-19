//! Kog's web player.
//!
//! A client, not a second server: with per-client streams the browser is the
//! player, so this app owns its queue and plays the API's stream URLs through
//! an `<audio>` element.
//!
//! The layout mirrors the desktop window: a 48px toolbar, a sidebar holding the
//! file tree and the playlists, the playlist pane with a column header, and a
//! 92px transport bar. The shell is a fixed-height grid, so only the pane
//! scrolls, never the page. Below 820px the sidebar becomes a drawer and the
//! rows go compact for phones.
//!
//! The playlist columns mirror `qml/PlaylistHeader.qml`'s defaults: the index,
//! Title, Artist, Album, Length and track number. Tags arrive from
//! `POST /api/metadata` in one batch per refresh and are cached by locator, so
//! re-renders never refetch.

use std::collections::{HashMap, HashSet};
use std::rc::Rc;

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
    /// Location as shown in the subtitle (relative where known).
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

/// One row of `POST /api/metadata`, shaped like the playlist columns.
///
/// Every field is present in the API response; the ones the web pane does not
/// render yet are still parsed so the struct matches the wire shape.
#[allow(dead_code)]
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MetaRow {
    title: Option<String>,
    artist: Option<String>,
    album: Option<String>,
    genre: Option<String>,
    year: Option<u32>,
    track_number: Option<u32>,
    duration: Option<f64>,
    sample_rate: Option<u32>,
    channels: Option<u16>,
    bits_per_sample: Option<u8>,
    codec: Option<String>,
    bitrate: Option<u32>,
}

/// The columns the pane sorts by, matching the visible header order.
#[derive(Clone, Copy, PartialEq, Eq)]
enum SortKey {
    Index,
    Title,
    Artist,
    Album,
    Duration,
    Track,
}

/// Repeat policy, cycled by the transport's repeat toggle.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Repeat {
    Off,
    One,
    All,
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

/// `m:ss`, or `h:mm:ss` past an hour, matching the desktop's `timeLabel`.
fn clock(seconds: f64) -> String {
    if !seconds.is_finite() || seconds < 0.0 {
        return "-:--".to_owned();
    }
    let total = seconds.round() as u64;
    let (hours, minutes, seconds) = (total / 3600, (total % 3600) / 60, total % 60);
    if hours > 0 {
        format!("{hours}:{minutes:02}:{seconds:02}")
    } else {
        format!("{minutes}:{seconds:02}")
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

/// The metadata cache key: the same locator fields the server hashes.
fn meta_key(entry: &Entry) -> String {
    format!(
        "{}|{}|{}|{}",
        entry.kind,
        entry.path,
        entry.entry,
        entry.fragment.clone().unwrap_or_default()
    )
}

/// A cached metadata row for an entry, if one has been fetched.
fn meta_for(cache: &HashMap<String, Option<MetaRow>>, entry: &Entry) -> Option<MetaRow> {
    cache.get(&meta_key(entry)).and_then(|row| row.clone())
}

/// The parent of an absolute path, or the empty string at the filesystem root.
fn parent_path(path: &str) -> String {
    let trimmed = path.trim_end_matches('/');
    if trimmed.is_empty() {
        return String::new();
    }
    match trimmed.rfind('/') {
        Some(0) => "/".to_owned(),
        Some(index) => trimmed[..index].to_owned(),
        None => String::new(),
    }
}

/// Whether `child` is `root` or lives inside it.
fn is_under(child: &str, root: &str) -> bool {
    if root.trim().is_empty() {
        return false;
    }
    let root = root.trim_end_matches('/');
    child == root || child.starts_with(&format!("{root}/"))
}

/// A random queue position other than `current`, for shuffle playback.
fn random_other(current: usize, len: usize) -> usize {
    if len <= 1 {
        return current;
    }
    let mut candidate = current;
    for _ in 0..8 {
        let roll = (js_sys::Math::random() * len as f64).floor() as usize;
        candidate = roll.min(len - 1);
        if candidate != current {
            break;
        }
    }
    candidate
}

/// `POST` a JSON body and decode a JSON response, carrying the same auth and
/// errors as the `get_json` wrapper.
async fn post_json(
    url: String,
    header: Option<String>,
    body: serde_json::Value,
) -> Result<serde_json::Value, String> {
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
    let (files_expanded, set_files_expanded) = signal(true);
    let (playlists_expanded, set_playlists_expanded) = signal(true);

    // Library tree, loaded one directory at a time as it is expanded.
    let (children, set_children) = signal(HashMap::<String, Vec<Entry>>::new());
    let (expanded, set_expanded) = signal(HashSet::<String>::new());
    let (library_root, set_library_root) = signal(String::new());
    // The directory the tree is rooted at: "" means the server's library root.
    let (tree_root, set_tree_root) = signal(String::new());
    let (tree_selected, set_tree_selected) = signal(String::new());
    let (tree_search, set_tree_search) = signal(String::new());

    let (playlists, set_playlists) = signal(Vec::<(i64, String, i64)>::new());
    let (queue, set_queue) = signal(Vec::<Entry>::new());
    let (list_name, set_list_name) = signal(String::new());
    let (current, set_current) = signal(0_usize);
    let (playing, set_playing) = signal(false);
    let (filter, set_filter) = signal(String::new());
    let (sort_key, set_sort_key) = signal(SortKey::Index);
    let (sort_asc, set_sort_asc) = signal(true);
    let (volume, set_volume) = signal(0.9_f64);
    let (volume_before_mute, set_volume_before_mute) = signal(0.9_f64);
    let (position, set_position) = signal(0.0_f64);
    let (duration, set_duration) = signal(0.0_f64);
    let (shuffle, set_shuffle) = signal(false);
    let (repeat_mode, set_repeat_mode) = signal(Repeat::Off);
    // Tag cache keyed by locator. An `Rc` so reading it clones a pointer, not
    // the map, on every cell render.
    let (metadata, set_metadata) =
        signal_local(Rc::new(HashMap::<String, Option<MetaRow>>::new()));

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
                            set_library_root.set(relative);
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

    // Root the tree at `path` ("" is the server's library root), collapsing the
    // old expansion the way the desktop tree resets when its root changes.
    let goto_root = {
        let load_dir = load_dir.clone();
        move |path: String| {
            set_tree_selected.set(String::new());
            set_expanded.set(HashSet::new());
            let loaded = children.get().contains_key(&path);
            set_tree_root.set(path.clone());
            if !loaded {
                load_dir(path);
            }
        }
    };

    let go_up = {
        let goto_root = goto_root.clone();
        move || {
            let current = tree_root.get();
            if current.is_empty() {
                return;
            }
            let library = library_root.get();
            let parent = parent_path(&current);
            let target = if parent.is_empty()
                || parent == library
                || !is_under(&parent, &library)
            {
                String::new()
            } else {
                parent
            };
            goto_root(target);
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

    // The playlists header's "+", as the desktop sidebar offers.
    let create_playlist = {
        let load_playlists = load_playlists.clone();
        move || {
            let url = format!("{}/api/playlists", base());
            let header = auth().header();
            let load_playlists = load_playlists.clone();
            leptos::task::spawn_local(async move {
                let body = serde_json::json!({ "name": "New Playlist" });
                if let Err(error) = post_json(url, header, body).await {
                    let _ = error;
                }
                load_playlists();
            });
        }
    };

    let connect = {
        let load_dir = load_dir.clone();
        let load_playlists = load_playlists.clone();
        move || {
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

    // The page is served by the Kog server itself, so connect on load rather
    // than landing on an empty shell until the user finds the server settings.
    let (auto_connected, set_auto_connected) = signal(false);
    let auto_connect = connect.clone();
    Effect::new(move |_| {
        if auto_connected.get_untracked() {
            return;
        }
        set_auto_connected.set(true);
        auto_connect();
    });

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

    // One batched tag lookup for everything the pane currently shows. The cache
    // is read untracked so a successful fetch does not immediately schedule the
    // same request again; a re-render of the rows is the only effect.
    Effect::new(move |_| {
        if !connected.get() {
            return;
        }
        let entries = queue.get();
        if entries.is_empty() {
            return;
        }
        let known = metadata.get_untracked();
        let missing: Vec<Entry> = entries
            .into_iter()
            .filter(|entry| !known.contains_key(&meta_key(entry)))
            .collect();
        if missing.is_empty() {
            return;
        }
        let url = format!("{}/api/metadata", base());
        let header = auth().header();
        for chunk in missing.chunks(1000) {
            let body = serde_json::Value::Array(
                chunk
                    .iter()
                    .map(|entry| {
                        serde_json::json!({
                            "kind": entry.kind,
                            "path": entry.path,
                            "entry": entry.entry,
                            "fragment": entry.fragment,
                        })
                    })
                    .collect(),
            );
            let keys: Vec<String> = chunk.iter().map(meta_key).collect();
            let url = url.clone();
            let header = header.clone();
            leptos::task::spawn_local(async move {
                let Ok(value) = post_json(url, header, body).await else {
                    return;
                };
                let Ok(rows) = serde_json::from_value::<Vec<Option<MetaRow>>>(value) else {
                    return;
                };
                let updates: Vec<(String, Option<MetaRow>)> = keys
                    .into_iter()
                    .enumerate()
                    .map(|(index, key)| (key, rows.get(index).cloned().flatten()))
                    .collect();
                set_metadata.update(|map| {
                    let map = Rc::make_mut(map);
                    for (key, row) in updates {
                        map.insert(key, row);
                    }
                });
            });
        }
    });

    let jump = move |index: usize| {
        set_current.set(index);
        set_position.set(0.0);
        set_duration.set(0.0);
        set_playing.set(true);
    };

    // End-of-track advance: shuffle picks any other row, otherwise step the
    // queue and wrap only when repeat is on.
    let advance_after_track = move || -> Option<usize> {
        let len = queue.get().len();
        if len == 0 {
            return None;
        }
        if shuffle.get() {
            return Some(random_other(current.get(), len));
        }
        if current.get() + 1 < len {
            return Some(current.get() + 1);
        }
        if repeat_mode.get() == Repeat::All {
            return Some(0);
        }
        None
    };

    let step = move |delta: i64| {
        let len = queue.get().len();
        if len == 0 {
            return;
        }
        let next = if shuffle.get() {
            random_other(current.get(), len)
        } else if delta < 0 {
            if current.get() == 0 {
                if repeat_mode.get() == Repeat::All {
                    len - 1
                } else {
                    0
                }
            } else {
                current.get() - 1
            }
        } else if current.get() + 1 < len {
            current.get() + 1
        } else if repeat_mode.get() == Repeat::All {
            0
        } else {
            current.get()
        };
        if next != current.get() {
            jump(next);
        }
    };

    let toggle_mute = move |_| {
        if volume.get() > 0.0 {
            set_volume_before_mute.set(volume.get());
            set_volume.set(0.0);
        } else {
            let restored = volume_before_mute.get();
            set_volume.set(if restored > 0.0 { restored } else { 0.75 });
        }
    };

    let tree_rows = move || {
        let children = children.get();
        let expanded = expanded.get();
        let root = tree_root.get();
        let needle = tree_search.get().trim().to_lowercase();
        let mut out = Vec::new();
        flatten(&children, &expanded, &root, 0, &mut out);
        if !needle.is_empty() {
            out.retain(|row| row.name.to_lowercase().contains(&needle));
        }
        out
    };

    let current_root = move || {
        let root = tree_root.get();
        if root.is_empty() {
            library_root.get()
        } else {
            root
        }
    };

    // The visible rows carry their index into the queue, so filtering and
    // sorting never change what plays next.
    let view_rows = move || {
        let needle = filter.get().trim().to_lowercase();
        let cache = metadata.get();
        let key = sort_key.get();
        let mut rows: Vec<(usize, Entry)> = queue
            .get()
            .into_iter()
            .enumerate()
            .filter(|(_, entry)| {
                if needle.is_empty() {
                    return true;
                }
                if entry.name.to_lowercase().contains(&needle)
                    || entry.location.to_lowercase().contains(&needle)
                {
                    return true;
                }
                match meta_for(&cache, entry) {
                    Some(meta) => [meta.title, meta.artist, meta.album]
                        .into_iter()
                        .flatten()
                        .any(|value| value.to_lowercase().contains(&needle)),
                    None => false,
                }
            })
            .collect();
        if key != SortKey::Index {
            let value = |entry: &Entry| -> String {
                let meta = meta_for(&cache, entry);
                match key {
                    SortKey::Index => String::new(),
                    SortKey::Title => meta
                        .and_then(|meta| meta.title)
                        .unwrap_or_else(|| entry.name.clone()),
                    SortKey::Artist => meta.and_then(|meta| meta.artist).unwrap_or_default(),
                    SortKey::Album => meta.and_then(|meta| meta.album).unwrap_or_default(),
                    SortKey::Duration => meta
                        .and_then(|meta| meta.duration)
                        .map(|seconds| format!("{seconds:010.3}"))
                        .unwrap_or_default(),
                    SortKey::Track => meta
                        .and_then(|meta| meta.track_number)
                        .map(|number| format!("{number:06}"))
                        .unwrap_or_default(),
                }
            };
            rows.sort_by_key(|(_, entry)| value(entry).to_lowercase());
            if !sort_asc.get() {
                rows.reverse();
            }
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
    let sort_arrow = move |key: SortKey| -> &'static str {
        if sort_key.get() == key {
            if sort_asc.get() { " ▲" } else { " ▼" }
        } else {
            ""
        }
    };

    // The transport shows the current row's tags, with the file name as the
    // title fallback and artist • album as the subtitle, like the desktop bar.
    let now_title = move || {
        let entry = current_entry();
        let meta = entry.as_ref().and_then(|entry| meta_for(&metadata.get(), entry));
        meta.and_then(|meta| meta.title)
            .or_else(|| entry.as_ref().map(|entry| entry.name.clone()))
            .unwrap_or_else(|| "Kog".to_owned())
    };
    let now_subtitle = move || {
        let entry = current_entry();
        let meta = entry.as_ref().and_then(|entry| meta_for(&metadata.get(), entry));
        let artist = meta.as_ref().and_then(|meta| meta.artist.clone()).unwrap_or_default();
        let album = meta.as_ref().and_then(|meta| meta.album.clone()).unwrap_or_default();
        match (artist.is_empty(), album.is_empty()) {
            (false, false) => format!("{artist}  •  {album}"),
            (false, true) => artist,
            (true, false) => album,
            (true, true) => entry
                .as_ref()
                .map(|entry| entry.location.clone())
                .unwrap_or_else(|| "Ready to play".to_owned()),
        }
    };
    let repeat_badge = move || match repeat_mode.get() {
        Repeat::Off => "",
        Repeat::One => "1",
        Repeat::All => "∞",
    };
    let repeat_tip = move || match repeat_mode.get() {
        Repeat::Off => "Repeat off — click for one track",
        Repeat::One => "Repeat one track — click for all",
        Repeat::All => "Repeat all tracks — click to turn off",
    };
    let shuffle_tip = move || {
        if shuffle.get() {
            "Shuffle on — click to turn off"
        } else {
            "Shuffle off — click to turn on"
        }
    };

    view! {
        <div class="app">
            <header class="toolbar">
                <button
                    class="flat icon-button"
                    title="Show or hide the sidebar"
                    on:click=move |_| set_sidebar_open.update(|open| *open = !*open)
                >"☰"</button>
                <img class="logo" src="/icons/kog.svg" alt="Kog" />
                <div class="search">
                    <span class="pill-icon" aria-hidden="true">"⌕"</span>
                    <input
                        type="search"
                        placeholder="Search playlist"
                        prop:value=move || filter.get()
                        on:input=move |event| set_filter.set(event_target_value(&event))
                    />
                    <Show when=move || !filter.get().is_empty() fallback=|| ()>
                        <button
                            class="flat search-clear"
                            title="Clear playlist search"
                            on:click=move |_| set_filter.set(String::new())
                        >"×"</button>
                    </Show>
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
                    class="flat server"
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
                        <div
                            class="section-header"
                            role="button"
                            tabindex="0"
                            on:click=move |_| set_files_expanded.update(|open| *open = !*open)
                        >
                            <span class="expander">
                                {move || if files_expanded.get() { "▾" } else { "▸" }}
                            </span>
                            <span class="section-title">"Files"</span>
                        </div>
                        <Show when=move || files_expanded.get() fallback=|| ()>
                            <div class="section-body">
                                <Show
                                    when=move || connected.get()
                                    fallback=|| view! {
                                        <p class="empty">"Connect to a server to browse."</p>
                                    }
                                >
                                    <div class="tree-root">
                                        <button
                                            class="icon-button"
                                            title="Root the tree at the selected folder, or go up"
                                            on:click=move |_| {
                                                let selected = tree_selected.get();
                                                if !selected.is_empty()
                                                    && children.get().contains_key(&selected)
                                                {
                                                    goto_root(selected);
                                                } else {
                                                    go_up();
                                                }
                                            }
                                        >"▣"</button>
                                        <button
                                            class="icon-button"
                                            title="Refresh this folder"
                                            on:click={
                                                let load_dir = load_dir.clone();
                                                move |_| {
                                                    let root = tree_root.get();
                                                    load_dir(root);
                                                }
                                            }
                                        >"↻"</button>
                                        <button
                                            class="root-path"
                                            title=move || current_root()
                                            on:click=move |_| goto_root(String::new())
                                        >{move || current_root()}</button>
                                    </div>
                                    <div class="tree-search">
                                        <span class="pill-icon" aria-hidden="true">"⌕"</span>
                                        <input
                                            type="search"
                                            placeholder="Search files and folders…"
                                            prop:value=move || tree_search.get()
                                            on:input=move |event| {
                                                set_tree_search.set(event_target_value(&event))
                                            }
                                        />
                                    </div>
                                    <div class="tree-list">
                                        <Show when=move || !tree_root.get().is_empty() fallback=|| ()>
                                            <button
                                                class="tree-row parent-row"
                                                title="Go to parent folder"
                                                on:click={
                                                    let go_up = go_up.clone();
                                                    move |_| go_up()
                                                }
                                            >
                                                <span class="twisty"></span>
                                                <span class="tree-icon up"></span>
                                                <span class="label">".."</span>
                                            </button>
                                        </Show>
                                        <For
                                            each=tree_rows
                                            key=|row| format!("{}#{}", row.path, row.depth)
                                            let:row
                                        >
                                            {
                                                let row_click = row.clone();
                                                let row_dbl = row.clone();
                                                let toggle_dir = toggle_dir.clone();
                                                let queue_files = queue_files.clone();
                                                let selected = row.path.clone();
                                                let twisty = if row.is_dir {
                                                    if row.expanded { "▾" } else { "▸" }
                                                } else {
                                                    ""
                                                };
                                                let indent = 6 + row.depth * 16;
                                                view! {
                                                    <button
                                                        class="tree-row"
                                                        class:directory=row.is_dir
                                                        class:selected=move || {
                                                            tree_selected.get() == selected
                                                        }
                                                        style=format!("padding-left: {indent}px")
                                                        title=row.path.clone()
                                                        on:click=move |_| {
                                                            set_tree_selected.set(row_click.path.clone());
                                                            if row_click.is_dir {
                                                                toggle_dir(row_click.path.clone());
                                                            }
                                                        }
                                                        on:dblclick=move |_| {
                                                            if row_dbl.is_dir {
                                                                toggle_dir(row_dbl.path.clone());
                                                            } else if let Some(entry) =
                                                                queue_files(&row_dbl.parent)
                                                                    .into_iter()
                                                                    .find(|item| {
                                                                        item.path == row_dbl.path
                                                                    })
                                                            {
                                                                // Queue without starting playback: adding
                                                                // to the pane must never interrupt the
                                                                // current song, exactly as in the desktop
                                                                // tree.
                                                                let key = meta_key(&entry);
                                                                let present = queue
                                                                    .get_untracked()
                                                                    .iter()
                                                                    .any(|item| meta_key(item) == key);
                                                                if !present {
                                                                    set_queue.update(|items| {
                                                                        items.push(entry)
                                                                    });
                                                                }
                                                            }
                                                        }
                                                    >
                                                        <span class="twisty">{twisty}</span>
                                                        <span class=if row.is_dir {
                                                            "tree-icon dir"
                                                        } else {
                                                            "tree-icon file"
                                                        }></span>
                                                        <span class="label">{row.name.clone()}</span>
                                                    </button>
                                                }
                                            }
                                        </For>
                                    </div>
                                    <Show
                                        when=move || {
                                            tree_rows().is_empty() && library_root.get().is_empty()
                                        }
                                        fallback=|| ()
                                    >
                                        <p class="empty">
                                            "No music folder is configured on the server."
                                        </p>
                                    </Show>
                                </Show>
                            </div>
                        </Show>
                    </section>

                    <section class="section playlists">
                        <div
                            class="section-header"
                            role="button"
                            tabindex="0"
                            on:click=move |_| {
                                set_playlists_expanded.update(|open| *open = !*open)
                            }
                        >
                            <span class="expander">
                                {move || if playlists_expanded.get() { "▾" } else { "▸" }}
                            </span>
                            <span class="section-title">"Playlists"</span>
                            <button
                                class="section-add"
                                title="Create a playlist"
                                on:click=move |event| {
                                    event.stop_propagation();
                                    create_playlist();
                                }
                            >"+"</button>
                        </div>
                        <Show when=move || playlists_expanded.get() fallback=|| ()>
                            <div class="section-body">
                                <Show when=move || connected.get() fallback=|| ()>
                                    {
                                        let load_favorites = load_playlist.clone();
                                        view! {
                                            <button
                                                class="tree-row favorite-row"
                                                on:click=move |_| {
                                                    load_favorites(0, "Favorites".to_owned())
                                                }
                                            >
                                                <span class="twisty"></span>
                                                <span class="favorite-star">"★"</span>
                                                <span class="label">"Favorites"</span>
                                            </button>
                                        }
                                    }
                                    <For each=move || playlists.get() key=|item| item.0 let:item>
                                        {
                                            let load_one = load_playlist.clone();
                                            let id = item.0;
                                            let name = item.1.clone();
                                            let count = item.2;
                                            let label = name.clone();
                                            view! {
                                                <button
                                                    class="tree-row playlist-row"
                                                    on:click=move |_| {
                                                        load_one(id, name.clone())
                                                    }
                                                >
                                                    <span class="twisty"></span>
                                                    <span class="label">{label}</span>
                                                    <span class="count">{count}</span>
                                                </button>
                                            }
                                        }
                                    </For>
                                    <Show when=move || !connected.get() fallback=|| ()>
                                        <p class="empty">"Not connected."</p>
                                    </Show>
                                </Show>
                            </div>
                        </Show>
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
                        <button class="cell index-cell" on:click=move |_| toggle_sort(SortKey::Index)>
                            {move || format!("#{}", sort_arrow(SortKey::Index))}
                        </button>
                        <button class="cell title-cell" on:click=move |_| toggle_sort(SortKey::Title)>
                            {move || format!("Title{}", sort_arrow(SortKey::Title))}
                        </button>
                        <button class="cell artist-cell" on:click=move |_| toggle_sort(SortKey::Artist)>
                            {move || format!("Artist{}", sort_arrow(SortKey::Artist))}
                        </button>
                        <button class="cell album-cell" on:click=move |_| toggle_sort(SortKey::Album)>
                            {move || format!("Album{}", sort_arrow(SortKey::Album))}
                        </button>
                        <button class="cell duration-cell" on:click=move |_| toggle_sort(SortKey::Duration)>
                            {move || format!("Length{}", sort_arrow(SortKey::Duration))}
                        </button>
                        <button class="cell trackno-cell" on:click=move |_| toggle_sort(SortKey::Track)>
                            {move || format!("№{}", sort_arrow(SortKey::Track))}
                        </button>
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
                            <For
                                each=view_rows
                                key=|(index, entry)| format!("{index}:{}::{}", entry.path, entry.entry)
                                let:row
                            >
                                {
                                    let (index, entry) = row;
                                    let entry_title = entry.clone();
                                    let entry_artist = entry.clone();
                                    let entry_album = entry.clone();
                                    let entry_duration = entry.clone();
                                    let entry_track = entry;
                                    let title_text = move || {
                                        meta_for(&metadata.get(), &entry_title)
                                            .and_then(|meta| meta.title)
                                            .unwrap_or_else(|| entry_title.name.clone())
                                    };
                                    let title_tip = title_text.clone();
                                    let artist_text = move || {
                                        meta_for(&metadata.get(), &entry_artist)
                                            .and_then(|meta| meta.artist)
                                            .unwrap_or_default()
                                    };
                                    let artist_tip = artist_text.clone();
                                    let album_text = move || {
                                        meta_for(&metadata.get(), &entry_album)
                                            .and_then(|meta| meta.album)
                                            .unwrap_or_default()
                                    };
                                    let album_tip = album_text.clone();
                                    let duration_text = move || {
                                        if let Some(seconds) = meta_for(&metadata.get(), &entry_duration)
                                            .and_then(|meta| meta.duration)
                                        {
                                            return clock(seconds);
                                        }
                                        if current.get() == index && duration.get() > 0.0 {
                                            clock(duration.get())
                                        } else {
                                            String::new()
                                        }
                                    };
                                    let track_text = move || {
                                        meta_for(&metadata.get(), &entry_track)
                                            .and_then(|meta| meta.track_number)
                                            .map(|number| number.to_string())
                                            .unwrap_or_default()
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
                                            <span class="cell index-cell">{index + 1}</span>
                                            <span class="cell title-cell" title=move || title_tip()>
                                                {move || title_text()}
                                            </span>
                                            <span class="cell artist-cell" title=move || artist_tip()>
                                                {move || artist_text()}
                                            </span>
                                            <span class="cell album-cell" title=move || album_tip()>
                                                {move || album_text()}
                                            </span>
                                            <span class="cell duration-cell">{move || duration_text()}</span>
                                            <span class="cell trackno-cell">{move || track_text()}</span>
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
                    <div class="art">
                        <img src="/icons/kog.svg" alt="Album cover" />
                    </div>
                    <div class="now-text">
                        <div class="title">{move || now_title()}</div>
                        <div class="subtitle">{move || now_subtitle()}</div>
                    </div>
                </div>

                <div class="transport-center">
                    <div class="controls">
                        <button
                            class="toggle shuffle"
                            class:active=move || shuffle.get()
                            title=move || shuffle_tip()
                            disabled=move || queue.get().len() <= 1
                            on:click=move |_| set_shuffle.update(|value| *value = !*value)
                        >"⇄"</button>
                        <button
                            title="Previous"
                            disabled=move || queue.get().is_empty() || (!shuffle.get() && current.get() == 0 && repeat_mode.get() != Repeat::All)
                            on:click=move |_| step(-1)
                        >"⏮"</button>
                        <button
                            class="play"
                            title="Play or pause"
                            disabled=move || queue.get().is_empty()
                            on:click=move |_| set_playing.update(|value| *value = !*value)
                        >{move || if playing.get() { "⏸" } else { "▶" }}</button>
                        <button
                            title="Stop"
                            disabled=move || queue.get().is_empty()
                            on:click=move |_| {
                                set_playing.set(false);
                                set_position.set(0.0);
                                if let Some(audio) = audio_ref.get() {
                                    let _ = audio.set_current_time(0.0);
                                }
                            }
                        >"⏹"</button>
                        <button
                            title="Next"
                            disabled=move || queue.get().is_empty() || (current.get() + 1 >= queue.get().len() && repeat_mode.get() != Repeat::All && !shuffle.get())
                            on:click=move |_| step(1)
                        >"⏭"</button>
                        <button
                            class="toggle repeat"
                            class:active=move || repeat_mode.get() != Repeat::Off
                            title=move || repeat_tip()
                            disabled=move || queue.get().is_empty()
                            on:click=move |_| {
                                set_repeat_mode.update(|mode| {
                                    *mode = match mode {
                                        Repeat::Off => Repeat::One,
                                        Repeat::One => Repeat::All,
                                        Repeat::All => Repeat::Off,
                                    };
                                });
                            }
                        >
                            "↻"
                            <Show when=move || !repeat_badge().is_empty() fallback=|| ()>
                                <span class="badge">{move || repeat_badge()}</span>
                            </Show>
                        </button>
                    </div>
                    <div class="seek-row">
                        <span class="time elapsed">{move || clock(position.get())}</span>
                        <input
                            class="seek"
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
                        <span class="time total">{move || clock(duration.get())}</span>
                    </div>
                </div>

                <div class="transport-right">
                    <div class="volume-row">
                        <button
                            class="flat mute"
                            title=move || if volume.get() <= 0.0 { "Unmute" } else { "Mute" }
                            on:click=toggle_mute
                        >{move || if volume.get() <= 0.0 { "♪" } else { "♪" }}</button>
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
                    <div class="transport-status">
                        {move || {
                            let total = queue.get().len();
                            if total == 1 { "1 track".to_owned() } else { format!("{total} tracks") }
                        }}
                    </div>
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
                        if repeat_mode.get() == Repeat::One {
                            if let Some(audio) = audio_ref.get() {
                                let _ = audio.set_current_time(0.0);
                                let _ = audio.play();
                            }
                            set_position.set(0.0);
                            set_playing.set(true);
                        } else if let Some(next) = advance_after_track() {
                            jump(next);
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
                        <button class="primary" on:click=move |_| connect()>
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
