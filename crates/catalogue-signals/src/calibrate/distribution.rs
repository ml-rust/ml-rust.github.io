//! The percentiles of one named signal.

use super::percentile::percentiles;

/// The points every report prints.
pub const STANDARD_POINTS: [f64; 5] = [50.0, 75.0, 90.0, 95.0, 99.0];

/// The percentiles of one signal over a population.
#[derive(Debug, Clone, PartialEq)]
pub struct Distribution {
    /// The signal name.
    pub name: &'static str,
    /// The samples behind the percentiles. NaN values do not count.
    pub count: usize,
    /// The nearest-rank percentile at each requested point, as `(point, value)`.
    /// Empty when `count` is 0.
    pub points: Vec<(f64, f64)>,
}

impl Distribution {
    /// Builds the distribution of `samples` at `points`.
    ///
    /// NaN samples are dropped before counting. See [`percentiles`] for the
    /// rank definition.
    pub fn from_samples(name: &'static str, samples: &[f64], points: &[f64]) -> Self {
        let mut values: Vec<f64> = samples
            .iter()
            .copied()
            .filter(|value| !value.is_nan())
            .collect();
        Self {
            name,
            count: values.len(),
            points: percentiles(&mut values, points),
        }
    }

    /// The value at `point`, or `None` when the distribution holds no value there.
    pub fn value_at(&self, point: f64) -> Option<f64> {
        self.points
            .iter()
            .find(|(candidate, _)| (candidate - point).abs() < f64::EPSILON)
            .map(|&(_, value)| value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_builder_counts_and_ranks_the_samples() {
        let found = Distribution::from_samples("x", &[4.0, 1.0, 3.0, 2.0], &[50.0, 100.0]);
        assert_eq!(found.name, "x");
        assert_eq!(found.count, 4);
        assert_eq!(found.points, [(50.0, 2.0), (100.0, 4.0)]);
        assert_eq!(found.value_at(100.0), Some(4.0));
        assert_eq!(found.value_at(90.0), None);
    }

    #[test]
    fn nan_samples_leave_the_count() {
        let found = Distribution::from_samples("x", &[f64::NAN, 5.0], &[50.0]);
        assert_eq!(found.count, 1);
        assert_eq!(found.value_at(50.0), Some(5.0));
    }

    #[test]
    fn no_samples_give_no_points() {
        let found = Distribution::from_samples("x", &[], &STANDARD_POINTS);
        assert_eq!(found.count, 0);
        assert!(found.points.is_empty());
        assert_eq!(found.value_at(50.0), None);
    }
}
