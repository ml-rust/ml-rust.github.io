//! The entry files of `catalogue seed --write`.

use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use catalogue_signals::seed::Drafts;

use super::error::SeedError;

/// Writes `entries/<slug>.toml` under `data` for each draft and returns the paths.
///
/// The call serializes every draft and checks every path before it writes the
/// first file, so a draft that cannot be written leaves no file behind. It
/// refuses to overwrite: an existing file stops the call with an error that
/// names it.
pub fn write_drafts(data: &Path, drafts: &Drafts) -> Result<Vec<PathBuf>, SeedError> {
    let dir = data.join("entries");
    let mut files = Vec::with_capacity(drafts.entries.len());
    for (slug, entry) in &drafts.entries {
        let path = dir.join(format!("{slug}.toml"));
        if path.exists() {
            return Err(SeedError::Exists { path });
        }
        let text = toml::to_string(entry).map_err(|source| SeedError::Serialize {
            slug: slug.to_string(),
            source,
        })?;
        files.push((path, text));
    }
    if files.is_empty() {
        return Ok(Vec::new());
    }
    fs::create_dir_all(&dir).map_err(|source| io_error(&dir, source))?;
    let mut written = Vec::with_capacity(files.len());
    for (path, text) in files {
        create(&path, &text)?;
        written.push(path);
    }
    Ok(written)
}

/// Creates `path` and writes `text`. An existing file is an error.
fn create(path: &Path, text: &str) -> Result<(), SeedError> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|source| match source.kind() {
            io::ErrorKind::AlreadyExists => SeedError::Exists {
                path: path.to_path_buf(),
            },
            _ => io_error(path, source),
        })?;
    file.write_all(text.as_bytes())
        .map_err(|source| io_error(path, source))
}

fn io_error(path: &Path, source: io::Error) -> SeedError {
    SeedError::Io {
        path: path.to_path_buf(),
        source,
    }
}
