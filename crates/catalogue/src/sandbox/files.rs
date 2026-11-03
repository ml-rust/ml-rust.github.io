//! The list and result files of the sandbox check.

use std::fs::{File, OpenOptions};
use std::io::{self, BufReader, Write};
use std::path::{Path, PathBuf};

use catalogue_signals::atomic::write_atomic;
use catalogue_signals::sandbox::{BuildChecks, CheckItem, CheckRecord, SandboxError, read_lines};
use serde::de::DeserializeOwned;

use super::error::SandboxCommandError;

/// Reads one `T` per non-blank line of the file at `path`.
pub(super) fn read_file<T: DeserializeOwned>(path: &Path) -> Result<Vec<T>, SandboxError> {
    let file = File::open(path).map_err(|source| SandboxError::Read {
        path: path.to_path_buf(),
        source,
    })?;
    read_lines(BufReader::new(file), path)
}

/// Reads the result file of `catalogue sandbox check` at `path`.
///
/// # Errors
///
/// Returns [`SandboxError::Read`] when the file cannot be read and
/// [`SandboxError::Corrupt`] when a line is not a result.
pub fn read_build_checks(path: &Path) -> Result<BuildChecks, SandboxError> {
    Ok(BuildChecks::new(read_file::<CheckRecord>(path)?))
}

/// Writes `items` to `path` as JSON lines, replacing the file atomically.
pub(super) fn write_items(path: &Path, items: &[CheckItem]) -> Result<(), SandboxCommandError> {
    write_atomic(path, |file| {
        for item in items {
            serde_json::to_writer(&mut *file, item).map_err(io::Error::from)?;
            file.write_all(b"\n")?;
        }
        Ok(())
    })
    .map_err(|error| SandboxCommandError::Write {
        path: error.path,
        source: error.source,
    })
}

/// The result file, written one line per check so a stopped run keeps its results.
pub(super) struct ResultFile {
    file: File,
    path: PathBuf,
}

impl ResultFile {
    /// Creates or truncates the file at `path`.
    pub(super) fn create(path: &Path) -> Result<Self, SandboxCommandError> {
        let error = |source| SandboxCommandError::Write {
            path: path.to_path_buf(),
            source,
        };
        if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
            std::fs::create_dir_all(parent).map_err(error)?;
        }
        let file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(path)
            .map_err(error)?;
        Ok(Self {
            file,
            path: path.to_path_buf(),
        })
    }

    /// Appends `record` as one line and flushes it to disk.
    pub(super) fn append(&mut self, record: &CheckRecord) -> Result<(), SandboxCommandError> {
        let mut line = serde_json::to_vec(record).map_err(|error| self.error(error.into()))?;
        line.push(b'\n');
        self.file
            .write_all(&line)
            .and_then(|()| self.file.sync_data())
            .map_err(|source| self.error(source))
    }

    fn error(&self, source: io::Error) -> SandboxCommandError {
        SandboxCommandError::Write {
            path: self.path.clone(),
            source,
        }
    }
}

#[cfg(test)]
mod tests {
    use catalogue_core::Date;
    use catalogue_signals::sandbox::CheckResult;

    use super::*;

    fn scratch() -> tempfile::TempDir {
        let base = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target");
        std::fs::create_dir_all(&base).expect("target dir");
        tempfile::tempdir_in(base).expect("scratch directory")
    }

    #[test]
    fn results_written_line_by_line_read_back() {
        let dir = scratch();
        let path = dir.path().join("results").join("build-checks.jsonl");
        let record = CheckRecord {
            name: "alpha".to_owned(),
            version: "1.0.0".to_owned(),
            toolchain: "rustc 1.95.0".to_owned(),
            checked: Date::new(2026, 11, 2).expect("date"),
            result: CheckResult::Passes,
            detail: "cargo check passes".to_owned(),
        };
        let mut file = ResultFile::create(&path).expect("create");
        file.append(&record).expect("append");
        let checks = read_build_checks(&path).expect("read");
        assert_eq!(checks.get("alpha", "1.0.0"), Some(&record));
    }

    #[test]
    fn items_round_trip_and_a_missing_file_names_its_path() {
        let dir = scratch();
        let path = dir.path().join("items").join("list.jsonl");
        let items = vec![CheckItem {
            entry: "alpha".to_owned(),
            name: "alpha-core".to_owned(),
            version: "0.2.0".to_owned(),
        }];
        write_items(&path, &items).expect("write");
        assert_eq!(read_file::<CheckItem>(&path).expect("read"), items);
        let missing = dir.path().join("missing").join("none.jsonl");
        let error = read_build_checks(&missing).expect_err("missing");
        assert!(error.to_string().contains("none.jsonl"), "{error}");
    }
}
