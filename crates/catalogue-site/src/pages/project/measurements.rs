//! Every signal of an entry, in groups with a useful summary line.

use catalogue_core::{EntryReport, NOT_MEASURED, TierReport};

use super::tier::SignalRow;
use crate::pages::shared::{ago_text, grouped};

/// What the repository group says after its numbers.
const STARS_ALONE: &str = "Stars alone never qualify a project.";

/// The groups, in page order, and the signals each one holds.
const GROUPS: [(&str, &str, &[&str]); 6] = [
    (
        "adoption",
        "Adoption",
        &[
            "active_users",
            "new_adopters",
            "peak_users",
            "former_glory",
            "binary_downloads_per_release",
        ],
    ),
    (
        "maintenance",
        "Maintenance",
        &[
            "last_human_commit",
            "last_release",
            "first_release",
            "latest_version",
            "oldest_unanswered_days",
            "maintainer_replies_365",
            "repo_archived",
            "repo_gone",
            "repo_link_verified",
            "docs_present",
            "build_fails",
            "build_checked",
            "build_toolchain",
        ],
    ),
    (
        "security",
        "Security",
        &[
            "unpatched_vulnerabilities",
            "rustsec_unmaintained",
            "deprecation_notice",
            "deprecated_requirement",
        ],
    ),
    (
        "distribution",
        "Distribution",
        &[
            "release_count",
            "downloads_per_month",
            "binary_downloads_last3",
            "homebrew_on_request_365d",
            "repology_families",
            "docker_pull_growth_90d",
            "docker_pull_baseline",
            "all_prerelease",
            "all_zero_zero",
            "all_yanked",
            "crate_deleted",
        ],
    ),
    (
        "repository",
        "Repository",
        &[
            "stars",
            "forks",
            "contributors",
            "commits",
            "main_crate_left_out",
        ],
    ),
    (
        "choice-facts",
        "Facts that never count",
        &[
            "license",
            "runs_on",
            "runs_on_evidence",
            "native_code",
            "python_bindings",
            "dependents",
        ],
    ),
];

/// The group of a signal that no list above names, so that a new signal still shows.
const OTHER: (&str, &str) = ("other", "Other measurements");

/// One group of signals.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SignalGroup<'a> {
    /// The anchor of the group.
    pub id: &'static str,
    /// The group name.
    pub title: &'static str,
    /// The line that says what the group holds.
    pub summary: String,
    /// How many signals of the group have no value, as words. `None` for zero.
    pub missing: Option<String>,
    /// The signals.
    pub rows: Vec<SignalRow<'a>>,
}

/// Every signal of `record` in exactly one group, with no empty group.
pub(crate) fn signal_groups<'a>(
    report: &TierReport,
    record: &'a EntryReport,
) -> Vec<SignalGroup<'a>> {
    let named = |signal: &str| {
        GROUPS
            .iter()
            .any(|(_, _, signals)| signals.contains(&signal))
    };
    let mut groups: Vec<SignalGroup<'a>> = GROUPS
        .iter()
        .map(|&(id, title, signals)| {
            let rows = signals
                .iter()
                .filter_map(|name| record.signals.iter().find(|fact| fact.name == *name))
                .map(SignalRow::of)
                .collect();
            group(id, title, summary(id, report, record), rows)
        })
        .collect();
    let others: Vec<SignalRow<'a>> = record
        .signals
        .iter()
        .filter(|fact| !named(&fact.name))
        .map(SignalRow::of)
        .collect();
    groups.push(group(OTHER.0, OTHER.1, String::new(), others));
    groups.retain(|group| !group.rows.is_empty());
    groups
}

fn group<'a>(
    id: &'static str,
    title: &'static str,
    summary: String,
    rows: Vec<SignalRow<'a>>,
) -> SignalGroup<'a> {
    let empty = rows.iter().filter(|row| row.value.is_none()).count();
    let missing = (empty > 0 && summary != NOT_MEASURED && !summary.is_empty())
        .then(|| format!("{empty} not measured"));
    SignalGroup {
        id,
        title,
        summary,
        missing,
        rows,
    }
}

/// The summary line of the group `id`. The group of facts that never count has none.
fn summary(id: &str, report: &TierReport, record: &EntryReport) -> String {
    if id == "choice-facts" {
        return String::new();
    }
    let value = |name| record.value_of(name);
    let whole = |name| {
        value(name)
            .map(|text| catalogue_core::amount_text::whole(text).unwrap_or_else(|| text.to_owned()))
    };
    let text = match id {
        "adoption" => match (whole("active_users"), whole("new_adopters")) {
            (Some(users), Some(adopters)) => Some(format!(
                "{users} active dependents, {adopters} new dependents in 365 days"
            )),
            _ => value("binary_downloads_per_release")
                .map(|count| format!("{} binary downloads per release", grouped(count))),
        },
        "maintenance" => value("last_human_commit").map(|date| {
            let when = ago_text(date, report.generated).unwrap_or_else(|| date.to_owned());
            format!("Latest human commit: {}", lowered(&when))
        }),
        "security" => value("unpatched_vulnerabilities")
            .map(|found| format!("Unpatched vulnerabilities: {found}")),
        "distribution" => distribution(record),
        "repository" => {
            let parts: Vec<String> = ["stars", "forks", "contributors", "commits"]
                .into_iter()
                .filter_map(|name| whole(name).map(|count| format!("{} {name}", grouped(&count))))
                .collect();
            (!parts.is_empty()).then(|| format!("{}. {STARS_ALONE}", parts.join(", ")))
        }
        _ => None,
    };
    text.unwrap_or_else(|| NOT_MEASURED.to_owned())
}

