//! Terminal adapter for the portable saved-playlist draft model.
use super::*;
use kog_audio::playback_order::selection::{Command as Select, Gesture};
use kog_audio::playback_order::workspace::{CloseChoice, Command, QueueAction};

pub(super) fn restore() -> Option<serde_json::Value> {
    kog_audio::settings::setting_path("playlist-tabs-tui.json")
        .and_then(|p| std::fs::read(p).ok())
        .and_then(|b| serde_json::from_slice(&b).ok())
}
impl Ui {
    pub(super) fn workspace_command(&mut self, command: Command) {
        self.session_command(SessionCommand::Workspace { command });
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
        self.poll_session_ports();
    }
    fn workspace_cycle(&mut self, delta: isize) {
        let state = self
            .session
            .workspace_model()
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
        self.session_command(SessionCommand::AppendQueueToWorkspace {
            selected_only: !all,
        });
    }
    fn activate_workspace_selected(&mut self) {
        if !self.table_visible_tracks().contains(&self.workspace_cursor) {
            return;
        }
        self.cancel_track_drag();
        self.workspace_select(Select::Choose {
            index: self.workspace_cursor,
            gesture: Gesture::Replace,
        });
        self.workspace_command(Command::Queue {
            action: QueueAction::PlayNow,
        });
    }
    pub(super) fn workspace_key(&mut self, key: Key, size: (usize, usize)) -> bool {
        let state = self
            .session
            .workspace_model()
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
        if self.keyboard_column.is_some() {
            return false;
        }
        let visible = self.table_visible_tracks();
        let last = visible.len().saturating_sub(1);
        if !visible.contains(&self.workspace_cursor) {
            self.workspace_cursor = visible.first().copied().unwrap_or(0);
        }
        let command = match key {
            Key::CtrlS | Key::Char('s') => Some(Command::Save),
            Key::CtrlZ | Key::Char('u') => Some(Command::Undo),
            Key::CtrlY | Key::Char('r') => Some(Command::Redo),
            Key::Char('R') => Some(Command::Reload),
            Key::Delete | Key::Char('d') => Some(Command::Remove),
            Key::Char('K') | Key::AltUp => Some(Command::Nudge { delta: -1 }),
            Key::Char('J') | Key::AltDown => Some(Command::Nudge { delta: 1 }),
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
            Key::Enter => {
                self.activate_workspace_selected();
                return true;
            }
            Key::Char('x') | Key::CtrlSpace => Some(Command::Selection {
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
                let old = visible
                    .iter()
                    .position(|&index| index == self.workspace_cursor)
                    .unwrap_or(0);
                let position = match key {
                    Key::Home => 0,
                    Key::End => last,
                    Key::Up | Key::Char('k') | Key::ShiftUp => old.saturating_sub(1),
                    Key::PageUp => old.saturating_sub(self.layout(size).track_page),
                    Key::PageDown => old.saturating_add(self.layout(size).track_page).min(last),
                    _ => (old + 1).min(last),
                };
                self.workspace_cursor = visible.get(position).copied().unwrap_or(0);
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
            Key::Backspace | Key::CtrlUp | Key::CtrlDown | Key::Char('D') => {
                return true;
            }
            _ => return false,
        };
        if let Some(Command::Selection { command }) = command {
            self.workspace_select(command);
        } else if let Some(command) = command {
            self.workspace_command(command);
        }
        true
    }
    fn workspace_tab_cells(&self, width: usize) -> Vec<(usize, usize, String, String, bool)> {
        let state = self
            .session
            .workspace_model()
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
            let label = format!(
                "[ {}{}{} ]",
                name,
                if tab.dirty { "*" } else { "" },
                if tab.key == "queue" { "" } else { " ×" }
            );
            let len = cell_width(&label).min(width.saturating_sub(x));
            if len == 0 {
                break;
            }
            cells.push((x, len, tab.key.clone(), label, tab.key == state.active));
            x += len;
        }
        cells
    }
    fn workspace_controls(&self) -> Vec<(&'static str, Key, bool)> {
        let state = self.session.workspace_model().snapshot();
        if state.pending_close.is_some() {
            return vec![
                ("Unsaved changes: ", Key::Esc, false),
                ("[s] Save ", Key::Char('s'), true),
                ("[d] Discard ", Key::Char('d'), true),
                ("[c] Cancel ", Key::Char('c'), true),
            ];
        }
        if !self.is_draft() {
            return Vec::new();
        }
        vec![
            ("[p] Play Now ", Key::Char('p'), state.actions.queue),
            ("[n] Play Next ", Key::Char('n'), state.actions.queue),
            ("[a] Add ", Key::Char('a'), state.actions.queue),
            ("[s] Save ", Key::CtrlS, state.actions.save),
            ("[W] Close ", Key::CtrlW, true),
        ]
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
        if self.compact_mode
            || (!layout.show_sidebar && self.focus != Focus::Tracks)
            || x < left
            || y == 0
            || y >= layout.footer_top
        {
            return false;
        }
        // Existing column/scrollbar gestures retain ownership when crossing rows.
        if self.column_drag.is_some()
            || self.column_scroll_drag.is_some()
            || self.vertical_scroll_drag.is_some()
            || self.split_drag
            || self.volume_drag
        {
            return false;
        }
        let state = self.session.workspace_model().snapshot();
        if y == layout.tab_row && state.pending_close.is_none() {
            if button & 3 == 0 && button & (32 | 64 | 128) == 0 {
                if let Some((start, len, key, label, _)) = self
                    .workspace_tab_cells(size.0.saturating_sub(left))
                    .into_iter()
                    .find(|(start, len, ..)| (left + start..left + start + len).contains(&x))
                {
                    if key != "queue"
                        && cell_width(&label) == len
                        && x == left + start + len.saturating_sub(3)
                    {
                        self.workspace_command(Command::Close { key });
                    } else {
                        self.workspace_command(Command::Focus { key });
                    }
                    self.focus = Focus::Tracks;
                }
            }
            return true;
        }
        if y == layout.footer_top - 1 {
            if button & 3 == 0 && button & (32 | 64 | 128) == 0 {
                let mut start = left;
                for (label, key, enabled) in self.workspace_controls() {
                    let end = start + cell_width(label);
                    if enabled && (start..end).contains(&x) {
                        self.focus = Focus::Tracks;
                        self.workspace_key(key, size);
                        break;
                    }
                    start = end;
                }
            }
            return true;
        }
        if state.pending_close.is_some() {
            return true;
        }
        if !self.is_draft()
            || y == layout.track_header
            || self.vertical_scrollbar_at(&layout, size, x, y).is_some()
            || (self.scrollbar(&layout, size).is_some() && y == layout.footer_top - 2)
        {
            return false;
        }
        if !(layout.track_top..layout.track_top + layout.track_page).contains(&y) {
            return true;
        }
        self.focus = Focus::Tracks;
        if button & 0b1100_0000 == 64 {
            if button & 3 >= 2 || button & (4 | 16) != 0 {
                return false;
            }
            let maximum = self
                .table_visible_tracks()
                .len()
                .saturating_sub(layout.track_page);
            self.set_pane_scroll(
                2,
                self.workspace_offset
                    .saturating_add_signed(if button & 1 == 0 { -3 } else { 3 })
                    .min(maximum),
            );
            return true;
        }
        if button & 3 == 0 && button & 32 == 0 {
            let visible = self.table_visible_tracks();
            if let Some(&index) = visible.get(self.workspace_offset + y - layout.track_top) {
                self.workspace_cursor = index;
                let pending_range = self.range_click_pending.take();
                let ranged = button & (4 | 8) != 0 || pending_range == Some(Focus::Tracks);
                let modified = ranged || button & 16 != 0;
                let gesture = match (ranged, button & 16 != 0) {
                    (true, true) => Gesture::AddRange,
                    (true, false) => Gesture::Range,
                    (false, true) => Gesture::Toggle,
                    _ => Gesture::Replace,
                };
                // Keep a selected group together when beginning a drag.
                if modified || !state.actions.append || !state.selected.contains(&index) {
                    self.workspace_select(Select::Choose { index, gesture });
                }
                let now = Instant::now();
                let double = !modified
                    && self.last_click.is_some_and(|(when, pane, row)| {
                        pane == 2
                            && row == index
                            && now.duration_since(when) < Duration::from_millis(450)
                    });
                self.last_click = if double || modified {
                    None
                } else {
                    Some((now, 2, index))
                };
                if double {
                    self.activate_workspace_selected();
                } else {
                    self.track_drag = (!modified && state.actions.append)
                        .then_some(queue_drag::Source::Draft(index));
                    self.track_drop_target = None;
                }
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
        paint(
            screen,
            layout.tab_row + 1,
            left,
            "",
            width,
            Surface::Header,
            false,
        );
        for (x, len, _, label, active) in self.workspace_tab_cells(width) {
            paint(
                screen,
                layout.tab_row + 1,
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
        paint(
            screen,
            layout.footer_top,
            left,
            "",
            width,
            Surface::Header,
            false,
        );
        let controls = self.workspace_controls();
        if controls.is_empty() {
            let info = format!(
                "{} tracks · {} selected",
                self.tracks.len(),
                self.selected_tracks.len()
            );
            paint(
                screen,
                layout.footer_top,
                left,
                &info,
                width,
                Surface::Muted,
                false,
            );
        } else {
            let mut x = 0;
            for (label, _, enabled) in controls {
                let length = cell_width(label).min(width.saturating_sub(x));
                if length == 0 {
                    break;
                }
                paint(
                    screen,
                    layout.footer_top,
                    left + x,
                    label,
                    length,
                    if enabled {
                        Surface::Header
                    } else {
                        Surface::Muted
                    },
                    enabled,
                );
                x += length;
            }
        }
    }
}
