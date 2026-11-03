//! The rules page struct, its template binding and the page-level lines.

use askama::Template;
use catalogue_core::{Thresholds, Tier};

use super::sections::{RuleSection, sections};
use crate::pages::layout::Layout;
use crate::pages::shared::tier_name;

/// The rules page.
#[derive(Debug, Template)]
#[template(path = "rules.html")]
pub(crate) struct RulesPage {
    /// The shared layout.
    pub layout: Layout,
    /// The thresholds version and calibration lines.
    pub status: Vec<String>,
    /// Every tier with its meaning.
    pub tiers: Vec<TierLine>,
    /// Every rule, in the order the check applies them.
    pub sections: Vec<RuleSection>,
}

/// One tier and what it means.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TierLine {
    /// The display name.
    pub name: &'static str,
    /// What the tier means.
    pub meaning: &'static str,
}

impl RulesPage {
    /// The rules page in `layout`, showing `thresholds` when present.
    pub(crate) fn new(layout: Layout, thresholds: Option<&Thresholds>) -> Self {
        Self {
            layout,
            status: status(thresholds),
            tiers: Tier::ALL.into_iter().map(tier_line).collect(),
            sections: sections(thresholds),
        }
    }
}

/// The version and calibration lines for `thresholds`.
fn status(thresholds: Option<&Thresholds>) -> Vec<String> {
    let Some(t) = thresholds else {
        return vec!["data/thresholds.toml is absent. No tier is published.".to_owned()];
    };
    let calibration = match t.calibrated {
        Some(date) => format!("Calibrated on {date}."),
        None => "Not calibrated — tiers are not published.".to_owned(),
    };
    vec![
        format!("Thresholds version {}.", t.version),
        calibration,
        "The values stay frozen until the yearly recalibration. Any change raises the version."
            .to_owned(),
    ]
}

/// The display name and meaning of `tier`.
fn tier_line(tier: Tier) -> TierLine {
    let meaning = match tier {
        Tier::Featured => "Shown first. It passes every Featured gate.",
        Tier::Listed => "Shown in the normal listing. No other tier applies.",
        Tier::New => "A young, prerelease or barely used project.",
        Tier::Archived => "No longer maintained, or ended by its author.",
    };
    TierLine {
        name: tier_name(tier),
        meaning,
    }
}

#[cfg(test)]
mod tests {
    use catalogue_core::Date;

    use super::super::fixtures::thresholds;
    use super::*;

    #[test]
    fn status_names_the_version_and_the_calibration() {
        let date = Date::new(2026, 11, 3).expect("date");
        let calibrated = status(Some(&thresholds(Some(date))));
        assert_eq!(calibrated[0], "Thresholds version 3.");
        assert_eq!(calibrated[1], "Calibrated on 2026-11-03.");
        let uncalibrated = status(Some(&thresholds(None)));
        assert_eq!(uncalibrated[1], "Not calibrated — tiers are not published.");
        let absent = status(None);
        assert_eq!(
            absent,
            ["data/thresholds.toml is absent. No tier is published."]
        );
    }

    #[test]
    fn every_tier_has_a_line() {
        let names: Vec<&str> = Tier::ALL
            .into_iter()
            .map(|tier| tier_line(tier).name)
            .collect();
        assert_eq!(names, ["Featured", "Listed", "New", "Archived"]);
    }
}
