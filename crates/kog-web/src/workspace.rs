//! Browser presentation for the session-owned playlist workspace.
use super::*;
use kog_playback_policy::workspace::{CloseChoice, Command, QueueAction};
#[derive(Clone, Copy)]
pub struct Controller {
    pub backend: session::Controller,
    snapshot: Memo<std::sync::Arc<kog_playback_policy::workspace::Snapshot>>,
}
impl Controller {
    pub fn new(backend: session::Controller) -> Self {
        // Every row/menu/tab reads this view. Build it once per revision;
        // cloning the active playlist at each read makes rendering quadratic.
        let snapshot = Memo::new(move |_| {
            backend.revision.track();
            std::sync::Arc::new(backend.model.with_value(|model| model.workspace()))
        });
        Self { backend, snapshot }
    }
    pub fn snapshot(self) -> std::sync::Arc<kog_playback_policy::workspace::Snapshot> {
        self.snapshot.get()
    }
    pub fn open(self, id: i64, name: String) {
        let scope = self.backend.base.run(());
        self.send(Command::Open {
            key: format!("{scope}:{id}"),
            scope,
            playlist_id: id,
            name,
            readonly: id == 0,
        });
    }
    pub fn send(self, command: Command) {
        self.backend.send(SessionCommand::Workspace { command });
    }
}
#[component]
pub fn Tabs(controller: Controller) -> impl IntoView {
    let drag = js_sys::Function::new_with_args("event,move", include_str!("tab-drag.js"));
    let listener = window_event_listener(leptos::ev::pointerdown, move |event| {
        let callback = Closure::<dyn FnMut(String, wasm_bindgen::JsValue)>::new(
            move |key, before: wasm_bindgen::JsValue| {
                controller.send(Command::MoveTab {
                    key,
                    before: before.as_string(),
                });
            },
        )
        .into_js_value();
        let _ = drag.call2(&wasm_bindgen::JsValue::NULL, event.as_ref(), &callback);
    });
    on_cleanup(move || listener.remove());
    view! {
        <Show when=move || { controller.snapshot().tabs.len() > 1 }>
        <div class="playlist-tabs" role="tablist" aria-label="Open playlists">
            <For each=move || controller.snapshot().tabs.clone() key=|tab| (tab.key.clone(), tab.name.clone(), tab.dirty) let:tab>
                { let focus = tab.key.clone(); let active = tab.key.clone(); let close = tab.key.clone(); let closable = tab.key != "queue";
                  view! { <div class="playlist-tab" data-tab-key=tab.key.clone() class:active=move || controller.snapshot().active == active>
                    <button role="tab" aria-selected=move || controller.snapshot().active == focus
                        on:click={let key=tab.key.clone(); move |_| controller.send(Command::Focus { key:key.clone() })}>
                        {format!("{}{}",tab.name,if tab.dirty { " *" } else { "" })}
                    </button>
                    {closable.then(|| view! { <button title="Close playlist tab" on:click=move |_| controller.send(Command::Close { key:close.clone() })>"×"</button> })}
                  </div> }
                }
            </For>
        </div>
        </Show>
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
pub fn PlaybackMenu(controller: Controller, on_action: Callback<()>) -> impl IntoView {
    let send = move |action| {
        controller.send(Command::Queue { action });
        on_action.run(());
    };
    view! {
        <Show when=move || controller.snapshot().active != "queue">
            <button class="menu-item" role="menuitem" disabled=move || !controller.snapshot().actions.queue on:click=move |_| send(QueueAction::PlayNow)>"Play Now"</button>
            <button class="menu-item" role="menuitem" disabled=move || !controller.snapshot().actions.queue on:click=move |_| send(QueueAction::PlayNext)>"Play Next"</button>
            <button class="menu-item" role="menuitem" disabled=move || !controller.snapshot().actions.queue on:click=move |_| send(QueueAction::AddToQueue)>"Add to Queue"</button>
            <div class="menu-separator" role="separator"></div>
        </Show>
    }
}
#[component]
pub fn EditMenu(
    controller: Controller,
    on_action: Callback<()>,
    on_select_all: Callback<()>,
) -> impl IntoView {
    use kog_playback_policy::selection::Command as Select;
    let send = move |command| {
        controller.send(command);
        on_action.run(());
    };
    view! {
        <button class="menu-item" role="menuitem" disabled=move || !controller.snapshot().actions.undo on:click=move |_| send(Command::Undo)>
            "Undo"
        </button>
        <button class="menu-item" role="menuitem" disabled=move || !controller.snapshot().actions.redo on:click=move |_| send(Command::Redo)>
            "Redo"
        </button>
        <div class="menu-separator" role="separator"></div>
        <button class="menu-item" role="menuitem" disabled=move || !controller.snapshot().actions.select_all on:click=move |_| { on_select_all.run(()); on_action.run(()); }>"Select All"</button>
        <button class="menu-item" role="menuitem" disabled=move || !controller.snapshot().actions.clear_selection on:click=move |_| send(Command::Selection {command:Select::Clear})>"Clear Selection"</button>
        <button class="menu-item" role="menuitem" disabled=move || !controller.snapshot().actions.remove on:click=move |_| send(Command::Remove)>"Remove Selected"</button>
        <button class="menu-item" role="menuitem" disabled=move || !controller.snapshot().actions.clear on:click=move |_| send(Command::Clear)>
            {move || if controller.snapshot().active == "queue" { "Clear Play Queue" } else { "Clear Playlist" }}
        </button>
        <button class="menu-item" role="menuitem" disabled=move || !controller.snapshot().actions.move_up on:click=move |_| send(Command::Nudge {delta:-1})>"Move Up"</button>
        <button class="menu-item" role="menuitem" disabled=move || !controller.snapshot().actions.move_down on:click=move |_| send(Command::Nudge {delta:1})>"Move Down"</button>
        <div class="menu-separator" role="separator"></div>
        <button class="menu-item" role="menuitem" disabled=move || !controller.snapshot().actions.save on:click=move |_| send(Command::Save)>"Save Changes"</button>
        <button class="menu-item" role="menuitem" disabled=move || !controller.snapshot().actions.reload on:click=move |_| send(Command::Reload)>"Reload Saved Playlist"</button>
        <button class="menu-item" role="menuitem" disabled=move || !controller.snapshot().actions.add_play_queue on:click=move |_| {
            controller.backend.send(SessionCommand::AppendQueueToWorkspace {selected_only:false}); on_action.run(());
        }>"Add Play Queue"</button>
        <button class="menu-item" role="menuitem" disabled=move || !controller.snapshot().actions.add_queue_selection on:click=move |_| {
            controller.backend.send(SessionCommand::AppendQueueToWorkspace {selected_only:true}); on_action.run(());
        }>"Add Queue Selection"</button>
    }
}
