//! Trend formulas over a user history: peak, former glory and growth.

use jiff::civil;

/// Added to both the recent and the earlier user sums of the growth ratio.
pub(super) const GROWTH_SMOOTHING: f64 = 25.0;
/// History samples the growth ratio needs. With fewer, growth is 1.
pub(super) const GROWTH_SAMPLES: usize = 6;
/// The former-glory smoothing constant for a peak at or above [`GLORY_PEAK_FLOOR`].
pub(super) const GLORY_BASE: f64 = 10.0;
/// The peak below which the former-glory smoothing constant grows by the shortfall.
pub(super) const GLORY_PEAK_FLOOR: f64 = 30.0;

/// The highest mean of two adjacent samples, the only sample, or 0.
pub(super) fn peak(history: &[(civil::Date, f64)]) -> f64 {
    match history {
        [] => 0.0,
        [(_, only)] => *only,
        _ => history
            .windows(2)
            .filter_map(|pair| match pair {
                [(_, a), (_, b)] => Some((a + b) / 2.0),
                _ => None,
            })
            .fold(0.0, f64::max),
    }
}

/// `current` against `peak`, smoothed so that small crates decline slowly, capped at 1.
pub(super) fn former_glory(current: f64, peak: f64) -> f64 {
    let k = GLORY_BASE + (GLORY_PEAK_FLOOR - peak).max(0.0);
    ((current + k + 1.0) / (peak + k)).min(1.0)
}

/// The smoothed ratio of the last two samples to the samples four months before them.
///
/// With no users now, growth is at most 1.
pub(super) fn growth(history: &[(civil::Date, f64)], current: f64) -> f64 {
    if history.len() < GROWTH_SAMPLES {
        return 1.0;
    }
    let back = |steps: usize| {
        history
            .len()
            .checked_sub(steps)
            .and_then(|index| history.get(index))
            .map_or(0.0, |&(_, users)| users)
    };
    let recent = back(1) + back(2);
    let earlier = back(6) + back(5);
    let ratio = (recent + GROWTH_SMOOTHING) / (earlier + GROWTH_SMOOTHING);
    if current <= 0.0 {
        ratio.min(1.0)
    } else {
        ratio
    }
}

#[cfg(test)]
mod tests {
    use jiff::civil::date;

    use super::*;

    #[test]
    fn peak_is_the_best_adjacent_mean() {
        assert_eq!(peak(&[]), 0.0);
        assert_eq!(peak(&[(date(2026, 1, 31), 7.0)]), 7.0);
        let history = [
            (date(2026, 1, 31), 2.0),
            (date(2026, 2, 28), 10.0),
            (date(2026, 3, 31), 4.0),
            (date(2026, 4, 30), 9.0),
        ];
        assert_eq!(peak(&history), 7.0);
    }

    #[test]
    fn former_glory_smooths_small_peaks_and_caps_at_one() {
        assert_eq!(former_glory(0.0, 0.0), 1.0);
        assert_eq!(
            former_glory(10.0, 20.0),
            (10.0 + 20.0 + 1.0) / (20.0 + 20.0)
        );
        assert_eq!(
            former_glory(50.0, 100.0),
            (50.0 + 10.0 + 1.0) / (100.0 + 10.0)
        );
        assert_eq!(former_glory(100.0, 100.0), 1.0);
    }

    #[test]
    fn growth_compares_the_last_two_samples_with_four_months_before() {
        let samples = |values: &[f64]| -> Vec<(civil::Date, f64)> {
            values.iter().map(|&v| (date(2026, 1, 1), v)).collect()
        };
        let six = samples(&[1.0, 3.0, 0.0, 0.0, 20.0, 30.0]);
        assert_eq!(
            growth(&six, 30.0),
            (20.0 + 30.0 + 25.0) / (1.0 + 3.0 + 25.0)
        );
        let seven = samples(&[100.0, 1.0, 3.0, 0.0, 0.0, 20.0, 30.0]);
        assert_eq!(
            growth(&seven, 30.0),
            (20.0 + 30.0 + 25.0) / (1.0 + 3.0 + 25.0)
        );
        assert_eq!(growth(&six, 0.0), 1.0, "no users now caps growth at 1");
        let falling = samples(&[10.0, 10.0, 0.0, 0.0, 0.0, 0.0]);
        assert_eq!(growth(&falling, 0.0), 25.0 / 45.0);
        assert_eq!(growth(&samples(&[0.0, 0.0, 0.0, 0.0, 9.0]), 9.0), 1.0);
    }
}
