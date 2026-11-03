use super::context::Context;
use super::level::Tier;
use super::verdict::{Reason, Rule};

/// The demotion guard outcome.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Held {
    /// The published tier.
    pub tier: Tier,
    /// The lower tier the rules give while the demotion waits.
    pub pending: Option<Tier>,
    pub reason: Option<Reason>,
}

/// Rule 10: an entry Featured on the previous check keeps Featured when the rules give
/// a lower tier and no demotion judgment approves the drop. The caller skips this rule
/// for an instant archive.
pub(super) fn guard(context: &Context, computed: Tier) -> Held {
    let held = context.previous_tier == Some(Tier::Featured)
        && computed != Tier::Featured
        && !context.demotion_approved;
    if !held {
        return Held {
            tier: computed,
            pending: None,
            reason: None,
        };
    }
    Held {
        tier: Tier::Featured,
        pending: Some(computed),
        reason: Some(Reason::new(
            Rule::DemotionPending,
            format!(
                "rules give `{}`: stays featured until a demotion judgment is approved",
                computed.as_str()
            ),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tier::fixtures::context;

    fn after(previous: Option<Tier>, approved: bool) -> Context {
        Context {
            previous_tier: previous,
            demotion_approved: approved,
            ..context()
        }
    }

    #[test]
    fn featured_entry_holds_every_lower_tier() {
        for lower in [Tier::Listed, Tier::New, Tier::Archived] {
            let held = guard(&after(Some(Tier::Featured), false), lower);
            assert_eq!(held.tier, Tier::Featured);
            assert_eq!(held.pending, Some(lower));
            let detail = held.reason.map(|r| r.detail).unwrap_or_default();
            assert!(detail.contains(lower.as_str()), "{detail}");
        }
    }

    #[test]
    fn approved_demotion_drops_the_tier() {
        let held = guard(&after(Some(Tier::Featured), true), Tier::Listed);
        assert_eq!(held.tier, Tier::Listed);
        assert_eq!(held.pending, None);
        assert!(held.reason.is_none());
    }

    #[test]
    fn staying_featured_or_not_previously_featured_passes_through() {
        let same = guard(&after(Some(Tier::Featured), false), Tier::Featured);
        assert_eq!(same.tier, Tier::Featured);
        assert!(same.pending.is_none());
        for previous in [
            None,
            Some(Tier::Listed),
            Some(Tier::New),
            Some(Tier::Archived),
        ] {
            let held = guard(&after(previous, false), Tier::Listed);
            assert_eq!(held.tier, Tier::Listed);
            assert!(held.reason.is_none());
        }
    }
}
