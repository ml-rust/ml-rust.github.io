//! Private copies of the test key under the build directory.

use std::fs;
use std::path::{Path, PathBuf};

use tempfile::TempDir;

/// A scratch directory under the build directory.
pub fn scratch() -> TempDir {
    let base = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target");
    fs::create_dir_all(&base).expect("target dir");
    tempfile::tempdir_in(base).expect("scratch directory")
}

/// Copies the test key into `dir` as `app.pem` with unix mode `mode`.
pub fn key_copy(dir: &Path, mode: u32) -> PathBuf {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/test-app-key.pem");
    let path = dir.join("app.pem");
    fs::copy(fixture, &path).expect("copy key");
    set_mode(&path, mode);
    path
}

#[cfg(unix)]
fn set_mode(path: &Path, mode: u32) {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(mode)).expect("chmod");
}

#[cfg(not(unix))]
fn set_mode(_path: &Path, _mode: u32) {}
