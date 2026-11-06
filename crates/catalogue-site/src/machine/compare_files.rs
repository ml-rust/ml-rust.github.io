//! `data/compare/`: one small file per entry for the compare page, and an index for its picker.
//!
//! Each file borrows from the record that `data/catalogue.json` prints, so the two cannot drift.

use catalogue_core::{EntryKind, RESERVED_ENTRY_SLUG, Slug, Tier};
use serde::Serialize;

use super::catalogue_file::{
    CATALOGUE_SCHEMA, DescriptionRecord, EntryRecord, InstallRecord, SignalRecord, WhyRecord,
    entry_records, json_file,
};
use crate::build::OutputFile;
use crate::error::SiteError;
use crate::inputs::SiteInputs;

/// The signals the compare page shows.
const SHOWN_SIGNALS: [&str; 16] = [
    "downloads_per_month",
    "binary_downloads_per_release",
    "dependents",
    "active_users",
    "new_adopters",
    "stars",
    "forks",
    "contributors",
    "commits",
    "latest_version",
    "last_release",
    "last_human_commit",
    "runs_on",
    "native_code",
    "python_bindings",
    "license",
];

/// The path of the file named `name`. A [`Slug`] holds only `a-z`, `0-9` and `-`, so it cannot
/// leave the directory.
fn compare_path(name: &str) -> String {
    format!("data/compare/{name}.json")
}

/// What the compare page reads of one entry.
#[derive(Debug, Serialize)]
struct CompareEntry<'r, 'a> {
    slug: &'a Slug,
    name: &'a str,
    kind: EntryKind,
    repo: &'r Option<String>,
    description: &'r DescriptionRecord<'a>,
    install: &'r InstallRecord,
    docs: &'r Option<String>,
    tier: Option<Tier>,
    why: &'r Option<WhyRecord>,
    native_word: &'r str,
    signals: Vec<&'r SignalRecord<'a>>,
}

impl<'r, 'a> CompareEntry<'r, 'a> {
    fn of(record: &'r EntryRecord<'a>) -> Self {
        Self {
            slug: record.slug,
            name: record.name,
            kind: record.kind,
            repo: &record.repo,
            description: &record.description,
            install: &record.install,
            docs: &record.docs,
            tier: record.tier,
            why: &record.why,
            native_word: &record.native_word,
            signals: record
                .signals
                .iter()
                .filter(|signal| SHOWN_SIGNALS.contains(&signal.name))
                .collect(),
        }
    }
}

/// One line of the index.
#[derive(Debug, Serialize)]
struct IndexLine<'a> {
    slug: &'a Slug,
    name: &'a str,
}

/// The index file.
#[derive(Debug, Serialize)]
struct IndexFile<'a> {
    schema: u32,
    entries: Vec<IndexLine<'a>>,
}

/// The index and one file per entry of `inputs`.
///
/// A loaded catalogue holds no entry with [`RESERVED_ENTRY_SLUG`], so no entry file meets the index.
///
/// # Errors
///
/// Returns [`SiteError::Encode`] when a file fails to encode.
pub(crate) fn compare_files(inputs: &SiteInputs<'_>) -> Result<Vec<OutputFile>, SiteError> {
    let records = entry_records(inputs);
    let mut files = Vec::with_capacity(records.len() + 1);
    for record in &records {
        files.push(json_file(
            compare_path(record.slug.as_str()),
            &CompareEntry::of(record),
        )?);
    }
    let index = IndexFile {
        schema: CATALOGUE_SCHEMA,
        entries: records
            .iter()
            .map(|record| IndexLine {
                slug: record.slug,
                name: record.name,
            })
            .collect(),
    };
    files.push(json_file(compare_path(RESERVED_ENTRY_SLUG), &index)?);
    Ok(files)
}

#[cfg(test)]
mod tests {
    use catalogue_core::Movement;

    use super::*;
    use crate::testing::{catalogue, entry};

    static NO_MOVEMENT: Movement = Movement::NeedsHistory;

    fn built(names: &[&str]) -> Result<Vec<OutputFile>, SiteError> {
        let data = catalogue(names.iter().map(|name| entry(name, "")).collect());
        compare_files(&SiteInputs {
            catalogue: &data,
            report: None,
            changes: &[],
            first_check: None,
            movement: &NO_MOVEMENT,
            state: None,
        })
    }

    #[test]
    fn every_path_stays_in_the_directory_with_a_plain_name() {
        let files = built(&["alpha", "a-1"]).expect("builds");
        for file in &files {
            let name = file
                .path
                .strip_prefix("data/compare/")
                .and_then(|rest| rest.strip_suffix(".json"))
                .unwrap_or_else(|| panic!("{} leaves the directory", file.path));
            assert!(Slug::new(name).is_ok(), "{name}");
        }
    }

    #[test]
    fn a_slug_cannot_hold_a_path_step() {
        for bad in ["../x", "a/b", "a\\b", "a.b", "", "A", "%2e%2e"] {
            assert!(Slug::new(bad).is_err(), "{bad:?}");
        }
    }
}
