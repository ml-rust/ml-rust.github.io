//! The single pass over the archive entries.

use std::collections::HashSet;
use std::io::{self, Read};
use std::path::Path;

use catalogue_core::{canonical_crate_name, push_canonical_crate_name};
use flate2::read::MultiGzDecoder;

use super::error::DumpError;
use super::names::{Seen, TableName};
use super::parse::utc_date;
use super::snapshot::{
    CrateDetails, CrateRow, DailyDownloads, DeletedCrate, DependencyEdge, DumpMeta, DumpSnapshot,
    DumpWarnings, IdMap, IdSet, Owner, Rename, Requirement, VersionRow,
};
use super::tables::downloads::Window;
use super::tables::{crates, dependencies, downloads, metadata, owners, taxonomy, versions};
use crate::archive::{is_file, name_parts};

/// Reads a crates.io database dump from its gzip-compressed tar stream.
///
/// The reader matches entries by file name: `<directory>/metadata.json` and
/// `<directory>/data/<table>.csv`. It keeps daily downloads for the
/// `window_days` days that end on the dump's UTC date. Zero days keep none.
///
/// Memory grows with the kept data only. README text is scanned for a
/// deprecation notice row by row and never kept.
///
/// # Errors
///
/// Returns a [`DumpError`] when the stream is not a gzip tar archive, a needed
/// table or column is absent, a field does not parse, an id exceeds `u32`, or
/// a table comes before a table it joins against.
pub fn read(reader: impl Read, window_days: u16) -> Result<DumpSnapshot, DumpError> {
    read_with_details(reader, window_days, &[])
}

/// [`read`], and the [`CrateDetails`] of the default version of each crate in `names`.
///
/// Names match the way crates.io compares them. A row of any other crate allocates nothing
/// beyond what [`read`] allocates, because these columns are large and the dump holds millions
/// of rows. The `crates` table must come before `versions`, as it does in a crates.io dump.
///
/// # Errors
///
/// Returns a [`DumpError`] as [`read`] does. With names, the `license`, `links` and `features`
/// columns of `versions` and the `explicit_name` column of `dependencies` are needed too.
pub fn read_with_details(
    reader: impl Read,
    window_days: u16,
    names: &[&str],
) -> Result<DumpSnapshot, DumpError> {
    let mut archive = tar::Archive::new(MultiGzDecoder::new(reader));
    let mut state = State::new(window_days, names);
    for entry in archive.entries().map_err(archive_error)? {
        let entry = entry.map_err(archive_error)?;
        if !is_file(entry.header().entry_type()) {
            continue;
        }
        let kind = classify(&entry.path().map_err(archive_error)?);
        match kind {
            Some(EntryKind::Metadata(directory)) => state.metadata(entry, directory)?,
            Some(EntryKind::Table(table)) => state.table(table, entry)?,
            None => {}
        }
    }
    state.finish()
}

fn archive_error(source: io::Error) -> DumpError {
    DumpError::Io {
        entry: "archive".to_owned(),
        source,
    }
}

/// An archive entry the reader keeps data from.
#[derive(Debug, PartialEq, Eq)]
enum EntryKind {
    /// `<directory>/metadata.json`, with the directory name.
    Metadata(String),
    /// `<directory>/data/<table>.csv` for a kept table.
    Table(TableName),
}

fn classify(path: &Path) -> Option<EntryKind> {
    match name_parts(path)?.as_slice() {
        [directory, "metadata.json"] => Some(EntryKind::Metadata((*directory).to_owned())),
        [_, "data", file] => file
            .strip_suffix(".csv")
            .and_then(TableName::from_stem)
            .map(EntryKind::Table),
        _ => None,
    }
}

