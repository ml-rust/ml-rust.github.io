//! An entry as the site shows it once its merged judgments apply.

use std::collections::BTreeSet;

use url::Url;

use super::order::{latest, maintainer_status};
use crate::catalogue::Catalogue;
use crate::date::Date;
use crate::entry::Entry;
use crate::ids::Slug;
use crate::judgment::{Judgment, JudgmentKind, MaintainerStatus, ProjectKind};

/// One `correctness` judgment on an entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Correction<'a> {
    /// The issue number.
    pub issue: u64,
    /// The issue link.
    pub link: &'a Url,
    /// Approval date.
    pub date: Date,
    /// What the review found.
    pub finding: &'a str,
    /// Links that support the finding.
    pub evidence: &'a [Url],
}

/// An entry with its merged judgments applied.
///
/// Where several judgments of one kind set the same field, the newest by
/// `(date, issue)` wins. See [`Catalogue::judgments_of`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EffectiveEntry<'a> {
    /// The entry slug.
    pub slug: &'a Slug,
    /// The entry as its data file states it.
    pub entry: &'a Entry,
    /// The topics of the newest `category` judgment, else the entry's own topics.
    pub topics: &'a [Slug],
    /// The value of the newest `kind` judgment. `None` without one.
    pub kind_label: Option<ProjectKind>,
    /// The entry that replaces this one, from the newest `supersedes` judgment.
    pub superseded_by: Option<&'a Slug>,
    /// The entries this one replaces, sorted by slug.
    pub supersedes: Vec<&'a Slug>,
    /// Every `correctness` judgment, newest first.
    pub corrections: Vec<Correction<'a>>,
    /// The status of the newest `maintainer` judgment.
    pub maintainer_statement: Option<MaintainerStatus>,
    /// Every judgment on the entry with its issue number, newest first.
    pub judgments: Vec<(u64, &'a Judgment)>,
}

impl Catalogue {
    /// Entry `slug` with its merged judgments applied, or `None` for an unknown slug.
    pub fn effective(&self, slug: &Slug) -> Option<EffectiveEntry<'_>> {
        let (slug, entry) = self.entries.get_key_value(slug)?;
        let judgments = self.judgments_of(slug);
        let replaced: BTreeSet<&Slug> = self
            .judgments
            .values()
            .filter(|judgment| replacement(&judgment.judgment) == Some(slug))
            .map(|judgment| &judgment.entry)
            .filter(|old| self.superseded_by(old) == Some(slug))
            .collect();
        let corrections = judgments
            .iter()
            .filter_map(|&(issue, judgment)| match &judgment.judgment {
                JudgmentKind::Correctness { finding } => Some(Correction {
                    issue,
                    link: &judgment.issue,
                    date: judgment.date,
                    finding,
                    evidence: &judgment.evidence,
                }),
                _ => None,
            })
            .collect();
        Some(EffectiveEntry {
            slug,
            entry,
            topics: latest(&judgments, category_topics).unwrap_or(entry.topics.as_slice()),
            kind_label: latest(&judgments, project_kind),
            superseded_by: latest(&judgments, replacement),
            supersedes: replaced.into_iter().collect(),
            corrections,
            maintainer_statement: latest(&judgments, maintainer_status),
            judgments,
        })
    }

    /// The slugs whose effective topics hold `topic`, sorted by slug.
    ///
    /// Effective topics are those of [`EffectiveEntry::topics`]. A parent topic
    /// does not list the entries of its children.
    pub fn entries_in_topic(&self, topic: &Slug) -> Vec<&Slug> {
        self.entries
            .iter()
            .filter(|(slug, entry)| {
                self.latest_judgment(slug, category_topics)
                    .unwrap_or(entry.topics.as_slice())
                    .contains(topic)
            })
            .map(|(slug, _)| slug)
            .collect()
    }

    /// The entry that replaces `slug`, from the newest `supersedes` judgment on it.
    fn superseded_by(&self, slug: &Slug) -> Option<&Slug> {
        self.latest_judgment(slug, replacement)
    }
}

/// The topics a `category` judgment sets.
fn category_topics(kind: &JudgmentKind) -> Option<&[Slug]> {
    match kind {
        JudgmentKind::Category { topics } => Some(topics),
        _ => None,
    }
}

/// The kind a `kind` judgment sets.
fn project_kind(kind: &JudgmentKind) -> Option<ProjectKind> {
    match kind {
        JudgmentKind::Kind { value } => Some(*value),
        _ => None,
    }
}

