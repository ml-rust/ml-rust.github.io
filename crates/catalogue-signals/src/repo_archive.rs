//! The data files inside a gzip tar archive of the catalogue repo.
//!
//! GitHub serves the archive of a commit as one top directory that holds the
//! whole tree. [`read_repo_data`] keeps the files under `<top>/data/` that a
//! data directory read from disk keeps, mapped through [`DataPlace`]. It
//! reads no file and opens no connection.

use std::cell::Cell;
use std::io::{self, Read};

use catalogue_core::{DataPlace, DataSources, SourceFile};
use flate2::read::GzDecoder;
use thiserror::Error;

use crate::archive::{is_file, name_parts};

/// The data directory under the top directory of the archive.
pub const DATA_DIR: &str = "data";

/// The most bytes the archive unpacks to, every entry and header counted.
pub const MAX_UNPACKED_BYTES: u64 = 256 << 20;

/// The most bytes one data file holds.
pub const MAX_FILE_BYTES: u64 = 1 << 20;

/// A repo archive that cannot be read as the catalogue data.
#[derive(Debug, Error)]
pub enum RepoArchiveError {
    /// The bytes are not a readable gzip tar stream.
    #[error(
        "the repo archive is not a readable gzip tar stream: {reason}. Rerun later. If the error persists, report it"
    )]
    Read {
        /// What the reader reports.
        reason: String,
    },
    /// The archive unpacks past [`MAX_UNPACKED_BYTES`].
    #[error(
        "the repo archive unpacks past {limit} bytes. Nothing past the cap is read. Report it, so the cap can be checked"
    )]
    TooLarge {
        /// The cap in bytes.
        limit: u64,
    },
    /// A data file is larger than [`MAX_FILE_BYTES`].
    #[error(
        "data file `{entry}` holds {size} bytes, past the {limit}-byte cap. Shrink the file on main, then rerun"
    )]
    FileTooLarge {
        /// The archive entry path.
        entry: String,
        /// The entry size in bytes.
        size: u64,
        /// The cap in bytes.
        limit: u64,
    },
    /// An entry path is absolute, holds a `..` part or is not UTF-8.
    #[error(
        "archive entry `{entry}` has an absolute path, a `..` part or a name that is not UTF-8. The archive is refused. Report it"
    )]
    UnsafePath {
        /// The archive entry path, lossily decoded.
        entry: String,
    },
    /// An entry lies outside the top directory of the first entry.
    #[error(
        "archive entry `{entry}` lies outside the top directory `{top}`. The archive is refused. Report it"
    )]
    SecondTop {
        /// The archive entry path.
        entry: String,
        /// The top directory of the first entry.
        top: String,
    },
    /// An entry under `data/` is a symbolic or hard link.
    #[error(
        "archive entry `{entry}` is a link. Every file under data/ must be a regular file. Replace the link on main, then rerun"
    )]
    Link {
        /// The archive entry path.
        entry: String,
    },
    /// A data file is not valid UTF-8.
    #[error("data file `{entry}` is not valid UTF-8. Fix the file on main, then rerun")]
    NotUtf8 {
        /// The archive entry path.
        entry: String,
    },
    /// The archive holds no data file.
    #[error(
        "the repo archive holds no file under data/. Check that the commit is on the catalogue repo, then rerun"
    )]
    NoData,
}

