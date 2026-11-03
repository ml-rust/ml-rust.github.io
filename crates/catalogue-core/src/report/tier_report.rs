//! The tier report of one check: every entry's record, for the static site.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::entry::EntryReport;
use crate::catalogue::Catalogue;
use crate::date::Date;
use crate::date_text::{plain, plain_option};
use crate::ids::Slug;
use crate::registry::Thresholds;

/// The version of the report format. A change to any field raises it.
pub const REPORT_SCHEMA: u32 = 1;

/// The tier report of one check.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TierReport {
    /// The report format version, [`REPORT_SCHEMA`] when written.
    pub schema: u32,
    /// The run date of the check.
    #[serde(serialize_with = "plain")]
    pub generated: Date,
    /// The `version` of the thresholds the check applied.
    pub thresholds_version: u32,
    /// The calibration date of those thresholds. `None` before calibration.
    #[serde(serialize_with = "plain_option")]
    pub calibrated: Option<Date>,
    /// The record of each entry, keyed by slug.
    pub entries: BTreeMap<Slug, EntryReport>,
}

/// The slugs that a report and a catalogue disagree on.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error(
    "the tier report does not match the catalogue: missing {}, extra {}. Rerun `catalogue tiers --json` on this data",
    names(.missing),
    names(.extra)
)]
pub struct ReportMismatch {
    /// Catalogue entries with no record in the report, in slug order.
    pub missing: Vec<Slug>,
    /// Report records with no entry in the catalogue, in slug order.
    pub extra: Vec<Slug>,
}

/// The slugs joined with `, `, or `none` for an empty list.
fn names(slugs: &[Slug]) -> String {
    if slugs.is_empty() {
        return "none".to_owned();
    }
    let names: Vec<&str> = slugs.iter().map(Slug::as_str).collect();
    names.join(", ")
}

impl TierReport {
    /// The report of a check on `generated` under `thresholds`.
    pub fn new(
        generated: Date,
        thresholds: &Thresholds,
        entries: BTreeMap<Slug, EntryReport>,
    ) -> Self {
        Self {
            schema: REPORT_SCHEMA,
            generated,
            thresholds_version: thresholds.version,
            calibrated: thresholds.calibrated,
            entries,
        }
    }

    /// Checks that the report holds one record for each entry of `catalogue` and no other.
    ///
    /// # Errors
    ///
    /// Returns [`ReportMismatch`] with every missing and every extra slug.
    pub fn check(&self, catalogue: &Catalogue) -> Result<(), ReportMismatch> {
        let entries = catalogue.entries();
        let missing: Vec<Slug> = entries
            .keys()
            .filter(|slug| !self.entries.contains_key(*slug))
            .cloned()
            .collect();
        let extra: Vec<Slug> = self
            .entries
            .keys()
            .filter(|slug| !entries.contains_key(*slug))
            .cloned()
            .collect();
        if missing.is_empty() && extra.is_empty() {
            Ok(())
        } else {
            Err(ReportMismatch { missing, extra })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{thresholds, valid};
    use crate::tier::{Tier, Verdict};

    fn record() -> EntryReport {
        let verdict = Verdict {
            tier: Tier::Listed,
            stable: false,
            dormant: None,
            dead_now: false,
            reasons: Vec::new(),
            unknown: Vec::new(),
            demotion_pending: None,
        };
        EntryReport::new(&verdict, Vec::new(), Vec::new())
    }

    fn report(slugs: &[&str]) -> TierReport {
        let entries = slugs
            .iter()
            .map(|slug| (Slug::new(*slug).expect("slug"), record()))
            .collect();
        TierReport::new(
            Date::new(2026, 11, 3).expect("date"),
            &thresholds(),
            entries,
        )
    }

    #[test]
    fn new_takes_the_schema_and_the_threshold_version() {
        let report = report(&[]);
        assert_eq!(report.schema, REPORT_SCHEMA);
        assert_eq!(report.thresholds_version, thresholds().version);
        assert_eq!(report.calibrated, thresholds().calibrated);
    }

    #[test]
    fn check_passes_when_the_slugs_match() {
        let catalogue = Catalogue::load(valid()).expect("valid data");
        report(&["alpha", "beta"]).check(&catalogue).expect("match");
    }

    #[test]
    fn check_names_every_missing_and_extra_slug() {
        let catalogue = Catalogue::load(valid()).expect("valid data");
        let error = report(&["alpha", "zeta"])
            .check(&catalogue)
            .expect_err("mismatch");
        let slug = |text| Slug::new(text).expect("slug");
        assert_eq!(error.missing, [slug("beta")]);
        assert_eq!(error.extra, [slug("zeta")]);
        assert_eq!(
            error.to_string(),
            "the tier report does not match the catalogue: missing beta, extra zeta. Rerun `catalogue tiers --json` on this data"
        );
    }
}
