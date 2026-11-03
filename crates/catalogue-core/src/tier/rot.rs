use super::fact::{Fact, show};
use super::signals::Signals;
use super::verdict::{Reason, Rule};
use crate::date::Date;
use crate::registry::RotThresholds;

/// Number of rot signals the rules count.
pub(crate) const ROT_SIGNALS: u32 = 6;

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

/// R6: a library's latest release fails `cargo check` on current stable Rust.
///
/// The sandbox check runs in CI and records its date. A check older than
/// `build_check_max_age_days` on `today`, or one without a date, reads as unknown.
/// Apps have no crate release to check.
fn build_fails(signals: &Signals, today: Date, t: &RotThresholds) -> (Fact, String) {
    let fresh = signals
        .build_checked
        .is_some_and(|checked| today.days_since(checked) <= i64::from(t.build_check_max_age_days));
    let fact = if !signals.is_library() {
        Fact::known(false)
    } else if fresh {
        Fact::of("build_fails", signals.build_fails, |fails| fails)
    } else {
        Fact::unknown(vec!["build_fails"])
    };
    let detail = format!(
        "latest release fails `cargo check` on {}, checked {}",
        signals.build_toolchain.as_deref().unwrap_or("unknown"),
        show(signals.build_checked)
    );
    (fact, detail)
}

/// Rule 4: counts the rot signals R1 to R6 on `today`.
pub(super) fn rot(signals: &Signals, today: Date, t: &RotThresholds) -> Rot {
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
        build_fails(signals, today, t),
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
    use crate::tier::fixtures::{days_ago, featured_app, featured_library, thresholds, today};

    fn rot_of(signals: &Signals) -> Rot {
        rot(signals, today(), &thresholds().rot)
    }

    fn checked(fails: Option<bool>, age_days: i64) -> Signals {
        Signals {
            build_fails: fails,
            build_checked: Some(days_ago(age_days)),
            build_toolchain: Some("rustc 1.95.0".to_owned()),
            ..featured_library()
        }
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
            build_fails: Some(true),
            ..featured_library()
        };
        assert_eq!(rot_of(&signals).count(), ROT_SIGNALS);
    }

    #[test]
    fn r6_counts_a_fresh_failing_check_and_names_the_toolchain_and_date() {
        let found = rot_of(&checked(Some(true), 3));
        assert_eq!(found.count(), 1);
        assert_eq!(
            found.found[0].detail,
            format!(
                "latest release fails `cargo check` on rustc 1.95.0, checked {}",
                days_ago(3)
            )
        );
        let passes = rot_of(&checked(Some(false), 3));
        assert_eq!(passes.count(), 0);
        assert!(passes.clear().is_true());
    }

    #[test]
    fn r6_without_a_result_is_unknown() {
        let found = rot_of(&checked(None, 3));
        assert_eq!(found.count(), 0);
        assert_eq!(found.needs, ["build_fails"]);
        let undated = Signals {
            build_checked: None,
            ..checked(Some(true), 3)
        };
        assert_eq!(rot_of(&undated).needs, ["build_fails"]);
    }

    #[test]
    fn r6_reads_a_check_past_the_age_limit_as_unknown() {
        let at_limit = rot_of(&checked(Some(true), 45));
        assert_eq!(at_limit.count(), 1);
        let stale = rot_of(&checked(Some(true), 46));
        assert_eq!(stale.count(), 0);
        assert_eq!(stale.needs, ["build_fails"]);
        assert_eq!(stale.clear().value, None);
    }

    #[test]
    fn r6_never_fires_on_an_app() {
        let app = Signals {
            build_fails: Some(true),
            build_checked: Some(today()),
            ..featured_app()
        };
        assert_eq!(rot_of(&app).count(), 0);
        let unknown = rot_of(&Signals::unknown(EntryKind::App));
        assert!(!unknown.needs.contains(&"build_fails"));
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
                "repo_gone",
                "build_fails"
            ]
        );
        assert_eq!(found.clear().value, None);
    }
}
