//! The repo filter, the description text and the row order.

use std::collections::BTreeSet;

use catalogue_core::BelowFloor;
use jiff::Span;
use jiff::civil::Date;

use super::group::{Group, is_known};
use crate::github::RepoStats;
use crate::seed::ExistingIndex;

/// The most matched terms a lead shows.
pub const TERMS_SHOWN: usize = 2;

/// The most characters of a description a lead keeps, the cut mark included.
pub const DESCRIPTION_CHARS: usize = 60;

/// The mark that ends a cut description.
const CUT_MARK: &str = "...";

/// The one primary language the list keeps.
const LANGUAGE: &str = "Rust";

/// A repo for a person to review.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Lead {
    /// The current `owner/name` as GitHub spells it.
    pub name: String,
    /// The stars.
    pub stars: u32,
    /// The forks.
    pub forks: u32,
    /// The first [`TERMS_SHOWN`] terms the repo's crates matched.
    pub terms: Vec<String>,
    /// The description on one line, cut to [`DESCRIPTION_CHARS`] characters.
    pub description: String,
}

/// The outcome of [`select_leads`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Selected {
    /// The leads, by stars descending, then name.
    pub leads: Vec<Lead>,
    /// The repos that a rename moved onto a repo the catalogue already knows.
    pub renamed_known: usize,
}

/// Keeps the repos worth a review and sorts them.
///
/// `fetched` pairs each group with the stats GitHub gave it. A repo stays when it is no fork,
/// not archived, has `Rust` as primary language, was pushed on or after `today` minus
/// `limits.active_days`, and has at least `limits.min_stars` stars. A repo with no push date
/// or no primary language goes. GitHub follows a rename, so `known` is checked again against
/// the name GitHub gives, and two groups that resolve to one repo give one lead. Leads sort by
/// stars descending, then by name.
pub fn select_leads(
    fetched: Vec<(Group, RepoStats)>,
    limits: &BelowFloor,
    today: Date,
    known: &ExistingIndex,
) -> Selected {
    let cutoff = today.saturating_sub(Span::new().days(i64::from(limits.active_days)));
    let mut seen = BTreeSet::new();
    let mut renamed_known = 0;
    let mut leads = Vec::new();
    for (group, stats) in fetched {
        if !is_active_rust(&stats, limits.min_stars, cutoff) {
            continue;
        }
        let resolved = format!("github.com/{}", stats.name_with_owner.to_lowercase());
        if resolved != group.key && is_known(known, &resolved) {
            renamed_known += 1;
            continue;
        }
        if !seen.insert(resolved) {
            continue;
        }
        leads.push(Lead {
            name: stats.name_with_owner,
            stars: stats.stars,
            forks: stats.forks,
            terms: group.terms.into_iter().take(TERMS_SHOWN).collect(),
            description: one_line(stats.description.as_deref().unwrap_or_default()),
        });
    }
    leads.sort_by(|a, b| b.stars.cmp(&a.stars).then_with(|| a.name.cmp(&b.name)));
    Selected {
        leads,
        renamed_known,
    }
}

/// Whether the repo is no fork, not archived, written mainly in Rust, pushed on or after
/// `cutoff` and has at least `min_stars` stars.
fn is_active_rust(stats: &RepoStats, min_stars: u32, cutoff: Date) -> bool {
    !stats.is_fork
        && !stats.is_archived
        && stats.primary_language.as_deref() == Some(LANGUAGE)
        && stats.stars >= min_stars
        && stats
            .pushed_at
            .as_deref()
            .and_then(push_date)
            .is_some_and(|pushed| pushed >= cutoff)
}

/// The date of a push time such as `2026-01-02T03:04:05Z`.
fn push_date(pushed_at: &str) -> Option<Date> {
    pushed_at.get(..10)?.parse().ok()
}

/// `text` on one line, cut to [`DESCRIPTION_CHARS`] characters.
///
/// A control character, a tab and a line break become a space, and runs of spaces become one.
/// The text is untrusted, so no escape sequence reaches the terminal. A cut lands on a character
/// boundary and ends with `...`.
fn one_line(text: &str) -> String {
    let words: Vec<&str> = text
        .split(|c: char| c.is_control() || c.is_whitespace())
        .filter(|word| !word.is_empty())
        .collect();
    let line = words.join(" ");
    if line.chars().count() <= DESCRIPTION_CHARS {
        return line;
    }
    let keep = DESCRIPTION_CHARS - CUT_MARK.len();
    let mut cut: String = line.chars().take(keep).collect();
    let trimmed = cut.trim_end().len();
    cut.truncate(trimmed);
    cut.push_str(CUT_MARK);
    cut
}

#[cfg(test)]
mod tests {
    use jiff::civil::date;

    use super::*;
    use crate::github::RepoRef;

    const TODAY: Date = date(2026, 11, 5);

    fn limits() -> BelowFloor {
        BelowFloor {
            min_stars: 100,
            active_days: 730,
        }
    }

    fn group(repo: &str, terms: &[&str]) -> Group {
        Group {
            repo: RepoRef::parse(repo).expect("repo"),
            key: format!("github.com/{}", repo.to_lowercase()),
            crates: vec!["c".to_owned()],
            terms: terms.iter().map(|t| (*t).to_owned()).collect(),
        }
    }

    fn stats(name: &str) -> RepoStats {
        RepoStats {
            name_with_owner: name.to_owned(),
            stars: 500,
            forks: 9,
            is_fork: false,
            is_archived: false,
            primary_language: Some("Rust".to_owned()),
            pushed_at: Some("2026-10-01T00:00:00Z".to_owned()),
            description: Some("A database.".to_owned()),
        }
    }

