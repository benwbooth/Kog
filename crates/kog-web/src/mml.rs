//! Kog MML view: the whole song as MML by bars, following the audio clock.
//! The score is built once per revision; highlighting toggles classes on the
//! already-rendered tokens so a long song does not re-render every frame.
use super::*;
use kog_inspection::mml::Document;
use std::cell::Cell;

#[derive(Deserialize)]
struct Reply {
    status: String,
    revision: u64,
    recorded_ms: u64,
    total_ms: u64,
    detail: Option<String>,
    document: Option<Document>,
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

/// Text in `from..to` as colour runs; sounding tokens get an index so their
/// highlight can be toggled without re-rendering.
fn styled(document: &Document, from: usize, to: usize, html: &mut String) {
    let text = &document.text;
    let first = document.spans.partition_point(|span| span.from < from);
    let mut sounds = document.spans[first..]
        .iter()
        .enumerate()
        .map(|(offset, span)| (first + offset, span))
        .take_while(|(_, span)| span.from < to)
        .filter(|(_, span)| span.sound.is_some())
        .peekable();
    let mut cursor = from;
    let mut open: Option<usize> = None;
    for &(start, end, class) in document.styles_in(from, to) {
        let (start, end) = ((start as usize).max(from), (end as usize).min(to));
        if open.is_some_and(|close| start >= close) {
            html.push_str("</span>");
            open = None;
        }
        if let Some((index, span)) = sounds.next_if(|(_, span)| span.from <= start) {
            html.push_str(&escape(&text[cursor..span.from]));
            cursor = span.from;
            html.push_str(&format!("<span class=\"mml-sound\" data-i=\"{index}\">"));
            open = Some(span.to);
        }
        html.push_str(&escape(&text[cursor..start]));
        html.push_str(&format!("<span class=\"k{class}\">{}</span>", escape(&text[start..end])));
        cursor = end;
    }
    if open.is_some() {
        html.push_str("</span>");
    }
    html.push_str(&escape(&text[cursor..to]));
}

/// Bars become blocks; the palette becomes one class per token kind.
fn render(document: &Document) -> String {
    let mut html = String::from("<style>");
    for (class, colour) in document.palette.iter().enumerate() {
        html.push_str(&format!(".channel-mml .k{class}{{color:{colour}}}"));
    }
    html.push_str("</style>");
    let header_end = document.bars.first().map_or(document.text.len(), |bar| bar.from);
    // Tracks, pitch tables and macros can run to hundreds of lines; keep them
    // folded so the bars start at the top.
    let lines = document.text[..header_end].lines().count();
    html.push_str(&format!("<details><summary>Header · {lines} lines</summary><pre class=\"mml-header\">"));
    styled(document, 0, header_end, &mut html);
    html.push_str("</pre></details>");
    for bar in &document.bars {
        html.push_str(&format!("<pre class=\"mml-bar\" data-bar=\"{}\">", bar.index));
        styled(document, bar.from, bar.to, &mut html);
        html.push_str("</pre>");
    }
    html
}

#[component]
pub fn MmlView(
    audio: NodeRef<leptos::html::Audio>,
    follow: ReadSignal<bool>,
    authorization: Signal<Option<String>>,
) -> impl IntoView {
    let container = NodeRef::<leptos::html::Div>::new();
    let message = RwSignal::new("Recording every channel of this song…".to_owned());
    let document = Rc::new(RefCell::new(None::<Document>));
    let revision = Rc::new(Cell::new(None::<u64>));
    let source = Rc::new(RefCell::new(String::new()));
    let pending = Rc::new(Cell::new(false));
    let done = Rc::new(Cell::new(false));
    let last_request = Rc::new(Cell::new(0.0));
    // Bars on each track's line, remembered in this browser.
    let storage = window().local_storage().ok().flatten();
    let initial_bars = storage
        .as_ref()
        .and_then(|storage| storage.get_item("kog.mml.bars").ok().flatten())
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(4)
        .clamp(1, 16);
    let bars = RwSignal::new(initial_bars);
    let rewrap = RwSignal::new(false);
    let lit = Rc::new(RefCell::new(Vec::<usize>::new()));
    let current_bar = Rc::new(Cell::new(None::<usize>));
    let tick = Closure::<dyn FnMut()>::new(move || {
        let (Some(audio), Some(container)) = (audio.get_untracked(), container.get_untracked())
        else {
            return;
        };
        let location = audio.current_src();
        if rewrap.get_untracked() {
            rewrap.set(false);
            // A new width needs the whole text again, without a full re-record.
            revision.set(None);
            done.set(false);
            last_request.set(0.0);
        }
        if *source.borrow() != location {
            *source.borrow_mut() = location.clone();
            *document.borrow_mut() = None;
            revision.set(None);
            done.set(false);
            lit.borrow_mut().clear();
            current_bar.set(None);
            container.set_inner_html("");
            message.set("Recording every channel of this song…".into());
        }
        let Ok(url) = web_sys::Url::new(&location) else { return };
        if !url.pathname().ends_with("/api/stream") {
            message.set("An MML score is unavailable for this audio source.".into());
            return;
        }
        let start = url
            .search_params()
            .get("start_ms")
            .and_then(|s| s.parse::<f64>().ok())
            .unwrap_or(0.0)
            / 1000.0;
        let position = start + audio.current_time();
        if let Some(document) = document.borrow().as_ref() {
            let (indices, bar) = document.active_indices(position);
            let mut lit = lit.borrow_mut();
            let toggle = |index: usize, on: bool| {
                if let Ok(Some(element)) =
                    container.query_selector(&format!("[data-i=\"{index}\"]"))
                {
                    let _ = element.class_list().toggle_with_force("sounding", on);
                }
            };
            for index in lit.iter().filter(|i| !indices.contains(i)) {
                toggle(*index, false);
            }
            for index in indices.iter().filter(|i| !lit.contains(i)) {
                toggle(*index, true);
            }
            *lit = indices;
            let bar = bar.map(|bar| bar.index);
            if bar != current_bar.get() {
                let select = |index: usize| {
                    container
                        .query_selector(&format!("[data-bar=\"{index}\"]"))
                        .ok()
                        .flatten()
                };
                if let Some(element) = current_bar.get().and_then(select) {
                    let _ = element.class_list().remove_1("current");
                }
                if let Some(element) = bar.and_then(select) {
                    let _ = element.class_list().add_1("current");
                    if follow.get_untracked() {
                        // Measure against the scroller itself: it is not the
                        // bar's offset parent inside the positioned dialog.
                        let offset = element.get_bounding_client_rect().top()
                            - container.get_bounding_client_rect().top();
                        let top = f64::from(container.scroll_top()) + offset - 8.0;
                        container.set_scroll_top(top.max(0.0) as i32);
                    }
                }
                current_bar.set(bar);
            }
        }
        if done.get() || pending.get() || js_sys::Date::now() - last_request.get() < 1000.0 {
            return;
        }
        pending.set(true);
        last_request.set(js_sys::Date::now());
        url.set_pathname(&url.pathname().replace("/api/stream", "/api/mml"));
        url.search_params().delete("start_ms");
        url.search_params().set("bars", &bars.get_untracked().to_string());
        if let Some(have) = revision.get() {
            url.search_params().set("have", &have.to_string());
        }
        let endpoint = url.href();
        let header = authorization.get_untracked();
        let (document, revision, source, pending, done, lit, current_bar) = (
            document.clone(),
            revision.clone(),
            source.clone(),
            pending.clone(),
            done.clone(),
            lit.clone(),
            current_bar.clone(),
        );
        let container = container.clone();
        leptos::task::spawn_local(async move {
            let mut request = Request::get(&endpoint);
            if let Some(header) = header {
                request = request.header("Authorization", &header);
            }
            let result = async {
                let response = request.send().await.map_err(|e| e.to_string())?;
                if !response.ok() {
                    return Err(format!("MML request failed ({})", response.status()));
                }
                response.json::<Reply>().await.map_err(|e| e.to_string())
            }
            .await;
            pending.set(false);
            if *source.borrow() != location {
                return;
            }
            match result {
                Ok(reply) => {
                    if let Some(new) = reply.document {
                        // Keep the reader's place while a longer score replaces
                        // the partial one; replacing the HTML resets scrolling.
                        let scroll = container.scroll_top();
                        container.set_inner_html(&render(&new));
                        container.set_scroll_top(scroll);
                        lit.borrow_mut().clear();
                        current_bar.set(None);
                        *document.borrow_mut() = Some(new);
                        revision.set(Some(reply.revision));
                    }
                    let time = |ms: u64| format!("{}:{:02}", ms / 60_000, ms / 1000 % 60);
                    match reply.status.as_str() {
                        "ready" => {
                            done.set(true);
                            message.set(String::new());
                        }
                        "error" => {
                            done.set(true);
                            message.set(reply.detail.unwrap_or_else(|| "The score could not be recorded.".into()));
                        }
                        _ => message.set(format!(
                            "Still recording… {} of {}",
                            time(reply.recorded_ms),
                            time(reply.total_ms)
                        )),
                    }
                }
                Err(error) => message.set(error),
            }
        });
    });
    let timer = window()
        .set_interval_with_callback_and_timeout_and_arguments_0(tick.as_ref().unchecked_ref(), 50)
        .ok();
    let _tick = StoredValue::new_local(tick);
    on_cleanup(move || {
        if let Some(timer) = timer {
            window().clear_interval_with_handle(timer);
        }
    });
    view! {
        <div class="mml-tools">
            <label>"Bars per line "
                <select aria-label="Bars per line" on:change=move |ev| {
                    let value = event_target_value(&ev).parse::<usize>().unwrap_or(4);
                    bars.set(value);
                    if let Some(storage) = window().local_storage().ok().flatten() {
                        let _ = storage.set_item("kog.mml.bars", &value.to_string());
                    }
                    rewrap.set(true);
                }>
                    {[1usize, 2, 3, 4, 6, 8, 12, 16].into_iter().map(|n| view! {
                        <option value=n.to_string() selected=move || bars.get() == n>{n}</option>
                    }).collect_view()}
                </select>
            </label>
            <p class="mml-status">{move || message.get()}</p>
        </div>
        <div class="channel-mml" node_ref=container aria-label="Kog MML score"></div>
    }
}
