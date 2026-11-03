use super::context::Context;
use super::fact::{Fact, show};
use super::rot::Rot;
use super::signals::Signals;
use super::verdict::{Reason, Rule};
use crate::judgment::MaintainerStatus;
use crate::registry::FinishedThresholds;

/// The finished rule on one check.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Finished {
    pub stable: bool,
    pub reason: Option<Reason>,
    pub needs: Vec<&'static str>,
}

/// Adoption that keeps a dormant entry finished. A library passes on new adopters or on
/// `former_glory`. An app passes on binary downloads per release.
fn adopted(signals: &Signals, t: &FinishedThresholds) -> Fact {
    if signals.is_library() {
        Fact::any(vec![
            Fact::of("new_adopters", signals.new_adopters, |n| {
                n >= t.min_new_adopters
            }),
            Fact::of("former_glory", signals.former_glory, |share| {
                share >= t.min_former_glory
            }),
        ])
    } else {
        Fact::of(
            "binary_downloads_per_release",
            signals.binary_downloads_per_release,
            |n| n >= t.min_binary_downloads_per_release,
        )
    }
}

/// Rule 6: an entry is finished, and `stable`, when it is dormant with no rot signal
/// and still adopted, or when the maintainer states `finished` and it has no rot
/// signal. "No rot signal" needs every rot check known.
pub(super) fn finished(
    signals: &Signals,
    context: &Context,
    dormant: &Fact,
    rot: &Rot,
    t: &FinishedThresholds,
) -> Finished {
    let clear = rot.clear();
    let by_metrics = Fact::all(vec![dormant.clone(), clear.clone(), adopted(signals, t)]);
    let stated = context.maintainer_statement == Some(MaintainerStatus::Finished);
    let by_statement = Fact::all(vec![Fact::known(stated), clear]);
    let reason = if by_metrics.is_true() {
        let evidence = if signals.is_library() {
            format!(
                "new_adopters {}, former_glory {}",
                show(signals.new_adopters),
                show(signals.former_glory)
            )
        } else {
            format!(
                "binary downloads per release {}",
                show(signals.binary_downloads_per_release)
            )
        };
        Some(Reason::new(
            Rule::Finished,
            format!("dormant, no rot, still adopted ({evidence}): stable"),
        ))
    } else if by_statement.is_true() {
        Some(Reason::new(
            Rule::Finished,
            "the maintainer states `finished` and no rot signal exists: stable",
        ))
    } else {
        None
    };
    let result = Fact::any(vec![by_metrics, by_statement]);
    Finished {
        stable: result.is_true(),
        reason,
        needs: result.needs,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tier::fixtures::{context, dormant_library, featured_app, thresholds};
    use crate::tier::rot::rot;

    fn check(signals: &Signals, ctx: &Context, dormant: Fact) -> Finished {
        let t = thresholds();
        let found = rot(signals, &t.rot);
        finished(signals, ctx, &dormant, &found, &t.finished)
    }

    fn dormant() -> Fact {
        Fact::known(true)
    }

    #[test]
    fn dormant_library_without_adoption_is_not_finished() {
        let found = check(&dormant_library(), &context(), dormant());
        assert!(!found.stable);
        assert!(found.reason.is_none());
        assert!(found.needs.is_empty());
    }

    #[test]
    fn new_adopters_at_the_limit_finish_a_library() {
        let signals = Signals {
            new_adopters: Some(1.0),
            ..dormant_library()
        };
        let found = check(&signals, &context(), dormant());
        assert!(found.stable);
        let detail = found.reason.map(|r| r.detail).unwrap_or_default();
        assert!(detail.contains("new_adopters 1"), "{detail}");
        let below = Signals {
            new_adopters: Some(0.99),
            ..dormant_library()
        };
        assert!(!check(&below, &context(), dormant()).stable);
    }

    #[test]
    fn former_glory_at_the_limit_finishes_a_library() {
        let signals = Signals {
            former_glory: Some(0.8),
            ..dormant_library()
        };
        assert!(check(&signals, &context(), dormant()).stable);
        let below = Signals {
            former_glory: Some(0.79),
            ..dormant_library()
        };
        assert!(!check(&below, &context(), dormant()).stable);
    }

    #[test]
    fn active_or_rotten_entries_are_not_finished_by_metrics() {
        let adopted = Signals {
            new_adopters: Some(3.0),
            ..dormant_library()
        };
        assert!(!check(&adopted, &context(), Fact::known(false)).stable);
        let rotten = Signals {
            repo_gone: Some(true),
            ..adopted.clone()
        };
        assert!(!check(&rotten, &context(), dormant()).stable);
        let unknown_rot = Signals {
            repo_gone: None,
            ..adopted
        };
        let found = check(&unknown_rot, &context(), dormant());
        assert!(!found.stable);
        assert_eq!(found.needs, ["repo_gone"]);
    }

    #[test]
    fn app_finishes_on_downloads_per_release() {
        let at = Signals {
            binary_downloads_per_release: Some(200),
            ..featured_app()
        };
        assert!(check(&at, &context(), dormant()).stable);
        let below = Signals {
            binary_downloads_per_release: Some(199),
            ..featured_app()
        };
        assert!(!check(&below, &context(), dormant()).stable);
        let unknown = Signals {
            binary_downloads_per_release: None,
            ..featured_app()
        };
        let found = check(&unknown, &context(), dormant());
        assert_eq!(found.needs, ["binary_downloads_per_release"]);
    }

    #[test]
    fn finished_statement_needs_no_dormancy_but_needs_no_rot() {
        let ctx = Context {
            maintainer_statement: Some(MaintainerStatus::Finished),
            ..context()
        };
        let found = check(&dormant_library(), &ctx, Fact::known(false));
        assert!(found.stable);
        let detail = found.reason.map(|r| r.detail).unwrap_or_default();
        assert!(detail.contains("states `finished`"), "{detail}");
        let rotten = Signals {
            repo_gone: Some(true),
            ..dormant_library()
        };
        assert!(!check(&rotten, &ctx, Fact::known(false)).stable);
    }

    #[test]
    fn maintained_statement_is_not_finished() {
        let ctx = Context {
            maintainer_statement: Some(MaintainerStatus::Maintained),
            ..context()
        };
        assert!(!check(&dormant_library(), &ctx, Fact::known(false)).stable);
    }

    #[test]
    fn unknown_dormancy_is_named() {
        let adopted = Signals {
            new_adopters: Some(3.0),
            ..dormant_library()
        };
        let found = check(&adopted, &context(), Fact::unknown(vec!["last_release"]));
        assert!(!found.stable);
        assert_eq!(found.needs, ["last_release"]);
    }
}
