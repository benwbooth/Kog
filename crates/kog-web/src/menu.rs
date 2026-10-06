//! Application menu navigation shared by the root and its nested flyouts.
use super::*;

#[derive(Clone, Copy)]
struct State {
    path: RwSignal<Vec<&'static str>>,
    close: Callback<()>,
}

fn focus(element: &web_sys::Element) {
    if let Some(element) = element.dyn_ref::<web_sys::HtmlElement>() {
        let _ = element.focus();
    }
}

fn items(panel: &web_sys::Element) -> Vec<web_sys::Element> {
    let Ok(nodes) = panel.query_selector_all(
        ":scope > .menu-item:not(:disabled), :scope > .menu-branch > .menu-item:not(:disabled)",
    ) else {
        return Vec::new();
    };
    (0..nodes.length())
        .filter_map(|index| nodes.item(index)?.dyn_into::<web_sys::Element>().ok())
        .filter(|element| element.get_bounding_client_rect().height() > 0.0)
        .collect()
}

#[component]
pub fn Menu(on_close: Callback<()>, children: Children) -> impl IntoView {
    let close = Callback::new(move |_| {
        on_close.run(());
        if let Some(button) = document().get_element_by_id("app-menu-button") {
            focus(&button);
        }
    });
    provide_context(State {
        path: RwSignal::new(Vec::new()),
        close,
    });
    view! {
        <div class="menu-scrim" on:click=move |_| close.run(())></div>
        <Panel label="Kog menu" depth=0>{children()}</Panel>
    }
}

#[component]
fn Panel(
    label: &'static str,
    depth: usize,
    #[prop(optional)] anchor: Option<NodeRef<leptos::html::Button>>,
    #[prop(default = true)] autofocus: bool,
    children: Children,
) -> impl IntoView {
    let state = expect_context::<State>();
    let panel = NodeRef::<leptos::html::Div>::new();
    let position = RwSignal::new(String::new());
    let back = move || {
        state
            .path
            .update(|path| path.truncate(depth.saturating_sub(1)));
        if let Some(button) = anchor.and_then(|node| node.get()) {
            let _ = button.focus();
        }
    };
    Effect::new(move |_| {
        let Some(panel) = panel.get() else { return };
        // Measure after mounting; flyouts flip left and move up near viewport edges.
        if let Some(button) = anchor.and_then(|node| node.get()) {
            let anchor = button.get_bounding_client_rect();
            let rect = panel.get_bounding_client_rect();
            let window = window();
            let width = window
                .inner_width()
                .ok()
                .and_then(|v| v.as_f64())
                .unwrap_or(1024.0);
            let height = window
                .inner_height()
                .ok()
                .and_then(|v| v.as_f64())
                .unwrap_or(768.0);
            let left = if anchor.right() + rect.width() + 8.0 > width {
                anchor.left() - rect.width()
            } else {
                anchor.right() + 2.0
            };
            let top = anchor.top().min(height - rect.height() - 8.0).max(8.0);
            position.set(format!("left:{}px;top:{top}px", left.max(8.0)));
        }
        if autofocus {
            if let Some(first) = items(&panel).first() {
                focus(first);
            }
        }
    });
    view! {
        <div node_ref=panel class="context-menu app-menu-panel" class:app-menu=depth == 0
            class:is-ancestor=move || state.path.with(|path| path.len() > depth)
            role="menu" aria-label=label style=move || position.get()
            on:pointermove=move |event: web_sys::PointerEvent| {
                event.stop_propagation();
                if event.pointer_type() != "mouse" { return; }
                let Some(target) = event.target().and_then(|target| target.dyn_into::<web_sys::Element>().ok()) else { return };
                if let Ok(Some(item)) = target.closest(".menu-item") {
                    if !item.has_attribute("aria-haspopup") {
                        state.path.update(|path| path.truncate(depth));
                    }
                }
            }
            on:keydown=move |event: web_sys::KeyboardEvent| {
                event.stop_propagation();
                let Some(panel) = panel.get() else { return };
                let items = items(&panel);
                let active = document().active_element();
                let index = items.iter().position(|item| Some(item) == active.as_ref()).unwrap_or(0);
                match event.key().as_str() {
                    "ArrowDown" | "ArrowUp" | "Home" | "End" if !items.is_empty() => {
                        event.prevent_default();
                        let next = match event.key().as_str() {
                            "Home" => 0, "End" => items.len() - 1,
                            "ArrowUp" => (index + items.len() - 1) % items.len(),
                            _ => (index + 1) % items.len(),
                        };
                        focus(&items[next]);
                    }
                    "ArrowRight" => {
                        event.prevent_default();
                        if let Some(item) = active.filter(|item| item.has_attribute("aria-haspopup")) {
                            if let Some(button) = item.dyn_ref::<web_sys::HtmlElement>() { button.click(); }
                        }
                    }
                    "ArrowLeft" | "Escape" => {
                        event.prevent_default();
                        if depth > 0 { back(); } else { state.close.run(()); }
                    }
                    "Tab" => state.close.run(()),
                    _ => {},
                }
            }>
            {(depth > 0).then(|| view! {
                <button class="menu-item menu-back" role="menuitem" on:click=move |_| back()>
                    <span aria-hidden="true">"‹"</span><span>{label}</span>
                </button>
                <div class="menu-separator menu-back-separator" role="separator"></div>
            })}
            {children()}
        </div>
    }
}

#[component]
pub fn Submenu(label: &'static str, depth: usize, children: ChildrenFn) -> impl IntoView {
    let state = expect_context::<State>();
    let anchor = NodeRef::<leptos::html::Button>::new();
    let autofocus = RwSignal::new(false);
    let expanded = move || state.path.with(|path| path.get(depth - 1) == Some(&label));
    let open = move |keyboard| {
        autofocus.set(keyboard);
        state.path.update(|path| {
            path.truncate(depth - 1);
            path.push(label);
        });
    };
    view! {
        <div class="menu-branch">
            <button node_ref=anchor class="menu-item" class:submenu-open=expanded role="menuitem"
                aria-haspopup="menu" aria-expanded=move || expanded().to_string()
                on:pointerenter=move |event: web_sys::PointerEvent| {
                    let compact = window().match_media("(max-width: 820px), (pointer: coarse) and (max-width: 1200px)")
                        .ok().flatten().is_some_and(|query| query.matches());
                    if event.pointer_type() == "mouse" && !compact && !expanded() { open(false); }
                }
                on:click=move |_| {
                    if expanded() {
                        if let Some(button) = anchor.get() {
                            if let Some(panel) = button.next_element_sibling() {
                                if let Some(first) = items(&panel).first() { focus(first); }
                            }
                        }
                    } else { open(true); }
                }>
                <span>{label}</span><span class="submenu-arrow" aria-hidden="true">"›"</span>
            </button>
            <Show when=expanded>
                {let children = children.clone(); view! {
                    <Panel label=label depth=depth anchor=anchor autofocus=autofocus.get_untracked()>{children()}</Panel>
                }}
            </Show>
        </div>
    }
}
