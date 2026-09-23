use std::collections::HashSet;

/// Select playlist rows by their queue indices, but extend ranges in the
/// current visible order (which may be filtered or sorted).
pub fn click_selection(
    selected: &HashSet<usize>,
    anchor: Option<usize>,
    visible: &[usize],
    clicked: usize,
    shift: bool,
    toggle: bool,
) -> (HashSet<usize>, Option<usize>) {
    if shift {
        if let (Some(start), Some(end)) = (
            anchor.and_then(|index| visible.iter().position(|row| *row == index)),
            visible.iter().position(|row| *row == clicked),
        ) {
            let mut next = if toggle { selected.clone() } else { HashSet::new() };
            for &index in &visible[start.min(end)..=start.max(end)] {
                next.insert(index);
            }
            return (next, anchor);
        }
    }
    if toggle {
        let mut next = selected.clone();
        if !next.insert(clicked) {
            next.remove(&clicked);
        }
        let anchor = (!next.is_empty()).then_some(clicked);
        (next, anchor)
    } else {
        (HashSet::from([clicked]), Some(clicked))
    }
}

#[cfg(test)]
mod tests {
    use super::click_selection;
    use std::collections::HashSet;

    #[test]
    fn shift_extends_visible_range_in_both_directions() {
        let visible = [8, 2, 5, 1];
        let (selected, anchor) = click_selection(&HashSet::new(), None, &visible, 2, false, false);
        let (selected, anchor) = click_selection(&selected, anchor, &visible, 1, true, false);
        assert_eq!(selected, HashSet::from([2, 5, 1]));
        assert_eq!(anchor, Some(2));
        let (selected, anchor) = click_selection(&selected, anchor, &visible, 8, true, false);
        assert_eq!(selected, HashSet::from([8, 2]));
        assert_eq!(anchor, Some(2));
    }

    #[test]
    fn control_shift_adds_a_range_and_control_click_toggles() {
        let visible = [8, 2, 5, 1];
        let (selected, anchor) = click_selection(&HashSet::new(), None, &visible, 8, false, false);
        let (selected, anchor) = click_selection(&selected, anchor, &visible, 1, false, true);
        assert_eq!(selected, HashSet::from([8, 1]));
        let (selected, anchor) = click_selection(&selected, anchor, &visible, 2, true, true);
        assert_eq!(selected, HashSet::from([8, 2, 5, 1]));
        assert_eq!(anchor, Some(1));
        let (selected, anchor) = click_selection(&selected, anchor, &visible, 8, false, true);
        assert_eq!(selected, HashSet::from([2, 5, 1]));
        assert_eq!(anchor, Some(8));
    }

    #[test]
    fn hidden_anchor_falls_back_to_clicked_row() {
        let (selected, anchor) = click_selection(
            &HashSet::from([9]), Some(9), &[2, 5, 1], 5, true, false,
        );
        assert_eq!(selected, HashSet::from([5]));
        assert_eq!(anchor, Some(5));
    }
}
