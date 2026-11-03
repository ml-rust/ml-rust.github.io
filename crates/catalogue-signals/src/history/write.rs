//! Writes one snapshot file.

use std::io::{self, BufWriter, Write};
use std::path::{Path, PathBuf};

use flate2::Compression;
use flate2::write::GzEncoder;
use serde::Serialize;

use super::error::HistoryError;
use super::model::{Row, SUFFIX, Snapshot};
use crate::atomic::write_atomic;

/// One JSON line of a snapshot file.
#[derive(Serialize)]
struct Line<'a> {
    slug: &'a str,
    #[serde(flatten)]
    row: &'a Row,
}

/// Writes `snapshot` to `<dir>/<date>.jsonl.gz` and returns the path.
///
/// The function creates `dir` when it is absent and writes to a temporary
/// file that it renames into place, so a reader never sees a partial file and
/// a failed write leaves no temporary file. A second write for the same date replaces the
/// file. Lines follow slug order.
///
/// # Errors
///
/// Returns [`HistoryError::Write`] when the directory, the temporary file or the
/// rename fails.
pub fn write(dir: &Path, snapshot: &Snapshot) -> Result<PathBuf, HistoryError> {
    let target = dir.join(format!("{}{SUFFIX}", snapshot.date));
    write_atomic(&target, |file| {
        let mut encoder = GzEncoder::new(BufWriter::new(file), Compression::default());
        for (slug, row) in &snapshot.rows {
            serde_json::to_writer(&mut encoder, &Line { slug, row })?;
            encoder.write_all(b"\n")?;
        }
        encoder
            .finish()?
            .into_inner()
            .map(|_| ())
            .map_err(io::IntoInnerError::into_error)
    })
    .map_err(|error| HistoryError::Write {
        path: error.path,
        source: error.source,
    })?;
    Ok(target)
}
