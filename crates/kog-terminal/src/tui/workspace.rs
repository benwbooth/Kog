//! Terminal adapter for the portable saved-playlist draft model.
use super::*;
use kog_audio::playback_order::selection::{Command as Select, Gesture};
use kog_audio::playback_order::workspace::{CloseChoice, Command, QueueAction};

/// The most of a playlist's name a tab shows.
const TAB_NAME_CHARS: usize = 19;

/// Cells at the right of the tab row for its ‹ › scroll arrows.
const TAB_ARROWS_WIDTH: usize = 4;

const CLOSE_CHOICES: [(CloseChoice, &str); 3] = [
    (CloseChoice::Save, "[Save]"),
    (CloseChoice::Discard, "[Discard]"),
    (CloseChoice::Cancel, "[Cancel]"),
];

pub(super) struct TabDrag {
    key: String,
    start: (usize, usize),
    point: (usize, usize),
    moved: bool,
    before: Option<Option<String>>,
    marker: Option<usize>,
    scroll_at: Instant,
}

struct CloseDialog {
    x: usize,
    y: usize,
    width: usize,
    height: usize,
}

impl CloseDialog {
    fn new(size: (usize, usize)) -> Self {
        let width = size.0.saturating_sub(4).clamp(20, 64).min(size.0);
        let height = if width >= 31 { 9 } else { 11 };
        Self {
            x: size.0.saturating_sub(width) / 2,
            y: size.1.saturating_sub(height) / 2,
            width,
            height,
        }
    }

    fn button(&self, index: usize) -> (usize, usize, usize) {
        let width = cell_width(CLOSE_CHOICES[index].1);
        if self.width >= 31 {
            let preceding: usize = CLOSE_CHOICES[..index]
                .iter()
                .map(|(_, label)| cell_width(label) + 2)
                .sum();
            (
                self.x + (self.width - 27) / 2 + preceding,
                self.y + 5,
                width,
            )
        } else {
            (self.x + 2, self.y + 4 + index, width)
        }
    }
}

