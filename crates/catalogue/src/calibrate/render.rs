//! The text blocks of `catalogue calibrate`.

use catalogue_signals::calibrate::{Distribution, STANDARD_POINTS};

use crate::render::or_none;

/// The points to compute for a report that also prints `at`.
///
/// The standard points come first, then `at` unless the standard points hold it.
pub(super) fn points_for(at: f64) -> Vec<f64> {
    let mut points = STANDARD_POINTS.to_vec();
    if !points.contains(&at) {
        points.push(at);
    }
    points
}

/// The blocks of `distributions`, with a blank line between blocks.
pub(super) fn render_distributions(distributions: &[Distribution], at: f64) -> String {
    distributions
        .iter()
        .map(|distribution| render_distribution(distribution, at))
        .collect::<Vec<_>>()
        .join("\n\n")
}

/// The block of one signal.
///
/// The block holds `signal: <name>`, `population: <n>`, one `p<point>: <value>`
/// line for each standard point and `at p<at>: <value>`. Values print with 2
/// decimals, or `-` when the population is empty.
fn render_distribution(distribution: &Distribution, at: f64) -> String {
    let value = |point: f64| or_none(distribution.value_at(point).map(|v| format!("{v:.2}")));
    let mut lines = vec![
        format!("signal: {}", distribution.name),
        format!("population: {}", distribution.count),
    ];
    lines.extend(
        STANDARD_POINTS
            .iter()
            .map(|&point| format!("p{point}: {}", value(point))),
    );
    lines.push(format!("at p{at}: {}", value(at)));
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_block_lists_the_standard_points_then_the_chosen_one() {
        let found = Distribution::from_samples("x", &[1.0, 2.0, 3.0, 4.0], &points_for(80.0));
        assert_eq!(
            render_distribution(&found, 80.0),
            "signal: x\npopulation: 4\np50: 2.00\np75: 3.00\np90: 4.00\np95: 4.00\n\
             p99: 4.00\nat p80: 4.00"
        );
    }

    #[test]
    fn a_standard_point_is_computed_once() {
        assert_eq!(points_for(90.0), STANDARD_POINTS);
        assert_eq!(points_for(80.0).len(), STANDARD_POINTS.len() + 1);
    }

    #[test]
    fn an_empty_population_prints_dashes() {
        let found = Distribution::from_samples("x", &[], &points_for(90.0));
        assert_eq!(
            render_distribution(&found, 90.0),
            "signal: x\npopulation: 0\np50: -\np75: -\np90: -\np95: -\np99: -\nat p90: -"
        );
    }

    #[test]
    fn blocks_have_a_blank_line_between_them() {
        let points = points_for(90.0);
        let blocks = [
            Distribution::from_samples("a", &[1.0], &points),
            Distribution::from_samples("b", &[2.5], &points),
        ];
        let text = render_distributions(&blocks, 90.0);
        assert_eq!(text.matches("\n\n").count(), 1);
        assert!(text.contains("at p90: 1.00\n\nsignal: b\npopulation: 1\np50: 2.50"));
    }
}
