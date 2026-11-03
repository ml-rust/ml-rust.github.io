//! The download state machine.

use std::fs::{self, File, OpenOptions};
use std::io::{BufWriter, Read, Write};
use std::path::{Path, PathBuf};

use super::error::FetchError;
use super::http::ReqwestTransport;
use super::state::{self, DumpState};
use super::transport::{Response, Transport, parse_content_range};

/// Where crates.io publishes the database dump.
const DUMP_URL: &str = "https://static.crates.io/db-dump.tar.gz";

const COMPLETE_NAME: &str = "db-dump.tar.gz";
const PARTIAL_NAME: &str = "db-dump.tar.gz.partial";
const STATE_NAME: &str = "state.json";
const STATE_TEMP_NAME: &str = "state.json.tmp";
/// Bytes buffered before a write to disk.
const WRITE_BUFFER: usize = 1 << 20;
/// Bytes read from the body per call.
const READ_CHUNK: usize = 64 << 10;

/// What a call to [`dump`] did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FetchOutcome {
    /// The server confirmed that the cached file is current.
    Unchanged {
        /// The cached dump file.
        path: PathBuf,
    },
    /// The dump file is new or completed.
    Downloaded {
        /// The cached dump file.
        path: PathBuf,
        /// The size of the file in bytes.
        bytes: u64,
    },
}

/// The directory under `cache_dir` that holds the dump files.
pub fn dump_dir(cache_dir: &Path) -> PathBuf {
    cache_dir.join("dump")
}

/// The path of the complete dump file under `cache_dir`.
pub fn dump_path(cache_dir: &Path) -> PathBuf {
    dump_dir(cache_dir).join(COMPLETE_NAME)
}

/// Downloads the crates.io database dump into `cache_dir`.
///
/// A complete cached file is revalidated with `If-None-Match`. An interrupted
/// download resumes with `Range` and `If-Range`. See [`dump_with`].
///
/// # Errors
///
/// Returns a [`FetchError`] when the client cannot start, a request fails, the
/// server answers with another status than 200, 206 or 304, the body ends
/// early, or a cache file cannot be written.
pub fn dump(cache_dir: &Path) -> Result<FetchOutcome, FetchError> {
    dump_with(&ReqwestTransport::new()?, DUMP_URL, cache_dir)
}

/// Downloads `url` into `cache_dir` through `transport`.
///
/// The directory `<cache_dir>/dump/` holds `db-dump.tar.gz` (complete),
/// `db-dump.tar.gz.partial` (in progress) and `state.json`. The body streams
/// to the partial file in chunks. After the body ends, the byte count must
/// equal the announced total. Then the partial file is synced and renamed over
/// the complete file, and `state.json` is replaced atomically.
///
/// # Errors
///
/// Returns a [`FetchError`] as [`dump`] does.
pub fn dump_with(
    transport: &dyn Transport,
    url: &str,
    cache_dir: &Path,
) -> Result<FetchOutcome, FetchError> {
    let dir = dump_dir(cache_dir);
    fs::create_dir_all(&dir).map_err(FetchError::io(&dir))?;
    let paths = Paths {
        complete: dir.join(COMPLETE_NAME),
        partial: dir.join(PARTIAL_NAME),
        state: dir.join(STATE_NAME),
        state_temp: dir.join(STATE_TEMP_NAME),
    };
    let job = Job {
        transport,
        url,
        paths: &paths,
    };
    match state::read(&paths.state)? {
        Some(state) if state.complete && exists(&paths.complete)? => job.revalidate(&state),
        Some(state) if !state.complete => job.resume(&state),
        _ => job.fresh(),
    }
}

struct Paths {
    complete: PathBuf,
    partial: PathBuf,
    state: PathBuf,
    state_temp: PathBuf,
}

struct Job<'a> {
    transport: &'a dyn Transport,
    url: &'a str,
    paths: &'a Paths,
}

