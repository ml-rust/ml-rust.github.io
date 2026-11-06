//! The newest README of each entry, in one file beside the snapshots.
//!
//! Snapshots leave READMEs out, because a copy in every snapshot grows the history fast. A run
//! that reuses stored values reads them here. The file name is no calendar date, so
//! [`read_all`](super::read_all) ignores it.

use std::collections::BTreeMap;
use std::fs::File;
use std::io::{self, BufRead, BufReader, BufWriter, Write};
use std::path::Path;

use catalogue_core::Date;
use catalogue_core::date_text::plain;
use flate2::Compression;
use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use serde::{Deserialize, Serialize};

use super::error::HistoryError;
use crate::atomic::write_atomic;
use crate::link::RepoReadme;

/// The file that holds the store, inside the history directory.
pub const READMES_FILE: &str = "readmes.jsonl.gz";

/// What the newest README fetch of one entry found.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReadmeRecord {
    /// The date of the fetch.
    #[serde(serialize_with = "plain")]
    pub date: Date,
    /// The repo the fetch read.
    pub of: String,
    /// The README. `None` when the repo holds none, so a later run does not ask again for it.
    pub readme: Option<RepoReadme>,
}

/// One JSON line of the file.
#[derive(Serialize, Deserialize)]
struct Line {
    slug: String,
    #[serde(flatten)]
    record: ReadmeRecord,
}

/// The newest README record of each entry, by slug.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ReadmeStore {
    /// The records by slug.
    pub records: BTreeMap<String, ReadmeRecord>,
}

impl ReadmeStore {
    /// Reads the store of the history directory `dir`. An absent file reads as an empty store.
    ///
    /// # Errors
    ///
    /// Returns [`HistoryError::Read`] when the file cannot be opened, and
    /// [`HistoryError::Corrupt`] when it is not gzip or holds a line that is not a valid record.
    pub fn read(dir: &Path) -> Result<Self, HistoryError> {
        let path = dir.join(READMES_FILE);
        let file = match File::open(&path) {
            Ok(file) => file,
            Err(source) if source.kind() == io::ErrorKind::NotFound => return Ok(Self::default()),
            Err(source) => return Err(HistoryError::Read { path, source }),
        };
        let corrupt = |line: Option<usize>, reason: String| HistoryError::Corrupt {
            path: path.clone(),
            line,
            reason,
        };
        let mut records = BTreeMap::new();
        let reader = BufReader::new(GzDecoder::new(BufReader::new(file)));
        for (index, text) in reader.lines().enumerate() {
            let number = index.saturating_add(1);
            let text = text.map_err(|error| corrupt(None, error.to_string()))?;
            let line: Line = serde_json::from_str(&text)
                .map_err(|error| corrupt(Some(number), error.to_string()))?;
            records.insert(line.slug, line.record);
        }
        Ok(Self { records })
    }

    /// Replaces the store of the history directory `dir` atomically. Lines follow slug order, so
    /// equal records give equal bytes.
    ///
    /// # Errors
    ///
    /// Returns [`HistoryError::Write`] when the directory, the temporary file or the rename fails.
    pub fn write(&self, dir: &Path) -> Result<(), HistoryError> {
        write_atomic(&dir.join(READMES_FILE), |file| {
            let mut encoder = GzEncoder::new(BufWriter::new(file), Compression::default());
            for (slug, record) in &self.records {
                let line = Line {
                    slug: slug.clone(),
                    record: record.clone(),
                };
                serde_json::to_writer(&mut encoder, &line)?;
                encoder.write_all(b"\n")?;
            }
            encoder
                .finish()?
                .into_inner()
                .map(|_| ())
                .map_err(io::IntoInnerError::into_error)
        })
        .map_err(|error| HistoryError::Write {
            path: error.path,
            source: error.source,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::history::read::read_all;

    fn scratch() -> tempfile::TempDir {
        let base = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target");
        std::fs::create_dir_all(&base).expect("target dir");
        tempfile::tempdir_in(&base).expect("temp dir")
    }

    fn store() -> ReadmeStore {
        let date = Date::new(2026, 11, 1).expect("date");
        let with_text = ReadmeRecord {
            date,
            of: "o/a".to_owned(),
            readme: Some(RepoReadme {
                text: "# A".to_owned(),
                path: "README.md".to_owned(),
                branch: "main".to_owned(),
                truncated: false,
                source: "https://github.com/o/a/blob/main/README.md"
                    .parse()
                    .expect("url"),
            }),
        };
        let without = ReadmeRecord {
            date,
            of: "o/b".to_owned(),
            readme: None,
        };
        ReadmeStore {
            records: [("a".to_owned(), with_text), ("b".to_owned(), without)]
                .into_iter()
                .collect(),
        }
    }

    #[test]
    fn a_store_round_trips_and_equal_records_give_equal_bytes() {
        let dir = scratch();
        store().write(dir.path()).expect("writes");
        let first = std::fs::read(dir.path().join(READMES_FILE)).expect("reads");
        assert_eq!(ReadmeStore::read(dir.path()).expect("reads"), store());
        store().write(dir.path()).expect("writes again");
        let second = std::fs::read(dir.path().join(READMES_FILE)).expect("reads");
        assert_eq!(first, second);
    }

    #[test]
    fn an_absent_file_reads_as_empty_and_a_corrupt_file_is_an_error() {
        let dir = scratch();
        assert_eq!(
            ReadmeStore::read(dir.path()).expect("reads"),
            ReadmeStore::default()
        );
        std::fs::write(dir.path().join(READMES_FILE), b"not gzip").expect("writes");
        let error = ReadmeStore::read(dir.path()).expect_err("corrupt");
        assert!(matches!(error, HistoryError::Corrupt { .. }), "{error}");
    }

    #[test]
    fn the_snapshot_reader_ignores_the_file() {
        let dir = scratch();
        store().write(dir.path()).expect("writes");
        assert!(read_all(dir.path()).expect("reads").is_empty());
    }
}
