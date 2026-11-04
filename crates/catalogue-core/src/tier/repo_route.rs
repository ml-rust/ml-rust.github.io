use super::fact::{Fact, show};
use super::signals::Signals;
use crate::registry::RepoRouteThresholds;

/// The text that opens every statement of the route. The site's sentence parser reads it.
const ROUTE: &str = "repository route";

/// The repository route: stars, forks and contributors each reach their limit.
///
/// Stars alone never pass. A value below its limit fails the route even when another value is
/// unknown. Otherwise a missing value leaves the route unknown, never false.
pub(super) fn repo_route(signals: &Signals, t: &RepoRouteThresholds) -> Fact {
    Fact::all(vec![
        Fact::of("stars", signals.stars, |n| n >= t.min_stars),
        Fact::of("forks", signals.forks, |n| n >= t.min_forks),
        Fact::of("contributors", signals.contributors, |n| {
            n >= t.min_contributors
        }),
    ])
}

/// Every part of the route with its value and its limit.
pub(super) fn route_text(signals: &Signals, t: &RepoRouteThresholds) -> String {
    format!(
        "{ROUTE} stars {} (needs {}), forks {} (needs {}), contributors {} (needs {})",
        show(signals.stars),
        t.min_stars,
        show(signals.forks),
        t.min_forks,
        show(signals.contributors),
        t.min_contributors
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entry::EntryKind;
    use crate::tier::fixtures::thresholds;

    fn signals(stars: Option<u64>, forks: Option<u64>, contributors: Option<u64>) -> Signals {
        Signals {
            stars,
            forks,
            contributors,
            ..Signals::unknown(EntryKind::Library)
        }
    }

    fn route(found: &Signals) -> Fact {
        repo_route(found, &thresholds().repo_route)
    }

    #[test]
    fn stars_alone_fail_however_many_there_are() {
        let found = signals(Some(u64::MAX), Some(0), Some(0));
        assert!(route(&found).is_false());
    }

    #[test]
    fn all_three_at_their_limits_pass() {
        let t = thresholds().repo_route;
        let found = signals(
            Some(t.min_stars),
            Some(t.min_forks),
            Some(t.min_contributors),
        );
        assert!(route(&found).is_true());
    }

    #[test]
    fn each_part_below_its_limit_fails() {
        let t = thresholds().repo_route;
        let (stars, forks, contributors) = (t.min_stars, t.min_forks, t.min_contributors);
        for found in [
            signals(Some(stars - 1), Some(forks), Some(contributors)),
            signals(Some(stars), Some(forks - 1), Some(contributors)),
            signals(Some(stars), Some(forks), Some(contributors - 1)),
        ] {
            assert!(route(&found).is_false(), "{found:?}");
        }
    }

    #[test]
    fn one_unknown_stays_unknown_and_names_what_it_lacks() {
        let t = thresholds().repo_route;
        let found = signals(Some(t.min_stars), None, Some(t.min_contributors));
        let fact = route(&found);
        assert_eq!(fact.value, None);
        assert_eq!(fact.needs, ["forks"]);
        let none = route(&signals(None, None, None));
        assert_eq!(none.needs, ["stars", "forks", "contributors"]);
    }

    #[test]
    fn a_value_below_its_limit_fails_even_with_another_unknown() {
        let fact = route(&signals(Some(1), None, Some(1)));
        assert!(fact.is_false());
        assert!(fact.needs.is_empty());
    }

    #[test]
    fn the_text_states_every_part_with_its_number() {
        let found = signals(Some(3_000), None, Some(5));
        assert_eq!(
            route_text(&found, &thresholds().repo_route),
            "repository route stars 3000 (needs 2000), forks unknown (needs 200), contributors 5 (needs 25)"
        );
    }
}
