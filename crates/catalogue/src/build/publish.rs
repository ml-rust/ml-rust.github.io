//! Writes the built files into a fresh directory and swaps it in for `--out`.

use std::collections::BTreeSet;
use std::ffi::OsString;
use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};

use catalogue_site::OutputFile;

use super::error::BuildError;

/// Checks one site path: relative, `/`-separated, no empty, `.` or `..` segment.
fn check_path(path: &str) -> Result<PathBuf, BuildError> {
    let unsafe_path = || BuildError::UnsafePath {
        path: path.to_owned(),
    };
    if path.is_empty() || path.contains(['\\', '\0']) {
        return Err(unsafe_path());
    }
    if path
        .split('/')
        .any(|segment| segment.is_empty() || segment == "." || segment == "..")
    {
        return Err(unsafe_path());
    }
    let buf = PathBuf::from(path);
    if buf
        .components()
        .all(|part| matches!(part, Component::Normal(_)))
    {
        Ok(buf)
    } else {
        Err(unsafe_path())
    }
}

/// Checks every path of `files`: safe, and unique.
fn check_paths(files: &[OutputFile]) -> Result<Vec<PathBuf>, BuildError> {
    let mut seen = BTreeSet::new();
    let mut paths = Vec::with_capacity(files.len());
    for file in files {
        paths.push(check_path(&file.path)?);
        if !seen.insert(file.path.as_str()) {
            return Err(BuildError::DuplicatePath {
                path: file.path.clone(),
            });
        }
    }
    Ok(paths)
}

fn write_error(path: &Path) -> impl FnOnce(io::Error) -> BuildError {
    let path = path.to_path_buf();
    move |source| BuildError::Write { path, source }
}

/// A sibling of `out` named `.<name>.<suffix>`, in the same directory.
fn sibling(out: &Path, suffix: &str) -> Result<PathBuf, BuildError> {
    let Some(name) = out.file_name() else {
        return Err(BuildError::Out {
            path: out.to_path_buf(),
            reason: "the path has no directory name".to_owned(),
        });
    };
    let mut hidden = OsString::from(".");
    hidden.push(name);
    hidden.push(".");
    hidden.push(suffix);
    Ok(out.with_file_name(hidden))
}

/// Refuses an existing `out` that is not a directory, or that holds `protected`
/// input: replacing it deletes the data, the report or the history.
fn check_out(out: &Path, protected: &[&Path]) -> Result<(), BuildError> {
    let refuse = |reason: String| BuildError::Out {
        path: out.to_path_buf(),
        reason,
    };
    match fs::metadata(out) {
        Ok(meta) if meta.is_dir() => {}
        Ok(_) => return Err(refuse("it exists and is not a directory".to_owned())),
        Err(source) if source.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(source) => return Err(write_error(out)(source)),
    }
    let canonical = fs::canonicalize(out).map_err(write_error(out))?;
    for path in protected {
        let Ok(found) = fs::canonicalize(path) else {
            continue;
        };
        if found.starts_with(&canonical) {
            return Err(refuse(format!(
                "it holds {}, which the build reads and the replacement deletes",
                path.display()
            )));
        }
    }
    Ok(())
}

fn remove_dir_if_present(path: &Path) -> Result<(), BuildError> {
    match fs::remove_dir_all(path) {
        Ok(()) => Ok(()),
        Err(source) if source.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(source) => Err(write_error(path)(source)),
    }
}

fn write_files(root: &Path, files: &[OutputFile], paths: &[PathBuf]) -> Result<(), BuildError> {
    for (file, relative) in files.iter().zip(paths) {
        let target = root.join(relative);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(write_error(parent))?;
        }
        fs::write(&target, &file.bytes).map_err(write_error(&target))?;
    }
    Ok(())
}

