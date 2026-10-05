//! Browser presentation for the session-owned playlist workspace.
use super::*;
use kog_playback_policy::workspace::{CloseChoice, Command, QueueAction};
#[derive(Clone, Copy)]
pub struct Controller {
    pub backend: session::Controller,
}
impl Controller {
    pub fn snapshot(self) -> kog_playback_policy::workspace::Snapshot {
        self.backend.revision.track();
        self.backend.model.with_value(|model| model.workspace())
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
    view! {
        <Show when=move || { controller.snapshot().tabs.len() > 1 }>
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
pub fn EditMenu(controller: Controller, on_action: Callback<()>) -> impl IntoView {
    use kog_playback_policy::selection::Command as Select;
    let send = move |command| {
        controller.send(command);
        on_action.run(());
    };
    view! {
        <div class="menu-group">"Edit"</div>
        <button class="menu-item" disabled=move || !controller.snapshot().actions.undo on:click=move |_| send(Command::Undo)>
            {move || if controller.snapshot().active == "queue" { "Undo Append" } else { "Undo" }}
        </button>
        <button class="menu-item" disabled=move || !controller.snapshot().actions.redo on:click=move |_| send(Command::Redo)>
            {move || if controller.snapshot().active == "queue" { "Redo Append" } else { "Redo" }}
        </button>
        <div class="menu-separator"></div>
        <button class="menu-item" disabled=move || !controller.snapshot().actions.select_all on:click=move |_| send(Command::Selection {command:Select::All})>"Select All"</button>
        <button class="menu-item" disabled=move || !controller.snapshot().actions.clear_selection on:click=move |_| send(Command::Selection {command:Select::Clear})>"Clear Selection"</button>
        <button class="menu-item" disabled=move || !controller.snapshot().actions.remove on:click=move |_| send(Command::Remove)>"Remove Selected"</button>
        <button class="menu-item" disabled=move || !controller.snapshot().actions.clear on:click=move |_| send(Command::Clear)>
            {move || if controller.snapshot().active == "queue" { "Clear Play Queue" } else { "Clear Playlist" }}
        </button>
        <button class="menu-item" disabled=move || !controller.snapshot().actions.move_up on:click=move |_| send(Command::Nudge {delta:-1})>"Move Up"</button>
        <button class="menu-item" disabled=move || !controller.snapshot().actions.move_down on:click=move |_| send(Command::Nudge {delta:1})>"Move Down"</button>
        <div class="menu-separator"></div>
        <button class="menu-item" disabled=move || !controller.snapshot().actions.save on:click=move |_| send(Command::Save)>"Save Changes"</button>
        <button class="menu-item" disabled=move || !controller.snapshot().actions.reload on:click=move |_| send(Command::Reload)>"Reload Saved Playlist"</button>
        <button class="menu-item" disabled=move || !controller.snapshot().actions.add_play_queue on:click=move |_| {
            controller.backend.send(SessionCommand::AppendQueueToWorkspace {selected_only:false}); on_action.run(());
        }>"Add Play Queue"</button>
        <button class="menu-item" disabled=move || !controller.snapshot().actions.add_queue_selection on:click=move |_| {
            controller.backend.send(SessionCommand::AppendQueueToWorkspace {selected_only:true}); on_action.run(());
        }>"Add Queue Selection"</button>
    }
}

#[component]
pub fn Editor(
    controller: Controller,
    queue: ReadSignal<Vec<Entry>>,
    selected: ReadSignal<HashSet<usize>>,
) -> impl IntoView {
    view! {
        <div class="playlist-editor" on:keydown=move |event:web_sys::KeyboardEvent| {
            if event.ctrl_key() || event.meta_key() {
                let command = match event.key().as_str() { "s" => Some(Command::Save), "z" if event.shift_key()=>Some(Command::Redo), "z"=>Some(Command::Undo), "y"=>Some(Command::Redo), "a"=>Some(Command::Select {indices:(0..controller.snapshot().entries.len()).collect()}), _=>None };
                if let Some(command)=command { event.prevent_default(); event.stop_propagation(); controller.send(command); }
            } else if event.key()=="Delete" { event.prevent_default(); event.stop_propagation(); controller.send(Command::Remove); }
        }>
            <div class="workspace-actions">
                <button disabled=move || !controller.snapshot().actions.queue on:click=move |_| controller.send(Command::Queue { action:QueueAction::PlayNow })>"Play Now"</button>
                <button disabled=move || !controller.snapshot().actions.queue on:click=move |_| controller.send(Command::Queue { action:QueueAction::PlayNext })>"Play Next"</button>
                <button disabled=move || !controller.snapshot().actions.queue on:click=move |_| controller.send(Command::Queue { action:QueueAction::AddToQueue })>"Add to Queue"</button>
                <button disabled=move || !controller.snapshot().actions.save
                    on:click=move |_| controller.send(Command::Save)>"Save"</button>
                <button disabled=move || !controller.snapshot().actions.reload on:click=move |_| controller.send(Command::Reload)>"Reload"</button>
            </div>
            <div class="workspace-actions">
                <button disabled=move || !controller.snapshot().actions.add_play_queue on:click=move |_| controller.backend.send(SessionCommand::AppendQueueToWorkspace {selected_only:false})>"Add Play Queue"</button>
                <button disabled=move || !controller.snapshot().actions.add_queue_selection on:click=move |_| controller.backend.send(SessionCommand::AppendQueueToWorkspace {selected_only:true})>"Add Queue Selection"</button>
                <button disabled=move || !controller.snapshot().actions.remove on:click=move |_| controller.send(Command::Remove)>"Remove"</button>
                <button disabled=move || !controller.snapshot().actions.move_up on:click=move |_| controller.send(Command::Nudge { delta:-1 })>"Move Up"</button>
                <button disabled=move || !controller.snapshot().actions.move_down on:click=move |_| controller.send(Command::Nudge { delta:1 })>"Move Down"</button>
                <button disabled=move || !controller.snapshot().actions.undo on:click=move |_| controller.send(Command::Undo)>"Undo"</button>
                <button disabled=move || !controller.snapshot().actions.redo on:click=move |_| controller.send(Command::Redo)>"Redo"</button>
                <button disabled=move || !controller.snapshot().actions.select_all on:click=move |_| controller.send(Command::Selection { command:kog_playback_policy::selection::Command::All })>"Select All"</button>
                <button disabled=move || !controller.snapshot().actions.clear_selection on:click=move |_| controller.send(Command::Selection { command:kog_playback_policy::selection::Command::Clear })>"Clear Selection"</button>
            </div>
            <p class="workspace-status">{move || {let state=controller.snapshot(); state.error.unwrap_or_else(||format!("{} tracks · {} selected · Play Now uses {}",state.entries.len(),state.selected.len(),match state.selected.len(){0=>"the whole playlist",1=>"the playlist starting at the selected track",_=>"the selected tracks"}))}}</p>
            <div class="workspace-entries" role="listbox" aria-multiselectable="true">
                <For each={move || controller.snapshot().entries.into_iter().enumerate().collect::<Vec<_>>()} key=|(i,entry)|format!("{i}:{entry}") let:row>
                    {let (index,entry)=row; let label=entry["title"].as_str().or_else(||entry["name"].as_str()).filter(|value|!value.is_empty()).map(str::to_owned).unwrap_or_else(||last_segment(entry["entry"].as_str().filter(|value|!value.is_empty()).or_else(||entry["path"].as_str()).unwrap_or_default()));
                     view! { <button role="option" class:selected=move || controller.snapshot().selected.contains(&index)
                       aria-selected=move || controller.snapshot().selected.contains(&index)
                       on:click=move |event:web_sys::MouseEvent| {
                         use kog_playback_policy::selection::{Command as Select, Gesture};
                         let gesture = match (event.shift_key(), event.ctrl_key() || event.meta_key()) {
                             (true,true)=>Gesture::AddRange, (true,false)=>Gesture::Range,
                             (false,true)=>Gesture::Toggle, _=>Gesture::Replace,
                         };
                         controller.send(Command::Selection { command:Select::Choose {index,gesture} });
                       }>{format!("{}. {label}",index+1)}</button> }
                    }
                </For>
            </div>
        </div>
    }
}
