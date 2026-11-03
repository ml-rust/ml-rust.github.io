use super::fact::{Fact, show};
use super::signals::Signals;
use super::verdict::{Reason, Rule};
use crate::registry::RotThresholds;

/// Number of rot signals the rules count.
pub(crate) const ROT_SIGNALS: u32 = 5;

/// The rot signals found on one check.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Rot {
    /// One reason per rot signal found.
    pub found: Vec<Reason>,
    /// Signals a rot check lacked. Each such check counts neither way.
    pub needs: Vec<&'static str>,
}

impl Rot {
    /// The number of rot signals found.
    pub fn count(&self) -> u32 {
        u32::try_from(self.found.len()).unwrap_or(u32::MAX)
    }

    /// Known free of rot: no signal found and no check unknown.
    pub fn clear(&self) -> Fact {
        if !self.found.is_empty() {
            Fact::known(false)
        } else if self.needs.is_empty() {
            Fact::known(true)
        } else {
            Fact::unknown(self.needs.clone())
        }
    }
}

/// R1: an outside issue unanswered for at least `unanswered_days`, and no maintainer
/// reply over the reply window.
fn unanswered(signals: &Signals, t: &RotThresholds) -> (Fact, String) {
    let fact = Fact::all(vec![
        Fact::of(
            "oldest_unanswered_days",
            signals.oldest_unanswered_days,
            |days| days >= t.unanswered_days,
        ),
        Fact::of(
            "maintainer_replies_365",
            signals.maintainer_replies_365,
            |n| n == 0,
        ),
    ]);
    let detail = format!(
        "oldest unanswered issue {} days (limit {}), no maintainer reply in {} days",
        show(signals.oldest_unanswered_days),
        t.unanswered_days,
        t.maintainer_silence_days
    );
    (fact, detail)
}

/// R2: a vulnerability advisory left unpatched past the grace window.
fn unpatched(signals: &Signals, t: &RotThresholds) -> (Fact, String) {
    let ids = signals.unpatched_vulnerabilities.as_deref();
    let fact = Fact::of("unpatched_vulnerabilities", ids, |ids| !ids.is_empty());
    let detail = format!(
        "unpatched past {} days: {}",
        t.advisory_grace_days,
        ids.unwrap_or_default().join(", ")
    );
    (fact, detail)
}

/// R3: a library's `former_glory` below `lost_glory_below`. Apps have no `former_glory`.
fn lost_glory(signals: &Signals, t: &RotThresholds) -> (Fact, String) {
    let fact = if signals.is_library() {
        Fact::of("former_glory", signals.former_glory, |share| {
            share < t.lost_glory_below
        })
    } else {
        Fact::known(false)
    };
    let detail = format!(
        "former_glory {} < {}",
        show(signals.former_glory),
        t.lost_glory_below
    );
    (fact, detail)
}

/// R4: the latest release requires a deprecated dependency.
fn deprecated_requirement(signals: &Signals) -> (Fact, String) {
    let requirement = signals.deprecated_requirement.as_deref();
    let fact = Fact::known(requirement.is_some());
    let detail = format!(
        "deprecated requirement `{}`",
        requirement.unwrap_or_default()
    );
    (fact, detail)
}

/// R5: the repository no longer exists.
fn repo_gone(signals: &Signals) -> (Fact, String) {
    let fact = Fact::of("repo_gone", signals.repo_gone, |gone| gone);
    (fact, "repository is gone".to_owned())
}