/// Reads the data files of a repo archive from its gzip tar bytes in `reader`.
///
/// Every entry must share one top directory. The reader keeps regular files
/// at `<top>/data/<path>` where [`DataPlace::of`] places `<path>`, and skips
/// every other entry. Each kept file gets the path `data/<path>`. Lists are
/// sorted by path.
///
/// # Errors
///
/// Returns a [`RepoArchiveError`] when the stream is not gzip tar, unpacks past
/// [`MAX_UNPACKED_BYTES`], holds an unsafe path, a second top directory, a link
/// under `data/`, a data file past [`MAX_FILE_BYTES`] or not UTF-8, or no data
/// file.
pub fn read_repo_data(reader: impl Read) -> Result<DataSources, RepoArchiveError> {
    let hit = Cell::new(false);
    let capped = Capped {
        inner: GzDecoder::new(reader),
        left: MAX_UNPACKED_BYTES,
        hit: &hit,
    };
    let mut archive = tar::Archive::new(capped);
    let fail = |error: io::Error| read_error(&hit, &error);
    let mut sources = DataSources::default();
    let mut top: Option<String> = None;
    let mut kept = 0_usize;
    for entry in archive.entries().map_err(fail)? {
        let mut entry = entry.map_err(fail)?;
        let kind = entry.header().entry_type();
        if kind.is_pax_global_extensions() {
            continue;
        }
        let path = entry.path().map_err(fail)?.into_owned();
        let label = path.to_string_lossy().into_owned();
        let Some(parts) = name_parts(&path) else {
            return Err(RepoArchiveError::UnsafePath { entry: label });
        };
        let Some((first, rest)) = parts.split_first() else {
            continue;
        };
        let top_dir = top.get_or_insert_with(|| (*first).to_owned());
        if top_dir.as_str() != *first {
            return Err(RepoArchiveError::SecondTop {
                entry: label,
                top: top_dir.clone(),
            });
        }
        let Some((dir, inside)) = rest.split_first() else {
            continue;
        };
        if *dir != DATA_DIR {
            continue;
        }
        if kind.is_symlink() || kind.is_hard_link() {
            return Err(RepoArchiveError::Link { entry: label });
        }
        if !is_file(kind) {
            continue;
        }
        let Some(place) = DataPlace::of(inside) else {
            continue;
        };
        let size = entry.size();
        if size > MAX_FILE_BYTES {
            return Err(too_large(label, size));
        }
        let contents = read_file(&mut entry, &label, &hit)?;
        sources.put(
            place,
            SourceFile {
                path: format!("{DATA_DIR}/{}", inside.join("/")),
                contents,
            },
        );
        kept = kept.saturating_add(1);
    }
    if kept == 0 {
        return Err(RepoArchiveError::NoData);
    }
    sources.sort();
    Ok(sources)
}

/// Reads the text of one data file entry named `label`.
fn read_file(
    entry: &mut impl Read,
    label: &str,
    hit: &Cell<bool>,
) -> Result<String, RepoArchiveError> {
    let mut bytes = Vec::new();
    entry
        .take(MAX_FILE_BYTES.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|error| read_error(hit, &error))?;
    let size = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
    if size > MAX_FILE_BYTES {
        return Err(too_large(label.to_owned(), size));
    }
    String::from_utf8(bytes).map_err(|_| RepoArchiveError::NotUtf8 {
        entry: label.to_owned(),
    })
}

/// The error for data file `entry` of `size` bytes, past [`MAX_FILE_BYTES`].
fn too_large(entry: String, size: u64) -> RepoArchiveError {
    RepoArchiveError::FileTooLarge {
        entry,
        size,
        limit: MAX_FILE_BYTES,
    }
}

/// The error for a failed read: [`RepoArchiveError::TooLarge`] when the cap
/// stopped it, [`RepoArchiveError::Read`] otherwise.
fn read_error(hit: &Cell<bool>, error: &io::Error) -> RepoArchiveError {
    if hit.get() {
        RepoArchiveError::TooLarge {
            limit: MAX_UNPACKED_BYTES,
        }
    } else {
        RepoArchiveError::Read {
            reason: error.to_string(),
        }
    }
}

/// A reader that fails once more than `left` bytes pass, and sets `hit`.
struct Capped<'a, R> {
    inner: R,
    left: u64,
    hit: &'a Cell<bool>,
}