impl Job<'_> {
    fn get(&self, headers: &[(&str, String)]) -> Result<Response, FetchError> {
        self.transport.get(self.url, headers)
    }

    fn status_error(&self, status: u16) -> FetchError {
        FetchError::Status {
            url: self.url.to_owned(),
            status,
        }
    }

    fn fresh(&self) -> Result<FetchOutcome, FetchError> {
        let response = self.get(&[])?;
        match response.status {
            200 => self.receive(response, 0),
            status => Err(self.status_error(status)),
        }
    }

    /// Sends a conditional request for a complete cached file.
    fn revalidate(&self, state: &DumpState) -> Result<FetchOutcome, FetchError> {
        let headers = match (&state.etag, &state.last_modified) {
            (Some(etag), _) => vec![("If-None-Match", etag.clone())],
            (None, Some(date)) => vec![("If-Modified-Since", date.clone())],
            (None, None) => return self.fresh(),
        };
        let response = self.get(&headers)?;
        match response.status {
            304 => Ok(FetchOutcome::Unchanged {
                path: self.paths.complete.clone(),
            }),
            200 => self.receive(response, 0),
            status => Err(self.status_error(status)),
        }
    }

    /// Continues the `.partial` file, or starts over when it cannot continue.
    fn resume(&self, state: &DumpState) -> Result<FetchOutcome, FetchError> {
        let len = match fs::metadata(&self.paths.partial) {
            Ok(meta) => meta.len(),
            Err(source) if source.kind() == std::io::ErrorKind::NotFound => 0,
            Err(source) => return Err(FetchError::io(&self.paths.partial)(source)),
        };
        let (Some(etag), Some(total)) = (&state.etag, state.content_length) else {
            return self.fresh();
        };
        if len == 0 || len > total {
            return self.fresh();
        }
        if len == total {
            let mut done = state.clone();
            self.commit(&mut done)?;
            return Ok(FetchOutcome::Downloaded {
                path: self.paths.complete.clone(),
                bytes: total,
            });
        }
        let response = self.get(&[
            ("Range", format!("bytes={len}-")),
            ("If-Range", etag.clone()),
        ])?;
        match response.status {
            206 => {
                let range = response.headers.get("content-range");
                match range.and_then(parse_content_range) {
                    Some((start, total)) if start == len => {
                        let mut next = state.clone();
                        next.content_length = Some(total);
                        self.store(response, len, next)
                    }
                    _ => Err(FetchError::ContentRange {
                        url: self.url.to_owned(),
                        value: range.unwrap_or("<missing>").to_owned(),
                        offset: len,
                    }),
                }
            }
            200 => self.receive(response, 0),
            status => Err(self.status_error(status)),
        }
    }

    /// Stores a `200` body from byte zero, with state taken from its headers.
    fn receive(&self, response: Response, offset: u64) -> Result<FetchOutcome, FetchError> {
        let state = state::from_headers(&response.headers);
        self.store(response, offset, state)
    }

    /// Streams the body into the partial file, checks its size, then commits.
    fn store(
        &self,
        mut response: Response,
        offset: u64,
        mut state: DumpState,
    ) -> Result<FetchOutcome, FetchError> {
        state.complete = false;
        state::write(&state, &self.paths.state, &self.paths.state_temp)?;
        let partial = &self.paths.partial;
        let file = if offset == 0 {
            File::create(partial)
        } else {
            OpenOptions::new().append(true).open(partial)
        }
        .map_err(FetchError::io(partial))?;
        let mut writer = BufWriter::with_capacity(WRITE_BUFFER, file);
        let mut total = offset;
        let mut chunk = vec![0_u8; READ_CHUNK];
        loop {
            let read = match response.body.read(&mut chunk) {
                Ok(0) => break,
                Ok(read) => read,
                Err(source) if source.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(source) => {
                    writer.flush().map_err(FetchError::io(partial))?;
                    return Err(FetchError::Body {
                        url: self.url.to_owned(),
                        source,
                    });
                }
            };
            writer
                .write_all(chunk.get(..read).unwrap_or_default())
                .map_err(FetchError::io(partial))?;
            total += read as u64;
        }
        writer.flush().map_err(FetchError::io(partial))?;
        drop(writer);
        if let Some(expected) = state.content_length
            && expected != total
        {
            return Err(FetchError::Incomplete {
                path: partial.clone(),
                received: total,
                expected,
            });
        }
        state.content_length = Some(total);
        self.commit(&mut state)?;
        Ok(FetchOutcome::Downloaded {
            path: self.paths.complete.clone(),
            bytes: total,
        })
    }

    /// Syncs the partial file, renames it over the complete file and records it.
    fn commit(&self, state: &mut DumpState) -> Result<(), FetchError> {
        let partial = &self.paths.partial;
        OpenOptions::new()
            .write(true)
            .open(partial)
            .and_then(|file| file.sync_all())
            .map_err(FetchError::io(partial))?;
        fs::rename(partial, &self.paths.complete).map_err(FetchError::io(&self.paths.complete))?;
        state.complete = true;
        state::write(state, &self.paths.state, &self.paths.state_temp)
    }
}

fn exists(path: &Path) -> Result<bool, FetchError> {
    match fs::metadata(path) {
        Ok(meta) => Ok(meta.is_file()),
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(source) => Err(FetchError::io(path)(source)),
    }
}
