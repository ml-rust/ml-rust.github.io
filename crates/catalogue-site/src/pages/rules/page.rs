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
    pub status: Vec<StatusLine>,
    /// Every tier with its meaning.
    pub tiers: Vec<TierLine>,
    /// Every rule, in the order the check applies them.
    pub sections: Vec<RuleSection>,
}

/// One status line, ending in a date when it names one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StatusLine {
    /// The text before the date, or the whole line.
    pub text: String,
    /// The date the line ends with.
    pub date: Option<String>,
}

impl StatusLine {
    fn plain(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            date: None,
        }
    }
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
fn status(thresholds: Option<&Thresholds>) -> Vec<StatusLine> {
    let Some(t) = thresholds else {
        return vec![StatusLine::plain(
            "data/thresholds.toml is absent. No tier is published.",
        )];
    };
    let calibration = match t.calibrated {
        Some(date) => StatusLine {
            text: "Calibrated on ".to_owned(),
            date: Some(date.to_string()),
        },
        None => StatusLine::plain("Not calibrated — tiers are not published."),
    };
    vec![
        StatusLine::plain(format!("Thresholds version {}.", t.version)),
        calibration,
        StatusLine::plain(
            "The values stay frozen until the yearly recalibration. Any change raises the version.",
        ),
    ]
}

/// The display name and meaning of `tier`.
fn tier_line(tier: Tier) -> TierLine {
    let meaning = match tier {
        Tier::Featured => "Shown first. It passes every Featured gate.",
        Tier::Listed => "Shown in the normal listing. No other tier applies.",
        Tier::New => "A project whose first release is recent.",
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
        assert_eq!(calibrated[0].text, "Thresholds version 3.");
        assert_eq!(calibrated[1].text, "Calibrated on ");
        assert_eq!(calibrated[1].date.as_deref(), Some("2026-11-03"));
        let uncalibrated = status(Some(&thresholds(None)));
        assert_eq!(
            uncalibrated[1].text,
            "Not calibrated — tiers are not published."
        );
        assert_eq!(uncalibrated[1].date, None);
        let absent = status(None);
        assert_eq!(
            absent,
            [StatusLine::plain(
                "data/thresholds.toml is absent. No tier is published."
            )]
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
