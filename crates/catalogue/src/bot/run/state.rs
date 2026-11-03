//! The bot state directory and the state file, a cache of the last check per issue.

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsString;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use catalogue_signals::atomic::write_atomic;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::cache_dir::{CacheDirError, default_cache_dir};

/// The state file name inside the state directory.
pub const STATE_FILE: &str = "bot-state.json";

/// The state file version this build reads and writes.
pub const STATE_VERSION: u32 = 1;

/// The variable systemd sets from `StateDirectory=`.
pub const STATE_DIRECTORY_VAR: &str = "STATE_DIRECTORY";

/// The state directory: `configured` when set, else the first path in
/// `state_directory`, else the default cache directory from `xdg_cache_home`
/// and `home`.
///
/// # Errors
///
/// Returns [`CacheDirError`] when no source gives a directory.
pub fn state_dir(
    configured: Option<&Path>,
    state_directory: Option<OsString>,
    xdg_cache_home: Option<OsString>,
    home: Option<OsString>,
) -> Result<PathBuf, CacheDirError> {
    if let Some(dir) = configured {
        return Ok(dir.to_path_buf());
    }
    let from_systemd = state_directory
        .and_then(|value| std::env::split_paths(&value).find(|path| !path.as_os_str().is_empty()));
    match from_systemd {
        Some(dir) => Ok(dir),
        None => default_cache_dir(xdg_cache_home, home),
    }
}

/// What the bot last wrote for one issue.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IssueState {
    /// The id of the bot comment.
    pub comment_id: u64,
    /// The digest of the issue body the comment covers, in hex.
    pub body_sha256: String,
    /// The digest of the report the comment holds, in hex.
    pub report_sha256: String,
    /// The `main` commit the check read. `None` when unknown.
    pub main: Option<String>,
}

/// The state file in its JSON form.
#[derive(Serialize, Deserialize)]
struct StateFile {
    version: u32,
    issues: BTreeMap<String, IssueState>,
}

/// The version field alone, read before the rest.
#[derive(Deserialize)]
struct Version {
    version: u32,
}

/// The state file cannot be written.
#[derive(Debug, Error)]
#[error(
    "cannot write the bot state {}: {source}. Check that the state directory is writable, then rerun",
    .path.display()
)]
pub struct StateError {
    /// The file or directory that fails.
    pub path: PathBuf,
    /// The error from the operating system.
    pub source: io::Error,
}

/// The last check per issue number.
///
/// The state is a cache only. The marker in each bot comment is the durable record.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BotState {
    issues: BTreeMap<u64, IssueState>,
}

