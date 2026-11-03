use super::signals::Signals;
use super::verdict::{Reason, Rule};
use crate::date::Date;

/// Whether the entry is dormant, with the signals it lacked.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Dormancy {
    /// `None` when neither activity date is known.
    pub dormant: Option<bool>,
    pub reason: Reason,
    pub needs: Vec<&'static str>,
}

/// Rule 3: an entry is dormant when the days since the later of `last_release` and
/// `last_human_commit` reach E. Neither date known leaves dormancy unknown. A dormant
/// verdict from one date names the other, unknown date in `needs`.
pub(super) fn dormancy(signals: &Signals, today: Date, expected_days: i64) -> Dormancy {
    let latest = match (signals.last_release, signals.last_human_commit) {
        (Some(release), Some(commit)) => Some(release.max(commit)),
        (release, commit) => release.or(commit),
    };
    let Some(latest) = latest else {
        return Dormancy {
            dormant: None,
            reason: Reason::new(
                Rule::Dormancy,
                "last release and last human commit unknown: dormancy unknown",
            ),
            needs: vec!["last_release", "last_human_commit"],
        };
    };
    let idle = today.days_since(latest);
    let dormant = idle >= expected_days;
    let mut needs = Vec::new();
    if dormant {
        if signals.last_release.is_none() {
            needs.push("last_release");
        }
        if signals.last_human_commit.is_none() {
            needs.push("last_human_commit");
        }
    }
    let outcome = if dormant { "dormant" } else { "active" };
    Dormancy {
        dormant: Some(dormant),
        reason: Reason::new(
            Rule::Dormancy,
            format!("last activity {latest}, {idle} days ago, E = {expected_days}: {outcome}"),
        ),
        needs,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tier::fixtures::{days_ago, featured_library, today};

    fn active_on(release: Option<i64>, commit: Option<i64>) -> Signals {
        Signals {
            last_release: release.map(days_ago),
            last_human_commit: commit.map(days_ago),
            ..featured_library()
        }
    }

    #[test]
    fn dormant_from_exactly_e_days() {
        let at = dormancy(&active_on(Some(365), Some(365)), today(), 365);
        assert_eq!(at.dormant, Some(true));
        assert!(at.reason.detail.contains("365 days ago, E = 365: dormant"));
        let before = dormancy(&active_on(Some(364), Some(400)), today(), 365);
        assert_eq!(before.dormant, Some(false));
    }

    #[test]
    fn the_later_date_counts() {
        let commit_recent = dormancy(&active_on(Some(900), Some(10)), today(), 365);
        assert_eq!(commit_recent.dormant, Some(false));
        let release_recent = dormancy(&active_on(Some(10), Some(900)), today(), 365);
        assert_eq!(release_recent.dormant, Some(false));
    }

    #[test]
    fn one_unknown_date_is_named_only_when_dormant() {
        let dormant = dormancy(&active_on(Some(400), None), today(), 365);
        assert_eq!(dormant.dormant, Some(true));
        assert_eq!(dormant.needs, ["last_human_commit"]);
        let active = dormancy(&active_on(None, Some(10)), today(), 365);
        assert_eq!(active.dormant, Some(false));
        assert!(active.needs.is_empty());
    }

    #[test]
    fn both_dates_unknown_leave_dormancy_unknown() {
        let unknown = dormancy(&active_on(None, None), today(), 365);
        assert_eq!(unknown.dormant, None);
        assert_eq!(unknown.needs, ["last_release", "last_human_commit"]);
    }
}
