use super::fact::Fact;
use super::featured::Gate;
use super::signals::Signals;
use crate::registry::AppFeaturedThresholds;

/// Number of app use signals the Featured rule counts.
pub(crate) const APP_SIGNALS: u32 = 4;

/// The app gate: at least `min_signals` of binary downloads, Homebrew installs,
/// Repology families, and Docker pull growth reach their thresholds.
pub(super) fn app_gate(signals: &Signals, t: &AppFeaturedThresholds) -> Gate {
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
            "{passing} of {APP_SIGNALS} use signals pass, {} needed",
            t.min_signals
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tier::fixtures::{featured_app, thresholds};

    fn check(signals: &Signals) -> Fact {
        app_gate(signals, &thresholds().app_featured).fact
    }

    fn fails(signals: &Signals, needle: &str) {
        let gate = app_gate(signals, &thresholds().app_featured);
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
        fails(&one, "1 of 4 use signals pass, 2 needed");
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
        fails(&hopeless, "0 of 4 use signals pass");
    }
}
