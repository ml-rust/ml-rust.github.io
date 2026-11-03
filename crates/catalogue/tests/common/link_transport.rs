//! A transport that answers the requests of an online `catalogue tiers` run.

use std::cell::RefCell;
use std::io::Cursor;

use catalogue_signals::http::{Headers, HttpError, Response, Transport};

/// A gzip tar archive that holds no entry, so its package records no commit.
const EMPTY_PACKAGE: &[u8] = &[
    0x1f, 0x8b, 0x08, 0x00, 0x00, 0x00, 0x00, 0x00, 0x02, 0x03, 0x63, 0x60, 0x18, 0x05, 0xa3, 0x60,
    0x14, 0x8c, 0x54, 0x00, 0x00, 0x2e, 0xaf, 0xb5, 0xef, 0x00, 0x04, 0x00, 0x00,
];

/// The package URL the transport serves.
pub const PACKAGE_URL: &str = "https://static.crates.io/crates/alpha-crate/alpha-crate-1.3.0.crate";

/// Serves the empty package at [`PACKAGE_URL`], a missing repo for GraphQL and 404 for the rest.
#[derive(Default)]
pub struct Router {
    urls: RefCell<Vec<String>>,
}

impl Router {
    /// The URL of each request, in order.
    pub fn urls(&self) -> Vec<String> {
        self.urls.borrow().clone()
    }

    fn answer(&self, url: &str, status: u16, body: &[u8]) -> Result<Response, HttpError> {
        self.urls.borrow_mut().push(url.to_owned());
        Ok(Response {
            status,
            headers: Headers::default(),
            body: Box::new(Cursor::new(body.to_vec())),
        })
    }
}

impl Transport for Router {
    fn get(&self, url: &str, _headers: &[(&str, String)]) -> Result<Response, HttpError> {
        if url == PACKAGE_URL {
            return self.answer(url, 200, EMPTY_PACKAGE);
        }
        self.answer(url, 404, b"")
    }

    fn post(
        &self,
        url: &str,
        _headers: &[(&str, String)],
        _body: &[u8],
    ) -> Result<Response, HttpError> {
        self.answer(url, 200, br#"{"data":{"repository":null}}"#)
    }
}
