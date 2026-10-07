//! Musical channel inspector. Cached windows are selected against the browser
//! audio clock; the encoder's current position never drives the highlighted row.
use super::*;
use kog_inspection::{Channel, Snapshot, Window as ChannelWindow, note_name};
use std::cell::Cell;
use std::collections::VecDeque;

#[derive(Deserialize)]
struct Reply {
    status: String,
    window: Option<ChannelWindow>,
    detail: Option<String>,
}

#[component]
pub fn Inspector(
    audio: NodeRef<leptos::html::Audio>,
    open: ReadSignal<bool>,
    stopped: ReadSignal<bool>,
    close: Callback<()>,
    toggle_play: Callback<()>,
    authorization: Signal<Option<String>>,
) -> impl IntoView {
    let state = RwSignal::new(Snapshot::default());
    let message = RwSignal::new(String::new());
    let mode = RwSignal::new("both".to_owned());
    let follow = RwSignal::new(true);
    let tracker = NodeRef::<leptos::html::Div>::new();
    let windows = Rc::new(RefCell::new(VecDeque::<ChannelWindow>::new()));
    let source = Rc::new(RefCell::new(String::new()));
    let pending = Rc::new(Cell::new(false));
    let last_request = Rc::new(Cell::new(0.0));
    let tick = Closure::<dyn FnMut()>::new(move || {
        if !open.get_untracked() {
            return;
        }
        if stopped.get_untracked() {
            state.set(Snapshot::default());
            message.set("Play a track to inspect its channels.".into());
            return;
        }
        let Some(audio) = audio.get_untracked() else {
            state.set(Snapshot::default());
            message.set("Play a track to inspect its channels.".into());
            return;
        };
        let location = audio.current_src();
        if *source.borrow() != location {
            *source.borrow_mut() = location.clone();
            windows.borrow_mut().clear();
            state.set(Snapshot::default());
            last_request.set(0.0);
            pending.set(false);
        }
        let Ok(url) = web_sys::Url::new(&location) else {
            return;
        };
        if !url.pathname().ends_with("/api/stream") {
            message.set("Channel data is unavailable for this audio source.".into());
            return;
        }
        let start = url
            .search_params()
            .get("start_ms")
            .and_then(|s| s.parse::<f64>().ok())
            .unwrap_or(0.0)
            / 1000.0;
        let position = start + audio.current_time();
        let requested = {
            let windows = windows.borrow();
            if let Some(window) = windows
                .iter()
                .find(|w| position >= w.start && position < w.end)
            {
                state.set(window.snapshot(position, !audio.paused(), audio.seeking()));
                message.set(window.description.detail.clone());
                (position > window.end - 0.3 && !windows.iter().any(|w| w.start >= window.end))
                    .then_some(window.end + 0.001)
            } else {
                state.set(Snapshot {
                    position,
                    playing: !audio.paused(),
                    seeking: audio.seeking(),
                    ..Snapshot::default()
                });
                Some(position)
            }
        };
        let Some(requested) = requested else { return };
        if pending.get() || js_sys::Date::now() - last_request.get() < 400.0 {
            return;
        }
        pending.set(true);
        last_request.set(js_sys::Date::now());
        url.set_pathname(&url.pathname().replace("/api/stream", "/api/inspection"));
        url.search_params().set("position", &requested.to_string());
        let endpoint = url.href();
        let windows = windows.clone();
        let source = source.clone();
        let pending = pending.clone();
        let header = authorization.get_untracked();
        leptos::task::spawn_local(async move {
            let mut request = Request::get(&endpoint);
            if let Some(header) = header {
                request = request.header("Authorization", &header);
            }
            let result = async {
                let response = request.send().await.map_err(|e| e.to_string())?;
                if !response.ok() {
                    return Err(format!(
                        "Channel data request failed ({})",
                        response.status()
                    ));
                }
                response.json::<Reply>().await.map_err(|e| e.to_string())
            }
            .await;
            if *source.borrow() != location {
                return;
            }
            pending.set(false);
            match result {
                Ok(reply) if reply.status == "ready" => {
                    if let Some(window) = reply.window {
                        let mut windows = windows.borrow_mut();
                        windows.retain(|old| old.start != window.start);
                        windows.push_back(window);
                        while windows.len() > 3 {
                            windows.pop_front();
                        }
                    }
                }
                Ok(reply) => {
                    if windows.borrow().is_empty() {
                        message.set(
                            reply
                                .detail
                                .unwrap_or_else(|| "Waiting for decoder data…".into()),
                        );
                    }
                }
                Err(error) => {
                    if windows.borrow().is_empty() {
                        message.set(error);
                    }
                }
            }
        });
    });
    let timer = window()
        .set_interval_with_callback_and_timeout_and_arguments_0(tick.as_ref().unchecked_ref(), 33)
        .ok();
    let _tick = StoredValue::new_local(tick);
    on_cleanup(move || {
        if let Some(timer) = timer {
            window().clear_interval_with_handle(timer);
        }
    });
    Effect::new(move |_| {
        let _ = state.get().current_row;
        if follow.get() {
            if let Some(container) = tracker.get() {
                if let Ok(Some(row)) = container.query_selector("tr.current") {
                    if let Some(row) = row.dyn_ref::<web_sys::HtmlElement>() {
                        container.set_scroll_top(
                            (row.offset_top() - container.client_height() / 2).max(0),
                        );
                    }
                }
            }
        }
    });
    view! {
        <Show when=move || open.get()>
            <div class="scrim" on:click=move |_| close.run(())></div>
            <section class="channel-inspector" role="dialog" aria-modal="true" aria-label="Channel Inspector">
                <header><h2>"Channel Inspector"</h2><button type="button" aria-label="Close Channel Inspector" on:click=move |_| close.run(())>"×"</button></header>
                <div class="channel-tools">
                    <button type="button" on:click=move |_| toggle_play.run(())>{move || if state.get().playing { "Pause" } else { "Play" }}</button>
                    <select aria-label="Inspector view" on:change=move |ev| mode.set(event_target_value(&ev))>
                        <option value="both">"Keyboards and tracker"</option><option value="keyboards">"Keyboards"</option><option value="tracker">"Tracker"</option>
                    </select>
                    <label><input type="checkbox" prop:checked=move || follow.get() on:change=move |ev| follow.set(event_target_checked(&ev))/>{"Follow playback"}</label>
                    <span>{move || format!("{} · {:.3} s", state.get().description.backend, state.get().position)}</span>
                </div>
                <p class="channel-description">{move || message.get()}</p>
                <Show when=move || mode.get() != "tracker">
                    <div class="channel-keyboards">
                        <For each=move || { state.get().channels.into_iter().map(|c| c.id).collect::<Vec<_>>() } key=|id| *id children=move |id| { view! { <Keyboard id=id state=state/> } }/>
                    </div>
                </Show>
                <Show when=move || mode.get() != "keyboards">
                    <div class="channel-tracker" node_ref=tracker>
                        <table><thead><tr><th>"Time / row"</th>{move || state.get().channels.into_iter().map(|channel| view!{<th>{channel.name}</th>}).collect_view()}<th>"Song data"</th></tr></thead>
                        <tbody>{move || {
                            let snapshot = state.get();
                            snapshot.rows.iter().enumerate().map(|(index,row)| {
                                let cells = snapshot.channels.iter().map(|channel| {
                                    let cells = row.cells.iter().filter(|cell| cell.channel == channel.id).collect::<Vec<_>>();
                                    let text = cells.iter().map(|cell| format!("{} {} {}", cell.notes,cell.instrument,cell.volume)).collect::<Vec<_>>().join(" · ");
                                    let effects = cells.iter().flat_map(|cell| &cell.effects).map(|f| format!("{} {}",f.name,f.value)).collect::<Vec<_>>().join(" · ");
                                    view! {<td title=effects.clone()><b>{text}</b><small>{effects.clone()}</small></td>}
                                }).collect_view();
                                view!{<tr class:current=snapshot.current_row==Some(index)><th>{row.label.clone()}</th>{cells}<td>{row.global.iter().map(|f| format!("{} {}",f.name,f.value)).collect::<Vec<_>>().join(" · ")}</td></tr>}
                            }).collect_view()
                        }}</tbody></table>
                    </div>
                </Show>
            </section>
        </Show>
    }
}

