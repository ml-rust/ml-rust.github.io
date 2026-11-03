//! The order judgments apply in. The newest by `(date, issue)` wins.

use std::cmp::Reverse;

use crate::catalogue::Catalogue;
use crate::ids::Slug;
use crate::judgment::{Judgment, JudgmentKind, MaintainerStatus};

impl Catalogue {
    /// The judgments on entry `slug` with their issue numbers, newest first.
    ///
    /// A later `date` is newer. On the same date the higher issue number is newer.
    pub fn judgments_of(&self, slug: &Slug) -> Vec<(u64, &Judgment)> {
        let mut found: Vec<(u64, &Judgment)> = self
            .judgments
            .iter()
            .filter(|(_, judgment)| &judgment.entry == slug)
            .map(|(&issue, judgment)| (issue, judgment))
            .collect();
        found.sort_by_key(|&(issue, judgment)| Reverse((judgment.date, issue)));
        found
    }

    /// The value `pick` takes from the newest judgment on `slug` that it accepts.
    ///
    /// `pick` returns `None` for a judgment kind it does not read.
    pub fn latest_judgment<'a, T>(
        &'a self,
        slug: &Slug,
        pick: impl FnMut(&'a JudgmentKind) -> Option<T>,
    ) -> Option<T> {
        latest(&self.judgments_of(slug), pick)
    }

    /// The status of the newest `maintainer` judgment on `slug`.
    pub fn maintainer_statement(&self, slug: &Slug) -> Option<MaintainerStatus> {
        self.latest_judgment(slug, maintainer_status)
    }
}

/// The value `pick` takes from the first judgment of `newest_first` that it accepts.
pub(super) fn latest<'a, T>(
    newest_first: &[(u64, &'a Judgment)],
    mut pick: impl FnMut(&'a JudgmentKind) -> Option<T>,
) -> Option<T> {
    newest_first
        .iter()
        .find_map(|&(_, judgment)| pick(&judgment.judgment))
}

/// The status a `maintainer` judgment sets.
pub(super) fn maintainer_status(kind: &JudgmentKind) -> Option<MaintainerStatus> {
    match kind {
        JudgmentKind::Maintainer { status } => Some(*status),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{dated_judgment, put, valid};

    fn maintainer(status: &str) -> String {
        format!("type = \"maintainer\"\nstatus = \"{status}\"\n")
    }

    fn alpha() -> Slug {
        Slug::new("alpha").expect("slug")
    }

    #[test]
    fn judgments_sort_by_date_then_issue_newest_first() {
        let mut src = valid();
        for (number, date) in [(10, "2026-12-01"), (11, "2026-01-01"), (12, "2026-12-01")] {
            put(
                &mut src,
                &format!("judgments/{number}.toml"),
                &dated_judgment(number, "alpha", date, &maintainer("maintained")),
            );
        }
        let catalogue = Catalogue::load(src).expect("valid data");
        let order: Vec<u64> = catalogue
            .judgments_of(&alpha())
            .into_iter()
            .map(|(issue, _)| issue)
            .collect();
        assert_eq!(order, [12, 10, 7, 11]);
        let beta = Slug::new("beta").expect("slug");
        assert!(catalogue.judgments_of(&beta).is_empty());
    }

    #[test]
    fn the_newest_accepted_judgment_wins() {
        let mut src = valid();
        let judgments = [
            (10, "2026-03-01", "maintained"),
            (11, "2026-05-01", "finished"),
            (12, "2026-04-01", "maintained"),
        ];
        for (number, date, status) in judgments {
            put(
                &mut src,
                &format!("judgments/{number}.toml"),
                &dated_judgment(number, "alpha", date, &maintainer(status)),
            );
        }
        let catalogue = Catalogue::load(src).expect("valid data");
        assert_eq!(
            catalogue.maintainer_statement(&alpha()),
            Some(MaintainerStatus::Finished)
        );
        let none = Catalogue::load(valid()).expect("valid data");
        assert_eq!(none.maintainer_statement(&alpha()), None);
    }
}
