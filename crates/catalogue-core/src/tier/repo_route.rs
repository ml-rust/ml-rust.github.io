use super::fact::{Fact, show};
use super::signals::Signals;
use crate::registry::RepoRouteThresholds;

/// The text that opens every statement of the route. The site's sentence parser reads it.
const ROUTE: &str = "repository route";

/// The main crate that keeps an entry off the route. Only an entry that lists crates has one.
fn left_out(signals: &Signals) -> Option<&str> {
    signals.main_crate_left_out.as_deref()
}

/// The repository route: stars, forks and contributors each reach their limit.
///
/// The route measures a whole repository, so an entry that lists crates and leaves out the
/// repository's main crate fails it whatever the numbers say. Stars alone never pass. A value below
/// its limit fails the route even when another value is unknown. Otherwise a missing value leaves
/// the route unknown, never false.
pub(super) fn repo_route(signals: &Signals, t: &RepoRouteThresholds) -> Fact {
    if left_out(signals).is_some() {
        return Fact::known(false);
    }
    Fact::all(vec![
        Fact::of("stars", signals.stars, |n| n >= t.min_stars),
        Fact::of("forks", signals.forks, |n| n >= t.min_forks),
        Fact::of("contributors", signals.contributors, |n| {
            n >= t.min_contributors
        }),
    ])
}

/// Every part of the route with its value and its limit, or the main crate that the entry leaves
/// out.
pub(super) fn route_text(signals: &Signals, t: &RepoRouteThresholds) -> String {
    if let Some(name) = left_out(signals) {
        return format!("{ROUTE}: the repo's main crate {name} is not in this entry");
    }
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

    /// An entry of `kind` far past every limit that leaves out `wasmtime`.
    fn side_crates_only(kind: EntryKind) -> Signals {
        Signals {
            stars: Some(u64::MAX),
            forks: Some(u64::MAX),
            contributors: Some(u64::MAX),
            main_crate_left_out: Some("wasmtime".to_owned()),
            ..Signals::unknown(kind)
        }
    }

    #[test]
    fn a_library_without_the_main_crate_fails_at_any_numbers_and_says_why() {
        let found = side_crates_only(EntryKind::Library);
        assert!(route(&found).is_false());
        assert_eq!(
            route_text(&found, &thresholds().repo_route),
            "repository route: the repo's main crate wasmtime is not in this entry"
        );
    }

    #[test]
    fn a_library_without_the_main_crate_fails_with_unknown_numbers() {
        let found = Signals {
            stars: None,
            ..side_crates_only(EntryKind::Library)
        };
        let fact = route(&found);
        assert!(fact.is_false());
        assert!(fact.needs.is_empty());
    }

    #[test]
    fn a_library_with_its_main_crate_listed_passes_on_the_numbers() {
        let found = Signals {
            main_crate_left_out: None,
            ..side_crates_only(EntryKind::Library)
        };
        assert!(route(&found).is_true());
    }

    #[test]
    fn an_app_that_lists_side_crates_fails_like_a_library() {
        let found = side_crates_only(EntryKind::App);
        assert!(route(&found).is_false());
        assert_eq!(
            route_text(&found, &thresholds().repo_route),
            "repository route: the repo's main crate wasmtime is not in this entry"
        );
    }

    #[test]
    fn an_app_with_no_finding_passes_on_the_numbers() {
        let found = Signals {
            main_crate_left_out: None,
            ..side_crates_only(EntryKind::App)
        };
        assert!(route(&found).is_true());
        assert!(route_text(&found, &thresholds().repo_route).contains("stars "));
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