#[component]
fn Keyboard(id: u32, state: RwSignal<Snapshot>) -> impl IntoView {
    let channel = Memo::new(move |_| {
        state
            .get()
            .channels
            .into_iter()
            .find(|c| c.id == id)
            .unwrap_or_default()
    });
    let canvas = NodeRef::<leptos::html::Canvas>::new();
    Effect::new(move |_| {
        let channel = channel.get();
        let Some(canvas) = canvas.get() else { return };
        let Some(context) = canvas
            .get_context("2d")
            .ok()
            .flatten()
            .and_then(|c| c.dyn_into::<web_sys::CanvasRenderingContext2d>().ok())
        else {
            return;
        };
        draw_keyboard(&context, &channel, 760.0, 64.0);
    });
    view! {
        <div class="channel-voice">
            <div class="channel-identity"><strong>{move || channel.get().name}</strong><span>{move || channel.get().instrument}</span>
                <meter min="0" max="1" prop:value=move || channel.get().level></meter>
                <span>{move || {let c=channel.get(); if c.notes.is_empty() {if c.active {c.kind.to_uppercase()} else {"—".into()}} else {c.notes.iter().map(|n|note_name(n.key)).collect::<Vec<_>>().join(" ")}}}</span>
            </div>
            <div class="channel-keys"><canvas node_ref=canvas width="760" height="64" aria-label=move || format!("{} active piano keys",channel.get().name)></canvas>
                <details><summary>"Controls and effects"</summary><p>{move || channel.get().fields.iter().map(|f|format!("{}: {}",f.name,f.value)).collect::<Vec<_>>().join(" · ")}</p></details>
            </div>
        </div>
    }
}

