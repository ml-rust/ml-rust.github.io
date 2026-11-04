//! A transport whose answers each lead test sets by repo or by URL. It records every request.

use std::cell::RefCell;
use std::io::Cursor;

use catalogue_signals::http::{Headers, HttpError, Response, Transport};
use serde_json::Value;

/// The GraphQL answer for a repo GitHub does not hold.
const GONE: &str = r#"{"data":{"repository":null},"errors":[{"type":"NOT_FOUND","path":["repository"],"message":"x"}]}"#;

/// Answers each request by its rules. The first rule that matches wins.
///
/// A GraphQL request with no rule finds no repo. A GET with no rule answers 404.
#[derive(Default)]
pub struct Script {
    every_request: Option<u16>,
    repos: Vec<(String, u16, String)>,
    urls: Vec<(String, u16, String)>,
    requests: RefCell<Vec<String>>,
}

impl Script {
    /// Answers every request with `status`.
    pub fn everything(status: u16) -> Self {
        Self {
            every_request: Some(status),
            ..Self::default()
        }
    }

    /// Answers the GraphQL request for `repo`, written `owner/name`, with `status` and `body`.
    pub fn repo(mut self, repo: &str, status: u16, body: &str) -> Self {
        self.repos.push((repo.to_owned(), status, body.to_owned()));
        self
    }

    /// Answers a GET whose URL contains `part` with `status` and `body`.
    pub fn url(mut self, part: &str, status: u16, body: &str) -> Self {
        self.urls.push((part.to_owned(), status, body.to_owned()));
        self
    }

    /// Each request so far: `POST owner/name` or `GET url`.
    pub fn requests(&self) -> Vec<String> {
        self.requests.borrow().clone()
    }

    fn record(&self, request: String) {
        self.requests.borrow_mut().push(request);
    }
}

fn reply(status: u16, body: &str) -> Result<Response, HttpError> {
    Ok(Response {
        status,
        headers: Headers::default(),
        body: Box::new(Cursor::new(body.as_bytes().to_vec())),
    })
}

/// The `owner/name` the variables of a GraphQL body name.
fn requested_repo(body: &[u8]) -> String {
    let value: Value = serde_json::from_slice(body).expect("the body is JSON");
    let part = |key: &str| {
        value["variables"][key]
            .as_str()
            .expect("the variable is text")
            .to_owned()
    };
    format!("{}/{}", part("owner"), part("name"))
}

impl Transport for Script {
    fn get(&self, url: &str, _headers: &[(&str, String)]) -> Result<Response, HttpError> {
        self.record(format!("GET {url}"));
        if let Some(status) = self.every_request {
            return reply(status, "");
        }
        match self.urls.iter().find(|(part, _, _)| url.contains(part)) {
            Some((_, status, body)) => reply(*status, body),
            None => reply(404, ""),
        }
    }

    fn post(
        &self,
        _url: &str,
        _headers: &[(&str, String)],
        body: &[u8],
    ) -> Result<Response, HttpError> {
        let repo = requested_repo(body);
        self.record(format!("POST {repo}"));
        if let Some(status) = self.every_request {
            return reply(status, "");
        }
        match self.repos.iter().find(|(name, _, _)| *name == repo) {
            Some((_, status, answer)) => reply(*status, answer),
            None => reply(200, GONE),
        }
    }

    fn patch(
        &self,
        _url: &str,
        _headers: &[(&str, String)],
        _body: &[u8],
    ) -> Result<Response, HttpError> {
        reply(404, "")
    }

    fn delete(
        &self,
        _url: &str,
        _headers: &[(&str, String)],
        _body: &[u8],
    ) -> Result<Response, HttpError> {
        reply(404, "")
    }
}
