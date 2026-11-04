//! The report of `catalogue discover crates`: candidates with their signals.

use std::collections::HashMap;

use catalogue_core::canonical_crate_name;
use catalogue_signals::discover::{self, Candidate, Rules};
use catalogue_signals::dump::{DumpSnapshot, IdSet, utc_date};
use catalogue_signals::usage;

/// The result of `discover crates`.
#[derive(Debug, PartialEq, Eq)]
pub struct Report {
    /// The header and one line per candidate.
    pub text: String,
    /// Core crates absent from the dump.
    pub unknown_core_crates: Vec<String>,
}

/// Finds the candidates in `snapshot` and renders them with their signals.
///
/// `listed` maps a normalised crate name to the slug of the entry that lists it.
pub fn build_report(
    snapshot: &DumpSnapshot,
    rules: &Rules,
    listed: &HashMap<String, &str>,
) -> Report {
    let found = discover::candidates(snapshot, rules);
    let targets: IdSet = found.candidates.iter().map(|c| c.crate_id).collect();
    let signals = usage::compute(snapshot, &targets, utc_date(snapshot.meta.timestamp));
    let mut rows: Vec<Row<'_>> = found
        .candidates
        .iter()
        .map(|candidate| {
            let (current, new_adopters) = signals
                .get(&candidate.crate_id)
                .map_or((0.0, 0.0), |signal| (signal.current, signal.new_adopters));
            Row {
                candidate,
                current,
                new_adopters,
                listed: listed.get(&canonical_crate_name(&candidate.name)).copied(),
            }
        })
        .collect();
    Report {
        text: render(&mut rows),
        unknown_core_crates: found.unknown_core_crates,
    }
}

/// One candidate with its signals.
struct Row<'a> {
    candidate: &'a Candidate,
    current: f64,
    new_adopters: f64,
    listed: Option<&'a str>,
}

