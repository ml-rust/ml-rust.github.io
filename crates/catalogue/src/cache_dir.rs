use std::ffi::OsString;
use std::path::PathBuf;

use thiserror::Error;

/// The directory name under the user cache directory.
const APP_DIR: &str = "ml-rust-catalogue";

/// No cache directory can be derived from the environment.
#[derive(Debug, Error)]
pub enum CacheDirError {
    /// Neither `XDG_CACHE_HOME` (absolute) nor `HOME` is usable.
    #[error(
        "cannot derive a cache directory: XDG_CACHE_HOME is unset or not absolute, and HOME is unset. Pass --cache-dir <DIR>"
    )]
    Unset,
}

/// Picks the default cache directory from the values of `XDG_CACHE_HOME` and `HOME`.
///
/// A non-empty absolute `xdg_cache_home` gives `<xdg_cache_home>/ml-rust-catalogue`.
/// Otherwise a non-empty `home` gives `<home>/.cache/ml-rust-catalogue`.
pub fn default_cache_dir(
    xdg_cache_home: Option<OsString>,
    home: Option<OsString>,
) -> Result<PathBuf, CacheDirError> {
    let xdg = xdg_cache_home
        .map(PathBuf::from)
        .filter(|path| path.is_absolute());
    if let Some(base) = xdg {
        return Ok(base.join(APP_DIR));
    }
    match home.filter(|value| !value.is_empty()) {
        Some(home) => Ok(PathBuf::from(home).join(".cache").join(APP_DIR)),
        None => Err(CacheDirError::Unset),
    }
}

/// Resolves the cache directory: `explicit` when given, else the environment default.
pub fn resolve_cache_dir(explicit: Option<PathBuf>) -> Result<PathBuf, CacheDirError> {
    match explicit {
        Some(path) => Ok(path),
        None => default_cache_dir(std::env::var_os("XDG_CACHE_HOME"), std::env::var_os("HOME")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn os(value: &str) -> Option<OsString> {
        Some(OsString::from(value))
    }

    #[test]
    fn absolute_xdg_cache_home_wins() {
        let dir = default_cache_dir(os("/x/cache"), os("/home/u")).expect("resolves");
        assert_eq!(dir, PathBuf::from("/x/cache/ml-rust-catalogue"));
    }

    #[test]
    fn relative_xdg_cache_home_falls_back_to_home() {
        let dir = default_cache_dir(os("cache"), os("/home/u")).expect("resolves");
        assert_eq!(dir, PathBuf::from("/home/u/.cache/ml-rust-catalogue"));
    }

    #[test]
    fn empty_xdg_cache_home_falls_back_to_home() {
        let dir = default_cache_dir(os(""), os("/home/u")).expect("resolves");
        assert_eq!(dir, PathBuf::from("/home/u/.cache/ml-rust-catalogue"));
    }

    #[test]
    fn home_alone_is_enough() {
        let dir = default_cache_dir(None, os("/home/u")).expect("resolves");
        assert_eq!(dir, PathBuf::from("/home/u/.cache/ml-rust-catalogue"));
    }

    #[test]
    fn neither_value_is_an_error_that_names_the_flag() {
        let error = default_cache_dir(None, os("")).expect_err("unset");
        assert!(error.to_string().contains("--cache-dir"));
        assert!(default_cache_dir(None, None).is_err());
    }
}
