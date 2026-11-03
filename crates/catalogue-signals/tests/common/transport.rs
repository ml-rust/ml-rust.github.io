//! A scripted transport that replays canned responses and records requests.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::io::Cursor;

use catalogue_signals::http::{Headers, HttpError, Response, Transport};

/// One canned HTTP response.
pub struct FakeResponse {
    pub(crate) status: u16,
    pub(crate) headers: Vec<(String, String)>,
    pub(crate) body: Vec<u8>,
}

/// The URL and extra headers of one request.
pub(crate) type Call = (String, Vec<(String, String)>);

/// A transport that replays canned responses and records each request.
pub struct FakeTransport {
    responses: RefCell<VecDeque<FakeResponse>>,
    pub(crate) calls: RefCell<Vec<Call>>,
    pub(crate) posted: RefCell<Vec<Vec<u8>>>,
}

impl FakeTransport {
    pub fn new(responses: impl IntoIterator<Item = FakeResponse>) -> Self {
        Self {
            responses: RefCell::new(responses.into_iter().collect()),
            calls: RefCell::new(Vec::new()),
            posted: RefCell::new(Vec::new()),
        }
    }

    fn reply(&self, url: &str, headers: &[(&str, String)]) -> Result<Response, HttpError> {
        self.calls.borrow_mut().push((
            url.to_owned(),
            headers
                .iter()
                .map(|(name, value)| ((*name).to_owned(), value.clone()))
                .collect(),
        ));
        let next = self.responses.borrow_mut().pop_front();
        let fake = next.ok_or_else(|| HttpError::Request {
            url: url.to_owned(),
            source: "no canned response left".into(),
        })?;
        Ok(Response {
            status: fake.status,
            headers: Headers::new(fake.headers),
            body: Box::new(Cursor::new(fake.body)),
        })
    }
}

impl Transport for FakeTransport {
    fn get(&self, url: &str, headers: &[(&str, String)]) -> Result<Response, HttpError> {
        self.reply(url, headers)
    }

    fn post(
        &self,
        url: &str,
        headers: &[(&str, String)],
        body: &[u8],
    ) -> Result<Response, HttpError> {
        self.posted.borrow_mut().push(body.to_vec());
        self.reply(url, headers)
    }

    fn patch(
        &self,
        url: &str,
        headers: &[(&str, String)],
        body: &[u8],
    ) -> Result<Response, HttpError> {
        self.post(url, headers, body)
    }

    fn delete(
        &self,
        url: &str,
        headers: &[(&str, String)],
        body: &[u8],
    ) -> Result<Response, HttpError> {
        self.post(url, headers, body)
    }
}
