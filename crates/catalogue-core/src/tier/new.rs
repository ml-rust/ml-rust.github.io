use super::fact::{Fact, show};
use super::signals::Signals;
use super::verdict::{Reason, Rule};
use crate::date::Date;
use crate::registry::NewThresholds;

/// The new rule on one check.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct New {
    pub fact: Fact,
    pub reason: Option<Reason>,
}

/// Rule 7: an entry is new when its first release is fewer than `max_age_days` days old.
///
/// An unknown first release leaves the rule unknown.
pub(super) fn new(signals: &Signals, today: Date, t: &NewThresholds) -> New {
    let fact = Fact::of("first_release", signals.first_release, |first| {
        today.days_since(first) < i64::from(t.max_age_days)
    });
    let reason = fact.is_true().then(|| {
        Reason::new(
            Rule::New,
            format!(
                "new: first release {} is under {} days old",
                show(signals.first_release),
                t.max_age_days
            ),
        )
    });
    New { fact, reason }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entry::EntryKind;
    use crate::tier::fixtures::{days_ago, featured_app, featured_library, thresholds, today};

    fn check(signals: &Signals) -> New {
        new(signals, today(), &thresholds().new)
    }

    #[test]
    fn established_library_is_not_new() {
        let found = check(&featured_library());
        assert!(found.fact.is_false());
        assert!(found.reason.is_none());
    }

    #[test]
    fn first_release_under_the_age_limit_is_new() {
        let young = Signals {
            first_release: Some(days_ago(179)),
            ..featured_library()
        };
        let found = check(&young);
        assert!(found.fact.is_true());
        let detail = found.reason.map(|r| r.detail).unwrap_or_default();
        assert!(detail.contains("under 180 days old"), "{detail}");
        let at_limit = Signals {
            first_release: Some(days_ago(180)),
            ..featured_library()
        };
        assert!(check(&at_limit).fact.is_false());
    }

    #[test]
    fn prerelease_versions_do_not_make_an_entry_new() {
        let signals = Signals {
            all_prerelease: Some(true),
            ..featured_library()
        };
        assert!(check(&signals).fact.is_false());
    }

    #[test]
    fn few_active_users_do_not_make_an_old_entry_new() {
        let signals = Signals {
            active_users: Some(0.0),
            ..featured_library()
        };
        let found = check(&signals);
        assert!(found.fact.is_false());
        assert!(found.reason.is_none());
        assert!(check(&featured_app()).fact.is_false());
    }

    #[test]
    fn an_unknown_first_release_leaves_new_unknown() {
        let found = check(&Signals::unknown(EntryKind::Library));
        assert_eq!(found.fact, Fact::unknown(vec!["first_release"]));
        assert!(found.reason.is_none());
        let app = check(&Signals::unknown(EntryKind::App));
        assert_eq!(app.fact.value, None);
    }
}