fn draw_keyboard(
    ctx: &web_sys::CanvasRenderingContext2d,
    channel: &Channel,
    width: f64,
    height: f64,
) {
    let black = |key: usize| matches!(key % 12, 1 | 3 | 6 | 8 | 10);
    let white_width = width / 75.0;
    ctx.clear_rect(0.0, 0.0, width, height);
    for pass in [false, true] {
        let mut white = 0;
        for key in 0..128 {
            let is_black = black(key);
            if is_black == pass {
                let note = channel
                    .notes
                    .iter()
                    .find(|n| n.key.round() as i32 == key as i32);
                let x = if is_black {
                    white as f64 * white_width - white_width * 0.32
                } else {
                    white as f64 * white_width
                };
                let w = if is_black {
                    white_width * 0.64
                } else {
                    white_width - 1.0
                };
                let h = if is_black { height * 0.62 } else { height };
                ctx.set_fill_style_str(match note {
                    Some(n) if n.held => "#4fc3f7",
                    Some(_) => "#70d9aa",
                    None if is_black => "#171b22",
                    None => "#e7eaf0",
                });
                ctx.fill_rect(x, 0.0, w, h);
                if !is_black && key % 12 == 0 {
                    ctx.set_fill_style_str("#303845");
                    ctx.set_font("8px sans-serif");
                    let _ =
                        ctx.fill_text(&format!("C{}", key as i32 / 12 - 1), x + 1.0, height - 3.0);
                }
                if let Some(note) = note {
                    let bend = note.key - note.key.round();
                    if bend.abs() > 0.02 {
                        ctx.set_fill_style_str("#ea6c24");
                        ctx.fill_rect(x + w / 2.0 + f64::from(bend) * w - 1.0, 2.0, 2.0, h - 4.0);
                    }
                }
            }
            if !is_black {
                white += 1;
            }
        }
    }
}
