//! Auto-fit widths for the desktop and web playlists: wide enough for the
//! values most rows hold, without stretching a column for a few outliers.
//! (The terminal player fits every value.) The Qt header carries the same
//! rule in QML, in PlaylistHeader's `typicalWidth`.

/// The share of values that fit whole; the widest rest are cut off.
const TYPICAL: f64 = 0.9;
/// Below this many values there is no telling an outlier from the norm.
const FEWEST: usize = 8;

/// The width that fits 90% of the values whole: the 90th-percentile width.
pub fn width_without_outliers(widths: &[f64]) -> f64 {
    let mut sorted: Vec<f64> = widths.iter().copied().filter(|width| width.is_finite()).collect();
    if sorted.is_empty() {
        return 0.0;
    }
    sorted.sort_by(f64::total_cmp);
    let widest = *sorted.last().unwrap();
    if sorted.len() < FEWEST {
        return widest;
    }
    let index = ((sorted.len() as f64 * TYPICAL).ceil() as usize).clamp(1, sorted.len()) - 1;
    sorted[index]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_long_value_does_not_widen_the_column() {
        let mut widths = vec![100.0; 40];
        widths.push(900.0);
        assert_eq!(width_without_outliers(&widths), 100.0);
    }

    #[test]
    fn the_widest_tenth_is_cut_off() {
        let widths: Vec<f64> = (0..40).map(|index| 80.0 + index as f64 * 2.0).collect();
        assert_eq!(width_without_outliers(&widths), 150.0);
    }

    #[test]
    fn a_few_values_fit_the_widest() {
        assert_eq!(width_without_outliers(&[40.0, 300.0, 60.0]), 300.0);
    }
}
