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

/// Rule 7: an entry is new when its first release is fewer than `max_age_days` days
/// old, when every version is a prerelease or `0.0.x`, or, for a library, when it has
/// fewer than `min_active_users` active users.
pub(super) fn new(signals: &Signals, today: Date, t: &NewThresholds) -> New {
    let young = Fact::of("first_release", signals.first_release, |first| {
        today.days_since(first) < i64::from(t.max_age_days)
    });
    let prerelease = Fact::of("all_prerelease", signals.all_prerelease, |all| all);
    let few_users = if signals.is_library() {
        Fact::of("active_users", signals.active_users, |users| {
            users < t.min_active_users
        })
    } else {
        Fact::known(false)
    };
    let mut why = Vec::new();
    if young.is_true() {
        why.push(format!(
            "first release {} is under {} days old",
            show(signals.first_release),
            t.max_age_days
        ));
    }
    if prerelease.is_true() {
        why.push("every version is a prerelease or 0.0.x".to_owned());
    }
    if few_users.is_true() {
        why.push(format!(
            "{} active users < {}",
            show(signals.active_users),
            t.min_active_users
        ));
    }
    let fact = Fact::any(vec![young, prerelease, few_users]);
    let reason = fact
        .is_true()
        .then(|| Reason::new(Rule::New, format!("new: {}", why.join(", "))));
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
    fn all_prerelease_is_new() {
        let signals = Signals {
            all_prerelease: Some(true),
            ..featured_library()
        };
        assert!(check(&signals).fact.is_true());
    }

    #[test]
    fn few_active_users_make_a_library_new_but_not_an_app() {
        let below = Signals {
            active_users: Some(1.5),
            ..featured_library()
        };
        assert!(check(&below).fact.is_true());
        let at = Signals {
            active_users: Some(2.0),
            ..featured_library()
        };
        assert!(check(&at).fact.is_false());
        assert!(check(&featured_app()).fact.is_false());
    }

    #[test]
    fn unknown_signals_leave_new_unknown_unless_one_holds() {
        let found = check(&Signals::unknown(EntryKind::Library));
        assert_eq!(
            found.fact,
            Fact::unknown(vec!["first_release", "all_prerelease", "active_users"])
        );
        assert!(found.reason.is_none());
        let prerelease = Signals {
            all_prerelease: Some(true),
            ..Signals::unknown(EntryKind::App)
        };
        assert!(check(&prerelease).fact.is_true());
    }
}
