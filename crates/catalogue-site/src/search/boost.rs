//! The ranking boost of each tier.

use catalogue_core::Tier;

/// The ranking boost of each tier, in [`Tier::ALL`] order.
///
/// A search hit score is multiplied by the boost of the entry's tier.
pub const TIER_BOOSTS: [(Tier, f32); 4] = [
    (Tier::Featured, 1.5),
    (Tier::Listed, 1.1),
    (Tier::New, 1.0),
    (Tier::Archived, 0.7),
];

/// The boost of an entry without a tier: a build without a tier report.
pub const NO_TIER_BOOST: f32 = 1.0;

/// The boost of `tier`, from [`TIER_BOOSTS`]. [`NO_TIER_BOOST`] for `None`.
pub(crate) fn tier_boost(tier: Option<Tier>) -> f32 {
    tier.and_then(|tier| {
        TIER_BOOSTS
            .iter()
            .find(|(listed, _)| *listed == tier)
            .map(|&(_, boost)| boost)
    })
    .unwrap_or(NO_TIER_BOOST)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_table_lists_every_tier_once_in_order() {
        let tiers: Vec<Tier> = TIER_BOOSTS.iter().map(|&(tier, _)| tier).collect();
        assert_eq!(tiers, Tier::ALL);
    }

    #[test]
    fn each_tier_takes_its_boost() {
        assert_eq!(tier_boost(Some(Tier::Featured)), 1.5);
        assert_eq!(tier_boost(Some(Tier::Listed)), 1.1);
        assert_eq!(tier_boost(Some(Tier::New)), 1.0);
        assert_eq!(tier_boost(Some(Tier::Archived)), 0.7);
        assert_eq!(tier_boost(None), NO_TIER_BOOST);
    }
}