/// Rule 4: counts the rot signals R1 to R5.
pub(super) fn rot(signals: &Signals, t: &RotThresholds) -> Rot {
    let mut out = Rot {
        found: Vec::new(),
        needs: Vec::new(),
    };
    for (fact, detail) in [
        unanswered(signals, t),
        unpatched(signals, t),
        lost_glory(signals, t),
        deprecated_requirement(signals),
        repo_gone(signals),
    ] {
        match fact.value {
            Some(true) => out.found.push(Reason::new(Rule::Rot, detail)),
            Some(false) => {}
            None => out.needs.extend(fact.needs),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entry::EntryKind;
    use crate::tier::fixtures::{featured_app, featured_library, thresholds};

    fn rot_of(signals: &Signals) -> Rot {
        rot(signals, &thresholds().rot)
    }

    #[test]
    fn a_healthy_library_has_no_rot() {
        let found = rot_of(&featured_library());
        assert_eq!(found.count(), 0);
        assert!(found.clear().is_true());
    }

    #[test]
    fn r1_needs_both_an_old_issue_and_silence() {
        let both = Signals {
            oldest_unanswered_days: Some(270),
            maintainer_replies_365: Some(0),
            ..featured_library()
        };
        let found = rot_of(&both);
        assert_eq!(found.count(), 1);
        assert!(
            found.found[0]
                .detail
                .contains("oldest unanswered issue 270 days")
        );
        let young = Signals {
            oldest_unanswered_days: Some(269),
            ..both.clone()
        };
        assert_eq!(rot_of(&young).count(), 0);
        let replied = Signals {
            maintainer_replies_365: Some(1),
            ..both
        };
        assert_eq!(rot_of(&replied).count(), 0);
    }

    #[test]
    fn r1_with_one_unknown_half_is_unknown_only_when_the_other_holds() {
        let unknown_replies = Signals {
            oldest_unanswered_days: Some(300),
            maintainer_replies_365: None,
            ..featured_library()
        };
        let found = rot_of(&unknown_replies);
        assert_eq!(found.count(), 0);
        assert_eq!(found.needs, ["maintainer_replies_365"]);
        assert_eq!(found.clear().value, None);
        let young = Signals {
            oldest_unanswered_days: Some(10),
            maintainer_replies_365: None,
            ..featured_library()
        };
        assert!(rot_of(&young).needs.is_empty());
    }

    #[test]
    fn r2_counts_unpatched_advisories() {
        let signals = Signals {
            unpatched_vulnerabilities: Some(vec!["RUSTSEC-2026-0001".to_owned()]),
            ..featured_library()
        };
        let found = rot_of(&signals);
        assert_eq!(found.count(), 1);
        assert!(found.found[0].detail.contains("RUSTSEC-2026-0001"));
        let unknown = Signals {
            unpatched_vulnerabilities: None,
            ..featured_library()
        };
        assert_eq!(rot_of(&unknown).needs, ["unpatched_vulnerabilities"]);
    }

    #[test]
    fn r3_fires_strictly_below_the_limit_on_libraries_only() {
        let at = Signals {
            former_glory: Some(0.5),
            ..featured_library()
        };
        assert_eq!(rot_of(&at).count(), 0);
        let below = Signals {
            former_glory: Some(0.49),
            ..featured_library()
        };
        assert_eq!(rot_of(&below).count(), 1);
        let app = Signals {
            former_glory: Some(0.1),
            ..featured_app()
        };
        assert_eq!(rot_of(&app).count(), 0);
        assert!(rot_of(&featured_app()).clear().is_true());
    }

    #[test]
    fn r4_and_r5_are_counted() {
        let signals = Signals {
            deprecated_requirement: Some("tokio 0.1".to_owned()),
            repo_gone: Some(true),
            ..featured_library()
        };
        let found = rot_of(&signals);
        assert_eq!(found.count(), 2);
        assert!(found.found[0].detail.contains("`tokio 0.1`"));
        assert_eq!(found.found[1].detail, "repository is gone");
    }

    #[test]
    fn every_signal_fires_together() {
        let signals = Signals {
            oldest_unanswered_days: Some(400),
            maintainer_replies_365: Some(0),
            unpatched_vulnerabilities: Some(vec!["RUSTSEC-2026-0002".to_owned()]),
            former_glory: Some(0.1),
            deprecated_requirement: Some("tokio 0.1".to_owned()),
            repo_gone: Some(true),
            ..featured_library()
        };
        assert_eq!(rot_of(&signals).count(), ROT_SIGNALS);
    }

    #[test]
    fn unknown_signals_are_named_and_not_counted() {
        let found = rot_of(&Signals::unknown(EntryKind::Library));
        assert_eq!(found.count(), 0);
        assert_eq!(
            found.needs,
            [
                "oldest_unanswered_days",
                "maintainer_replies_365",
                "unpatched_vulnerabilities",
                "former_glory",
                "repo_gone"
            ]
        );
        assert_eq!(found.clear().value, None);
    }
}
