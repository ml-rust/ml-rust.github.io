//! Atomic file replacement shared by every writer in the crate.

use std::ffi::OsString;
use std::fs::{self, File};
use std::io;
use std::path::{Path, PathBuf};

use thiserror::Error;

/// A step of an atomic write that fails.
#[derive(Debug, Error)]
#[error("{}: {source}", .path.display())]
pub(crate) struct AtomicError {
    /// The file or directory that fails. The temporary file for a create,
    /// write or sync error, the parent directory for a directory error, and
    /// the target for a rename error.
    pub(crate) path: PathBuf,
    /// The error from the operating system.
    pub(crate) source: io::Error,
}

/// Wraps an OS error as an [`AtomicError`] for `path`.
fn at(path: &Path) -> impl FnOnce(io::Error) -> AtomicError {
    let path = path.to_path_buf();
    move |source| AtomicError { path, source }
}

/// The temporary file beside `target`: `.<file name>.tmp` in the same directory.
fn temp_path(target: &Path) -> Result<PathBuf, AtomicError> {
    let Some(name) = target.file_name() else {
        return Err(AtomicError {
            path: target.to_path_buf(),
            source: io::Error::new(io::ErrorKind::InvalidInput, "the path has no file name"),
        });
    };
    let mut temp_name = OsString::from(".");
    temp_name.push(name);
    temp_name.push(".tmp");
    Ok(target.with_file_name(temp_name))
}

/// Writes `target` atomically.
///
/// The function creates the parent directory, creates `.<file name>.tmp`
/// beside `target`, runs `fill` on it, syncs it, and renames it over
/// `target`, so a reader never sees a partial file. A failing step removes the
/// temporary file and leaves `target` unchanged.
///
/// # Errors
///
/// Returns an [`AtomicError`] that names the failing path.
pub(crate) fn write_atomic(
    target: &Path,
    fill: impl FnOnce(&mut File) -> io::Result<()>,
) -> Result<(), AtomicError> {
    let temp = temp_path(target)?;
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent).map_err(at(parent))?;
    }
    let mut file = File::create(&temp).map_err(at(&temp))?;
    let filled = fill(&mut file).and_then(|()| file.sync_all());
    drop(file);
    let result = filled
        .map_err(at(&temp))
        .and_then(|()| fs::rename(&temp, target).map_err(at(target)));
    if result.is_err() {
        let _ = fs::remove_file(&temp);
    }
    result
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use super::*;

    fn scratch() -> tempfile::TempDir {
        tempfile::tempdir_in(concat!(env!("CARGO_MANIFEST_DIR"), "/../../target"))
            .expect("scratch directory")
    }

    #[test]
    fn writes_then_replaces_the_target() {
        let dir = scratch();
        let target = dir.path().join("out.txt");
        write_atomic(&target, |file| file.write_all(b"one")).expect("first write");
        assert_eq!(fs::read(&target).expect("read"), b"one");
        write_atomic(&target, |file| file.write_all(b"two")).expect("second write");
        assert_eq!(fs::read(&target).expect("read"), b"two");
        assert!(!dir.path().join(".out.txt.tmp").exists());
    }

    #[test]
    fn a_failing_fill_leaves_no_temp_file_and_the_old_target() {
        let dir = scratch();
        let target = dir.path().join("out.txt");
        write_atomic(&target, |file| file.write_all(b"old")).expect("first write");
        let error = write_atomic(&target, |file| {
            file.write_all(b"partial")?;
            Err(io::Error::other("fill fails"))
        })
        .expect_err("fill fails");
        assert_eq!(error.path, dir.path().join(".out.txt.tmp"));
        assert_eq!(fs::read(&target).expect("read"), b"old");
        assert!(!dir.path().join(".out.txt.tmp").exists());
    }

    #[test]
    fn a_failing_fill_creates_no_target() {
        let dir = scratch();
        let target = dir.path().join("out.txt");
        write_atomic(&target, |_| Err(io::Error::other("fill fails"))).expect_err("fill fails");
        assert!(!target.exists());
        assert!(!dir.path().join(".out.txt.tmp").exists());
    }

    #[test]
    fn a_failing_rename_names_the_target_and_removes_the_temp_file() {
        let dir = scratch();
        let target = dir.path().join("out");
        fs::create_dir(&target).expect("directory at the target");
        let error = write_atomic(&target, |file| file.write_all(b"x")).expect_err("rename fails");
        assert_eq!(error.path, target);
        assert!(!dir.path().join(".out.tmp").exists());
    }

    #[test]
    fn creates_the_parent_directory() {
        let dir = scratch();
        let target = dir.path().join("a").join("b").join("out.txt");
        write_atomic(&target, |file| file.write_all(b"x")).expect("write");
        assert_eq!(fs::read(&target).expect("read"), b"x");
    }

    #[test]
    fn a_path_without_a_file_name_is_an_error() {
        let error = write_atomic(Path::new("/"), |_| Ok(())).expect_err("no file name");
        assert_eq!(error.source.kind(), io::ErrorKind::InvalidInput);
    }
}
