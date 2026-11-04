//! The rules page struct, its template binding and the page-level lines.

use askama::Template;
use catalogue_core::Thresholds;

use super::sections::{RuleGroup, groups};
use crate::pages::layout::Layout;
use crate::pages::shared::{TierLine, date_text, tier_lines};

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
    /// Every rule, in groups a reader follows, each tier first.
    pub groups: Vec<RuleGroup>,
}

/// One status line, ending in a date when it names one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StatusLine {
    /// The text before the date, or the whole line.
    pub text: String,
    /// The date the line ends with.
    pub date: Option<String>,
    /// The date the line ends with, as a person reads it.
    pub date_text: Option<String>,
}

impl StatusLine {
    fn plain(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            date: None,
            date_text: None,
        }
    }
}

impl RulesPage {
    /// The rules page in `layout`, showing `thresholds` when present.
    pub(crate) fn new(layout: Layout, thresholds: Option<&Thresholds>) -> Self {
        Self {
            layout,
            status: status(thresholds),
            tiers: tier_lines(),
            groups: groups(thresholds),
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
            date_text: date_text(&date.to_string()),
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
        assert_eq!(calibrated[1].date_text.as_deref(), Some("3 Nov 2026"));
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
}