    fn kept(name: &str, change: impl FnOnce(&mut RepoStats)) -> usize {
        let mut repo = stats(name);
        change(&mut repo);
        select_leads(
            vec![(group(name, &["llm"]), repo)],
            &limits(),
            TODAY,
            &ExistingIndex::default(),
        )
        .leads
        .len()
    }

    #[test]
    fn a_repo_that_passes_every_filter_stays_with_its_fields() {
        let selected = select_leads(
            vec![(group("a/b", &["x", "y", "z"]), stats("A/b"))],
            &limits(),
            TODAY,
            &ExistingIndex::default(),
        );
        assert_eq!(
            selected.leads,
            [Lead {
                name: "A/b".to_owned(),
                stars: 500,
                forks: 9,
                terms: vec!["x".to_owned(), "y".to_owned()],
                description: "A database.".to_owned(),
            }]
        );
    }

    #[test]
    fn the_star_limit_keeps_the_exact_count_and_drops_one_below() {
        assert_eq!(kept("a/b", |r| r.stars = 100), 1);
        assert_eq!(kept("a/b", |r| r.stars = 99), 0);
        assert_eq!(kept("a/b", |r| r.stars = 101), 1);
    }

    #[test]
    fn the_window_keeps_the_cutoff_day_and_drops_the_day_before() {
        let cutoff = TODAY.saturating_sub(Span::new().days(730));
        assert_eq!(cutoff, date(2024, 11, 5));
        assert_eq!(
            kept("a/b", |r| r.pushed_at =
                Some("2024-11-05T00:00:00Z".to_owned())),
            1
        );
        assert_eq!(
            kept("a/b", |r| r.pushed_at =
                Some("2024-11-04T23:59:59Z".to_owned())),
            0
        );
        assert_eq!(
            kept("a/b", |r| r.pushed_at =
                Some("2026-11-05T00:00:00Z".to_owned())),
            1
        );
    }

    #[test]
    fn a_fork_an_archived_repo_and_a_missing_push_date_go() {
        assert_eq!(kept("a/b", |r| r.is_fork = true), 0);
        assert_eq!(kept("a/b", |r| r.is_archived = true), 0);
        assert_eq!(kept("a/b", |r| r.pushed_at = None), 0);
        assert_eq!(kept("a/b", |r| r.pushed_at = Some("soon".to_owned())), 0);
    }

    #[test]
    fn only_rust_as_primary_language_stays() {
        assert_eq!(
            kept("a/b", |r| r.primary_language = Some("Python".to_owned())),
            0
        );
        assert_eq!(
            kept("a/b", |r| r.primary_language = Some("rust".to_owned())),
            0
        );
        assert_eq!(kept("a/b", |r| r.primary_language = None), 0);
    }

    #[test]
    fn leads_sort_by_stars_descending_then_name() {
        let with = |name: &str, stars: u32| {
            let mut repo = stats(name);
            repo.stars = stars;
            (group(name, &["llm"]), repo)
        };
        let selected = select_leads(
            vec![with("b/low", 200), with("z/top", 900), with("a/low", 200)],
            &limits(),
            TODAY,
            &ExistingIndex::default(),
        );
        let names: Vec<&str> = selected.leads.iter().map(|l| l.name.as_str()).collect();
        assert_eq!(names, ["z/top", "a/low", "b/low"]);
    }

    #[test]
    fn a_rename_onto_a_known_repo_is_dropped_and_counted() {
        let known = ExistingIndex::default().with_repo("github.com/new/name", "entry");
        let selected = select_leads(
            vec![(group("old/name", &["llm"]), stats("New/Name"))],
            &limits(),
            TODAY,
            &known,
        );
        assert!(selected.leads.is_empty());
        assert_eq!(selected.renamed_known, 1);
    }

    #[test]
    fn two_groups_that_resolve_to_one_repo_give_one_lead() {
        let selected = select_leads(
            vec![
                (group("old/name", &["llm"]), stats("New/Name")),
                (group("new/name", &["llm"]), stats("New/Name")),
            ],
            &limits(),
            TODAY,
            &ExistingIndex::default(),
        );
        assert_eq!(selected.leads.len(), 1);
    }

    #[test]
    fn a_description_is_cut_at_a_character_boundary() {
        let text = "é".repeat(DESCRIPTION_CHARS + 5);
        let line = one_line(&text);
        assert_eq!(line.chars().count(), DESCRIPTION_CHARS);
        assert!(line.ends_with("..."));
        assert_eq!(
            line.chars().filter(|&c| c == 'é').count(),
            DESCRIPTION_CHARS - 3
        );
        let wide = "日本語".repeat(30);
        assert_eq!(one_line(&wide).chars().count(), DESCRIPTION_CHARS);
    }

    #[test]
    fn a_description_at_the_limit_stays_whole() {
        let text = "ü".repeat(DESCRIPTION_CHARS);
        assert_eq!(one_line(&text), text);
        let over = "ü".repeat(DESCRIPTION_CHARS + 1);
        assert_ne!(one_line(&over), over);
    }

    #[test]
    fn a_description_loses_tabs_breaks_and_escapes() {
        assert_eq!(one_line("a\tb\n\n c\u{1b}[31m  d"), "a b c [31m d");
        assert_eq!(one_line(""), "");
    }
}
