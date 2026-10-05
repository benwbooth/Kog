//! Queue drag preview uses one terminal row for the insertion gap. Both hit
//! testing and painting account for that row, so repeated motion stays put.
use super::*;

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

    pub(super) fn queue_drop_gap_at(
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
        if workspace.active != "queue" || workspace.pending_close.is_some() {
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
            self.offsets[2],
            y - layout.track_top,
            self.track_drop_target,
        )
        .or(self.track_drop_target)
        .map(|gap| gap.min(self.visible_track_count()))
    }

    pub(super) fn finish_track_drag(&mut self, x: usize, y: usize, size: (usize, usize)) {
        // A click alone never reorders. Resolve the release against the preview
        // before clearing it, and reject releases outside the queue viewport.
        let target = self
            .track_drop_target
            .and_then(|_| self.queue_drop_gap_at(x, y, size));
        let source = self.track_drag;
        self.cancel_track_drag();
        let (Some(source), Some(gap)) = (source, target) else {
            return;
        };
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
