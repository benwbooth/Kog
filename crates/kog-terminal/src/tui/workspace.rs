//! Terminal adapter for the portable saved-playlist draft model.
use super::*;
use kog_audio::playback_order::selection::{Command as Select, Gesture};
use kog_audio::playback_order::workspace::{CloseChoice, Command, Effect, QueueAction, Workspace};

pub(super) fn restore() -> Workspace {
    kog_audio::settings::setting_path("playlist-tabs-tui.json")
        .and_then(|path| std::fs::read(path).ok())
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .and_then(|value| Workspace::restore(value).ok())
        .unwrap_or_default()
}
impl Ui {
    pub(super) fn workspace_command(&mut self, command: Command) {
        let effect =
            self.workspace
                .apply_ui(command, self.tracks.len(), self.selected_tracks.len());
        if let Some(path) = kog_audio::settings::setting_path("playlist-tabs-tui.json") {
            let result = (|| -> Result<(), String> {
                let parent = path.parent().ok_or("Invalid workspace settings path")?;
                std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
                let mut file =
                    tempfile::NamedTempFile::new_in(parent).map_err(|e| e.to_string())?;
                serde_json::to_writer(&mut file, &self.workspace).map_err(|e| e.to_string())?;
                file.persist(path).map_err(|e| e.to_string())?;
                Ok(())
            })();
            if let Err(error) = result {
                self.status = format!("Could not preserve playlist draft: {error}");
            }
        }
        match effect {
            Ok(Effect::None) => {}
            Ok(Effect::Load {
                key,
                playlist_id,
                generation,
                ..
            }) => {
                let entries = if playlist_id == 0 {
                    self.library.db().starred_entries()
                } else {
                    self.library.db().playlist_entries(playlist_id)
                };
                self.workspace_command(match entries {
                    Ok(entries) => Command::Loaded {
                        key,
                        generation,
                        entries: entries
                            .into_iter()
                            .map(kog_server::api::entry_json)
                            .collect(),
                    },
                    Err(error) => Command::LoadFailed {
                        key,
                        generation,
                        error,
                    },
                });
            }
            Ok(Effect::Save {
                key,
                playlist_id,
                revision,
                entries,
                expected_entries,
                ..
            }) => {
                let result =
                    kog_server::api::stored_entries_from_json(&entries).and_then(|entries| {
                        let expected =
                            kog_server::api::stored_entries_from_json(&expected_entries)?;
                        self.library.db().replace_entries_checked(
                            playlist_id,
                            &entries,
                            Some(&expected),
                        )
                    });
                let command = match result {
                    Ok(()) => {
                        self.reload_lists();
                        self.status = "Playlist saved".into();
                        Command::Saved { key, revision }
                    }
                    Err(error) => Command::SaveFailed {
                        key,
                        revision,
                        error,
                    },
                };
                self.workspace_command(command);
            }
            Ok(Effect::Queue { mode, entries, .. }) => {
                let entries = kog_server::api::queue_entries_from_json(&entries);
                let decoders = self
                    .decoders
                    .background_worker(self.decoder_settings.clone());
                let root = self.library.root();
                let (sender, receiver) = mpsc::channel();
                std::thread::spawn(move || {
                    let mut tracks = Vec::new();
                    for entry in entries {
                        let name = track_from_entry(entry.clone()).name;
                        tracks.extend(
                            expand_stored_entry(&decoders, root.as_deref(), &entry, &name)
                                .into_iter()
                                .map(|(name, entry)| Track { name, entry }),
                        );
                    }
                    let _ = sender.send(tracks);
                });
                self.workspace_jobs.push_back((mode, receiver));
                self.status = "Preparing playlist tracks…".into();
            }
            Err(error) => self.status = error,
        }
    }
    pub(super) fn open_playlist_tab(&mut self, index: usize) {
        let Some((id, name)) = self.lists.get(index).cloned() else {
            return;
        };
        self.workspace_command(Command::Open {
            key: format!("local:{id}"),
            scope: "local".into(),
            playlist_id: id,
            name,
            readonly: id == 0,
        });
        self.workspace_cursor = 0;
        self.workspace_offset = 0;
        self.focus = Focus::Tracks;
        self.keyboard_column = None;
    }
    pub(super) fn poll_workspace(&mut self) {
        while let Some((mode, receiver)) = self.workspace_jobs.front() {
            let tracks = match receiver.try_recv() {
                Ok(tracks) => tracks,
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.workspace_jobs.pop_front();
                    self.status = "Playlist preparation stopped".into();
                    continue;
                }
            };
            let mode = *mode;
            self.workspace_jobs.pop_front();
            let start = self.tracks.len();
            let count = tracks.len();
            self.tracks.extend(tracks);
            self.order_tracks_changed();
            if let Some(PlaybackDecision::Play(index)) =
                self.order.apply_queue_action(mode, start, count)
            {
                self.selected[2] = index;
                self.play_selected();
            }
            self.status = format!("Added {count} tracks to Play Queue");
        }
    }
    fn workspace_cycle(&mut self, delta: isize) {
        let state = self
            .workspace
            .snapshot_for(self.tracks.len(), self.selected_tracks.len());
        let index = state
            .tabs
            .iter()
            .position(|tab| tab.key == state.active)
            .unwrap_or(0);
        let next = (index as isize + delta).rem_euclid(state.tabs.len() as isize) as usize;
        self.workspace_command(Command::Focus {
            key: state.tabs[next].key.clone(),
        });
        self.workspace_cursor = 0;
        self.workspace_offset = 0;
        self.focus = Focus::Tracks;
    }
    fn workspace_append_queue(&mut self, all: bool) {
        let entries = self
            .tracks
            .iter()
            .enumerate()
            .filter(|(i, _)| all || self.selected_tracks.contains(i))
            .map(|(_, track)| kog_server::api::entry_json(track.entry.clone()))
            .collect();
        self.workspace_command(Command::Append { entries });
    }
    pub(super) fn workspace_key(&mut self, key: Key, size: (usize, usize)) -> bool {
        let state = self
            .workspace
            .snapshot_for(self.tracks.len(), self.selected_tracks.len());
        if state.pending_close.is_some() {
            let choice = match key {
                Key::Char('s') | Key::CtrlS => Some(CloseChoice::Save),
                Key::Char('d') => Some(CloseChoice::Discard),
                Key::Esc | Key::Char('c') => Some(CloseChoice::Cancel),
                _ => None,
            };
            if let Some(choice) = choice {
                self.workspace_command(Command::ResolveClose { choice });
            }
            return true;
        }
        match key {
            Key::Alt('[') => {
                self.workspace_cycle(-1);
                return true;
            }
            Key::Alt(']') => {
                self.workspace_cycle(1);
                return true;
            }
            Key::Alt('1') => {
                self.workspace_command(Command::Focus {
                    key: "queue".into(),
                });
                self.focus = Focus::Tracks;
                return true;
            }
            Key::CtrlW if self.focus == Focus::Tracks => {
                self.workspace_command(Command::Close { key: state.active });
                return true;
            }
            _ => {}
        }
        if self.focus != Focus::Tracks || state.active == "queue" {
            return false;
        }
        let last = state.entries.len().saturating_sub(1);
        self.workspace_cursor = self.workspace_cursor.min(last);
        let command = match key {
            Key::CtrlS | Key::Char('s') => Some(Command::Save),
            Key::CtrlZ | Key::Char('u') => Some(Command::Undo),
            Key::CtrlY | Key::Char('r') => Some(Command::Redo),
            Key::Char('R') => Some(Command::Reload),
            Key::Delete | Key::Char('d') => Some(Command::Remove),
            Key::Char('K') => Some(Command::Nudge { delta: -1 }),
            Key::Char('J') => Some(Command::Nudge { delta: 1 }),
            Key::Char('p') => Some(Command::Queue {
                action: QueueAction::PlayNow,
            }),
            Key::Char('n') => Some(Command::Queue {
                action: QueueAction::PlayNext,
            }),
            Key::Char('a') => Some(Command::Queue {
                action: QueueAction::AddToQueue,
            }),
            Key::Char('A') => {
                self.workspace_append_queue(true);
                return true;
            }
            Key::Char('C') => {
                self.workspace_append_queue(false);
                return true;
            }
            Key::CtrlA => Some(Command::Selection {
                command: Select::All,
            }),
            Key::Esc => Some(Command::Selection {
                command: Select::Clear,
            }),
            Key::Char('x') | Key::CtrlSpace | Key::Enter => Some(Command::Selection {
                command: Select::Choose {
                    index: self.workspace_cursor,
                    gesture: Gesture::Toggle,
                },
            }),
            Key::Up
            | Key::Down
            | Key::Char('j')
            | Key::Char('k')
            | Key::PageUp
            | Key::PageDown
            | Key::Home
            | Key::End
            | Key::ShiftUp
            | Key::ShiftDown => {
                let old = self.workspace_cursor;
                self.workspace_cursor = match key {
                    Key::Home => 0,
                    Key::End => last,
                    Key::Up | Key::Char('k') | Key::ShiftUp => old.saturating_sub(1),
                    Key::PageUp => old.saturating_sub(size.1.saturating_sub(9)),
                    Key::PageDown => old.saturating_add(size.1.saturating_sub(9)).min(last),
                    _ => (old + 1).min(last),
                };
                Some(Command::Selection {
                    command: Select::Choose {
                        index: self.workspace_cursor,
                        gesture: if matches!(key, Key::ShiftUp | Key::ShiftDown) {
                            Gesture::Range
                        } else {
                            Gesture::Replace
                        },
                    },
                })
            }
            // Queue-only operations must not silently act on the hidden queue.
            Key::Backspace | Key::CtrlUp | Key::CtrlDown | Key::Char('D') | Key::Char('H') => {
                return true;
            }
            _ => return false,
        };
        if let Some(command) = command {
            self.workspace_command(command);
        }
        true
    }
    fn workspace_tab_cells(&self, width: usize) -> Vec<(usize, usize, String, String, bool)> {
        let state = self
            .workspace
            .snapshot_for(self.tracks.len(), self.selected_tracks.len());
        let mut cells = Vec::new();
        let mut x = 0;
        let active = state
            .tabs
            .iter()
            .position(|tab| tab.key == state.active)
            .unwrap_or(0);
        let max_tabs = (width / 20).max(1);
        let start = active.saturating_sub(max_tabs - 1);
        for tab in state.tabs.into_iter().skip(start) {
            let name: String = tab.name.chars().take(19).collect();
            let label = format!(" {}{} ", name, if tab.dirty { "*" } else { "" });
            let len = cell_width(&label).min(width.saturating_sub(x));
            if len == 0 {
                break;
            }
            cells.push((x, len, tab.key.clone(), label, tab.key == state.active));
            x += len;
        }
        cells
    }
    pub(super) fn workspace_mouse(
        &mut self,
        button: u16,
        x: usize,
        y: usize,
        size: (usize, usize),
    ) -> bool {
        let layout = self.layout(size);
        let left = layout.playlist_left();
        if self.compact_mode || (!layout.show_sidebar && self.focus != Focus::Tracks) {
            return false;
        }
        let state = self
            .workspace
            .snapshot_for(self.tracks.len(), self.selected_tracks.len());
        if state.pending_close.is_some() {
            return true;
        }
        if x < left || y >= layout.footer_top {
            return false;
        }
        if y == layout.footer_top.saturating_sub(1) {
            if button & 3 == 0 && button & 32 == 0 {
                if let Some((_, _, key, _, _)) = self
                    .workspace_tab_cells(size.0.saturating_sub(left))
                    .into_iter()
                    .find(|(start, len, _, _, _)| (left + start..left + start + len).contains(&x))
                {
                    self.workspace_command(Command::Focus { key });
                    self.focus = Focus::Tracks;
                    self.workspace_offset = 0;
                    self.workspace_cursor = 0;
                }
            }
            return true;
        }
        if state.active == "queue" {
            return false;
        }
        if y == 0 {
            return false;
        }
        self.focus = Focus::Tracks;
        if button & 0b1100_0000 == 64 {
            self.workspace_cursor = self
                .workspace_cursor
                .saturating_add_signed(if button & 1 == 0 { -3 } else { 3 })
                .min(state.entries.len().saturating_sub(1));
            return true;
        }
        if button & 3 != 0 || button & 32 != 0 {
            return true;
        }
        if y == 1 {
            let relative = x - left;
            let actions = [
                ("[p] Play Now ", Key::Char('p')),
                ("[n] Play Next ", Key::Char('n')),
                ("[a] Add to Queue ", Key::Char('a')),
                ("[s] Save ", Key::Char('s')),
                ("[W] Close", Key::CtrlW),
            ];
            let mut start = 0;
            for (label, key) in actions {
                let end = start + cell_width(label);
                if (start..end).contains(&relative) {
                    self.workspace_key(key, size);
                    break;
                }
                start = end;
            }
        } else if y >= 4 {
            let index = self.workspace_offset + y - 4;
            if index < state.entries.len() {
                self.workspace_cursor = index;
                let gesture = match (button & 4 != 0, button & 16 != 0) {
                    (true, true) => Gesture::AddRange,
                    (true, false) => Gesture::Range,
                    (false, true) => Gesture::Toggle,
                    _ => Gesture::Replace,
                };
                self.workspace_command(Command::Selection {
                    command: Select::Choose { index, gesture },
                });
            }
        }
        true
    }
    pub(super) fn draw_workspace(
        &mut self,
        screen: &mut String,
        size: (usize, usize),
        layout: &Layout,
    ) {
        if self.compact_mode || (!layout.show_sidebar && self.focus != Focus::Tracks) {
            return;
        }
        let left = layout.playlist_left() + 1;
        let width = size.0.saturating_sub(left - 1);
        let state = self
            .workspace
            .snapshot_for(self.tracks.len(), self.selected_tracks.len());
        if state.active != "queue" {
            let page = layout.footer_top.saturating_sub(5).max(1);
            self.workspace_cursor = self
                .workspace_cursor
                .min(state.entries.len().saturating_sub(1));
            if self.workspace_cursor < self.workspace_offset {
                self.workspace_offset = self.workspace_cursor;
            }
            if self.workspace_cursor >= self.workspace_offset + page {
                self.workspace_offset = self.workspace_cursor + 1 - page;
            }
            for y in 1..layout.footer_top {
                paint(screen, y + 1, left, "", width, Surface::Main, false);
            }
            for (row, actions) in [
                (
                    2,
                    vec![
                        ("[p] Play Now ", state.actions.queue),
                        ("[n] Play Next ", state.actions.queue),
                        ("[a] Add to Queue ", state.actions.queue),
                        ("[s] Save ", state.actions.save),
                        ("[W] Close", true),
                    ],
                ),
                (
                    3,
                    vec![
                        ("x select · ", state.actions.select_all),
                        ("d remove · ", state.actions.remove),
                        ("K up · ", state.actions.move_up),
                        ("J down · ", state.actions.move_down),
                        ("u undo · ", state.actions.undo),
                        ("r redo · ", state.actions.redo),
                        ("A add queue · ", state.actions.add_play_queue),
                        ("C add selection", state.actions.add_queue_selection),
                    ],
                ),
            ] {
                let mut offset = 0;
                for (label, enabled) in actions {
                    let length = cell_width(label).min(width.saturating_sub(offset));
                    paint(
                        screen,
                        row,
                        left + offset,
                        label,
                        length,
                        if enabled {
                            Surface::Header
                        } else {
                            Surface::Muted
                        },
                        enabled && row == 2,
                    );
                    offset += length;
                }
            }
            let info = state.error.clone().unwrap_or_else(|| {
                format!(
                    "{} tracks · {} selected · Alt+[ / Alt+] tabs · Ctrl+W close",
                    state.entries.len(),
                    state.selected.len()
                )
            });
            paint(screen, 4, left, &info, width, Surface::Main, false);
            for (index, entry) in state
                .entries
                .iter()
                .enumerate()
                .skip(self.workspace_offset)
                .take(page)
            {
                let track = kog_server::api::stored_entries_from_json(std::slice::from_ref(entry))
                    .ok()
                    .and_then(|rows| rows.into_iter().next())
                    .map(track_from_entry);
                let label = track
                    .map(|track| {
                        format!(
                            "{} {:>4}  {}",
                            if state.selected.contains(&index) {
                                "*"
                            } else {
                                " "
                            },
                            index + 1,
                            track.name
                        )
                    })
                    .unwrap_or_default();
                let surface = if index == self.workspace_cursor || state.selected.contains(&index) {
                    Surface::Selected
                } else if index % 2 == 1 {
                    Surface::MainAlt
                } else {
                    Surface::Main
                };
                paint(
                    screen,
                    index - self.workspace_offset + 5,
                    left,
                    &label,
                    width,
                    surface,
                    false,
                );
            }
        }
        paint(
            screen,
            layout.footer_top,
            left,
            "",
            width,
            Surface::Header,
            false,
        );
        for (x, len, _, label, active) in self.workspace_tab_cells(width) {
            paint(
                screen,
                layout.footer_top,
                left + x,
                &label,
                len,
                if active {
                    Surface::Selected
                } else {
                    Surface::Header
                },
                active,
            );
        }
        if state.pending_close.is_some() {
            paint(
                screen,
                4,
                left,
                "Unsaved changes: [s] Save  [d] Discard  [c/Esc] Cancel",
                width,
                Surface::Selected,
                true,
            );
        }
    }
}
