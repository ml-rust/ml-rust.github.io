use super::context::Context;
use super::fact::Fact;
use super::featured::featured;
use super::level::Tier;
use super::new::new;
use super::signals::Signals;
use super::verdict::{Reason, Rule};
use crate::registry::Thresholds;

/// The tier of an entry that is not archived, with its reasons and missing signals.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Admitted {
    pub tier: Tier,
    pub reasons: Vec<Reason>,
    pub needs: Vec<&'static str>,
}

/// Rules 7 to 9 for an entry that is not archived: New, then Featured, then Listed.
///
/// Featured needs the entry active: not dormant, or `stable`. An org affiliated entry
/// that passes Featured without an outside approval is Listed.
pub(super) fn admit(
    signals: &Signals,
    context: &Context,
    thresholds: &Thresholds,
    dormant: Fact,
    stable: bool,
) -> Admitted {
    let young = new(signals, context.today, &thresholds.new);
    if let Some(reason) = young.reason {
        return Admitted {
            tier: Tier::New,
            reasons: vec![reason],
            needs: Vec::new(),
        };
    }
    let mut needs = young.fact.needs;
    let active = Fact::any(vec![!dormant, Fact::known(stable)]);
    let gate = featured(signals, active, thresholds);
    needs.extend(gate.fact.needs.iter().copied());
    let passed = gate.fact.is_true();
    let mut reasons = vec![gate.reason];
    let tier = if passed && context.org_affiliated && !context.featured_approved {
        reasons.push(Reason::new(
            Rule::FeaturedNeedsOutsideApproval,
            "org affiliated: Featured waits for a featured-approval judgment from an outside maintainer",
        ));
        Tier::Listed
    } else if passed {
        Tier::Featured
    } else {
        reasons.push(Reason::new(
            Rule::Listed,
            "listed: no archive, new or featured rule applies",
        ));
        Tier::Listed
    };
    Admitted {
        tier,
        reasons,
        needs,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tier::fixtures::{context, days_ago, featured_library, thresholds};

    fn run(signals: &Signals, ctx: &Context, dormant: bool, stable: bool) -> Admitted {
        admit(signals, ctx, &thresholds(), Fact::known(dormant), stable)
    }

    fn rules(admitted: &Admitted) -> Vec<Rule> {
        admitted.reasons.iter().map(|reason| reason.rule).collect()
    }

    #[test]
    fn active_healthy_library_is_featured() {
        let admitted = run(&featured_library(), &context(), false, false);
        assert_eq!(admitted.tier, Tier::Featured);
        assert_eq!(rules(&admitted), [Rule::Featured]);
        assert!(admitted.needs.is_empty());
    }

    #[test]
    fn new_comes_before_featured() {
        let signals = Signals {
            first_release: Some(days_ago(30)),
            ..featured_library()
        };
        let admitted = run(&signals, &context(), false, false);
        assert_eq!(admitted.tier, Tier::New);
        assert_eq!(rules(&admitted), [Rule::New]);
    }

    #[test]
    fn dormant_entry_is_featured_only_when_stable() {
        let dormant = run(&featured_library(), &context(), true, false);
        assert_eq!(dormant.tier, Tier::Listed);
        assert_eq!(rules(&dormant), [Rule::Featured, Rule::Listed]);
        assert!(dormant.reasons[0].detail.contains("dormant and not stable"));
        let stable = run(&featured_library(), &context(), true, true);
        assert_eq!(stable.tier, Tier::Featured);
    }

    #[test]
    fn unknown_dormancy_blocks_featured_and_is_named() {
        let admitted = admit(
            &featured_library(),
            &context(),
            &thresholds(),
            Fact::unknown(vec!["last_release", "last_human_commit"]),
            false,
        );
        assert_eq!(admitted.tier, Tier::Listed);
        assert_eq!(admitted.needs, ["last_release", "last_human_commit"]);
    }

    #[test]
    fn org_affiliated_entry_needs_an_outside_approval() {
        let ctx = Context {
            org_affiliated: true,
            ..context()
        };
        let waiting = run(&featured_library(), &ctx, false, false);
        assert_eq!(waiting.tier, Tier::Listed);
        assert_eq!(
            rules(&waiting),
            [Rule::Featured, Rule::FeaturedNeedsOutsideApproval]
        );
        let approved = Context {
            featured_approved: true,
            ..ctx
        };
        let admitted = run(&featured_library(), &approved, false, false);
        assert_eq!(admitted.tier, Tier::Featured);
    }

    #[test]
    fn approval_alone_does_not_make_featured() {
        let ctx = Context {
            org_affiliated: true,
            featured_approved: true,
            ..context()
        };
        let signals = Signals {
            docs_present: Some(false),
            ..featured_library()
        };
        let admitted = run(&signals, &ctx, false, false);
        assert_eq!(admitted.tier, Tier::Listed);
        assert!(!rules(&admitted).contains(&Rule::FeaturedNeedsOutsideApproval));
    }
}