impl<R: Read> Read for Capped<'_, R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let read = self.inner.read(buf)?;
        let count = u64::try_from(read).unwrap_or(u64::MAX);
        match self.left.checked_sub(count) {
            Some(left) => {
                self.left = left;
                Ok(read)
            }
            None => {
                self.hit.set(true);
                Err(io::Error::other("the archive unpacks past the cap"))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use flate2::Compression;
    use flate2::write::GzEncoder;
    use tar::{Builder, EntryType, Header};

    use super::*;

    /// One archive entry: the path, the type and the bytes.
    struct Item<'a>(&'a str, EntryType, &'a [u8]);

    fn file<'a>(path: &'a str, bytes: &'a [u8]) -> Item<'a> {
        Item(path, EntryType::Regular, bytes)
    }

    /// A gzip tar archive of `items`. Paths go in the raw header, so unsafe ones survive.
    fn archive(items: &[Item<'_>]) -> Vec<u8> {
        let mut builder = Builder::new(Vec::new());
        for Item(path, kind, bytes) in items {
            let mut header = Header::new_gnu();
            let name = path.as_bytes();
            header.as_old_mut().name[..name.len()].copy_from_slice(name);
            header.set_entry_type(*kind);
            header.set_size(if kind.is_file() {
                bytes.len() as u64
            } else {
                0
            });
            header.set_mode(0o644);
            if kind.is_symlink() {
                header.set_link_name("x").expect("link name");
            }
            header.set_cksum();
            let data: &[u8] = if kind.is_file() { bytes } else { &[] };
            builder.append(&header, data).expect("append");
        }
        let tar = builder.into_inner().expect("tar");
        let mut gz = GzEncoder::new(Vec::new(), Compression::fast());
        gz.write_all(&tar).expect("gzip");
        gz.finish().expect("gzip")
    }

    #[test]
    fn keeps_the_data_files_of_the_top_directory() {
        let bytes = archive(&[
            Item("o-r-abc1234/", EntryType::Directory, b""),
            file("o-r-abc1234/README.md", b"readme"),
            file("o-r-abc1234/data/topics.toml", b"t = 1\n"),
            file("o-r-abc1234/data/entries/b.toml", b"b"),
            file("o-r-abc1234/data/entries/a.toml", b"a"),
            file("o-r-abc1234/data/entries/notes.txt", b"x"),
            file("o-r-abc1234/data/judgments/7.toml", b"j"),
            file("o-r-abc1234/data/LICENSE", b"cc"),
            file("o-r-abc1234/crates/data/topics.toml", b"wrong"),
        ]);
        let sources = read_repo_data(bytes.as_slice()).expect("reads");
        let entries: Vec<(&str, &str)> = sources
            .entries
            .iter()
            .map(|f| (f.path.as_str(), f.contents.as_str()))
            .collect();
        assert_eq!(
            entries,
            [("data/entries/a.toml", "a"), ("data/entries/b.toml", "b")]
        );
        assert_eq!(sources.judgments.len(), 1);
        let topics = sources.topics.expect("topics");
        assert_eq!(topics.path, "data/topics.toml");
        assert_eq!(topics.contents, "t = 1\n");
        assert_eq!(sources.maintainers, None);
    }

    #[test]
    fn unsafe_paths_and_links_are_refused() {
        let parent = archive(&[file("o-r-1/data/../../etc/x.toml", b"x")]);
        assert!(matches!(
            read_repo_data(parent.as_slice()),
            Err(RepoArchiveError::UnsafePath { .. })
        ));
        let absolute = archive(&[file("/o-r-1/data/topics.toml", b"x")]);
        assert!(matches!(
            read_repo_data(absolute.as_slice()),
            Err(RepoArchiveError::UnsafePath { .. })
        ));
        let link = archive(&[
            file("o-r-1/data/topics.toml", b"x"),
            Item("o-r-1/data/entries/a.toml", EntryType::Symlink, b""),
        ]);
        assert!(matches!(
            read_repo_data(link.as_slice()),
            Err(RepoArchiveError::Link { .. })
        ));
        let outside_link = archive(&[
            Item("o-r-1/docs/a", EntryType::Symlink, b""),
            file("o-r-1/data/topics.toml", b"x"),
        ]);
        assert!(read_repo_data(outside_link.as_slice()).is_ok());
    }

    #[test]
    fn a_second_top_directory_is_refused() {
        let bytes = archive(&[
            file("o-r-1/data/topics.toml", b"x"),
            file("evil/data/entries/a.toml", b"x"),
        ]);
        assert!(matches!(
            read_repo_data(bytes.as_slice()),
            Err(RepoArchiveError::SecondTop { .. })
        ));
    }

    #[test]
    fn large_and_binary_files_are_refused() {
        let big = vec![b'a'; usize::try_from(MAX_FILE_BYTES).expect("fits") + 1];
        let bytes = archive(&[file("o-r-1/data/topics.toml", &big)]);
        assert!(matches!(
            read_repo_data(bytes.as_slice()),
            Err(RepoArchiveError::FileTooLarge { .. })
        ));
        let bytes = archive(&[file("o-r-1/data/topics.toml", &[0xff, 0xfe])]);
        assert!(matches!(
            read_repo_data(bytes.as_slice()),
            Err(RepoArchiveError::NotUtf8 { .. })
        ));
    }

    #[test]
    fn an_archive_without_data_or_gzip_is_an_error() {
        let bytes = archive(&[file("o-r-1/README.md", b"x")]);
        assert!(matches!(
            read_repo_data(bytes.as_slice()),
            Err(RepoArchiveError::NoData)
        ));
        assert!(matches!(
            read_repo_data(&b"not gzip"[..]),
            Err(RepoArchiveError::Read { .. })
        ));
    }

    #[test]
    fn the_unpacked_size_is_capped() {
        let hit = Cell::new(false);
        let mut capped = Capped {
            inner: &[0_u8; 10][..],
            left: 4,
            hit: &hit,
        };
        let mut out = Vec::new();
        assert!(capped.read_to_end(&mut out).is_err());
        assert!(hit.get());
        assert!(matches!(
            read_error(&hit, &io::Error::other("x")),
            RepoArchiveError::TooLarge { .. }
        ));
    }
}
