//! The drafts and the report of `catalogue seed`.

use std::collections::HashMap;

use catalogue_core::{Date, Topics};
use catalogue_signals::discover::{self, Rules};
use catalogue_signals::dump::{DumpSnapshot, IdSet, utc_date};
use catalogue_signals::seed::{Drafts, ExistingIndex, draft_entries};
use catalogue_signals::usage;

/// The result of one seed run.
#[derive(Debug, PartialEq, Eq)]
pub struct Seed {
    /// The drafts and the skipped crates.
    pub drafts: Drafts,
    /// The report: the drafts, then the skipped crates.
    pub text: String,
}

/// Drafts entries for the candidates of `snapshot` that match `rules`.
///
/// A candidate is eligible when it has at least `min_active_users` active
/// users, as of the dump date, and no entry in `existing` lists its crate.
/// Eligible crates of one repository form one draft. The report holds
/// `drafts: <n>`, one line `slug`, the highest `current` among the family's
/// crates with 2 decimals (the tier engine reads the family maximum, so the
/// primary's value alone understates a family) and the comma-joined topics per draft, then `skipped: <n>` and one
/// line `crate`, reason per skipped crate. Fields separate with a tab.
pub fn build_seed(
    snapshot: &DumpSnapshot,
    rules: &Rules,
    topics: &Topics,
    existing: &ExistingIndex,
    min_active_users: f64,
) -> Seed {
    let found = discover::candidates(snapshot, rules);
    let targets: IdSet = found.candidates.iter().map(|c| c.crate_id).collect();
    let today = utc_date(snapshot.meta.timestamp);
    let signals = usage::compute(snapshot, &targets, today);
    let drafts = draft_entries(
        snapshot,
        &found.candidates,
        &signals,
        topics,
        existing,
        min_active_users,
        Date::from(today),
    );
    let current: HashMap<&str, f64> = found
        .candidates
        .iter()
        .filter_map(|c| Some((c.name.as_str(), signals.get(&c.crate_id)?.current)))
        .collect();
    let text = render(&drafts, &current);
    Seed { drafts, text }
}

fn render(drafts: &Drafts, current: &HashMap<&str, f64>) -> String {
    let mut lines = vec![format!("drafts: {}", drafts.entries.len())];
    for (slug, entry) in &drafts.entries {
        let users = entry
            .crates
            .iter()
            .filter_map(|name| current.get(name.as_str()).copied())
            .fold(0.0, f64::max);
        let topics: Vec<&str> = entry.topics.iter().map(|t| t.as_str()).collect();
        lines.push(format!("{slug}\t{users:.2}\t{}", topics.join(",")));
    }
    lines.push(format!("skipped: {}", drafts.skipped.len()));
    for (name, reason) in &drafts.skipped {
        lines.push(format!("{name}\t{reason}"));
    }
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use catalogue_signals::dump::{CrateRow, DumpMeta, Owner, OwnerKind, VersionRow};
    use jiff::Timestamp;

    use super::*;

    fn snapshot() -> DumpSnapshot {
        let at = |text: &str| -> Timestamp { text.parse().expect("timestamp") };
        let row = |name: &str, repository: Option<&str>| CrateRow {
            name: name.to_owned(),
            created_at: at("2025-01-01T00:00:00Z"),
            updated_at: at("2026-09-01T00:00:00Z"),
            description: Some("A crate.".to_owned()),
            homepage: None,
            repository: repository.map(str::to_owned),
            deprecation: None,
            has_readme: true,
        };
        let version = |crate_id| VersionRow {
            crate_id,
            num: "1.0.0".to_owned(),
            created_at: at("2025-02-01T12:00:00Z"),
            yanked: false,
            rust_version: None,
            edition: None,
            published_by: None,
        };
        let mut snapshot = DumpSnapshot {
            meta: DumpMeta {
                timestamp: at("2026-11-02T00:00:00Z"),
                crates_io_commit: String::new(),
                directory: String::new(),
            },
            crates: Default::default(),
            versions: Default::default(),
            default_version: Default::default(),
            edges: Vec::new(),
            default_requirements: Default::default(),
            owners: Default::default(),
            users: Default::default(),
            teams: Default::default(),
            categories: Default::default(),
            keywords: Default::default(),
            deleted: Vec::new(),
            downloads: Default::default(),
            warnings: Default::default(),
        };
        for (id, name, repository) in [
            (1, "has-repo", Some("https://github.com/o/has-repo")),
            (2, "no-repo", None),
        ] {
            snapshot.crates.insert(id, row(name, repository));
            snapshot.versions.insert(id, version(id));
            snapshot.default_version.insert(id, id);
            snapshot.keywords.insert(id, vec!["llm".to_owned()]);
        }
        snapshot
    }

    fn rules() -> Rules {
        Rules {
            keywords: vec!["llm".to_owned()],
            ..Rules::default()
        }
    }

    fn topics() -> Topics {
        toml::from_str(
            "[[topic]]\nid = \"inference\"\nname = \"Inference\"\nkeywords = [\"llm\"]\n",
        )
        .expect("topics parse")
    }

    #[test]
    fn candidates_without_users_are_not_drafted_at_a_positive_minimum() {
        let seed = build_seed(
            &snapshot(),
            &rules(),
            &topics(),
            &ExistingIndex::default(),
            1.0,
        );
        assert_eq!(seed.text, "drafts: 0\nskipped: 0");
    }

    #[test]
    fn the_report_lists_drafts_then_skips() {
        let seed = build_seed(
            &snapshot(),
            &rules(),
            &topics(),
            &ExistingIndex::default(),
            0.0,
        );
        assert_eq!(
            seed.text,
            "drafts: 1\nhas-repo\t0.00\tinference\nskipped: 1\nno-repo\tno repository"
        );
        assert_eq!(seed.drafts.entries.len(), 1);
    }

    #[test]
    fn a_draft_shows_the_highest_current_of_its_family() {
        let mut snap = snapshot();
        let row = snap.crates.get(&1).cloned();
        let version = snap.versions.get(&1).cloned();
        let (Some(row), Some(version)) = (row, version) else {
            panic!("fixture crate 1 exists");
        };
        snap.crates.insert(3, row);
        if let Some(derive) = snap.crates.get_mut(&3) {
            derive.name = "has-repo-derive".to_owned();
        }
        snap.versions.insert(3, version);
        snap.default_version.insert(3, 3);
        snap.keywords.insert(3, vec!["llm".to_owned()]);
        let owner = Owner {
            id: 1,
            kind: OwnerKind::User,
        };
        snap.owners.insert(1, vec![owner]);
        snap.owners.insert(3, vec![owner]);
        let seed = build_seed(&snap, &rules(), &topics(), &ExistingIndex::default(), 0.0);
        assert_eq!(seed.drafts.entries.len(), 1);
        for (low, high) in [
            ("has-repo", "has-repo-derive"),
            ("has-repo-derive", "has-repo"),
        ] {
            let current = HashMap::from([(low, 1.0), (high, 3.0)]);
            let text = render(&seed.drafts, &current);
            let users = text.lines().nth(1).and_then(|line| line.split('\t').nth(1));
            assert_eq!(users, Some("3.00"), "{low} low, {high} high");
        }
    }

    #[test]
    fn listed_crates_are_left_out() {
        let existing = ExistingIndex::new(["has_repo", "no-repo"], []);
        let seed = build_seed(&snapshot(), &rules(), &topics(), &existing, 0.0);
        assert_eq!(seed.text, "drafts: 0\nskipped: 0");
    }
}
