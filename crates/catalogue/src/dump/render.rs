//! Text output of the `catalogue dump` actions.

use catalogue_signals::dump::DumpSnapshot;
use catalogue_signals::fetch::FetchOutcome;

/// The one-line result of `dump fetch`.
pub fn render_outcome(outcome: &FetchOutcome) -> String {
    match outcome {
        FetchOutcome::Unchanged { path } => format!("unchanged: {}", path.display()),
        FetchOutcome::Downloaded { path, bytes } => {
            format!("downloaded: {} ({bytes} bytes)", path.display())
        }
    }
}

/// The `key: value` lines of `dump stats`, in a fixed order.
pub fn render_stats(snapshot: &DumpSnapshot, window_days: u16) -> String {
    let lines = [
        ("dump_timestamp", snapshot.meta.timestamp.to_string()),
        ("dump_commit", snapshot.meta.crates_io_commit.clone()),
        ("dump_directory", snapshot.meta.directory.clone()),
        ("window_days", window_days.to_string()),
        ("crates", snapshot.crates.len().to_string()),
        ("versions", snapshot.versions.len().to_string()),
        (
            "default_versions",
            snapshot.default_version.len().to_string(),
        ),
        ("dependency_edges", snapshot.edges.len().to_string()),
        (
            "crates_with_default_requirements",
            snapshot.default_requirements.len().to_string(),
        ),
        ("crates_with_owners", snapshot.owners.len().to_string()),
        ("users", snapshot.users.len().to_string()),
        ("teams", snapshot.teams.len().to_string()),
        (
            "crates_with_categories",
            snapshot.categories.len().to_string(),
        ),
        ("crates_with_keywords", snapshot.keywords.len().to_string()),
        ("deleted_crates", snapshot.deleted.len().to_string()),
        (
            "crates_with_downloads",
            snapshot.downloads.len().to_string(),
        ),
        (
            "warning_unknown_category",
            snapshot.warnings.unknown_category.to_string(),
        ),
        (
            "warning_unknown_keyword",
            snapshot.warnings.unknown_keyword.to_string(),
        ),
        (
            "warning_unknown_download_version",
            snapshot.warnings.unknown_download_version.to_string(),
        ),
    ];
    lines
        .iter()
        .map(|(key, value)| format!("{key}: {value}"))
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use catalogue_signals::dump::DumpMeta;

    use super::*;

    #[test]
    fn outcome_lines_have_a_fixed_format() {
        let unchanged = FetchOutcome::Unchanged {
            path: PathBuf::from("/c/dump/db-dump.tar.gz"),
        };
        assert_eq!(
            render_outcome(&unchanged),
            "unchanged: /c/dump/db-dump.tar.gz"
        );
        let downloaded = FetchOutcome::Downloaded {
            path: PathBuf::from("/c/dump/db-dump.tar.gz"),
            bytes: 12,
        };
        assert_eq!(
            render_outcome(&downloaded),
            "downloaded: /c/dump/db-dump.tar.gz (12 bytes)"
        );
    }

    #[test]
    fn stats_lists_every_key_in_a_fixed_order() {
        let snapshot = DumpSnapshot {
            meta: DumpMeta {
                timestamp: Default::default(),
                crates_io_commit: "abc123".to_owned(),
                directory: "2026-03-02-020017".to_owned(),
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
        let text = render_stats(&snapshot, 90);
        let keys: Vec<&str> = text
            .lines()
            .filter_map(|line| line.split_once(": ").map(|(key, _)| key))
            .collect();
        assert_eq!(
            keys,
            [
                "dump_timestamp",
                "dump_commit",
                "dump_directory",
                "window_days",
                "crates",
                "versions",
                "default_versions",
                "dependency_edges",
                "crates_with_default_requirements",
                "crates_with_owners",
                "users",
                "teams",
                "crates_with_categories",
                "crates_with_keywords",
                "deleted_crates",
                "crates_with_downloads",
                "warning_unknown_category",
                "warning_unknown_keyword",
                "warning_unknown_download_version",
            ]
        );
        assert!(text.contains("dump_commit: abc123"));
        assert!(text.contains("crates: 0"));
    }
}
