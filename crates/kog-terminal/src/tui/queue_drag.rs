//! Playlist drag preview uses one terminal row for the insertion gap. Both hit
//! testing and painting account for that row, so repeated motion stays put.
use super::*;

#[derive(Clone, Copy)]
pub(super) enum Source {
    Queue(u64),
    // Draft indices remain valid for the gesture: switching tabs, editing the
    // draft, or receiving different entries cancels the drag.
    Draft(usize),
}

pub(super) fn queue_row(offset: usize, row: usize, gap: Option<usize>) -> Option<usize> {
    let position = offset + row;
    match gap.filter(|&gap| gap >= offset) {
        Some(gap) if position == gap => None,
        Some(gap) if position > gap => Some(position - 1),
        _ => Some(position),
    }
}

impl Ui {
    pub(super) fn cancel_track_drag(&mut self) -> bool {
        self.track_drop_target = None;
        self.track_drag.take().is_some()
    }

    pub(super) fn track_drop_gap_at(
        &self,
        x: usize,
        y: usize,
        size: (usize, usize),
    ) -> Option<usize> {
        let layout = self.layout(size);
        if self.compact_mode
            || size.0 < 20
            || size.1 < 14
            || (!layout.show_sidebar && self.focus != Focus::Tracks)
            || self.modal.is_some()
            || self.menu_open
            || self.prompt.is_some()
            || self.exit_confirm_open
            || self.folder_chooser.is_some()
        {
            return None;
        }
        let workspace = self.session.workspace_model().snapshot();
        if workspace.pending_close.is_some() || (self.is_draft() && !workspace.actions.append) {
            return None;
        }
        let right = size
            .0
            .saturating_sub(usize::from(self.track_scrollbar(&layout, size).is_some()));
        if !(layout.playlist_left()..right).contains(&x)
            || !(layout.track_top..layout.track_top + layout.track_page).contains(&y)
        {
            return None;
        }
        queue_row(
            self.table_offset(),
            y - layout.track_top,
            self.track_drop_target,
        )
        .or(self.track_drop_target)
        .map(|gap| gap.min(self.table_visible_count()))
    }

    pub(super) fn finish_track_drag(&mut self, x: usize, y: usize, size: (usize, usize)) {
        // A click alone never reorders. Resolve the release against the preview
        // before clearing it, and reject releases outside the active table.
        let target = self
            .track_drop_target
            .and_then(|_| self.track_drop_gap_at(x, y, size));
        let source = self.track_drag;
        // Delay collapsing a selected group until a plain click is released;
        // a drag instead moves that entire group. Motion clears last_click.
        let clicked = match source {
            Some(Source::Draft(index))
                if self.track_drop_target.is_none()
                    && self
                        .last_click
                        .is_some_and(|(_, pane, row)| pane == 2 && row == index)
                    && self.track_drop_gap_at(x, y, size).and_then(|position| {
                        self.table_visible_tracks().get(position).copied()
                    }) == Some(index) =>
            {
                Some(index)
            }
            _ => None,
        };
        self.cancel_track_drag();
        if let Some(index) = clicked {
            self.workspace_select(kog_audio::playback_order::selection::Command::Choose {
                index,
                gesture: kog_audio::playback_order::selection::Gesture::Replace,
            });
        }
        let (Some(source), Some(gap)) = (source, target) else {
            return;
        };
        if let Source::Draft(from) = source {
            if !self.is_draft() {
                return;
            }
            let selected = self.session.workspace_model().snapshot().selected;
            let Some(position) = selected.iter().position(|&index| index == from) else {
                return;
            };
            let target = self
                .table_visible_tracks()
                .get(gap)
                .copied()
                .unwrap_or(self.workspace_tracks.len());
            let cursor =
                target - selected.iter().filter(|&&index| index < target).count() + position;
            self.workspace_command(kog_audio::playback_order::workspace::Command::Move { target });
            self.workspace_cursor = cursor;
            self.workspace_manual_scroll = None;
            return;
        }
        let Source::Queue(source) = source else {
            return;
        };
        if self.is_draft() {
            return;
        }
        let Some(from) = self
            .session
            .snapshot()
            .row_ids
            .iter()
            .position(|&id| id == source)
        else {
            return;
        };
        let target = self
            .session
            .visible()
            .get(gap)
            .copied()
            .unwrap_or(self.tracks.len());
        if target == from || target == from + 1 {
            return;
        }
        self.session_command(SessionCommand::Move {
            indices: vec![from],
            target,
        });
        if let Some(index) = self
            .session
            .snapshot()
            .row_ids
            .iter()
            .position(|&id| id == source)
        {
            self.selected[2] = index;
        }
        self.manual_scroll_selection[2] = None;
    }
}

#[cfg(test)]
mod tests {
    use super::queue_row;

    #[test]
    fn insertion_row_preserves_rows_on_each_side() {
        assert_eq!(queue_row(0, 0, Some(1)), Some(0));
        assert_eq!(queue_row(0, 1, Some(1)), None);
        assert_eq!(queue_row(0, 2, Some(1)), Some(1));
        assert_eq!(queue_row(0, 3, Some(1)), Some(2));
    }

    #[test]
    fn first_and_last_gaps_have_their_own_row() {
        assert_eq!(queue_row(0, 0, Some(0)), None);
        assert_eq!(queue_row(0, 1, Some(0)), Some(0));
        assert_eq!(queue_row(0, 4, Some(4)), None);
    }

    #[test]
    fn scroll_offset_ignores_an_offscreen_gap() {
        assert_eq!(queue_row(5, 0, Some(3)), Some(5));
        assert_eq!(queue_row(5, 0, Some(7)), Some(5));
        assert_eq!(queue_row(5, 2, Some(7)), None);
        assert_eq!(queue_row(5, 3, Some(7)), Some(7));
        assert_eq!(queue_row(5, 3, None), Some(8));
    }
}
