//! Nearest-rank percentiles.

use std::cmp::Ordering;

/// The nearest-rank percentile of `values` at each of `points`.
///
/// For `n` values, the percentile `p` is the value of rank
/// `ceil(p / 100 * n)` in ascending order, counted from 1. A `p` at or below 0
/// reads rank 1 and a `p` above 100 reads rank `n`. The result holds the value
/// of every point in the order of `points`, as `(point, value)`.
///
/// `values` is sorted in place. NaN values do not count: they sort last and
/// leave the ranks alone. A point that is NaN or infinite gives no entry. Empty
/// input, or input of NaN only, gives an empty result.
pub fn percentiles(values: &mut [f64], points: &[f64]) -> Vec<(f64, f64)> {
    values.sort_unstable_by(nan_last);
    let valid = values.partition_point(|value| !value.is_nan());
    let Some(sorted) = values.get(..valid).filter(|sorted| !sorted.is_empty()) else {
        return Vec::new();
    };
    let count = sorted.len();
    points
        .iter()
        .filter(|point| point.is_finite())
        .filter_map(|&point| {
            let value = sorted.get(rank(point, count).saturating_sub(1))?;
            Some((point, *value))
        })
        .collect()
}

/// The 1-based rank of the percentile `point` among `count` values.
///
/// The product comes before the division, so a whole `point` gives the exact
/// quotient whenever `point * count` is a multiple of 100.
fn rank(point: f64, count: usize) -> usize {
    let count = count as f64;
    (point * count / 100.0).ceil().clamp(1.0, count) as usize
}

/// Orders numbers ascending with NaN after every number.
fn nan_last(a: &f64, b: &f64) -> Ordering {
    a.is_nan()
        .cmp(&b.is_nan())
        .then_with(|| a.partial_cmp(b).unwrap_or(Ordering::Equal))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(values: &[f64], point: f64) -> Option<f64> {
        let mut owned = values.to_vec();
        percentiles(&mut owned, &[point])
            .first()
            .map(|&(_, value)| value)
    }

    #[test]
    fn ten_values_give_the_rank_of_each_point() {
        let values: Vec<f64> = (1..=10).map(f64::from).collect();
        assert_eq!(at(&values, 50.0), Some(5.0));
        assert_eq!(at(&values, 70.0), Some(7.0));
        assert_eq!(at(&values, 75.0), Some(8.0));
        assert_eq!(at(&values, 90.0), Some(9.0));
        assert_eq!(at(&values, 99.0), Some(10.0));
        assert_eq!(at(&values, 100.0), Some(10.0));
    }

    #[test]
    fn input_order_does_not_matter() {
        assert_eq!(at(&[9.0, 1.0, 5.0, 3.0, 7.0], 50.0), Some(5.0));
    }

    #[test]
    fn a_single_value_answers_every_point() {
        let mut values = [4.5];
        let found = percentiles(&mut values, &[0.0, 1.0, 50.0, 100.0]);
        assert_eq!(found, [(0.0, 4.5), (1.0, 4.5), (50.0, 4.5), (100.0, 4.5)]);
    }

    #[test]
    fn ties_repeat_the_value() {
        let values = [2.0, 2.0, 2.0, 8.0];
        assert_eq!(at(&values, 50.0), Some(2.0));
        assert_eq!(at(&values, 75.0), Some(2.0));
        assert_eq!(at(&values, 76.0), Some(8.0));
    }

    #[test]
    fn points_keep_their_order_and_repeat() {
        let mut values = [1.0, 2.0, 3.0, 4.0];
        let found = percentiles(&mut values, &[100.0, 25.0, 100.0]);
        assert_eq!(found, [(100.0, 4.0), (25.0, 1.0), (100.0, 4.0)]);
    }

    #[test]
    fn empty_input_gives_an_empty_result() {
        assert!(percentiles(&mut [], &[50.0]).is_empty());
    }

    #[test]
    fn no_points_give_an_empty_result() {
        assert!(percentiles(&mut [1.0], &[]).is_empty());
    }

    #[test]
    fn points_outside_the_range_clamp_to_the_ends() {
        let values = [1.0, 2.0, 3.0];
        assert_eq!(at(&values, -5.0), Some(1.0));
        assert_eq!(at(&values, 0.0), Some(1.0));
        assert_eq!(at(&values, 250.0), Some(3.0));
    }

    #[test]
    fn a_point_that_is_not_finite_gives_no_entry() {
        let mut values = [1.0, 2.0];
        assert!(percentiles(&mut values, &[f64::NAN, f64::INFINITY]).is_empty());
    }

    #[test]
    fn nan_values_do_not_count() {
        let values = [f64::NAN, 3.0, 1.0, -f64::NAN, 2.0];
        assert_eq!(at(&values, 50.0), Some(2.0));
        assert_eq!(at(&values, 100.0), Some(3.0));
        assert!(percentiles(&mut [f64::NAN], &[50.0]).is_empty());
    }
}
