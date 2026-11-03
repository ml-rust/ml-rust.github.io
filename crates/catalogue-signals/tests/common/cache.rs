//! A temporary cache directory for the download tests.

use std::path::PathBuf;

use tempfile::TempDir;

/// A temporary cache directory under the build directory.
pub fn cache() -> (TempDir, PathBuf) {
    let guard = TempDir::new_in(env!("CARGO_TARGET_TMPDIR")).expect("temp dir");
    let path = guard.path().to_path_buf();
    (guard, path)
}