/// The summary of the distribution group, from what the entry has.
fn distribution(record: &EntryReport) -> Option<String> {
    let value = |name| record.value_of(name);
    if let (Some(releases), Some(downloads)) =
        (value("release_count"), value("downloads_per_month"))
    {
        return Some(format!(
            "{} releases, {} downloads over 30 days",
            grouped(releases),
            grouped(downloads)
        ));
    }
    if let Some(downloads) = value("binary_downloads_last3") {
        return Some(format!(
            "{} binary downloads over the last 3 releases",
            grouped(downloads)
        ));
    }
    value("repology_families").map(|families| format!("{families} Repology repository families"))
}

/// `text` with its first letter lowered, for the middle of a sentence.
fn lowered(text: &str) -> String {
    let mut chars = text.chars();
    chars.next().map_or_else(String::new, |first| {
        first.to_lowercase().chain(chars).collect()
    })
}

#[cfg(test)]
mod tests {
    use catalogue_core::{Date, REPORT_SCHEMA, SignalFact, Tier};

    use super::*;

    fn fact(name: &str, value: Option<&str>) -> SignalFact {
        SignalFact {
            name: name.to_owned(),
            label: name.to_owned(),
            value: value.map(str::to_owned),
            source: None,
            fetched: None,
            counts_toward_tier: true,
        }
    }

    fn record(signals: Vec<SignalFact>) -> EntryReport {
        EntryReport {
            tier: Tier::Listed,
            stable: false,
            dormant: Some(false),
            demotion_pending: None,
            reasons: Vec::new(),
            unknown: Vec::new(),
            notes: Vec::new(),
            signals,
            search_terms: Vec::new(),
            readme: None,
        }
    }

    fn report() -> TierReport {
        TierReport {
            schema: REPORT_SCHEMA,
            generated: Date::new(2026, 11, 4).expect("date"),
            thresholds_version: 1,
            calibrated: None,
            entries: std::collections::BTreeMap::new(),
        }
    }

    #[test]
    fn every_signal_lands_in_exactly_one_group() {
        let names = [
            "binary_downloads_per_release",
            "last_release",
            "stars",
            "docs_present",
            "a_signal_nobody_named_yet",
        ];
        let record = record(names.iter().map(|name| fact(name, Some("1"))).collect());
        let groups = signal_groups(&report(), &record);
        let mut seen: Vec<&str> = groups
            .iter()
            .flat_map(|group| group.rows.iter().map(|row| row.label))
            .collect();
        seen.sort_unstable();
        let mut expected = names;
        expected.sort_unstable();
        assert_eq!(seen, expected);
        assert_eq!(groups.last().map(|group| group.id), Some("other"));
    }

    #[test]
    fn no_group_lists_a_signal_twice() {
        let mut all: Vec<&str> = GROUPS
            .iter()
            .flat_map(|(_, _, signals)| signals.iter().copied())
            .collect();
        let total = all.len();
        all.sort_unstable();
        all.dedup();
        assert_eq!(all.len(), total);
    }

    #[test]
    fn each_summary_reads_from_the_group_values() {
        let record = record(vec![
            fact("active_users", Some("38.4")),
            fact("new_adopters", Some("2")),
            fact("last_human_commit", Some("2026-11-01")),
            fact("unpatched_vulnerabilities", Some("RUSTSEC-1")),
            fact("release_count", Some("38")),
            fact("downloads_per_month", Some("1840052")),
            fact("stars", Some("1200")),
            fact("forks", Some("80")),
            fact("repo_gone", None),
        ]);
        let groups = signal_groups(&report(), &record);
        let summaries: Vec<&str> = groups.iter().map(|group| group.summary.as_str()).collect();
        assert_eq!(
            summaries,
            [
                "38 active dependents, 2 new dependents in 365 days",
                "Latest human commit: 3 days ago",
                "Unpatched vulnerabilities: RUSTSEC-1",
                "38 releases, 1,840,052 downloads over 30 days",
                "1,200 stars, 80 forks. Stars alone never qualify a project.",
            ]
        );
        assert_eq!(groups[1].missing.as_deref(), Some("1 not measured"));
        assert_eq!(groups[0].missing, None);
    }

    #[test]
    fn the_repository_group_holds_its_four_facts_and_never_counts_holds_none() {
        let record = record(vec![
            fact("stars", Some("1200")),
            fact("forks", Some("80")),
            fact("contributors", Some("38")),
            fact("commits", Some("4200")),
            fact("license", Some("MIT")),
        ]);
        let groups = signal_groups(&report(), &record);
        assert_eq!(groups[0].id, "repository");
        assert_eq!(groups[0].rows.len(), 4);
        assert_eq!(
            groups[0].summary,
            "1,200 stars, 80 forks, 38 contributors, 4,200 commits. Stars alone never qualify a project."
        );
        assert_eq!(groups[1].id, "choice-facts");
        assert_eq!(groups[1].summary, "");
        assert_eq!(groups[1].missing, None);
        assert_eq!(groups[1].rows.len(), 1);
    }

    #[test]
    fn a_group_with_no_value_says_not_measured() {
        let record = record(vec![fact("active_users", None)]);
        let groups = signal_groups(&report(), &record);
        assert_eq!(groups[0].summary, NOT_MEASURED);
        assert_eq!(groups[0].missing, None);
    }
}
