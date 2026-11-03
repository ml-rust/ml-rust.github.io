//! The advisories of a RustSec archive, keyed by crate.

use std::collections::HashMap;

use catalogue_core::canonical_crate_name;
use jiff::{Span, civil};

use super::advisory::{Advisory, AdvisoryKind};

/// Days an unpatched vulnerability must stand before it counts against a crate.
///
/// RustSec gives maintainers 60 days to ship a fix before an advisory is
/// treated as an unpatched risk of the crate.
pub const UNPATCHED_GRACE_DAYS: i64 = 60;

/// Every advisory read from a RustSec archive.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Advisories {
    /// Advisories by canonical crate name, each list sorted by date then id.
    pub by_crate: HashMap<String, Vec<Advisory>>,
    /// One line per file or field the reader skips, naming the archive path and the reason.
    pub warnings: Vec<String>,
}

impl Advisories {
    /// The advisories of crate `name`, in any spelling crates.io treats as equal.
    pub fn for_crate(&self, name: &str) -> &[Advisory] {
        self.by_crate
            .get(&canonical_crate_name(name))
            .map(Vec::as_slice)
            .unwrap_or_default()
    }

    /// The latest advisory of kind [`AdvisoryKind::Unmaintained`] that is not withdrawn.
    pub fn unmaintained(&self, name: &str) -> Option<&Advisory> {
        self.for_crate(name)
            .iter()
            .rev()
            .find(|advisory| advisory.kind == AdvisoryKind::Unmaintained && !is_withdrawn(advisory))
    }

    /// The vulnerabilities of crate `name` with no fix that are at least `min_age_days` old on `today`.
    ///
    /// An advisory counts when it is a [`AdvisoryKind::Vulnerability`], is not
    /// withdrawn, lists no patched version, and carries a date on or before
    /// `today` minus `min_age_days`. Pass [`UNPATCHED_GRACE_DAYS`] for the
    /// RustSec rule.
    pub fn unpatched_vulnerabilities(
        &self,
        name: &str,
        today: civil::Date,
        min_age_days: i64,
    ) -> Vec<&Advisory> {
        let cutoff = shifted_back(today, min_age_days);
        self.for_crate(name)
            .iter()
            .filter(|advisory| {
                advisory.kind == AdvisoryKind::Vulnerability
                    && !is_withdrawn(advisory)
                    && advisory.patched.is_empty()
                    && advisory.date <= cutoff
            })
            .collect()
    }
}

fn is_withdrawn(advisory: &Advisory) -> bool {
    advisory.withdrawn.is_some()
}

/// `date` minus `days`, clamped to the supported calendar range.
fn shifted_back(date: civil::Date, days: i64) -> civil::Date {
    let clamp = if days < 0 {
        civil::Date::MAX
    } else {
        civil::Date::MIN
    };
    Span::new()
        .try_days(days)
        .and_then(|span| date.checked_sub(span))
        .unwrap_or(clamp)
}

#[cfg(test)]
mod tests {
    use jiff::civil::date;

    use super::*;

    fn advisory(id: &str, day: civil::Date, kind: AdvisoryKind, patched: &[&str]) -> Advisory {
        Advisory {
            id: id.to_owned(),
            package: "foo-bar".to_owned(),
            date: day,
            kind,
            withdrawn: None,
            patched: patched.iter().map(|&v| v.to_owned()).collect(),
            unaffected: Vec::new(),
            url: None,
        }
    }

    fn with(advisories: Vec<Advisory>) -> Advisories {
        let mut by_crate = HashMap::new();
        by_crate.insert("foo-bar".to_owned(), advisories);
        Advisories {
            by_crate,
            warnings: Vec::new(),
        }
    }

    #[test]
    fn lookup_reads_any_spelling_of_the_name() {
        let notice = advisory("A", date(2025, 1, 1), AdvisoryKind::Notice, &[]);
        let all = with(vec![notice]);
        assert_eq!(all.for_crate("Foo_Bar").len(), 1);
        assert_eq!(all.for_crate("foo-bar").len(), 1);
        assert!(all.for_crate("other").is_empty());
    }

    #[test]
    fn grace_period_boundary_is_inclusive() {
        let today = date(2026, 11, 3);
        let on = date(2026, 9, 4);
        let after = date(2026, 9, 5);
        assert_eq!(on.until(today).ok().map(|span| span.get_days()), Some(60));
        let all = with(vec![
            advisory("ON", on, AdvisoryKind::Vulnerability, &[]),
            advisory("AFTER", after, AdvisoryKind::Vulnerability, &[]),
        ]);
        let ids: Vec<&str> = all
            .unpatched_vulnerabilities("foo-bar", today, UNPATCHED_GRACE_DAYS)
            .iter()
            .map(|a| a.id.as_str())
            .collect();
        assert_eq!(ids, ["ON"]);
    }

    #[test]
    fn patched_withdrawn_and_informational_advisories_do_not_count() {
        let old = date(2020, 1, 1);
        let mut withdrawn = advisory("W", old, AdvisoryKind::Vulnerability, &[]);
        withdrawn.withdrawn = Some(date(2021, 1, 1));
        let all = with(vec![
            advisory("P", old, AdvisoryKind::Vulnerability, &[">= 1.0.0"]),
            withdrawn,
            advisory("U", old, AdvisoryKind::Unsound, &[]),
            advisory("V", old, AdvisoryKind::Vulnerability, &[]),
        ]);
        let found = all.unpatched_vulnerabilities("foo-bar", date(2026, 11, 3), 60);
        assert_eq!(found.len(), 1);
        assert_eq!(found.first().map(|a| a.id.as_str()), Some("V"));
    }

    #[test]
    fn unmaintained_is_the_latest_one_that_is_not_withdrawn() {
        let mut late = advisory("LATE", date(2024, 1, 1), AdvisoryKind::Unmaintained, &[]);
        late.withdrawn = Some(date(2025, 1, 1));
        let all = with(vec![
            advisory("EARLY", date(2022, 1, 1), AdvisoryKind::Unmaintained, &[]),
            late,
        ]);
        let found = all.unmaintained("foo-bar").map(|a| a.id.as_str());
        assert_eq!(found, Some("EARLY"));
        assert!(all.unmaintained("other").is_none());
    }
}
