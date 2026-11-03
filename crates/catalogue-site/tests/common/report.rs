//! A tier report over the fixture catalogue.

use std::collections::BTreeMap;

use catalogue_core::{Catalogue, Date, EntryReport, REPORT_SCHEMA, Slug, Tier, TierReport};

/// A report with one Listed record per slug, under the catalogue's thresholds.
///
/// The catalogue must hold thresholds.
pub fn report(catalogue: &Catalogue, slugs: &[&str]) -> TierReport {
    let thresholds = catalogue.thresholds().expect("fixture has thresholds");
    let entries = slugs
        .iter()
        .map(|slug| {
            let record = EntryReport {
                tier: Tier::Listed,
                stable: false,
                dormant: Some(false),
                demotion_pending: None,
                reasons: Vec::new(),
                unknown: Vec::new(),
                notes: Vec::new(),
                signals: Vec::new(),
            };
            (Slug::new(*slug).expect("slug"), record)
        })
        .collect::<BTreeMap<_, _>>();
    TierReport {
        schema: REPORT_SCHEMA,
        generated: Date::new(2026, 11, 4).expect("date"),
        thresholds_version: thresholds.version,
        calibrated: thresholds.calibrated,
        entries,
    }
}
