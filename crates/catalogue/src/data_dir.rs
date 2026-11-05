use std::ffi::OsStr;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use catalogue_core::{Catalogue, DataPlace, DataSources, Diagnostics, SourceFile};
use thiserror::Error;

/// A failure to read the data directory.
#[derive(Debug, Error)]
pub enum DataDirError {
    /// The data directory does not exist.
    #[error("data directory {} does not exist", .path.display())]
    Missing {
        /// The directory that was asked for.
        path: PathBuf,
    },
    /// The data path exists and is not a directory.
    #[error("data path {} is not a directory", .path.display())]
    NotADirectory {
        /// The path that was asked for.
        path: PathBuf,
    },
    /// The operating system refused a read.
    #[error("{}: {}", .path.display(), .source)]
    Io {
        /// The file or directory that failed.
        path: PathBuf,
        /// The error from the operating system.
        source: io::Error,
    },
}

/// Everything read from a data directory.
#[derive(Debug)]
pub struct DataDir {
    /// File contents, sorted by file name.
    pub sources: DataSources,
    /// Files that cannot be read as data, such as a file without the `.toml` extension.
    pub diagnostics: Diagnostics,
}

/// Reads `entries/*.toml`, `judgments/*.toml`, `maintainers.toml`,
/// `topics.toml`, `categories.toml`, `synonyms.toml`, `discovery.toml`,
/// `thresholds.toml`, `deprecated.toml`, `backends.toml` and `showcase.toml` from `root`.
///
/// A missing `entries/` or `judgments/` directory reads as empty. A missing
/// optional file reads as absent.
pub fn read_data_dir(root: &Path) -> Result<DataDir, DataDirError> {
    match fs::metadata(root) {
        Ok(meta) if meta.is_dir() => {}
        Ok(_) => {
            return Err(DataDirError::NotADirectory {
                path: root.to_path_buf(),
            });
        }
        Err(source) if source.kind() == io::ErrorKind::NotFound => {
            return Err(DataDirError::Missing {
                path: root.to_path_buf(),
            });
        }
        Err(source) => return Err(io_error(root, source)),
    }
    let mut diagnostics = Diagnostics::new();
    let mut sources = DataSources::default();
    for place in DataPlace::LISTS {
        for file in read_toml_dir(&root.join(place.name()), &mut diagnostics)? {
            sources.put(place, file);
        }
    }
    for place in DataPlace::FILES {
        if let Some(file) = read_optional(&root.join(place.name()))? {
            sources.put(place, file);
        }
    }
    Ok(DataDir {
        sources,
        diagnostics,
    })
}

/// Reads and checks the data directory at `root`.
///
/// The inner result holds the checked catalogue, or every diagnostic from the
/// directory listing and the data rules.
pub fn load_catalogue(root: &Path) -> Result<Result<Catalogue, Diagnostics>, DataDirError> {
    let dir = read_data_dir(root)?;
    let mut diagnostics = dir.diagnostics;
    Ok(match Catalogue::load(dir.sources) {
        Ok(catalogue) if diagnostics.is_empty() => Ok(catalogue),
        Ok(_) => Err(diagnostics),
        Err(found) => {
            diagnostics.extend(found);
            Err(diagnostics)
        }
    })
}

fn io_error(path: &Path, source: io::Error) -> DataDirError {
    DataDirError::Io {
        path: path.to_path_buf(),
        source,
    }
}

fn read_source(path: &Path) -> Result<SourceFile, DataDirError> {
    let contents = fs::read_to_string(path).map_err(|source| io_error(path, source))?;
    Ok(source_file(path, contents))
}

fn source_file(path: &Path, contents: String) -> SourceFile {
    SourceFile {
        path: path.display().to_string(),
        contents,
    }
}

fn read_optional(path: &Path) -> Result<Option<SourceFile>, DataDirError> {
    match fs::read_to_string(path) {
        Ok(contents) => Ok(Some(source_file(path, contents))),
        Err(source) if source.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(source) => Err(io_error(path, source)),
    }
}

fn read_toml_dir(
    dir: &Path,
    diagnostics: &mut Diagnostics,
) -> Result<Vec<SourceFile>, DataDirError> {
    let listing = match fs::read_dir(dir) {
        Ok(listing) => listing,
        Err(source) if source.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(source) => return Err(io_error(dir, source)),
    };
    let mut paths = Vec::new();
    for item in listing {
        let item = item.map_err(|source| io_error(dir, source))?;
        paths.push(item.path());
    }
    paths.sort();
    let mut files = Vec::new();
    for path in paths {
        let is_toml_file = path.is_file()
            && path
                .file_name()
                .and_then(OsStr::to_str)
                .is_some_and(DataPlace::is_record_name);
        if is_toml_file {
            files.push(read_source(&path)?);
        } else {
            diagnostics.push(path.display().to_string(), "not a .toml file");
        }
    }
    Ok(files)
}
