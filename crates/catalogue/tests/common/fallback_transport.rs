//! A transport whose answers each test sets by URL or by repo, for the fallback tests.

use std::io::Cursor;

use catalogue_signals::http::{Headers, HttpError, Response, Transport};

/// The Homebrew URL of the `gamma` formula.
pub const HOMEBREW: &str = "formulae.brew.sh";
/// The Repology URL of the `gamma` project.
pub const REPOLOGY: &str = "repology.org";
/// The Docker Hub URL of the `ml-rust/gamma` image.
pub const DOCKER: &str = "hub.docker.com";
/// The GitHub release listing of `ml-rust/gamma`.
pub const RELEASES: &str = "ml-rust/gamma/releases";
/// The package download of `alpha-crate`.
pub const PACKAGE: &str = "static.crates.io";

/// The GraphQL answer for a repo GitHub holds.
pub fn found_repo(name: &str, stars: u32) -> String {
    format!(
        r#"{{"data":{{"repository":{{"nameWithOwner":"ml-rust/{name}","stargazerCount":{stars}}}}}}}"#
    )
}

/// The answer of one request.
#[derive(Clone)]
pub struct Answer {
    status: u16,
    body: String,
}

/// An answer with `status` and `body`.
pub fn answer(status: u16, body: &str) -> Answer {
    Answer {
        status,
        body: body.to_owned(),
    }
}

/// Answers each request by its rules. The first rule that matches wins.
///
/// A GraphQL request with no rule finds no repo. A GET with no rule answers 404.
#[derive(Default)]
pub struct Rules {
    every_request: Option<Answer>,
    repos: Vec<(String, Answer)>,
    urls: Vec<(String, Answer)>,
}

impl Rules {
    /// Answers every request with `status`.
    pub fn everything(status: u16) -> Self {
        Self {
            every_request: Some(answer(status, "")),
            ..Self::default()
        }
    }

    /// Answers the GraphQL request for the repo `name` with `answer`.
    pub fn repo(mut self, name: &str, answer: Answer) -> Self {
        self.repos.push((format!("\"{name}\""), answer));
        self
    }

    /// Answers a GET whose URL contains `part` with `answer`.
    pub fn url(mut self, part: &str, answer: Answer) -> Self {
        self.urls.push((part.to_owned(), answer));
        self
    }

    fn reply(answer: &Answer) -> Result<Response, HttpError> {
        Ok(Response {
            status: answer.status,
            headers: Headers::default(),
            body: Box::new(Cursor::new(answer.body.clone().into_bytes())),
        })
    }
}

impl Transport for Rules {
    fn get(&self, url: &str, _headers: &[(&str, String)]) -> Result<Response, HttpError> {
        if let Some(answer) = &self.every_request {
            return Self::reply(answer);
        }
        let rule = self
            .urls
            .iter()
            .find(|(part, _)| url.contains(part.as_str()));
        match rule {
            Some((_, answer)) => Self::reply(answer),
            None => Self::reply(&answer(404, "")),
        }
    }

    fn post(
        &self,
        _url: &str,
        _headers: &[(&str, String)],
        body: &[u8],
    ) -> Result<Response, HttpError> {
        if let Some(answer) = &self.every_request {
            return Self::reply(answer);
        }
        let text = String::from_utf8_lossy(body);
        let rule = self
            .repos
            .iter()
            .find(|(name, _)| text.contains(name.as_str()));
        match rule {
            Some((_, answer)) => Self::reply(answer),
            None => Self::reply(&answer(200, r#"{"data":{"repository":null}}"#)),
        }
    }

    fn patch(
        &self,
        _url: &str,
        _headers: &[(&str, String)],
        _body: &[u8],
    ) -> Result<Response, HttpError> {
        Self::reply(&answer(404, ""))
    }

    fn delete(
        &self,
        _url: &str,
        _headers: &[(&str, String)],
        _body: &[u8],
    ) -> Result<Response, HttpError> {
        Self::reply(&answer(404, ""))
    }
}