/// The data kept so far, plus the lookup tables used for joins.
struct State {
    window_days: u16,
    seen: Seen,
    meta: Option<DumpMeta>,
    crates: IdMap<CrateRow>,
    versions: IdMap<VersionRow>,
    default_version: IdMap<u32>,
    edges: Vec<DependencyEdge>,
    default_requirements: IdMap<Vec<Requirement>>,
    owners: IdMap<Vec<Owner>>,
    users: IdMap<String>,
    teams: IdMap<String>,
    category_slugs: IdMap<String>,
    keyword_names: IdMap<String>,
    categories: IdMap<Vec<String>>,
    keywords: IdMap<Vec<String>>,
    deleted: Vec<DeletedCrate>,
    downloads: IdMap<DailyDownloads>,
    warnings: DumpWarnings,
    /// The canonical names of the crates whose details the reader keeps.
    detail_names: HashSet<String>,
    /// The ids of those crates, set when `versions` starts.
    detail_crates: IdSet,
    /// The details of every version of those crates, by version id.
    version_details: IdMap<CrateDetails>,
    /// The renamed dependencies of their default versions, by crate id.
    renames: IdMap<Vec<Rename>>,
}

impl State {
    fn new(window_days: u16, names: &[&str]) -> Self {
        Self {
            detail_names: names
                .iter()
                .map(|name| canonical_crate_name(name))
                .collect(),
            detail_crates: IdSet::default(),
            version_details: IdMap::default(),
            renames: IdMap::default(),
            window_days,
            seen: Seen::default(),
            meta: None,
            crates: IdMap::default(),
            versions: IdMap::default(),
            default_version: IdMap::default(),
            edges: Vec::new(),
            default_requirements: IdMap::default(),
            owners: IdMap::default(),
            users: IdMap::default(),
            teams: IdMap::default(),
            category_slugs: IdMap::default(),
            keyword_names: IdMap::default(),
            categories: IdMap::default(),
            keywords: IdMap::default(),
            deleted: Vec::new(),
            downloads: IdMap::default(),
            warnings: DumpWarnings::default(),
        }
    }

    fn metadata(&mut self, input: impl Read, directory: String) -> Result<(), DumpError> {
        if self.meta.is_some() {
            return Err(DumpError::Metadata {
                reason: format!(
                    "the archive holds a second metadata.json at `{directory}/metadata.json`"
                ),
            });
        }
        self.meta = Some(metadata::read_metadata(input, directory)?);
        Ok(())
    }

    fn table(&mut self, table: TableName, input: impl Read) -> Result<(), DumpError> {
        let name = table.name();
        if self.seen.contains(table) {
            return Err(DumpError::DuplicateTable { table: name });
        }
        if let Some(needed) = table
            .needs()
            .iter()
            .find(|&&needed| !self.seen.contains(needed))
        {
            return Err(DumpError::OrderViolation {
                table: name,
                needs: needed.name(),
            });
        }
        match table {
            TableName::Categories => self.category_slugs = taxonomy::read_categories(input)?,
            TableName::Keywords => self.keyword_names = taxonomy::read_keywords(input)?,
            TableName::CratesCategories => {
                self.categories = taxonomy::read_crate_categories(
                    input,
                    &self.category_slugs,
                    &mut self.warnings.unknown_category,
                )?;
            }
            TableName::CratesKeywords => {
                self.keywords = taxonomy::read_crate_keywords(
                    input,
                    &self.keyword_names,
                    &mut self.warnings.unknown_keyword,
                )?;
            }
            TableName::Crates => self.crates = crates::read_crates(input)?,
            TableName::DeletedCrates => self.deleted = crates::read_deleted(input)?,
            TableName::Users => self.users = owners::read_users(input)?,
            TableName::Teams => self.teams = owners::read_teams(input)?,
            TableName::CrateOwners => self.owners = owners::read_owners(input)?,
            TableName::Versions if self.detail_names.is_empty() => {
                self.versions = versions::read_versions(input)?;
            }
            TableName::Versions => {
                if !self.seen.contains(TableName::Crates) {
                    return Err(DumpError::OrderViolation {
                        table: name,
                        needs: TableName::Crates.name(),
                    });
                }
                self.detail_crates = self.named_crates();
                let (rows, details) =
                    versions::read_versions_with_details(input, &self.detail_crates)?;
                self.versions = rows;
                self.version_details = details;
            }
            TableName::DefaultVersions => {
                self.default_version = versions::read_default_versions(input)?;
            }
            TableName::Dependencies => {
                let found = if self.detail_crates.is_empty() {
                    dependencies::read(input, &self.default_version)?
                } else {
                    dependencies::read_with_renames(
                        input,
                        &self.default_version,
                        &self.detail_crates,
                    )?
                };
                self.edges = found.edges;
                self.default_requirements = found.default_requirements;
                self.renames = found.renames;
            }
            TableName::VersionDownloads => {
                let meta = self.meta.as_ref().ok_or(DumpError::OrderViolation {
                    table: name,
                    needs: "metadata.json",
                })?;
                let end = utc_date(meta.timestamp);
                self.downloads = downloads::read(
                    input,
                    &self.versions,
                    Window::ending(end, self.window_days),
                    &mut self.warnings.unknown_download_version,
                )?;
            }
        }
        self.seen.insert(table);
        Ok(())
    }

