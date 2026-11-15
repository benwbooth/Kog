//! The pinned Home tab: mixes made for the listener by the server (Daily
//! Mixes, Discover Weekly, unplayed albums and more), one row of cards per
//! section. A card opens its mix as a read-only playlist tab; its play
//! button plays the mix now.
use super::*;
use kog_playback_policy::workspace::QueueAction;

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
struct HomeSong {
    #[serde(default)]
    kind: String,
    #[serde(default)]
    path: String,
    #[serde(default)]
    entry: String,
    #[serde(default)]
    fragment: Option<String>,
    #[serde(default)]
    title: String,
    #[serde(default)]
    album: String,
    #[serde(default)]
    artist: String,
}

/// Load a card's cover into `shown`: the server's thumbnail of the song's
/// cover, asked again while the server is still looking it up. A song
/// inside an archive is looked up by its tags.
fn load_cover(base: String, token: String, song: &HomeSong, shown: RwSignal<Option<String>>) {
    if !matches!(song.kind.as_str(), "local" | "archive") || song.path.is_empty() {
        return;
    }
    let mut url = format!(
        "{base}/api/art/thumb?kind={}&path={}&token={}&album={}&size=large",
        url_encode(&song.kind),
        url_encode(&song.path),
        url_encode(&token),
        url_encode(&song.album),
    );
    if song.kind == "archive" {
        url.push_str(&format!("&entry={}&artist={}", url_encode(&song.entry), url_encode(&song.artist)));
    }
    let image_query = format!("?token={}", url_encode(&token));
    leptos::task::spawn_local(async move {
        for _ in 0..40 {
            let Ok(response) = gloo_net::http::Request::get(&url)
                .header("X-Kog-Device", &device_id())
                .send()
                .await
            else {
                return;
            };
            match response.status() {
                202 => {
                    let _ = sleep_ms(1500).await;
                }
                200 => {
                    if let Some(image) = response
                        .json::<serde_json::Value>()
                        .await
                        .ok()
                        .and_then(|body| body["image"].as_str().map(str::to_owned))
                    {
                        let _ = shown.try_set(Some(format!("{base}/api/art/thumbs/{image}{image_query}")));
                    }
                    return;
                }
                _ => return,
            }
        }
    });
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
struct HomeMix {
    id: String,
    #[serde(default)]
    kind: String,
    #[serde(default)]
    title: String,
    #[serde(default)]
    subtitle: String,
    #[serde(default)]
    songs: Vec<HomeSong>,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
struct HomeSection {
    id: String,
    #[serde(default)]
    title: String,
    #[serde(default)]
    mixes: Vec<HomeMix>,
}

#[derive(Clone, Debug, Default, PartialEq)]
struct HomeState {
    generation: u64,
    sections: Vec<HomeSection>,
}

/// How far the server's background reading has got, and whether new mixes
/// are being made. Kept apart from the mixes so it changing redraws no card.
#[derive(Clone, Debug, Default, PartialEq)]
struct Progress {
    songs: u64,
    analysed: u64,
    embedded: u64,
    folders_left: u64,
    refreshing: bool,
}

fn grouped(value: u64) -> String {
    let digits = value.to_string();
    let mut out = String::new();
    for (index, digit) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index) % 3 == 0 {
            out.push(',');
        }
        out.push(digit);
    }
    out
}

impl Progress {
    fn text(&self) -> String {
        let mut text = format!("{} songs read, {} measured", grouped(self.songs), grouped(self.analysed));
        if self.embedded > 0 {
            text.push_str(&format!(", {} heard by the sound model", grouped(self.embedded)));
        }
        text.push_str(if self.folders_left > 0 {
            ". Still reading your library; mixes get better as it goes."
        } else {
            ". Your whole library has been read."
        });
        if self.refreshing {
            text = format!("Making new mixes… {text}");
        }
        text
    }
}

fn greeting() -> &'static str {
    let hour = js_sys::Date::new_0().get_hours();
    match hour {
        0..5 => "Good night",
        5..12 => "Good morning",
        12..18 => "Good afternoon",
        _ => "Good evening",
    }
}

