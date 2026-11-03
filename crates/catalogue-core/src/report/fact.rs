//! One signal as an entry page shows it: raw value, source link and fetch date.

use serde::{Deserialize, Serialize};
use url::Url;

use crate::date::Date;
use crate::date_text::plain_option;

/// The id of the GitHub star count fact. Stars never count toward a tier.
pub const STARS_SIGNAL: &str = "stars";

/// Where a signal value comes from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceRef {
    /// The source name shown beside the link, such as `crates.io database dump`.
    pub label: String,
    /// The page that holds the value.
    pub url: Url,
}

impl SourceRef {
    /// A source named `label` at `url`.
    pub fn new(label: impl Into<String>, url: Url) -> Self {
        Self {
            label: label.into(),
            url,
        }
    }
}

/// One signal of one entry, with its provenance.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SignalFact {
    /// The stable signal id, such as `last_release`.
    pub name: String,
    /// The signal name shown to readers.
    pub label: String,
    /// The raw value as text. `None` when the check did not measure it.
    pub value: Option<String>,
    /// Where the value comes from. `None` when there is no value.
    pub source: Option<SourceRef>,
    /// The date of the data behind the value. `None` when there is no value.
    #[serde(serialize_with = "plain_option")]
    pub fetched: Option<Date>,
    /// Whether a tier rule reads the value. False for a display-only fact, such as stars.
    pub counts_toward_tier: bool,
}

impl SignalFact {
    /// Every provenance rule this fact breaks, one line each.
    ///
    /// - A value needs a source and a fetch date.
    /// - A source link uses `https`.
    pub fn problems(&self) -> Vec<String> {
        let mut out = Vec::new();
        if self.value.is_some() {
            if self.source.is_none() {
                out.push(format!("signal `{}` has a value but no source", self.name));
            }
            if self.fetched.is_none() {
                out.push(format!(
                    "signal `{}` has a value but no fetch date",
                    self.name
                ));
            }
        }
        if let Some(source) = &self.source
            && source.url.scheme() != "https"
        {
            out.push(format!(
                "signal `{}` cites `{}`: a source link must use https",
                self.name, source.url
            ));
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fact(value: Option<&str>, url: Option<&str>, fetched: bool) -> SignalFact {
        SignalFact {
            name: "last_release".to_owned(),
            label: "Latest release".to_owned(),
            value: value.map(str::to_owned),
            source: url.map(|url| SourceRef::new("dump", Url::parse(url).expect("url"))),
            fetched: fetched.then(|| Date::new(2026, 11, 2).expect("date")),
            counts_toward_tier: true,
        }
    }

    #[test]
    fn a_cited_dated_value_and_an_empty_fact_pass() {
        let full = fact(
            Some("2026-09-01"),
            Some("https://crates.io/data-access"),
            true,
        );
        assert!(full.problems().is_empty());
        assert!(fact(None, None, false).problems().is_empty());
    }

    #[test]
    fn a_value_needs_a_source_and_a_date() {
        let bare = fact(Some("1"), None, false).problems();
        assert_eq!(
            bare,
            [
                "signal `last_release` has a value but no source",
                "signal `last_release` has a value but no fetch date",
            ]
        );
    }

    #[test]
    fn a_source_link_needs_https() {
        let plain = fact(Some("1"), Some("http://crates.io/data-access"), true).problems();
        assert_eq!(
            plain,
            [
                "signal `last_release` cites `http://crates.io/data-access`: a source link must use https"
            ]
        );
    }
}
