//! Shared playlist table projection. The playing queue and saved drafts keep
//! separate selection and scroll state while using the same columns and cells.
use super::*;

impl Ui {
    pub(super) fn is_draft(&self) -> bool {
        self.workspace_active != "queue"
    }

    pub(super) fn sync_workspace_view(&mut self) {
        let state = self.session.workspace_model().snapshot();
        let switched = self.workspace_active != state.active;
        if switched {
            self.cancel_track_drag();
            self.column_drag = None;
            self.column_scroll_drag = None;
            self.vertical_scroll_drag = None;
            self.keyboard_column = None;
            self.workspace_cursor = 0;
            self.workspace_offset = 0;
            self.workspace_manual_scroll = None;
            self.workspace_query.clear();
            self.workspace_active = state.active;
        }
        if switched || self.workspace_entries != state.entries {
            if self.is_draft() {
                self.cancel_track_drag();
            }
            self.last_click = None;
            self.workspace_tracks = match state.entries.iter().map(session::decode_track).collect()
            {
                Ok(tracks) => tracks,
                Err(error) => {
                    self.status = error;
                    Vec::new()
                }
            };
            self.workspace_entries = state.entries;
            self.workspace_cursor = self
                .workspace_cursor
                .min(self.workspace_tracks.len().saturating_sub(1));
            self.workspace_anchor = None;
            self.auto_fit_cache = None;
        }
        if let Some(error) = state.error {
            self.status = error;
        }
    }

    pub(super) fn table_tracks(&self) -> &[Track] {
        if self.is_draft() {
            &self.workspace_tracks
        } else {
            &self.tracks
        }
    }

    pub(super) fn table_visible_tracks(&self) -> Vec<usize> {
        if !self.is_draft() {
            return self.visible_tracks();
        }
        if self.workspace_query.trim().is_empty() {
            return (0..self.workspace_tracks.len()).collect();
        }
        self.workspace_tracks
            .iter()
            .enumerate()
            .filter_map(|(index, track)| {
                let meta = self.metadata_for(track);
                kog_audio::playback_order::sort::matches_query(
                    &[
                        &self.title_for(track),
                        &track.name,
                        &display_entry_path(&track.entry),
                        meta.map_or("", |m| &m.artist),
                        meta.map_or("", |m| &m.album),
                    ],
                    &self.workspace_query,
                )
                .then_some(index)
            })
            .collect()
    }

    pub(super) fn table_visible_count(&self) -> usize {
        if !self.is_draft() {
            self.session.visible().len()
        } else if self.workspace_query.trim().is_empty() {
            self.workspace_tracks.len()
        } else {
            self.table_visible_tracks().len()
        }
    }

    pub(super) fn table_offset(&self) -> usize {
        if self.is_draft() {
            self.workspace_offset
        } else {
            self.offsets[2]
        }
    }

    pub(super) fn table_cursor(&self) -> usize {
        if self.is_draft() {
            self.workspace_cursor
        } else {
            self.selected[2]
        }
    }

    pub(super) fn table_query(&self) -> &str {
        if self.is_draft() {
            &self.workspace_query
        } else {
            &self.playlist_query
        }
    }

    pub(super) fn filter_table(&mut self, query: String) {
        if self.is_draft() {
            self.workspace_query = query;
            self.workspace_offset = 0;
            self.workspace_manual_scroll = None;
            self.workspace_cursor = self.table_visible_tracks().first().copied().unwrap_or(0);
        } else {
            self.session_command(SessionCommand::Filter { query });
            self.offsets[2] = 0;
            if let Some(first) = self.visible_tracks().first() {
                self.selected[2] = *first;
            }
        }
    }

    pub(super) fn table_column_value(&self, index: usize, track: &Track, column: &str) -> String {
        // Playback state belongs to queue row identities, never draft indices.
        if self.is_draft() && column == "status" {
            String::new()
        } else {
            self.column_value(index, track, column)
        }
    }

    pub(super) fn set_pane_scroll(&mut self, pane: usize, offset: usize) {
        if pane == 2 && self.is_draft() {
            self.workspace_offset = offset;
            self.workspace_manual_scroll = Some(self.workspace_cursor);
        } else {
            self.offsets[pane] = offset;
            self.manual_scroll_selection[pane] = Some(self.selected[pane]);
        }
    }

    pub(super) fn keep_draft_cursor_visible(&mut self, visible: &[usize], page: usize) {
        let page = page.max(1);
        self.workspace_offset = self
            .workspace_offset
            .min(visible.len().saturating_sub(page));
        if self.workspace_manual_scroll == Some(self.workspace_cursor) {
            return;
        }
        self.workspace_manual_scroll = None;
        let position = visible
            .iter()
            .position(|&i| i == self.workspace_cursor)
            .unwrap_or(0);
        if position < self.workspace_offset {
            self.workspace_offset = position;
        } else if position >= self.workspace_offset + page {
            self.workspace_offset = position + 1 - page;
        }
    }

    pub(super) fn workspace_select(
        &mut self,
        command: kog_audio::playback_order::selection::Command,
    ) {
        let state = self.session.workspace_model().snapshot();
        let visible = self.table_visible_tracks();
        let mut selection = kog_audio::playback_order::selection::Selection {
            indices: state.selected,
            anchor: self.workspace_anchor,
        };
        if visible.is_empty() {
            selection = Default::default();
        } else {
            selection.apply(command, self.workspace_tracks.len(), &visible);
        }
        self.workspace_anchor = selection.anchor;
        self.workspace_manual_scroll = None;
        self.workspace_command(kog_audio::playback_order::workspace::Command::Selection {
            command: kog_audio::playback_order::selection::Command::Set {
                indices: selection.indices,
                anchor: selection.anchor,
            },
        });
    }
}