/// A quiet tint picked from a title, for a cover without art.
fn tint(text: &str) -> String {
    let mut hash: i32 = 0;
    for unit in text.encode_utf16() {
        hash = hash.wrapping_mul(31).wrapping_add(i32::from(unit));
    }
    let hue = hash.rem_euclid(360);
    format!("hsl({hue} 30% 26%)")
}

/// Up to four songs from different albums, for a card's cover mosaic.
fn cover_songs(mix: &HomeMix) -> Vec<HomeSong> {
    let mut seen = HashSet::new();
    mix.songs
        .iter()
        .filter(|song| {
            let key = if song.album.is_empty() { song.title.to_lowercase() } else { song.album.to_lowercase() };
            seen.insert(key)
        })
        .take(4)
        .cloned()
        .collect()
}

#[component]
pub fn HomeView(
    controller: workspace::Controller,
    token: ReadSignal<String>,
    #[prop(into)] active: Signal<bool>,
) -> impl IntoView {
    let state = RwSignal::new(HomeState::default());
    let progress = RwSignal::new(Progress::default());
    let ticks = StoredValue::new(0u32);
    let (loaded, set_loaded) = signal(false);
    let refresh = move || {
        let base = controller.backend.base.run(());
        let header = controller.backend.auth.run(());
        if base.is_empty() {
            return;
        }
        leptos::task::spawn_local(async move {
            let Ok(value) = session::request("GET", format!("{base}/api/discover"), header, None).await else {
                return;
            };
            let generation = value["generation"].as_u64().unwrap_or(0);
            let sections: Vec<HomeSection> = value["home"]["sections"]
                .as_array()
                .map(|sections| {
                    sections
                        .iter()
                        .filter_map(|section| serde_json::from_value(section.clone()).ok())
                        .collect()
                })
                .unwrap_or_default();
            let status = &value["status"];
            let next_progress = Progress {
                songs: status["songs"].as_u64().unwrap_or(0),
                analysed: status["analysed"].as_u64().unwrap_or(0),
                embedded: status["embedded"].as_u64().unwrap_or(0),
                folders_left: status["folders_left"].as_u64().unwrap_or(0),
                refreshing: value["refreshing"].as_bool().unwrap_or(false),
            };
            if progress.with_untracked(|current| *current != next_progress) {
                progress.set(next_progress);
            }
            let next = HomeState { generation, sections };
            if state.with_untracked(|current| *current != next) {
                state.set(next);
            }
            set_loaded.set(true);
        });
    };
    // Fetched when the tab comes forward, then now and then while it shows.
    Effect::new(move |_| {
        if active.get() {
            refresh();
        }
    });
    let interval = set_interval_with_handle(
        move || {
            // Every two seconds while new mixes are made, else every thirty.
            let tick = ticks.get_value().wrapping_add(1);
            ticks.set_value(tick);
            if active.get_untracked() && (progress.with_untracked(|p| p.refreshing) || tick % 15 == 0) {
                refresh();
            }
        },
        std::time::Duration::from_secs(2),
    )
    .ok();
    on_cleanup(move || {
        if let Some(interval) = interval {
            interval.clear();
        }
    });
    let remake = move |_| {
        progress.update(|progress| progress.refreshing = true);
        let base = controller.backend.base.run(());
        let header = controller.backend.auth.run(());
        leptos::task::spawn_local(async move {
            let _ = post_json(format!("{base}/api/discover/refresh"), header, serde_json::json!({})).await;
        });
    };
    view! {
        <div class="home-view">
            <div class="home-header">
                <h1>{greeting()}</h1>
                <Show when=move || state.with(|state| !state.sections.is_empty())>
                    <button class="flat home-refresh" class:spinning=move || progress.with(|p| p.refreshing)
                        type="button" title="Make new mixes"
                        aria-label="Make new mixes" on:click=remake
                        inner_html=include_str!("../../../qml/icons/view-refresh.svg")></button>
                </Show>
            </div>
            <Show when=move || state.with(|state| !state.sections.is_empty()) && progress.with(|p| p.songs > 0)>
                <p class="home-progress">{move || progress.with(Progress::text)}</p>
            </Show>
            <Show when=move || loaded.get() && state.with(|state| state.sections.is_empty())>
                <p class="home-empty">
                    {move || {
                        let songs = progress.with(|progress| progress.songs);
                        if songs > 0 {
                            format!("Kog is reading your library in the background ({songs} songs so far). Your mixes appear here as soon as it has enough to go on; listening to music helps it learn what you like.")
                        } else {
                            "Your mixes appear here once Kog has read some of your library. Listening to music helps it learn what you like.".to_owned()
                        }
                    }}
                </p>
            </Show>
            {move || state.with(|state| state.sections.clone()).into_iter().map(|section| view! {
                <section class="home-section">
                    <h2>{section.title.clone()}</h2>
                    <div class="home-row">
                        {section.mixes.into_iter().map(|mix| view! {
                            <MixCard mix=mix controller=controller token=token />
                        }).collect_view()}
                    </div>
                </section>
            }).collect_view()}
        </div>
    }
}