impl BotState {
    /// Reads the state file at `path`.
    ///
    /// A missing file reads as empty. A file that cannot be read, is not
    /// valid, or has an unknown version also reads as empty, with a warning.
    pub fn load(path: &Path) -> (Self, Option<String>) {
        let warn = |reason: String| {
            let warning = format!(
                "ignoring the bot state {}: {reason}. The run starts from the bot comments",
                path.display()
            );
            (Self::default(), Some(warning))
        };
        let text = match fs::read_to_string(path) {
            Ok(text) => text,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return (Self::default(), None);
            }
            Err(error) => return warn(error.to_string()),
        };
        match serde_json::from_str::<Version>(&text) {
            Ok(found) if found.version == STATE_VERSION => {}
            Ok(found) => return warn(format!("version {} is unknown", found.version)),
            Err(error) => return warn(error.to_string()),
        }
        let file: StateFile = match serde_json::from_str(&text) {
            Ok(file) => file,
            Err(error) => return warn(error.to_string()),
        };
        let mut issues = BTreeMap::new();
        for (key, entry) in file.issues {
            let Ok(number) = key.parse::<u64>() else {
                return warn(format!("issue key `{key}` is not a number"));
            };
            issues.insert(number, entry);
        }
        (Self { issues }, None)
    }

    /// Writes the state to `path` atomically.
    ///
    /// # Errors
    ///
    /// Returns a [`StateError`] when the directory, the temporary file or the
    /// rename fails.
    pub fn save(&self, path: &Path) -> Result<(), StateError> {
        let file = StateFile {
            version: STATE_VERSION,
            issues: self
                .issues
                .iter()
                .map(|(number, entry)| (number.to_string(), entry.clone()))
                .collect(),
        };
        write_atomic(path, |out| {
            serde_json::to_writer_pretty(&mut *out, &file).map_err(io::Error::from)?;
            out.write_all(b"\n")
        })
        .map_err(|error| StateError {
            path: error.path,
            source: error.source,
        })
    }

    /// The state of issue `number`.
    pub fn get(&self, number: u64) -> Option<&IssueState> {
        self.issues.get(&number)
    }

    /// Records `entry` for issue `number`.
    pub fn insert(&mut self, number: u64, entry: IssueState) {
        self.issues.insert(number, entry);
    }

    /// Drops every issue not in `keep`.
    pub fn retain(&mut self, keep: &BTreeSet<u64>) {
        self.issues.retain(|number, _| keep.contains(number));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn os(value: &str) -> Option<OsString> {
        Some(OsString::from(value))
    }

    #[test]
    fn the_config_wins_then_systemd_then_the_cache() {
        let configured = Path::new("/srv/state");
        assert_eq!(
            state_dir(Some(configured), os("/var/lib/a"), None, os("/home/u")).expect("dir"),
            PathBuf::from("/srv/state")
        );
        assert_eq!(
            state_dir(None, os("/var/lib/a:/var/lib/b"), None, os("/home/u")).expect("dir"),
            PathBuf::from("/var/lib/a")
        );
        assert_eq!(
            state_dir(None, os(""), None, os("/home/u")).expect("dir"),
            PathBuf::from("/home/u/.cache/ml-rust-catalogue")
        );
        assert!(state_dir(None, None, None, None).is_err());
    }

    fn scratch() -> tempfile::TempDir {
        let base = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target");
        fs::create_dir_all(&base).expect("target dir");
        tempfile::tempdir_in(base).expect("scratch directory")
    }

    fn entry() -> IssueState {
        IssueState {
            comment_id: 900,
            body_sha256: "a".repeat(64),
            report_sha256: "b".repeat(64),
            main: None,
        }
    }

    #[test]
    fn state_round_trips_and_prunes() {
        let dir = scratch();
        let path = dir.path().join(STATE_FILE);
        let (empty, warning) = BotState::load(&path);
        assert_eq!((empty, warning), (BotState::default(), None));
        let mut state = BotState::default();
        state.insert(12, entry());
        state.insert(13, entry());
        state.save(&path).expect("saves");
        let text = fs::read_to_string(&path).expect("reads");
        assert!(
            text.contains("\"version\": 1") && text.contains("\"12\""),
            "{text}"
        );
        let (loaded, warning) = BotState::load(&path);
        assert_eq!(warning, None);
        assert_eq!(loaded, state);
        state.retain(&BTreeSet::from([13]));
        assert!(state.get(12).is_none() && state.get(13).is_some());
    }

    #[test]
    fn a_bad_or_newer_file_starts_empty_with_a_warning() {
        let dir = scratch();
        let path = dir.path().join(STATE_FILE);
        for text in [
            "{\"version\": 2, \"issues\": {}}",
            "not json",
            "{\"version\": 1, \"issues\": {\"x\": {\"comment_id\": 1, \"body_sha256\": \"\", \"report_sha256\": \"\", \"main\": null}}}",
        ] {
            fs::write(&path, text).expect("write");
            let (state, warning) = BotState::load(&path);
            assert_eq!(state, BotState::default(), "{text}");
            assert!(
                warning.is_some_and(|w| w.contains("ignoring the bot state")),
                "{text}"
            );
        }
    }
}