/// The replacing entry a `supersedes` judgment names.
fn replacement(kind: &JudgmentKind) -> Option<&Slug> {
    match kind {
        JudgmentKind::Supersedes { by } => Some(by),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::load::DataSources;
    use crate::testing::{ALPHA, dated_judgment, put, valid};

    fn slug(text: &str) -> Slug {
        Slug::new(text).expect("slug")
    }

    fn with(judgments: &[(u32, &str, &str, &str)]) -> DataSources {
        let mut src = valid();
        for &(number, entry, date, body) in judgments {
            put(
                &mut src,
                &format!("judgments/{number}.toml"),
                &dated_judgment(number, entry, date, body),
            );
        }
        src
    }

    fn load(src: DataSources) -> Catalogue {
        Catalogue::load(src).expect("valid data")
    }

    fn category(topic: &str) -> String {
        format!("type = \"category\"\ntopics = [\"{topic}\"]\n")
    }

    fn supersedes(by: &str) -> String {
        format!("type = \"supersedes\"\nby = \"{by}\"\n")
    }

    #[test]
    fn without_judgments_the_entry_shows_its_own_data() {
        let catalogue = load(valid());
        let beta = catalogue.effective(&slug("beta")).expect("beta");
        assert_eq!(beta.topics, beta.entry.topics.as_slice());
        assert_eq!(beta.kind_label, None);
        assert_eq!(beta.superseded_by, None);
        assert!(beta.supersedes.is_empty());
        assert!(beta.corrections.is_empty());
        assert_eq!(beta.maintainer_statement, None);
        assert!(beta.judgments.is_empty());
        assert!(catalogue.effective(&slug("ghost")).is_none());
        assert_eq!(catalogue.entries_in_topic(&slug("tensors")).len(), 2);
        assert!(catalogue.entries_in_topic(&slug("ml")).is_empty());
    }

    #[test]
    fn the_later_category_judgment_wins() {
        let ml = category("ml");
        let tensors = category("tensors");
        let catalogue = load(with(&[
            (10, "alpha", "2026-06-01", ml.as_str()),
            (11, "alpha", "2026-04-01", tensors.as_str()),
        ]));
        let alpha = catalogue.effective(&slug("alpha")).expect("alpha");
        assert_eq!(alpha.topics, [slug("ml")]);
        assert_eq!(catalogue.entries_in_topic(&slug("ml")), [&slug("alpha")]);
        assert_eq!(
            catalogue.entries_in_topic(&slug("tensors")),
            [&slug("beta")]
        );
    }

    #[test]
    fn on_the_same_date_the_higher_issue_wins() {
        let ml = category("ml");
        let tensors = category("tensors");
        let catalogue = load(with(&[
            (10, "alpha", "2026-06-01", tensors.as_str()),
            (11, "alpha", "2026-06-01", ml.as_str()),
        ]));
        let alpha = catalogue.effective(&slug("alpha")).expect("alpha");
        assert_eq!(alpha.topics, [slug("ml")]);
    }

    #[test]
    fn supersedes_links_show_on_both_entries() {
        let mut src = with(&[
            (10, "alpha", "2026-03-01", supersedes("beta").as_str()),
            (11, "gamma", "2026-03-01", supersedes("alpha").as_str()),
            (12, "gamma", "2026-04-01", supersedes("beta").as_str()),
        ]);
        let gamma = ALPHA
            .replace("Alpha", "Gamma")
            .replace("ml-rust/alpha", "ml-rust/gamma")
            .replace("alpha-crate", "gamma-crate");
        put(&mut src, "entries/gamma.toml", &gamma);
        let catalogue = load(src);
        let alpha = catalogue.effective(&slug("alpha")).expect("alpha");
        assert_eq!(alpha.superseded_by, Some(&slug("beta")));
        assert!(alpha.supersedes.is_empty());
        let beta = catalogue.effective(&slug("beta")).expect("beta");
        assert_eq!(beta.superseded_by, None);
        assert_eq!(beta.supersedes, [&slug("alpha"), &slug("gamma")]);
        let gamma = catalogue.effective(&slug("gamma")).expect("gamma");
        assert_eq!(gamma.superseded_by, Some(&slug("beta")));
    }

    #[test]
    fn notes_and_labels_come_from_their_judgments() {
        let catalogue = load(with(&[
            (
                10,
                "alpha",
                "2026-01-01",
                "type = \"correctness\"\nfinding = \"Wrong.\"\n",
            ),
            (
                12,
                "alpha",
                "2026-02-01",
                "type = \"kind\"\nvalue = \"demo\"\n",
            ),
            (
                13,
                "alpha",
                "2026-03-01",
                "type = \"maintainer\"\nstatus = \"finished\"\n",
            ),
        ]));
        let alpha = catalogue.effective(&slug("alpha")).expect("alpha");
        let issues: Vec<u64> = alpha.corrections.iter().map(|note| note.issue).collect();
        assert_eq!(issues, [7, 10]);
        let older = alpha.corrections.get(1).expect("second correction");
        assert_eq!(older.finding, "Wrong.");
        assert_eq!(older.date, Date::new(2026, 1, 1).expect("date"));
        assert_eq!(older.evidence.len(), 1);
        assert!(older.link.as_str().ends_with("/issues/10"));
        assert_eq!(alpha.kind_label, Some(ProjectKind::Demo));
        assert_eq!(alpha.maintainer_statement, Some(MaintainerStatus::Finished));
        let order: Vec<u64> = alpha.judgments.iter().map(|&(issue, _)| issue).collect();
        assert_eq!(order, [7, 13, 12, 10]);
    }
}
