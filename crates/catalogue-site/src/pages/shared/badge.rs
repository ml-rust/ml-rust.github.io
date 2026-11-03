//! The tier badge: the tier name, with a class that colours it.

use catalogue_core::Tier;

use super::labels::tier_name;

/// The data `tier_badge.html` reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct TierBadge {
    /// The tier name the badge prints.
    pub name: &'static str,
    /// The CSS class of the tier.
    pub class: &'static str,
}

impl TierBadge {
    /// The badge of `tier`.
    pub(crate) fn of(tier: Tier) -> Self {
        let class = match tier {
            Tier::Featured => "tier-featured",
            Tier::Listed => "tier-listed",
            Tier::New => "tier-new",
            Tier::Archived => "tier-archived",
        };
        Self {
            name: tier_name(tier),
            class,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_tier_has_its_own_class() {
        let classes: Vec<&str> = Tier::ALL
            .into_iter()
            .map(|tier| TierBadge::of(tier).class)
            .collect();
        assert_eq!(
            classes,
            ["tier-featured", "tier-listed", "tier-new", "tier-archived"]
        );
        assert_eq!(TierBadge::of(Tier::New).name, "New");
    }
}
