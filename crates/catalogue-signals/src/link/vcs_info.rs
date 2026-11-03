//! Reads `.cargo_vcs_info.json` from a `.crate` stream.

use std::io::{self, Read};

use catalogue_core::CommitSha;
use flate2::read::MultiGzDecoder;
use serde::Deserialize;
use thiserror::Error;

use crate::archive::{is_file, name_parts};

/// The most bytes read from `.cargo_vcs_info.json`.
pub const MAX_VCS_INFO: u64 = 64 << 10;

/// The file name Cargo writes into a package.
const FILE_NAME: &str = ".cargo_vcs_info.json";

/// What `.cargo_vcs_info.json` records.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VcsInfo {
    /// The commit the package was built from, as 40 lowercase hex digits.
    /// `None` when the file holds no usable `git.sha1`.
    pub sha1: Option<String>,
    /// Whether the working tree was dirty at packaging time.
    pub dirty: bool,
}

/// A `.crate` stream the reader rejects.
#[derive(Debug, Error)]
pub enum VcsInfoError {
    /// The stream is not a gzip tar archive, or it breaks off.
    #[error("the archive stream cannot be read: {0}")]
    Archive(#[from] io::Error),
    /// The file is longer than [`MAX_VCS_INFO`].
    #[error("`.cargo_vcs_info.json` is longer than 64 KiB")]
    TooLarge,
    /// The file is not the expected JSON.
    #[error("`.cargo_vcs_info.json` is not the expected JSON: {0}")]
    Json(#[from] serde_json::Error),
}

#[derive(Deserialize)]
struct Raw {
    git: Option<RawGit>,
}

#[derive(Deserialize)]
struct RawGit {
    sha1: Option<String>,
    dirty: Option<bool>,
}

/// Reads `<name>-<version>/.cargo_vcs_info.json` from the gzip tar stream `reader`.
///
/// Returns `None` when the package holds no such file. The reader stops at the
/// file and leaves the rest of the stream unread. A `git.sha1` that is not 40
/// hex digits reads as absent. A missing `git` object gives `sha1: None`.
///
/// # Errors
///
/// Returns a [`VcsInfoError`] when the stream is not a gzip tar archive, the
/// file is longer than [`MAX_VCS_INFO`], or the file is not the expected JSON.
pub fn read_vcs_info(
    reader: impl Read,
    name: &str,
    version: &str,
) -> Result<Option<VcsInfo>, VcsInfoError> {
    let directory = format!("{name}-{version}");
    let mut archive = tar::Archive::new(MultiGzDecoder::new(reader));
    for entry in archive.entries()? {
        let mut entry = entry?;
        if !is_file(entry.header().entry_type()) {
            continue;
        }
        let wanted = match name_parts(&entry.path()?).as_deref() {
            Some([dir, file]) => *dir == directory && *file == FILE_NAME,
            _ => false,
        };
        if !wanted {
            continue;
        }
        let mut bytes = Vec::new();
        (&mut entry)
            .take(MAX_VCS_INFO + 1)
            .read_to_end(&mut bytes)?;
        if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > MAX_VCS_INFO {
            return Err(VcsInfoError::TooLarge);
        }
        let raw: Raw = serde_json::from_slice(&bytes)?;
        let git = raw.git;
        let dirty = git.as_ref().and_then(|git| git.dirty).unwrap_or(false);
        let sha1 = git
            .and_then(|git| git.sha1)
            .map(|sha1| sha1.to_ascii_lowercase())
            .filter(|sha1| CommitSha::new(sha1.as_str()).is_ok());
        return Ok(Some(VcsInfo { sha1, dirty }));
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use flate2::Compression;
    use flate2::write::GzEncoder;
    use tar::{EntryType, Header};

    use super::*;

    const SHA: &str = "a866b336f14aa57a07f0d0be9f8762746e64ecb4";

    /// A gzip tar with a regular file for each `(path, contents)` pair.
    fn package(files: &[(&str, &str)]) -> Vec<u8> {
        let mut builder = tar::Builder::new(GzEncoder::new(Vec::new(), Compression::fast()));
        for (path, contents) in files {
            let mut header = Header::new_gnu();
            header.set_entry_type(EntryType::Regular);
            header.set_size(contents.len() as u64);
            header.set_mode(0o644);
            builder
                .append_data(&mut header, path, contents.as_bytes())
                .expect("file entry");
        }
        builder
            .into_inner()
            .expect("tar stream")
            .finish()
            .expect("gzip stream")
    }

    fn read(files: &[(&str, &str)]) -> Result<Option<VcsInfo>, VcsInfoError> {
        read_vcs_info(package(files).as_slice(), "serde", "1.0.228")
    }

    fn info(sha1: Option<&str>, dirty: bool) -> VcsInfo {
        VcsInfo {
            sha1: sha1.map(str::to_owned),
            dirty,
        }
    }

    #[test]
    fn the_commit_is_found_after_other_files() {
        let json = format!(r#"{{"git":{{"sha1":"{SHA}"}},"path_in_vcs":"serde"}}"#);
        let found = read(&[
            ("serde-1.0.228/Cargo.toml", "[package]"),
            ("serde-1.0.228/.cargo_vcs_info.json", json.as_str()),
        ])
        .expect("reads");
        assert_eq!(found, Some(info(Some(SHA), false)));
    }

    #[test]
    fn a_package_without_the_file_gives_none() {
        let files: &[(&str, &str)] = &[
            ("serde-1.0.228/Cargo.toml", "x"),
            ("other-1.0.0/.cargo_vcs_info.json", "{}"),
            ("serde-1.0.228/src/.cargo_vcs_info.json", "{}"),
        ];
        assert_eq!(read(files).expect("reads"), None);
    }

    #[test]
    fn a_file_without_a_commit_gives_no_sha1() {
        for json in [
            r#"{"path_in_vcs":""}"#,
            r#"{"git":{}}"#,
            r#"{"git":null}"#,
            r#"{"git":{"sha1":"abc"}}"#,
            r#"{"git":{"sha1":"zzzzb336f14aa57a07f0d0be9f8762746e64ecb4"}}"#,
        ] {
            let found = read(&[("serde-1.0.228/.cargo_vcs_info.json", json)]);
            assert_eq!(found.expect("reads"), Some(info(None, false)), "{json}");
        }
    }

    #[test]
    fn the_dirty_flag_and_an_uppercase_commit_are_read() {
        let json = format!(
            r#"{{"git":{{"sha1":"{}","dirty":true}}}}"#,
            SHA.to_uppercase()
        );
        let found = read(&[("serde-1.0.228/.cargo_vcs_info.json", json.as_str())]);
        assert_eq!(found.expect("reads"), Some(info(Some(SHA), true)));
    }

    #[test]
    fn an_oversized_file_is_rejected() {
        let padding = " ".repeat(usize::try_from(MAX_VCS_INFO).expect("fits"));
        let json = format!(r#"{{"git":{{"sha1":"{SHA}"}}}}{padding}"#);
        let found = read(&[("serde-1.0.228/.cargo_vcs_info.json", json.as_str())]);
        assert!(matches!(found, Err(VcsInfoError::TooLarge)), "{found:?}");
    }

    #[test]
    fn a_file_at_the_limit_is_read() {
        let head = format!(r#"{{"git":{{"sha1":"{SHA}"}}}}"#);
        let padding = " ".repeat(usize::try_from(MAX_VCS_INFO).expect("fits") - head.len());
        let json = format!("{head}{padding}");
        let found = read(&[("serde-1.0.228/.cargo_vcs_info.json", json.as_str())]);
        assert_eq!(found.expect("reads"), Some(info(Some(SHA), false)));
    }

    #[test]
    fn bad_json_and_a_bad_stream_are_errors() {
        let found = read(&[("serde-1.0.228/.cargo_vcs_info.json", "{")]);
        assert!(matches!(found, Err(VcsInfoError::Json(_))), "{found:?}");
        let found = read_vcs_info(&b"not gzip"[..], "serde", "1.0.228");
        assert!(matches!(found, Err(VcsInfoError::Archive(_))), "{found:?}");
    }
}
