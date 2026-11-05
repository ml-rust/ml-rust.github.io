//! The report of `catalogue discover below-floor`: repos of crates with too few active users.

use std::collections::HashMap;

use catalogue_core::BelowFloor;
use catalogue_signals::discover::{self, Group, Lead, Selected};
use catalogue_signals::github::StatsFetch;
use catalogue_signals::seed::ExistingIndex;

/// The result of `discover below-floor`.
#[derive(Debug, PartialEq, Eq)]
pub struct BelowFloorReport {
    /// The header, one line per lead and the footer.
    pub text: String,
    /// One line per request that failed.
    pub warnings: Vec<String>,
    /// The repos no answer covers. Above zero, the list is partial and the run exits with an error.
    pub unfetched: usize,
}

/// The crates and repos a run examined.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Counts {
    /// Crates below the floor with a match other than a dependency.
    pub(crate) crates: usize,
    /// Repos those crates link to.
    pub(crate) repos: usize,
    /// Repos an entry, `excluded_repos` or a lead already covers.
    pub(crate) known: usize,
}

/// Selects the leads of `fetched` and renders the report.
///
/// `groups` are the repos that were asked for. The first line holds the lead count. The second
/// holds the crates and repos examined and the repos already known. Each lead has a line with
/// `owner/name`, the stars, the forks, up to 2 matched terms and the description, separated by
/// tabs. The last two lines hold the repos GitHub did not resolve and the repos no answer
/// covers. A line for a failed request goes to `warnings`.
pub(crate) fn build_below_floor_report(
    counts: Counts,
    groups: Vec<Group>,
    fetched: StatsFetch,
    limits: &BelowFloor,
    today: jiff::civil::Date,
    known: &ExistingIndex,
) -> BelowFloorReport {
    let mut by_repo: HashMap<String, Group> = groups
        .into_iter()
        .map(|group| (group.repo.to_string(), group))
        .collect();
    let unresolved = fetched.unresolved.len();
    let unfetched = fetched.unfetched();
    let warnings = fetched
        .failed
        .iter()
        .map(|batch| format!("{}. {} repos not fetched", batch.error, batch.repos.len()))
        .collect();
    let pairs: Vec<_> = fetched
        .found
        .into_iter()
        .filter_map(|(asked, stats)| by_repo.remove(&asked.to_string()).map(|g| (g, stats)))
        .collect();
    let selected = discover::select_leads(pairs, limits, today, known);
    BelowFloorReport {
        text: render(counts, &selected, unresolved, unfetched),
        warnings,
        unfetched,
    }
}

fn render(counts: Counts, selected: &Selected, unresolved: usize, unfetched: usize) -> String {
    let known = counts.known + selected.renamed_known;
    let mut lines = vec![
        format!("below-floor leads: {}", selected.leads.len()),
        format!(
            "examined: {} crates, {} repos. already known: {known}",
            counts.crates, counts.repos
        ),
    ];
    lines.extend(selected.leads.iter().map(lead_line));
    lines.push(format!("unresolved: {unresolved}"));
    lines.push(if unfetched == 0 {
        "unfetched: 0".to_owned()
    } else {
        format!("unfetched: {unfetched}. The list is partial. Rerun to fetch them")
    });
    lines.join("\n")
}

fn lead_line(lead: &Lead) -> String {
    format!(
        "{}\t{}\t{}\t{}\t{}",
        lead.name,
        lead.stars,
        lead.forks,
        lead.terms.join(","),
        lead.description
    )
}

#[cfg(test)]
mod tests {
    use catalogue_signals::github::{FailedBatch, GithubError, RepoRef, RepoStats};
    use jiff::civil::date;

    use super::*;

    fn repo(name: &str) -> RepoRef {
        RepoRef::parse(name).expect("repo")
    }

    fn group(name: &str, terms: &[&str]) -> Group {
        Group {
            repo: repo(name),
            key: format!("github.com/{name}"),
            crates: vec!["c".to_owned()],
            terms: terms.iter().map(|t| (*t).to_owned()).collect(),
        }
    }

    fn stats(name: &str, stars: u32, description: &str) -> RepoStats {
        RepoStats {
            name_with_owner: name.to_owned(),
            stars,
            forks: 3,
            is_fork: false,
            is_archived: false,
            primary_language: Some("Rust".to_owned()),
            pushed_at: Some("2026-10-01T00:00:00Z".to_owned()),
            description: Some(description.to_owned()),
        }
    }

    fn limits() -> BelowFloor {
        BelowFloor {
            min_stars: 100,
            active_days: 730,
        }
    }

    fn counts() -> Counts {
        Counts {
            crates: 40,
            repos: 30,
            known: 4,
        }
    }

    #[test]
    fn the_report_has_a_header_sorted_rows_and_a_footer() {
        let fetched = StatsFetch {
            found: vec![
                (repo("a/low"), stats("a/low", 150, "Low\tone")),
                (repo("z/top"), stats("Z/Top", 900, "A database")),
                (repo("s/small"), stats("s/small", 99, "Too few stars")),
            ],
            unresolved: vec![repo("g/gone"), repo("h/gone")],
            failed: Vec::new(),
        };
        let groups = vec![
            group("a/low", &["llm"]),
            group("z/top", &["database", "rag", "third"]),
            group("s/small", &["llm"]),
        ];
        let report = build_below_floor_report(
            counts(),
            groups,
            fetched,
            &limits(),
            date(2026, 11, 5),
            &ExistingIndex::default(),
        );
        assert_eq!(
            report.text,
            "below-floor leads: 2\n\
             examined: 40 crates, 30 repos. already known: 4\n\
             Z/Top\t900\t3\tdatabase,rag\tA database\n\
             a/low\t150\t3\tllm\tLow one\n\
             unresolved: 2\n\
             unfetched: 0"
        );
        assert!(report.warnings.is_empty());
    }

    #[test]
    fn a_failed_batch_is_a_warning_and_the_footer_counts_its_repos() {
        let fetched = StatsFetch {
            found: Vec::new(),
            unresolved: Vec::new(),
            failed: vec![FailedBatch {
                repos: vec![repo("a/b"), repo("c/d")],
                error: GithubError::Graphql {
                    repo: "2 repos from a/b to c/d".to_owned(),
                    message: "boom".to_owned(),
                },
            }],
        };
        let report = build_below_floor_report(
            counts(),
            vec![group("a/b", &["llm"]), group("c/d", &["llm"])],
            fetched,
            &limits(),
            date(2026, 11, 5),
            &ExistingIndex::default(),
        );
        assert_eq!(report.warnings.len(), 1);
        assert!(report.warnings[0].contains("boom"), "{:?}", report.warnings);
        assert!(report.warnings[0].ends_with("2 repos not fetched"));
        assert!(report.text.starts_with("below-floor leads: 0\n"));
        assert!(
            report
                .text
                .ends_with("unfetched: 2. The list is partial. Rerun to fetch them")
        );
    }

    #[test]
    fn a_rename_onto_a_known_repo_counts_as_known() {
        let known = ExistingIndex::default().with_repo("github.com/new/name", "entry");
        let fetched = StatsFetch {
            found: vec![(repo("old/name"), stats("New/Name", 500, "x"))],
            ..StatsFetch::default()
        };
        let report = build_below_floor_report(
            counts(),
            vec![group("old/name", &["llm"])],
            fetched,
            &limits(),
            date(2026, 11, 5),
            &known,
        );
        assert!(report.text.contains("already known: 5"), "{}", report.text);
        assert!(report.text.starts_with("below-floor leads: 0\n"));
    }
}
