//! Playlist comparisons shared by native and browser frontends.

use std::cmp::Ordering;

/// Compare text case-insensitively, treating runs of ASCII digits as numbers.
/// Leading zeroes do not change the numeric value.
pub fn natural_compare(left: &str, right: &str) -> Ordering {
    let left = left.to_lowercase().chars().collect::<Vec<_>>();
    let right = right.to_lowercase().chars().collect::<Vec<_>>();
    let mut left_index = 0;
    let mut right_index = 0;

    while left_index < left.len() && right_index < right.len() {
        if left[left_index].is_ascii_digit() && right[right_index].is_ascii_digit() {
            let left_end = left[left_index..]
                .iter()
                .position(|character| !character.is_ascii_digit())
                .map_or(left.len(), |offset| left_index + offset);
            let right_end = right[right_index..]
                .iter()
                .position(|character| !character.is_ascii_digit())
                .map_or(right.len(), |offset| right_index + offset);
            let left_significant = left_index
                + left[left_index..left_end]
                    .iter()
                    .take_while(|character| **character == '0')
                    .count();
            let right_significant = right_index
                + right[right_index..right_end]
                    .iter()
                    .take_while(|character| **character == '0')
                    .count();
            let left_digits = &left[left_significant..left_end];
            let right_digits = &right[right_significant..right_end];

            match left_digits.len().cmp(&right_digits.len()) {
                Ordering::Equal => match left_digits.cmp(right_digits) {
                    Ordering::Equal => {}
                    ordering => return ordering,
                },
                ordering => return ordering,
            }
            left_index = left_end;
            right_index = right_end;
            continue;
        }

        match left[left_index].cmp(&right[right_index]) {
            Ordering::Equal => {
                left_index += 1;
                right_index += 1;
            }
            ordering => return ordering,
        }
    }

    (left.len() - left_index).cmp(&(right.len() - right_index))
}

/// A star sort starts with favorites in ascending mode.
pub fn favorites_first(left: bool, right: bool) -> Ordering {
    right.cmp(&left)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbered_titles_sort_naturally() {
        assert_eq!(natural_compare("Track 2", "track 10"), Ordering::Less);
        assert_eq!(natural_compare("SONG 01", "song 1"), Ordering::Equal);
        assert_eq!(natural_compare("Alpha", "beta"), Ordering::Less);
    }

    #[test]
    fn ascending_stars_put_favorites_first() {
        assert_eq!(favorites_first(true, false), Ordering::Less);
        assert_eq!(favorites_first(false, true), Ordering::Greater);
        assert_eq!(favorites_first(false, false), Ordering::Equal);
    }
}