pub(super) fn restore() -> Option<serde_json::Value> {
    kog_audio::settings::setting_path("playlist-tabs-tui.json")
        .and_then(|p| std::fs::read(p).ok())
        .and_then(|b| serde_json::from_slice(&b).ok())
}
impl Ui {
    pub(super) fn menu_item_enabled(&self, page: MenuPage, index: usize) -> bool {
        let state = self.session.workspace();
        let actions = &state.actions;
        match (page, index) {
            (MenuPage::Main, 4) => !self.is_draft() && !self.tracks.is_empty(),
            (MenuPage::Main, 5) => !self.is_draft() && !self.selected_tracks.is_empty(),
            (MenuPage::Edit, 0) => actions.undo,
            (MenuPage::Edit, 1) => actions.redo,
            (MenuPage::Edit, 3) => actions.select_all && self.table_visible_count() > 0,
            (MenuPage::Edit, 4) => actions.clear_selection,
            (MenuPage::Edit, 5) => actions.remove,
            (MenuPage::Edit, 6) => actions.clear,
            (MenuPage::Edit, 7) => actions.move_up,
            (MenuPage::Edit, 8) => actions.move_down,
            (MenuPage::Edit, 10) => actions.save,
            (MenuPage::Edit, 11) => actions.reload,
            (MenuPage::Edit, 12) => actions.add_play_queue,
            (MenuPage::Edit, 13) => actions.add_queue_selection,
            (MenuPage::Edit, 15) => !self.is_draft() && !self.selected_tracks.is_empty(),
            (MenuPage::Draft, 0..=2) => actions.queue,
            (MenuPage::Draft, index) => self.menu_item_enabled(MenuPage::Edit, DRAFT_MENU_EDIT[index]),
            _ => true,
        }
    }
    pub(super) fn activate_draft_menu(&mut self, index: usize) {
        self.focus = Focus::Tracks;
        let action = match index {
            0 => QueueAction::PlayNow,
            1 => QueueAction::PlayNext,
            2 => QueueAction::AddToQueue,
            _ => return self.activate_edit_menu(DRAFT_MENU_EDIT[index]),
        };
        self.workspace_command(Command::Queue { action });
    }
    pub(super) fn activate_edit_menu(&mut self, index: usize) {
        self.focus = Focus::Tracks;
        let command = match index {
            0 => Command::Undo,
            1 => Command::Redo,
            3 | 4 => {
                let command = if index == 3 {
                    Select::All
                } else {
                    Select::Clear
                };
                if self.is_draft() {
                    self.workspace_select(command);
                } else {
                    self.workspace_command(Command::Selection { command });
                }
                return;
            }
            5 => Command::Remove,
            6 => Command::Clear,
            7 => Command::Nudge { delta: -1 },
            8 => Command::Nudge { delta: 1 },
            10 => Command::Save,
            11 => Command::Reload,
            12 | 13 => {
                self.workspace_append_queue(index == 12);
                return;
            }
            15 => {
                self.open_tag_editor();
                return;
            }
            _ => return,
        };
        self.workspace_command(command);
    }
    pub(super) fn workspace_command(&mut self, command: Command) {
        if matches!(
            command,
            Command::Focus { .. }
                | Command::Open { .. }
                | Command::Close { .. }
                | Command::ResolveClose { .. }
        ) {
            self.tab_start = None;
        }
        if matches!(
            command,
            Command::Close { .. } | Command::ResolveClose { .. }
        ) {
            self.workspace_close_selected = 2;
            self.cancel_track_drag();
        }
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
    pub(super) fn poll_tab_drag(&mut self, size: (usize, usize)) {
        if size.0 > 0 && size.1 > 0 {
            if let Some(drag) = self.tab_drag.as_ref().filter(|d| d.moved) {
                let (x, y) = drag.point;
                self.tab_drag_mouse(32, x, y, false, size);
            }
        }
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
        self.workspace_command(Command::Activate {
            index: self.workspace_cursor,
        });
    }
    /// Whether the tab row is on screen.
    pub(super) fn tab_bar_shown(&self, size: (usize, usize)) -> bool {
        let layout = self.layout(size);
        !self.compact_mode && (layout.show_sidebar || self.focus == Focus::Tracks)
    }

    /// Keys for the focused tab row, and ↑ from the first song back to it.
    pub(super) fn tab_bar_key(&mut self, key: Key, size: (usize, usize)) -> bool {
        if !self.tabs_focused {
            let at_top = if self.is_draft() {
                self.table_visible_tracks().first().is_none_or(|first| *first == self.workspace_cursor)
            } else {
                self.visible_tracks().first().is_none_or(|first| *first == self.selected[2])
            };
            if key == Key::Up
                && self.focus == Focus::Tracks
                && self.keyboard_column.is_none()
                && at_top
                && self.tab_bar_shown(size)
            {
                self.tabs_focused = true;
                return true;
            }
            return false;
        }
        if !self.tab_bar_shown(size) || self.session.workspace_model().pending_close().is_some() {
            self.tabs_focused = false;
            return false;
        }
        self.focus = Focus::Tracks;
        match key {
            Key::Left | Key::Char('h') => self.workspace_cycle(-1),
            Key::Right | Key::Char('l') => self.workspace_cycle(1),
            Key::Enter | Key::Down | Key::Char('j') | Key::Esc | Key::Tab => self.tabs_focused = false,
            Key::BackTab => {
                self.tabs_focused = false;
                self.focus = Focus::Library;
            }
            Key::Delete | Key::CtrlW => self.activate_tab_menu(0),
            Key::ShiftLeft => self.activate_tab_menu(3),
            Key::ShiftRight => self.activate_tab_menu(4),
            Key::Char('M') | Key::ContextMenu => {
                let layout = self.layout(size);
                self.open_context(MenuPage::TabBar, layout.playlist_left() + 1, layout.tab_row + 1, size);
            }
            // Space, volume and the other player keys still work here.
            _ => return false,
        }
        true
    }

    /// Run a tab menu item on the active tab.
    pub(super) fn activate_tab_menu(&mut self, index: usize) {
        let state = self.session.workspace_model().snapshot();
        let active = state.tabs.iter().position(|tab| tab.key == state.active).unwrap_or(0);
        match index {
            0 if state.active != "queue" => self.workspace_command(Command::Close { key: state.active.clone() }),
            1 => {
                for tab in &state.tabs {
                    if tab.key != state.active && tab.key != "queue" {
                        self.workspace_command(Command::Close { key: tab.key.clone() });
                    }
                }
            }
            3 if active > 0 => self.workspace_command(Command::MoveTab {
                key: state.active.clone(),
                before: Some(state.tabs[active - 1].key.clone()),
            }),
            4 if active + 1 < state.tabs.len() => self.workspace_command(Command::MoveTab {
                key: state.active.clone(),
                before: state.tabs.get(active + 2).map(|tab| tab.key.clone()),
            }),
            _ => {}
        }
        self.tab_start = None;
    }

    pub(super) fn workspace_key(&mut self, key: Key, size: (usize, usize)) -> bool {
        let state = self
            .session
            .workspace_model()
            .snapshot_for(self.tracks.len(), self.selected_tracks.len());
        if state.pending_close.is_some() {
            let choice = match key {
                Key::Char('s' | 'S') | Key::CtrlS => Some(CloseChoice::Save),
                Key::Char('d' | 'D') => Some(CloseChoice::Discard),
                Key::Esc | Key::Char('c' | 'C') => Some(CloseChoice::Cancel),
                Key::Enter | Key::Char(' ') => Some(CLOSE_CHOICES[self.workspace_close_selected].0),
                Key::Left | Key::Up | Key::BackTab => {
                    self.workspace_close_selected = (self.workspace_close_selected + 2) % 3;
                    None
                }
                Key::Right | Key::Down | Key::Tab => {
                    self.workspace_close_selected = (self.workspace_close_selected + 1) % 3;
                    None
                }
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
            Key::CtrlZ | Key::CtrlY if self.focus == Focus::Tracks => {
                self.workspace_command(if key == Key::CtrlZ {
                    Command::Undo
                } else {
                    Command::Redo
                });
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
    /// The tabs that fit `width` cells, leaving room for the ‹ › scroll
    /// arrows when some tabs are cut off or scrolled out of view.
    fn workspace_tab_cells(&self, width: usize) -> Vec<(usize, usize, String, String, bool)> {
        match self.tab_cells_within(width) {
            (_, Some(_)) => self.tab_cells_within(width.saturating_sub(TAB_ARROWS_WIDTH)).0,
            (cells, None) => cells,
        }
    }

    /// Whether the tab row can scroll left and right, when it overflows.
    fn workspace_tab_arrows(&self, width: usize) -> Option<(bool, bool)> {
        self.tab_cells_within(width).1?;
        self.tab_cells_within(width.saturating_sub(TAB_ARROWS_WIDTH)).1
    }

    /// The tabs laid out in `width`, and, when they do not all fit whole,
    /// whether tabs are hidden to the left and to the right.
    fn tab_cells_within(
        &self,
        width: usize,
    ) -> (Vec<(usize, usize, String, String, bool)>, Option<(bool, bool)>) {
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
        let start = self
            .tab_start
            .unwrap_or_else(|| active.saturating_sub(max_tabs - 1))
            .min(state.tabs.len().saturating_sub(1));
        let total = state.tabs.len();
        for tab in state.tabs.into_iter().skip(start) {
            let name: String = tab.name.chars().take(TAB_NAME_CHARS).collect();
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
        let shown_whole = cells.iter().filter(|cell| cell.1 == cell_width(&cell.3)).count();
        let left = start > 0;
        let right = start + shown_whole < total;
        (cells, (left || right).then_some((left, right)))
    }

    /// Scroll the tab row one tab left or right.
    fn scroll_tabs(&mut self, delta: isize, width: usize) {
        let state = self.session.workspace_model().snapshot();
        let start = self
            .workspace_tab_cells(width)
            .first()
            .and_then(|cell| state.tabs.iter().position(|t| t.key == cell.2))
            .unwrap_or(0);
        self.tab_start = Some(start.saturating_add_signed(delta).min(state.tabs.len().saturating_sub(1)));
    }
    fn workspace_controls(&self) -> Vec<(&'static str, Key, bool)> {
        let state = self.session.workspace_model().snapshot();
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

    pub(super) fn workspace_close_mouse(
        &mut self,
        button: u16,
        x: usize,
        y: usize,
        release: bool,
        size: (usize, usize),
    ) -> bool {
        if self.session.workspace_model().pending_close().is_none() {
            return false;
        }
        // The dialog owns all pointer input, including the sidebar and transport.
        if !release && button & 3 == 0 && button & (32 | 64 | 128) == 0 {
            let dialog = CloseDialog::new(size);
            for (index, &(choice, _)) in CLOSE_CHOICES.iter().enumerate() {
                let (left, row, width) = dialog.button(index);
                if y == row && (left..left + width).contains(&x) {
                    self.workspace_command(Command::ResolveClose { choice });
                    break;
                }
            }
        }
        true
    }

    pub(super) fn draw_workspace_close_dialog(&self, screen: &mut String, size: (usize, usize)) {
        let Some(key) = self.session.workspace_model().pending_close() else {
            return;
        };
        let state = self.session.workspace();
        let name = state
            .tabs
            .iter()
            .find(|tab| tab.key == key)
            .map(|tab| tab.name.as_str())
            .unwrap_or("Playlist");
        let dialog = CloseDialog::new(size);
        screen.push_str("\x1b[?25l");
        for row in 0..dialog.height {
            let border = if row == 0 {
                format!("╭{}╮", "─".repeat(dialog.width.saturating_sub(2)))
            } else if row == dialog.height - 1 {
                format!("╰{}╯", "─".repeat(dialog.width.saturating_sub(2)))
            } else {
                format!("│{}│", " ".repeat(dialog.width.saturating_sub(2)))
            };
            paint(
                screen,
                dialog.y + row + 1,
                dialog.x + 1,
                &border,
                dialog.width,
                Surface::Toolbar,
                false,
            );
        }
        let mut line = |row, text: &str, surface, bold| {
            paint(
                screen,
                dialog.y + row + 1,
                dialog.x + 3,
                text,
                dialog.width.saturating_sub(4),
                surface,
                bold,
            );
        };
        line(1, "Unsaved changes", Surface::Accent, true);
        line(2, name, Surface::Toolbar, true);
        line(
            3,
            if dialog.width >= 32 {
                "Save changes before closing?"
            } else {
                "Save changes?"
            },
            Surface::Toolbar,
            false,
        );
        if dialog.width >= 31 {
            line(7, "Tab/←→ · Enter · Esc cancels", Surface::Muted, false);
        } else {
            line(8, "↑↓ Tab · Enter", Surface::Muted, false);
            line(9, "Esc: cancel", Surface::Muted, false);
        }
        for (index, &(_, label)) in CLOSE_CHOICES.iter().enumerate() {
            let (x, y, width) = dialog.button(index);
            let selected = index == self.workspace_close_selected;
            let surface = if selected {
                Surface::Selected
            } else {
                Surface::Header
            };
            paint(screen, y + 1, x + 1, label, width, surface, selected);
            paint_mnemonic(
                screen,
                y + 1,
                x + 2,
                label.chars().nth(1).unwrap(),
                surface,
                selected,
            );
        }
    }

    pub(super) fn tab_drag_mouse(
        &mut self,
        button: u16,
        x: usize,
        y: usize,
        release: bool,
        size: (usize, usize),
    ) -> bool {
        let layout = self.layout(size);
        let left = layout.playlist_left();
        let state = self.session.workspace_model().snapshot();
        // Moving over the tabs with no button down is a hover, which
        // shows a cut-off tab's full name, not a drag.
        let hover = button & 32 != 0 && button & 3 == 3;
        if self.tab_drag.is_none()
            && (hover
                || self.compact_mode
                || self.modal.is_some()
                || self.menu_open
                || self.prompt.is_some()
                || self.folder_chooser.is_some()
                || self.exit_confirm_open
                || state.pending_close.is_some()
                || self.track_drag.is_some()
                || self.column_drag.is_some()
                || self.column_scroll_drag.is_some()
                || self.vertical_scroll_drag.is_some()
                || self.split_drag
                || self.volume_drag
                || (!layout.show_sidebar && self.focus != Focus::Tracks)
                || y != layout.tab_row
                || x < left)
        {
            return false;
        }
        if self.tab_drag.is_none() {
            let cells = self.workspace_tab_cells(size.0.saturating_sub(left));
            if button & 64 != 0 {
                let start = cells
                    .first()
                    .and_then(|cell| state.tabs.iter().position(|t| t.key == cell.2))
                    .unwrap_or(0);
                self.tab_start = Some(
                    start
                        .saturating_add_signed(if button & 1 == 0 { -1 } else { 1 })
                        .min(state.tabs.len() - 1),
                );
            } else if !release
                && button & (32 | 128 | 3) == 0
                && let Some((can_left, can_right)) = self.workspace_tab_arrows(size.0.saturating_sub(left))
                && x >= size.0.saturating_sub(TAB_ARROWS_WIDTH)
            {
                // The ‹ › arrows at the right end scroll the row one tab.
                let width = size.0.saturating_sub(left);
                if x < size.0.saturating_sub(2) {
                    if can_left {
                        self.scroll_tabs(-1, width);
                    }
                } else if can_right {
                    self.scroll_tabs(1, width);
                }
            } else if !release && button & (32 | 128 | 3) == 0 {
                if let Some((start, len, key, label, _)) = cells
                    .into_iter()
                    .find(|(start, len, ..)| (left + start..left + start + len).contains(&x))
                {
                    if key != "queue"
                        && cell_width(&label) == len
                        && x == left + start + len.saturating_sub(3)
                    {
                        self.workspace_command(Command::Close { key });
                    } else {
                        self.tab_drag = Some(TabDrag {
                            key,
                            start: (x, y),
                            point: (x, y),
                            moved: false,
                            before: None,
                            marker: None,
                            scroll_at: Instant::now(),
                        });
                    }
                }
            }
            return true;
        }
        if !release && button & 32 != 0 && button & 3 == 3 {
            self.tab_drag = None;
            return true;
        }
        let mut drag = self.tab_drag.take().unwrap();
        drag.point = (x, y);
        drag.moved |= x.abs_diff(drag.start.0) + y.abs_diff(drag.start.1) >= 2;
        drag.before = None;
        drag.marker = None;
        if y == layout.tab_row && x >= left && x < size.0 {
            let cells = self.workspace_tab_cells(size.0.saturating_sub(left));
            if drag.moved && drag.scroll_at.elapsed() >= Duration::from_millis(150) {
                let start = cells
                    .first()
                    .and_then(|cell| state.tabs.iter().position(|t| t.key == cell.2))
                    .unwrap_or(0);
                if x <= left + 2 {
                    self.tab_start = Some(start.saturating_sub(1));
                } else if x >= size.0.saturating_sub(3) {
                    self.tab_start = Some((start + 1).min(state.tabs.len() - 1));
                }
                drag.scroll_at = Instant::now();
            }
            let cells = self.workspace_tab_cells(size.0.saturating_sub(left));
            let next = cells
                .iter()
                .filter(|c| c.2 != drag.key)
                .find(|c| x < left + c.0 + c.1 / 2);
            if let Some(cell) = next {
                drag.before = Some(Some(cell.2.clone()));
                drag.marker = Some(left + cell.0);
            } else if let Some(last) = cells.last() {
                let index = state.tabs.iter().position(|t| t.key == last.2).unwrap();
                drag.before = Some(state.tabs.get(index + 1).map(|t| t.key.clone()));
                drag.marker = Some((left + last.0 + last.1).min(size.0 - 1));
            }
        }
        if release {
            if drag.moved {
                if let Some(before) = drag.before {
                    self.workspace_command(Command::MoveTab {
                        key: drag.key,
                        before,
                    });
                }
            } else if y == layout.tab_row && x >= left && x < size.0 {
                self.workspace_command(Command::Focus { key: drag.key });
                self.focus = Focus::Tracks;
            }
        } else {
            self.tab_drag = Some(drag);
        }
        true
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
        if button & 3 == 2 && button & 32 == 0 {
            // Right click: the row joins the selection unless it is in it.
            let visible = self.table_visible_tracks();
            if let Some(&index) = visible.get(self.workspace_offset + y - layout.track_top) {
                self.workspace_cursor = index;
                if !state.selected.contains(&index) {
                    self.workspace_select(Select::Choose { index, gesture: Gesture::Replace });
                }
                self.open_context(MenuPage::Draft, x, y, size);
            }
            return true;
        }
        if button & 3 == 0 && button & 32 == 0 {
            let visible = self.table_visible_tracks();
            if let Some(&index) = visible.get(self.workspace_offset + y - layout.track_top) {
                let relative = x.saturating_sub(left) + self.columns.scroll;
                if button & (4 | 8 | 16) == 0 && self.table_waveform_at(index, relative) {
                    self.show_visualizer();
                    return true;
                }
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
        hover_labels: &mut HoverLabels,
    ) {
        if self.compact_mode || (!layout.show_sidebar && self.focus != Focus::Tracks) {
            return;
        }
        let names: HashMap<String, String> = self
            .session
            .workspace_model()
            .snapshot()
            .tabs
            .into_iter()
            .map(|tab| (tab.key, tab.name))
            .collect();
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
        for (x, len, key, label, active) in self.workspace_tab_cells(width) {
            // A name cut to fit its tab reads in full on hover.
            if let (Some(name), Some(pointer)) = (names.get(&key), hover_labels.pointer)
                && pointer.1 == layout.tab_row
                && (left - 1 + x..left - 1 + x + len).contains(&pointer.0)
                && (name.chars().count() > TAB_NAME_CHARS || len < cell_width(&label))
            {
                hover_labels.label = Some(HoverLabel { text: name.clone() });
            }
            paint(
                screen,
                layout.tab_row + 1,
                left + x,
                &label,
                len,
                if active && self.tabs_focused {
                    Surface::MenuSelected
                } else if active {
                    Surface::Selected
                } else {
                    Surface::Header
                },
                active,
            );
        }
        if let Some((can_left, can_right)) = self.workspace_tab_arrows(width) {
            let arrows_left = left + width.saturating_sub(TAB_ARROWS_WIDTH);
            paint(screen, layout.tab_row + 1, arrows_left, " ", 1, Surface::Header, false);
            paint(screen, layout.tab_row + 1, arrows_left + 1, "‹", 1,
                if can_left { Surface::Header } else { Surface::Muted }, can_left);
            paint(screen, layout.tab_row + 1, arrows_left + 2, " ", 1, Surface::Header, false);
            paint(screen, layout.tab_row + 1, arrows_left + 3, "›", 1,
                if can_right { Surface::Header } else { Surface::Muted }, can_right);
        }
        if let Some(marker) = self
            .tab_drag
            .as_ref()
            .filter(|d| d.moved)
            .and_then(|d| d.marker)
        {
            paint(
                screen,
                layout.tab_row + 1,
                marker + 1,
                "│",
                1,
                Surface::Selected,
                true,
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