/// Replaces the directory `out` with one that holds `files`.
///
/// - Checks every path first: relative, free of `..`, empty and `.` segments, unique.
/// - Refuses an `out` that is a file, or that holds a path in `protected`.
/// - Writes the files into `.<name>.staging` beside `out`, on the same disk.
/// - Renames `out` to `.<name>.old`, renames the staging directory to `out`,
///   then removes the old directory. A failed second rename restores `out`.
///
/// A failed run leaves `out` as it was.
///
/// # Errors
///
/// Returns [`BuildError::UnsafePath`], [`BuildError::DuplicatePath`],
/// [`BuildError::Out`] or [`BuildError::Write`].
pub(crate) fn replace_dir(
    out: &Path,
    files: &[OutputFile],
    protected: &[&Path],
) -> Result<(), BuildError> {
    let paths = check_paths(files)?;
    let staging = sibling(out, "staging")?;
    let old = sibling(out, "old")?;
    check_out(out, protected)?;
    if let Some(parent) = out.parent().filter(|parent| !parent.as_os_str().is_empty()) {
        fs::create_dir_all(parent).map_err(write_error(parent))?;
    }
    // A crash between the two renames leaves the previous site in `old` and no `out`.
    if old.exists() && !out.exists() {
        fs::rename(&old, out).map_err(write_error(out))?;
    }
    remove_dir_if_present(&staging)?;
    remove_dir_if_present(&old)?;
    fs::create_dir(&staging).map_err(write_error(&staging))?;
    if let Err(error) = write_files(&staging, files, &paths) {
        let _ = fs::remove_dir_all(&staging);
        return Err(error);
    }
    let had_old = out.exists();
    if had_old && let Err(source) = fs::rename(out, &old) {
        let _ = fs::remove_dir_all(&staging);
        return Err(write_error(out)(source));
    }
    if let Err(source) = fs::rename(&staging, out) {
        if had_old && let Err(restore) = fs::rename(&old, out) {
            return Err(BuildError::Rollback {
                out: out.to_path_buf(),
                old,
                source: restore,
            });
        }
        let _ = fs::remove_dir_all(&staging);
        return Err(write_error(out)(source));
    }
    remove_dir_if_present(&old)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch() -> tempfile::TempDir {
        tempfile::tempdir_in(concat!(env!("CARGO_MANIFEST_DIR"), "/../../target"))
            .expect("scratch directory")
    }

    fn file(path: &str, text: &str) -> OutputFile {
        OutputFile {
            path: path.to_owned(),
            bytes: text.as_bytes().to_vec(),
        }
    }

    #[test]
    fn a_crash_between_the_renames_restores_the_previous_site_first() {
        let dir = scratch();
        let out = dir.path().join("dist");
        let old = sibling(&out, "old").expect("sibling");
        fs::create_dir(&old).expect("old");
        fs::write(old.join("kept.html"), "previous").expect("file");
        // `a` as a file and as a directory fails the write, after the restore.
        let clash = [file("a", "x"), file("a/b", "y")];
        assert!(replace_dir(&out, &clash, &[]).is_err());
        assert_eq!(
            fs::read_to_string(out.join("kept.html")).expect("restored"),
            "previous"
        );
        assert!(!old.exists());
        replace_dir(&out, &[file("index.html", "new")], &[]).expect("build");
        assert!(!out.join("kept.html").exists());
    }

    #[test]
    fn safe_paths_pass() {
        assert!(check_path("index.html").is_ok());
        assert!(check_path("topics/a/index.html").is_ok());
    }

    #[test]
    fn unsafe_paths_are_refused() {
        for bad in [
            "",
            "/etc/passwd",
            "../x",
            "a/../x",
            "a//b",
            "./a",
            "a/./b",
            "a/",
            "a\\b",
            "a\0b",
        ] {
            let error = check_path(bad).expect_err(bad);
            assert!(matches!(error, BuildError::UnsafePath { .. }), "{bad}");
        }
    }

    #[test]
    fn a_repeated_path_is_refused() {
        let files = [file("a.html", "1"), file("a.html", "2")];
        let error = check_paths(&files).expect_err("duplicate");
        assert!(matches!(error, BuildError::DuplicatePath { .. }));
    }

    #[test]
    fn a_new_directory_is_created_with_nested_files() {
        let dir = scratch();
        let out = dir.path().join("site");
        replace_dir(&out, &[file("a/b.html", "x")], &[]).expect("writes");
        assert_eq!(fs::read_to_string(out.join("a/b.html")).expect("read"), "x");
        assert!(!dir.path().join(".site.staging").exists());
        assert!(!dir.path().join(".site.old").exists());
    }

    #[test]
    fn an_existing_directory_is_replaced_and_stale_files_go() {
        let dir = scratch();
        let out = dir.path().join("site");
        replace_dir(&out, &[file("old.html", "1"), file("keep.html", "1")], &[]).expect("first");
        replace_dir(&out, &[file("keep.html", "2")], &[]).expect("second");
        assert!(!out.join("old.html").exists());
        assert_eq!(
            fs::read_to_string(out.join("keep.html")).expect("read"),
            "2"
        );
        assert!(!dir.path().join(".site.old").exists());
    }

    #[test]
    fn an_unsafe_path_leaves_the_directory_untouched() {
        let dir = scratch();
        let out = dir.path().join("site");
        replace_dir(&out, &[file("a.html", "1")], &[]).expect("first");
        let error = replace_dir(&out, &[file("../escape", "x")], &[]).expect_err("unsafe");
        assert!(matches!(error, BuildError::UnsafePath { .. }));
        assert!(out.join("a.html").exists());
        assert!(!dir.path().join("escape").exists());
    }

    #[test]
    fn a_file_at_the_out_path_is_refused() {
        let dir = scratch();
        let out = dir.path().join("site");
        fs::write(&out, "x").expect("file");
        let error = replace_dir(&out, &[file("a.html", "1")], &[]).expect_err("file");
        assert!(matches!(error, BuildError::Out { .. }));
        assert!(out.is_file());
    }

    #[test]
    fn an_out_directory_that_holds_an_input_is_refused() {
        let dir = scratch();
        let out = dir.path().join("site");
        fs::create_dir_all(out.join("data")).expect("dirs");
        let error =
            replace_dir(&out, &[file("a.html", "1")], &[&out.join("data")]).expect_err("holds");
        assert!(matches!(error, BuildError::Out { .. }));
        assert!(out.join("data").exists());
    }

    #[test]
    fn a_path_without_a_name_is_refused() {
        let error = replace_dir(Path::new("/"), &[], &[]).expect_err("root");
        assert!(matches!(error, BuildError::Out { .. }));
    }
}
