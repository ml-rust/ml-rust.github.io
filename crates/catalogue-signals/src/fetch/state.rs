//! The `state.json` record beside a cached download.

use std::fs;
use std::io::{self, Write};
use std::path::Path;

use jiff::Timestamp;
use serde::{Deserialize, Serialize};

use super::error::FetchError;
use crate::atomic::write_atomic;
use crate::http::Headers;

/// What the fetcher knows about a cached file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DumpState {
    /// The `ETag` of the response that supplies the file.
    pub etag: Option<String>,
    /// The `Last-Modified` of that response.
    pub last_modified: Option<String>,
    /// The announced size of the file in bytes.
    pub content_length: Option<u64>,
    /// When the fetcher began the download, as an RFC 3339 timestamp.
    pub fetched_at: String,
    /// Whether the complete file holds the whole body described above. A
    /// `false` value means the body is still in the `.partial` file.
    pub complete: bool,
}

/// Builds the state of a new body from its response headers.
pub(super) fn from_headers(headers: &Headers) -> DumpState {
    DumpState {
        etag: headers.get("etag").map(str::to_owned),
        last_modified: headers.get("last-modified").map(str::to_owned),
        content_length: headers
            .get("content-length")
            .and_then(|value| value.trim().parse().ok()),
        fetched_at: Timestamp::now().to_string(),
        complete: false,
    }
}

/// Reads the state file. A missing file reads as `None`.
pub(super) fn read(path: &Path) -> Result<Option<DumpState>, FetchError> {
    match fs::read(path) {
        Ok(bytes) => serde_json::from_slice(&bytes)
            .map(Some)
            .map_err(|source| FetchError::State {
                path: path.to_path_buf(),
                source,
            }),
        Err(source) if source.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(source) => Err(FetchError::io(path)(source)),
    }
}

/// Writes the state file atomically through [`write_atomic`].
pub(super) fn write(state: &DumpState, path: &Path) -> Result<(), FetchError> {
    let mut json = serde_json::to_vec_pretty(state).map_err(|source| FetchError::State {
        path: path.to_path_buf(),
        source,
    })?;
    json.push(b'\n');
    write_atomic(path, |file| file.write_all(&json)).map_err(|error| FetchError::Io {
        path: error.path,
        source: error.source,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn state_round_trips_through_json() {
        let state = DumpState {
            etag: Some("\"abc\"".to_owned()),
            last_modified: None,
            content_length: Some(7),
            fetched_at: "2026-11-03T02:10:00Z".to_owned(),
            complete: false,
        };
        let json = serde_json::to_string(&state).expect("serializes");
        let back: DumpState = serde_json::from_str(&json).expect("parses");
        assert_eq!(back, state);
    }
}
