//! A transport for the link unit tests: routes by URL, records each request, needs no network.

use std::cell::RefCell;
use std::collections::HashMap;
use std::io::Cursor;

use flate2::Compression;
use flate2::write::GzEncoder;
use tar::{EntryType, Header};

use crate::http::{Headers, HttpError, Pacer, Response, Transport};

/// One canned answer.
#[derive(Clone)]
pub(super) enum Reply {
    /// An answer with a status, headers and a body.
    Answer {
        status: u16,
        headers: Vec<(String, String)>,
        body: Vec<u8>,
    },
    /// No response arrives.
    Refused,
}

impl Reply {
    /// A 200 with a text `body`.
    pub(super) fn ok(body: &str) -> Self {
        Self::bytes(body.as_bytes().to_vec())
    }

    /// A 200 with a binary `body`.
    pub(super) fn bytes(body: Vec<u8>) -> Self {
        Self::Answer {
            status: 200,
            headers: Vec::new(),
            body,
        }
    }

    /// An empty answer with `status`.
    pub(super) fn status(status: u16) -> Self {
        Self::Answer {
            status,
            headers: Vec::new(),
            body: Vec::new(),
        }
    }

    /// The same answer with one more header.
    pub(super) fn header(mut self, name: &str, value: &str) -> Self {
        if let Self::Answer { headers, .. } = &mut self {
            headers.push((name.to_owned(), value.to_owned()));
        }
        self
    }
}

/// One request: the URL and the headers.
type Call = (String, Vec<(String, String)>);

/// A transport that answers each URL from its routes and every other URL with 404.
#[derive(Default)]
pub(super) struct Host {
    routes: HashMap<String, Reply>,
    calls: RefCell<Vec<Call>>,
}

impl Host {
    /// Answers `url` with `reply`.
    pub(super) fn route(mut self, url: &str, reply: Reply) -> Self {
        self.routes.insert(url.to_owned(), reply);
        self
    }

    /// The URL of each request, in order.
    pub(super) fn urls(&self) -> Vec<String> {
        self.calls
            .borrow()
            .iter()
            .map(|(url, _)| url.clone())
            .collect()
    }

    /// The extra headers of each request, in order.
    pub(super) fn headers(&self) -> Vec<Vec<(String, String)>> {
        self.calls
            .borrow()
            .iter()
            .map(|(_, headers)| headers.clone())
            .collect()
    }

    fn reply(&self, url: &str, headers: &[(&str, String)]) -> Result<Response, HttpError> {
        self.calls.borrow_mut().push((
            url.to_owned(),
            headers
                .iter()
                .map(|(name, value)| ((*name).to_owned(), value.clone()))
                .collect(),
        ));
        match self.routes.get(url).cloned().unwrap_or(Reply::status(404)) {
            Reply::Answer {
                status,
                headers,
                body,
            } => Ok(Response {
                status,
                headers: Headers::new(headers),
                body: Box::new(Cursor::new(body)),
            }),
            Reply::Refused => Err(HttpError::Request {
                url: url.to_owned(),
                source: "refused".into(),
            }),
        }
    }
}

impl Transport for Host {
    fn get(&self, url: &str, headers: &[(&str, String)]) -> Result<Response, HttpError> {
        self.reply(url, headers)
    }

    fn post(
        &self,
        url: &str,
        headers: &[(&str, String)],
        _body: &[u8],
    ) -> Result<Response, HttpError> {
        self.reply(url, headers)
    }

    fn patch(
        &self,
        url: &str,
        headers: &[(&str, String)],
        _body: &[u8],
    ) -> Result<Response, HttpError> {
        self.reply(url, headers)
    }

    fn delete(
        &self,
        url: &str,
        headers: &[(&str, String)],
        _body: &[u8],
    ) -> Result<Response, HttpError> {
        self.reply(url, headers)
    }
}

/// Lets every request through at once.
pub(super) struct NoWait;

impl Pacer for NoWait {
    fn wait(&mut self, _response_headers: &Headers) {}
}

/// An empty cache directory under the build directory.
pub(super) fn temp_cache() -> tempfile::TempDir {
    tempfile::tempdir_in(concat!(env!("CARGO_MANIFEST_DIR"), "/../../target")).expect("temp dir")
}

/// The `.crate` bytes of `name` and `version`. `vcs_info` is the `.cargo_vcs_info.json` text.
pub(super) fn package(name: &str, version: &str, vcs_info: Option<&str>) -> Vec<u8> {
    let mut files = vec![("Cargo.toml", "[package]\n")];
    if let Some(json) = vcs_info {
        files.insert(0, (".cargo_vcs_info.json", json));
    }
    let mut builder = tar::Builder::new(GzEncoder::new(Vec::new(), Compression::fast()));
    for (file, contents) in files {
        let mut header = Header::new_gnu();
        header.set_entry_type(EntryType::Regular);
        header.set_size(contents.len() as u64);
        header.set_mode(0o644);
        builder
            .append_data(
                &mut header,
                format!("{name}-{version}/{file}"),
                contents.as_bytes(),
            )
            .expect("file entry");
    }
    builder
        .into_inner()
        .expect("tar stream")
        .finish()
        .expect("gzip stream")
}
