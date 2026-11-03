//! The lock that keeps two bot runs from writing at once.

use std::fs::{self, File, OpenOptions, TryLockError};
use std::path::{Path, PathBuf};

use super::error::RunError;

/// The lock file name inside the state directory.
pub const LOCK_FILE: &str = "bot.lock";

/// An exclusive lock on `<state dir>/bot.lock`. Dropping it releases the lock.
#[derive(Debug)]
pub struct StateLock {
    _file: File,
}

/// Takes the lock in `dir` without waiting. The function creates `dir` when absent.
///
/// # Errors
///
/// Returns [`RunError::Locked`] when another process holds the lock, and
/// [`RunError::Lock`] when the directory or the file cannot be opened.
pub fn take_lock(dir: &Path) -> Result<StateLock, RunError> {
    let path = dir.join(LOCK_FILE);
    let failed = |path: PathBuf| move |source| RunError::Lock { path, source };
    fs::create_dir_all(dir).map_err(failed(dir.to_path_buf()))?;
    let file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(&path)
        .map_err(failed(path.clone()))?;
    match file.try_lock() {
        Ok(()) => Ok(StateLock { _file: file }),
        Err(TryLockError::WouldBlock) => Err(RunError::Locked { path }),
        Err(TryLockError::Error(source)) => Err(RunError::Lock { path, source }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_second_lock_is_refused_until_the_first_drops() {
        let base = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target");
        fs::create_dir_all(&base).expect("target dir");
        let dir = tempfile::tempdir_in(base).expect("scratch directory");
        let state = dir.path().join("state");
        let first = take_lock(&state).expect("first lock");
        let error = take_lock(&state).expect_err("held");
        assert!(matches!(error, RunError::Locked { .. }), "{error}");
        drop(first);
        take_lock(&state).expect("free again");
    }
}