#[component]
fn MixCard(mix: HomeMix, controller: workspace::Controller, token: ReadSignal<String>) -> impl IntoView {
    let covers = cover_songs(&mix);
    let single = mix.kind == "album" || covers.len() < 4;
    let shown: Vec<HomeSong> = if single { covers.into_iter().take(1).collect() } else { covers };
    let open = {
        let id = mix.id.clone();
        let title = mix.title.clone();
        move || {
            if let Some(playlist_id) = kog_playback_policy::workspace::mix_playlist_id(&id) {
                let scope = controller.backend.base.run(());
                controller.send(kog_playback_policy::workspace::Command::Open {
                    key: kog_playback_policy::workspace::mix_tab_key(&id),
                    scope,
                    playlist_id,
                    name: title.clone(),
                    readonly: true,
                });
            }
        }
    };
    let play = {
        let songs = mix.songs.clone();
        move |event: web_sys::MouseEvent| {
            event.stop_propagation();
            let entries: Vec<serde_json::Value> = songs
                .iter()
                .map(|song| {
                    let mut value = serde_json::json!({
                        "kind": song.kind,
                        "path": song.path,
                        "entry": song.entry,
                    });
                    if let Some(fragment) = &song.fragment {
                        value["fragment"] = fragment.clone().into();
                    }
                    value
                })
                .collect();
            controller.backend.send(SessionCommand::Expand {
                scope: controller.backend.base.run(()),
                entries,
                action: QueueAction::PlayNow,
            });
        }
    };
    let background = tint(&mix.title);
    let title = mix.title.clone();
    let subtitle = mix.subtitle.clone();
    let label = format!("Play {}", mix.title);
    let play_title = label.clone();
    let tooltip = subtitle.clone();
    let cover_title = title.clone();
    view! {
        <div class="home-card" role="button" tabindex="0" title=tooltip on:click={
                let open = open.clone();
                move |_| open()
            }
            on:keydown={
                let open = open.clone();
                move |event: web_sys::KeyboardEvent| {
                    if event.key() == "Enter" || event.key() == " " {
                        event.prevent_default();
                        open();
                    }
                }
            }>
            <div class="home-cover" class:single=single style=format!("background:{background}")>
                <span class="home-cover-title">{cover_title}</span>
                {shown.into_iter().map(|song| {
                    let cover = RwSignal::new(None::<String>);
                    load_cover(controller.backend.base.run(()), token.get_untracked(), &song, cover);
                    view! {
                        <img class="home-cover-tile" alt="" loading="lazy" decoding="async"
                            class:ready=move || cover.with(Option::is_some)
                            src=move || cover.get().unwrap_or_default() />
                    }
                }).collect_view()}
                <button class="home-play" type="button" aria-label=label title=play_title
                    on:click=play inner_html=icons::PLAY></button>
            </div>
            <div class="home-card-title">{title}</div>
            <div class="home-card-subtitle">{subtitle}</div>
        </div>
    }
}