/// The header and one tab-separated line per row.
///
/// Rows sort by `current` descending, then name. A line holds the name,
/// `current` and `new_adopters` with 2 decimals, the comma-joined reasons
/// with `deprecated` last, and `listed:<slug>` for a crate an entry lists.
fn render(rows: &mut [Row<'_>]) -> String {
    rows.sort_by(|a, b| {
        b.current
            .total_cmp(&a.current)
            .then_with(|| a.candidate.name.cmp(&b.candidate.name))
    });
    let mut lines = vec![format!("candidates: {}", rows.len())];
    for row in rows.iter() {
        let mut reasons: Vec<String> = row
            .candidate
            .reasons
            .iter()
            .map(ToString::to_string)
            .collect();
        if row.candidate.deprecated {
            reasons.push("deprecated".to_owned());
        }
        let mut line = format!(
            "{}\t{:.2}\t{:.2}\t{}",
            row.candidate.name,
            row.current,
            row.new_adopters,
            reasons.join(",")
        );
        if let Some(slug) = row.listed {
            line.push_str("\tlisted:");
            line.push_str(slug);
        }
        lines.push(line);
    }
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use catalogue_signals::discover::Reason;
    use catalogue_signals::dump::{CrateRow, DumpMeta, VersionRow};
    use jiff::Timestamp;

    use super::*;

    fn candidate(name: &str, reasons: Vec<Reason>, deprecated: bool) -> Candidate {
        Candidate {
            crate_id: 1,
            name: name.to_owned(),
            reasons,
            deprecated,
        }
    }

    #[test]
    fn rows_sort_by_current_descending_then_name() {
        let low = candidate("beta", vec![Reason::Keyword("ml".to_owned())], false);
        let tie = candidate("alpha", vec![Reason::Keyword("ml".to_owned())], false);
        let top = candidate("zeta", vec![Reason::Keyword("ml".to_owned())], false);
        let row = |candidate, current| Row {
            candidate,
            current,
            new_adopters: 0.0,
            listed: None,
        };
        let mut rows = vec![row(&low, 1.0), row(&tie, 1.0), row(&top, 2.5)];
        let text = render(&mut rows);
        let names: Vec<&str> = text
            .lines()
            .skip(1)
            .filter_map(|line| line.split('\t').next())
            .collect();
        assert_eq!(names, ["zeta", "alpha", "beta"]);
    }

    #[test]
    fn lines_have_a_fixed_format() {
        let plain = candidate(
            "plain",
            vec![
                Reason::Core,
                Reason::Category("science::ml".to_owned()),
                Reason::Keyword("llm".to_owned()),
                Reason::DependsOn("burn".to_owned()),
            ],
            false,
        );
        let old = candidate("old", vec![Reason::Keyword("llm".to_owned())], true);
        let mut rows = vec![
            Row {
                candidate: &plain,
                current: 3.0,
                new_adopters: 0.456,
                listed: Some("plain-entry"),
            },
            Row {
                candidate: &old,
                current: 0.0,
                new_adopters: 0.0,
                listed: None,
            },
        ];
        assert_eq!(
            render(&mut rows),
            "candidates: 2\n\
             plain\t3.00\t0.46\tcore,category:science::ml,keyword:llm,depends-on:burn\tlisted:plain-entry\n\
             old\t0.00\t0.00\tkeyword:llm,deprecated"
        );
    }

    #[test]
    fn no_candidates_print_only_the_header() {
        assert_eq!(render(&mut []), "candidates: 0");
    }

    fn snapshot() -> DumpSnapshot {
        let mut snap = DumpSnapshot {
            meta: DumpMeta {
                timestamp: Timestamp::UNIX_EPOCH,
                crates_io_commit: String::new(),
                directory: String::new(),
            },
            crates: Default::default(),
            versions: Default::default(),
            default_version: Default::default(),
            edges: Vec::new(),
            default_requirements: Default::default(),
            details: Default::default(),
            owners: Default::default(),
            users: Default::default(),
            teams: Default::default(),
            categories: Default::default(),
            keywords: Default::default(),
            deleted: Vec::new(),
            downloads: Default::default(),
            warnings: Default::default(),
        };
        for (id, name) in [(1, "numr"), (2, "Blazr_Core"), (3, "plain")] {
            snap.crates.insert(
                id,
                CrateRow {
                    name: name.to_owned(),
                    created_at: Timestamp::UNIX_EPOCH,
                    updated_at: Timestamp::UNIX_EPOCH,
                    description: None,
                    homepage: None,
                    repository: None,
                    deprecation: None,
                    has_readme: false,
                },
            );
            snap.versions.insert(
                id,
                VersionRow {
                    crate_id: id,
                    num: "1.0.0".to_owned(),
                    created_at: Timestamp::UNIX_EPOCH,
                    yanked: false,
                    rust_version: None,
                    edition: None,
                    published_by: None,
                    has_lib: Some(true),
                    has_bin: false,
                },
            );
            snap.default_version.insert(id, id);
        }
        snap.categories.insert(1, vec!["science::ml".to_owned()]);
        snap.keywords.insert(2, vec!["llm".to_owned()]);
        snap
    }

    #[test]
    fn report_renders_candidates_with_listed_marks_and_unknown_core_crates() {
        let rules = Rules {
            categories: vec!["science".to_owned()],
            keywords: vec!["llm".to_owned()],
            core_crates: vec!["ghost".to_owned()],
        };
        let listed: HashMap<String, &str> = [("blazr-core".to_owned(), "blazr")].into();
        let found = build_report(&snapshot(), &rules, &listed);
        assert_eq!(
            found.text,
            "candidates: 2\n\
             Blazr_Core\t0.00\t0.00\tkeyword:llm\tlisted:blazr\n\
             numr\t0.00\t0.00\tcategory:science::ml"
        );
        assert_eq!(found.unknown_core_crates, ["ghost"]);
    }
}
