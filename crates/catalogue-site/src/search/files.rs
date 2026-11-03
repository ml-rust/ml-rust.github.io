//! The search index files of the site.

use oxidoc_search::Record;
use oxidoc_search::builder::{build_lexical, build_records, encode_lexical};

use crate::build::OutputFile;
use crate::error::SiteError;

/// The directory of the search index files, below the site root.
pub const SEARCH_DIR: &str = "search";

/// The index files of `records`, each under [`SEARCH_DIR`].
///
/// The files are `search-meta.bin`, one `search-chunk-N.bin` per term prefix,
/// and `search-records.bin` when a record carries tags, a boost or weights.
/// The same records give the same bytes.
pub(crate) fn search_files(records: &[Record]) -> Result<Vec<OutputFile>, SiteError> {
    let index = build_lexical(records);
    let extras = build_records(records);
    let files = encode_lexical(&index, Some(&extras)).map_err(SiteError::Search)?;
    Ok(files
        .into_iter()
        .map(|file| OutputFile {
            path: format!("{SEARCH_DIR}/{}", file.name),
            bytes: file.bytes,
        })
        .collect())
}
