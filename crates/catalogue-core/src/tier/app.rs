use super::fact::Fact;
use super::featured::Gate;
use super::repo_route::{repo_route, route_text};
use super::signals::Signals;
use crate::registry::{AppFeaturedThresholds, RepoRouteThresholds};

/// Number of app use signals the Featured rule counts.
pub(crate) const APP_SIGNALS: u32 = 5;

/// The app gate: at least `min_signals` of binary downloads, Homebrew installs, Repology
/// families, Docker pull growth, and the repository route reach their thresholds.
///
/// An app has no dependents, so the route counts as one use signal. It never passes the gate
/// alone unless `min_signals` is 1.
pub(super) fn app_gate(
    signals: &Signals,
    t: &AppFeaturedThresholds,
    route: &RepoRouteThresholds,
) -> Gate {
    let uses = [
        Fact::of(
            "binary_downloads_last3",
            signals.binary_downloads_last3,
            |n| n >= t.binary_downloads,
        ),
        Fact::of(
            "homebrew_on_request_365d",
            signals.homebrew_on_request_365d,
            |n| n >= t.homebrew_on_request_365d,
        ),
        Fact::of("repology_families", signals.repology_families, |n| {
            n >= t.repology_families
        }),
        Fact::of(
            "docker_pull_growth_90d",
            signals.docker_pull_growth_90d,
            |n| n >= t.docker_pull_growth_90d,
        ),
        repo_route(signals, route),
    ];
    let count = |test: fn(&Fact) -> bool| {
        u32::try_from(uses.iter().filter(|fact| test(fact)).count()).unwrap_or(u32::MAX)
    };
    let passing = count(Fact::is_true);
    let open = count(|fact| fact.value.is_none());
    let fact = if passing >= t.min_signals {
        Fact::known(true)
    } else if passing.saturating_add(open) < t.min_signals {
        Fact::known(false)
    } else {
        Fact::unknown(uses.into_iter().flat_map(|fact| fact.needs).collect())
    };
    Gate {
        fact,
        failure: format!(
            "{passing} of {APP_SIGNALS} use signals pass, {} needed and {}",
            t.min_signals,
            route_text(signals, route)
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tier::fixtures::{featured_app, thresholds};

    fn check(signals: &Signals) -> Fact {
        let t = thresholds();
        app_gate(signals, &t.app_featured, &t.repo_route).fact
    }

    fn fails(signals: &Signals, needle: &str) {
        let t = thresholds();
        let gate = app_gate(signals, &t.app_featured, &t.repo_route);
        assert!(gate.fact.is_false(), "{:?}", gate.fact);
        assert!(gate.failure.contains(needle), "{}", gate.failure);
    }

    #[test]
    fn healthy_app_passes() {
        assert!(check(&featured_app()).is_true());
    }

    #[test]
    fn app_needs_two_signals_at_their_thresholds() {
        let two_at_limit = Signals {
            binary_downloads_last3: Some(1_000),
            homebrew_on_request_365d: Some(500),
            repology_families: Some(1),
            docker_pull_growth_90d: Some(9_999),
            ..featured_app()
        };
        assert!(check(&two_at_limit).is_true());
        let one = Signals {
            homebrew_on_request_365d: Some(499),
            ..two_at_limit
        };
        fails(
            &one,
            "1 of 5 use signals pass, 2 needed and repository route stars 100 (needs 2000), forks 10 (needs 200), contributors 5 (needs 25)",
        );
    }

    #[test]
    fn app_with_unknown_signals_is_unknown_only_while_they_can_decide() {
        let open = Signals {
            binary_downloads_last3: Some(5_000),
            homebrew_on_request_365d: None,
            repology_families: Some(0),
            docker_pull_growth_90d: None,
            ..featured_app()
        };
        let found = check(&open);
        assert_eq!(found.value, None);
        assert_eq!(
            found.needs,
            ["homebrew_on_request_365d", "docker_pull_growth_90d"]
        );
        let enough = Signals {
            repology_families: Some(2),
            ..open.clone()
        };
        assert!(check(&enough).is_true());
        let hopeless = Signals {
            binary_downloads_last3: Some(0),
            homebrew_on_request_365d: None,
            docker_pull_growth_90d: Some(0),
            ..open
        };
        fails(&hopeless, "0 of 5 use signals pass");
    }

    /// An app with no use signal but the ones it names here.
    fn only(binary: u64, stars: u64, forks: u64, contributors: u64) -> Signals {
        Signals {
            binary_downloads_last3: Some(binary),
            homebrew_on_request_365d: Some(0),
            repology_families: Some(0),
            docker_pull_growth_90d: Some(0),
            stars: Some(stars),
            forks: Some(forks),
            contributors: Some(contributors),
            ..featured_app()
        }
    }

    #[test]
    fn the_route_counts_as_one_use_signal_and_is_not_enough_alone() {
        let t = thresholds();
        let (binary, route) = (t.app_featured.binary_downloads, t.repo_route);
        let (stars, forks, contributors) =
            (route.min_stars, route.min_forks, route.min_contributors);
        fails(
            &only(0, stars, forks, contributors),
            "1 of 5 use signals pass",
        );
        assert!(check(&only(binary, stars, forks, contributors)).is_true());
        fails(
            &only(binary, stars, forks - 1, contributors),
            "1 of 5 use signals pass",
        );
    }

    #[test]
    fn stars_alone_add_no_use_signal() {
        let binary = thresholds().app_featured.binary_downloads;
        fails(&only(binary, u64::MAX, 0, 0), "1 of 5 use signals pass");
    }

    #[test]
    fn an_unknown_route_part_leaves_an_undecided_app_unknown() {
        let t = thresholds();
        let signals = Signals {
            forks: None,
            ..only(
                t.app_featured.binary_downloads,
                t.repo_route.min_stars,
                0,
                t.repo_route.min_contributors,
            )
        };
        let found = check(&signals);
        assert_eq!(found.value, None);
        assert_eq!(found.needs, ["forks"]);
    }
}
