//! The downloads the catalogue keeps in its cache directory.

use std::path::{Path, PathBuf};

use super::download::{FetchOutcome, cached};
use super::error::FetchError;
use crate::http::{ReqwestTransport, Transport};

/// Where crates.io publishes the database dump.
const DUMP_URL: &str = "https://static.crates.io/db-dump.tar.gz";
const DUMP_NAME: &str = "db-dump.tar.gz";

/// Where GitHub serves the RustSec advisory database archive of the `main` branch.
const RUSTSEC_URL: &str = "https://github.com/rustsec/advisory-db/archive/refs/heads/main.tar.gz";
const RUSTSEC_NAME: &str = "advisory-db.tar.gz";

/// The directory under `cache_dir` that holds the dump files.
pub fn dump_dir(cache_dir: &Path) -> PathBuf {
    cache_dir.join("dump")
}

/// The path of the complete dump file under `cache_dir`.
pub fn dump_path(cache_dir: &Path) -> PathBuf {
    dump_dir(cache_dir).join(DUMP_NAME)
}

/// Downloads the crates.io database dump into `cache_dir`.
///
/// A complete cached file is revalidated with `If-None-Match`. An interrupted
/// download resumes with `Range` and `If-Range`. See [`cached`].
///
/// # Errors
///
/// Returns a [`FetchError`] when the client cannot start or [`cached`] fails.
pub fn dump(cache_dir: &Path) -> Result<FetchOutcome, FetchError> {
    dump_with(&ReqwestTransport::new()?, DUMP_URL, cache_dir)
}

/// Downloads the dump from `url` into `<cache_dir>/dump/db-dump.tar.gz` through `transport`.
///
/// # Errors
///
/// Returns a [`FetchError`] as [`cached`] does.
pub fn dump_with(
    transport: &dyn Transport,
    url: &str,
    cache_dir: &Path,
) -> Result<FetchOutcome, FetchError> {
    cached(transport, url, &dump_dir(cache_dir), DUMP_NAME)
}

/// The directory under `cache_dir` that holds the RustSec archive files.
pub fn rustsec_dir(cache_dir: &Path) -> PathBuf {
    cache_dir.join("rustsec")
}

/// The path of the complete RustSec archive under `cache_dir`.
pub fn rustsec_path(cache_dir: &Path) -> PathBuf {
    rustsec_dir(cache_dir).join(RUSTSEC_NAME)
}

/// Downloads the RustSec advisory database archive into `cache_dir`.
///
/// The server sends an `ETag`, so a complete cached file is revalidated with
/// `If-None-Match`. See [`cached`].
///
/// # Errors
///
/// Returns a [`FetchError`] when the client cannot start or [`cached`] fails.
pub fn rustsec(cache_dir: &Path) -> Result<FetchOutcome, FetchError> {
    rustsec_with(&ReqwestTransport::new()?, RUSTSEC_URL, cache_dir)
}

/// Downloads the archive from `url` into `<cache_dir>/rustsec/advisory-db.tar.gz` through `transport`.
///
/// # Errors
///
/// Returns a [`FetchError`] as [`cached`] does.
pub fn rustsec_with(
    transport: &dyn Transport,
    url: &str,
    cache_dir: &Path,
) -> Result<FetchOutcome, FetchError> {
    cached(transport, url, &rustsec_dir(cache_dir), RUSTSEC_NAME)
}
