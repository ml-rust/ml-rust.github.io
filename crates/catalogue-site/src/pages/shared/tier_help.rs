//! What each tier means, for the rules page and the disclosure on every list.

use catalogue_core::Tier;

use super::badge::TierBadge;
use super::labels::tier_name;

/// One tier and what it means.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TierLine {
    /// The display name.
    pub name: &'static str,
    /// What the tier means.
    pub meaning: &'static str,
    /// The class that colours the tier word.
    pub class: &'static str,
    /// The anchor of the tier's own rule on the rules page.
    pub anchor: &'static str,
}

/// The sentence that says what Featured means.
pub(crate) const FEATURED_MEANING: &str = "Shown first. It passes every Featured gate.";

/// Every tier with its meaning, strongest first.
pub(crate) fn tier_lines() -> Vec<TierLine> {
    Tier::ALL.into_iter().map(tier_line).collect()
}

/// The display name and meaning of `tier`.
fn tier_line(tier: Tier) -> TierLine {
    let meaning = match tier {
        Tier::Featured => FEATURED_MEANING,
        Tier::Listed => "Shown in the normal listing. No other tier applies.",
        Tier::New => "A project whose first release is recent.",
        Tier::Archived => "No longer maintained, or ended by its author.",
    };
    let anchor = match tier {
        Tier::Featured => "featured-library",
        Tier::Listed => "listed",
        Tier::New => "new",
        Tier::Archived => "archived",
    };
    TierLine {
        name: tier_name(tier),
        meaning,
        class: TierBadge::of(tier).class,
        anchor,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_tier_has_a_line() {
        let names: Vec<&str> = tier_lines().into_iter().map(|line| line.name).collect();
        assert_eq!(names, ["Featured", "Listed", "New", "Archived"]);
    }
}
