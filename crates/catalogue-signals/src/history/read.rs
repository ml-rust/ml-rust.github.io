//! Reads the snapshot files of a history directory.

use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io::{self, BufRead, BufReader};
use std::path::Path;

use catalogue_core::{Date, Slug};
use flate2::read::GzDecoder;
use jiff::civil;
use serde::Deserialize;

use super::error::HistoryError;
use super::model::{Row, SUFFIX, Snapshot};
use super::query::History;

/// One JSON line of a snapshot file.
#[derive(Deserialize)]
struct Line {
    slug: String,
    #[serde(flatten)]
    row: Row,
}

/// Reads every `YYYY-MM-DD.jsonl.gz` file of `dir` into a [`History`].
///
/// A file whose name is not a calendar date followed by `.jsonl.gz` is ignored.
/// A directory that does not exist reads as an empty history.
///
/// # Errors
///
/// Returns [`HistoryError::List`] when `dir` exists but cannot be listed,
/// [`HistoryError::Read`] when a snapshot file cannot be opened, and
/// [`HistoryError::Corrupt`] when a snapshot file is not gzip, holds a line that
/// is not a valid row, or holds an invalid or repeated slug. A corrupt file is
/// never skipped.
pub fn read_all(dir: &Path) -> Result<History, HistoryError> {
    let list_error = |source: io::Error| HistoryError::List {
        path: dir.to_path_buf(),
        source,
    };
    let listing = match fs::read_dir(dir) {
        Ok(listing) => listing,
        Err(source) if source.kind() == io::ErrorKind::NotFound => return Ok(History::default()),
        Err(source) => return Err(list_error(source)),
    };
    let mut snapshots = Vec::new();
    for item in listing {
        let item = item.map_err(list_error)?;
        let Some(date) = item.file_name().to_str().and_then(date_of) else {
            continue;
        };
        if !item.path().is_file() {
            continue;
        }
        snapshots.push(read_file(&item.path(), date)?);
    }
    Ok(History::new(snapshots))
}

/// The date a snapshot file name holds, when the name is exactly `YYYY-MM-DD.jsonl.gz`.
fn date_of(name: &str) -> Option<Date> {
    let stem = name.strip_suffix(SUFFIX)?;
    let parsed: civil::Date = stem.parse().ok()?;
    (parsed.to_string() == stem).then(|| Date::from(parsed))
}

/// Reads one snapshot file dated `date`.
fn read_file(path: &Path, date: Date) -> Result<Snapshot, HistoryError> {
    let file = File::open(path).map_err(|source| HistoryError::Read {
        path: path.to_path_buf(),
        source,
    })?;
    let corrupt = |line: Option<usize>, reason: String| HistoryError::Corrupt {
        path: path.to_path_buf(),
        line,
        reason,
    };
    let reader = BufReader::new(GzDecoder::new(BufReader::new(file)));
    let mut rows = BTreeMap::new();
    for (index, text) in reader.lines().enumerate() {
        let number = index.saturating_add(1);
        let text = text.map_err(|error| corrupt(None, error.to_string()))?;
        let line: Line = serde_json::from_str(&text)
            .map_err(|error| corrupt(Some(number), error.to_string()))?;
        Slug::new(line.slug.as_str()).map_err(|error| corrupt(Some(number), error.to_string()))?;
        if rows.insert(line.slug.clone(), line.row).is_some() {
            return Err(corrupt(
                Some(number),
                format!("slug `{}` appears twice", line.slug),
            ));
        }
    }
    Ok(Snapshot { date, rows })
}