    /// The ids of the crates [`Self::detail_names`] names.
    fn named_crates(&self) -> IdSet {
        let mut canonical = String::new();
        let mut found = IdSet::default();
        for (&id, row) in &self.crates {
            canonical.clear();
            push_canonical_crate_name(&mut canonical, &row.name);
            if self.detail_names.contains(&canonical) {
                found.insert(id);
            }
        }
        found
    }

    /// The details of each kept crate's default version, by crate id.
    fn default_details(&mut self) -> IdMap<CrateDetails> {
        let mut out = IdMap::default();
        for &crate_id in &self.detail_crates {
            let details = self
                .default_version
                .get(&crate_id)
                .and_then(|version| self.version_details.remove(version));
            if let Some(mut details) = details {
                details.renames = self.renames.remove(&crate_id).unwrap_or_default();
                out.insert(crate_id, details);
            }
        }
        out
    }

    fn finish(mut self) -> Result<DumpSnapshot, DumpError> {
        let details = self.default_details();
        let meta = self.meta.ok_or_else(|| DumpError::Metadata {
            reason: "the archive holds no `<directory>/metadata.json`".to_owned(),
        })?;
        if let Some(missing) = TableName::ALL
            .into_iter()
            .find(|&table| !self.seen.contains(table))
        {
            return Err(DumpError::MissingTable {
                table: missing.name(),
            });
        }
        Ok(DumpSnapshot {
            meta,
            crates: self.crates,
            versions: self.versions,
            default_version: self.default_version,
            edges: self.edges,
            default_requirements: self.default_requirements,
            details,
            owners: self.owners,
            users: self.users,
            teams: self.teams,
            categories: self.categories,
            keywords: self.keywords,
            deleted: self.deleted,
            downloads: self.downloads,
            warnings: self.warnings,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_entries_by_file_name() {
        let cases = [
            (
                "2026-11-03-020017/metadata.json",
                Some(EntryKind::Metadata("2026-11-03-020017".to_owned())),
            ),
            (
                "./2026-11-03-020017/metadata.json",
                Some(EntryKind::Metadata("2026-11-03-020017".to_owned())),
            ),
            (
                "d/data/crates.csv",
                Some(EntryKind::Table(TableName::Crates)),
            ),
            (
                "d/data/version_downloads.csv",
                Some(EntryKind::Table(TableName::VersionDownloads)),
            ),
            ("d/data/crate_downloads.csv", None),
            ("d/data/crates.sql", None),
            ("d/schema.sql", None),
            ("d/README.md", None),
            ("d/x/metadata.json", None),
            ("d/other/crates.csv", None),
            ("/d/data/crates.csv", None),
            ("d/../data/crates.csv", None),
        ];
        for (path, expected) in cases {
            assert_eq!(classify(Path::new(path)), expected, "{path}");
        }
    }
}
