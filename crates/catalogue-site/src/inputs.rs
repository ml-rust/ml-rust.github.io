//! What one site build reads.

use catalogue_core::{Catalogue, Date, Slug, Tier, TierReport};

/// Everything one site build reads.
///
/// The thresholds come from [`Catalogue::thresholds`], so the rules page and
/// the refusal checks read the same checked file.
#[derive(Debug, Clone, Copy)]
pub struct SiteInputs<'a> {
    /// The checked data set.
    pub catalogue: &'a Catalogue,
    /// The tier report of the latest check. `None` builds every page without a tier.
    pub report: Option<&'a TierReport>,
    /// Tier changes, newest first. Pages show them only when `report` is given.
    pub changes: &'a [TierChangeRow],
}

/// One tier change of one entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TierChangeRow {
    /// The date of the check that gave the new tier.
    pub date: Date,
    /// The entry slug.
    pub slug: Slug,
    /// The tier before the change. `None` on the entry's first check.
    pub from: Option<Tier>,
    /// The new tier.
    pub to: Tier,
}
