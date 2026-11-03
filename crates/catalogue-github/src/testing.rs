//! Test doubles: a scripted transport and a pacer that never waits.
//!
//! Built for tests only, behind the `testing` feature.

use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
use std::io::Cursor;

use catalogue_signals::http::{Headers, HttpError, Pacer, Response, Transport};

/// The value recorded in place of an `Authorization` header.
pub const REDACTED: &str = "[redacted]";

/// The HTTP method of a scripted route.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Method {
    /// `GET`.
    Get,
    /// `POST`.
    Post,
    /// `PATCH`.
    Patch,
    /// `DELETE`.
    Delete,
}

/// One canned response.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reply {
    /// The HTTP status code.
    pub status: u16,
    /// The response headers.
    pub headers: Vec<(String, String)>,
    /// The body.
    pub body: Vec<u8>,
}

impl Reply {
    /// A reply with `status` and `body` and no header.
    pub fn new(status: u16, body: impl Into<Vec<u8>>) -> Self {
        Self {
            status,
            headers: Vec::new(),
            body: body.into(),
        }
    }

    /// Adds the header `name: value`.
    #[must_use]
    pub fn header(mut self, name: &str, value: &str) -> Self {
        self.headers.push((name.to_owned(), value.to_owned()));
        self
    }
}

/// One request the transport received.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Recorded {
    /// The method.
    pub method: Method,
    /// The URL.
    pub url: String,
    /// The extra headers. An `Authorization` value reads [`REDACTED`].
    pub headers: Vec<(String, String)>,
    /// The body. A GET has none.
    pub body: Vec<u8>,
}

impl Recorded {
    /// The first value of header `name`, matched without regard to case.
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(candidate, _)| candidate.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }

    /// The body parsed as JSON, or `None` when it is not JSON.
    pub fn json(&self) -> Option<serde_json::Value> {
        serde_json::from_slice(&self.body).ok()
    }
}

/// A [`Transport`] that answers from a route table keyed by method and URL.
///
/// A route holds a queue of replies. Each request takes the next one, and the
/// last one repeats. A request with no route fails with
/// [`HttpError::Request`]. Every request is recorded.
#[derive(Debug, Default)]
pub struct ScriptedTransport {
    routes: RefCell<HashMap<(Method, String), VecDeque<Reply>>>,
    recorded: RefCell<Vec<Recorded>>,
}

impl ScriptedTransport {
    /// A transport with no route.
    pub fn new() -> Self {
        Self::default()
    }

    /// Queues `reply` for `method` requests to `url`.
    #[must_use]
    pub fn route(mut self, method: Method, url: &str, reply: Reply) -> Self {
        self.routes
            .get_mut()
            .entry((method, url.to_owned()))
            .or_default()
            .push_back(reply);
        self
    }

    /// Every request so far, in order.
    pub fn requests(&self) -> Vec<Recorded> {
        self.recorded.borrow().clone()
    }

    /// The requests so far with `method`, in order.
    pub fn requests_with(&self, method: Method) -> Vec<Recorded> {
        self.recorded
            .borrow()
            .iter()
            .filter(|request| request.method == method)
            .cloned()
            .collect()
    }

    fn answer(
        &self,
        method: Method,
        url: &str,
        headers: &[(&str, String)],
        body: &[u8],
    ) -> Result<Response, HttpError> {
        self.recorded.borrow_mut().push(Recorded {
            method,
            url: url.to_owned(),
            headers: headers
                .iter()
                .map(|(name, value)| {
                    let value = if name.eq_ignore_ascii_case("authorization") {
                        REDACTED.to_owned()
                    } else {
                        value.clone()
                    };
                    ((*name).to_owned(), value)
                })
                .collect(),
            body: body.to_vec(),
        });
        let mut routes = self.routes.borrow_mut();
        let queue = routes.get_mut(&(method, url.to_owned()));
        let reply = match queue {
            Some(queue) if queue.len() > 1 => queue.pop_front(),
            Some(queue) => queue.front().cloned(),
            None => None,
        };
        let reply = reply.ok_or_else(|| HttpError::Request {
            url: url.to_owned(),
            source: format!("no scripted route for {method:?} {url}").into(),
        })?;
        Ok(Response {
            status: reply.status,
            headers: Headers::new(reply.headers),
            body: Box::new(Cursor::new(reply.body)),
        })
    }
}

impl Transport for ScriptedTransport {
    fn get(&self, url: &str, headers: &[(&str, String)]) -> Result<Response, HttpError> {
        self.answer(Method::Get, url, headers, &[])
    }

    fn post(
        &self,
        url: &str,
        headers: &[(&str, String)],
        body: &[u8],
    ) -> Result<Response, HttpError> {
        self.answer(Method::Post, url, headers, body)
    }

    fn patch(
        &self,
        url: &str,
        headers: &[(&str, String)],
        body: &[u8],
    ) -> Result<Response, HttpError> {
        self.answer(Method::Patch, url, headers, body)
    }

    fn delete(
        &self,
        url: &str,
        headers: &[(&str, String)],
        body: &[u8],
    ) -> Result<Response, HttpError> {
        self.answer(Method::Delete, url, headers, body)
    }
}

/// A [`Pacer`] that never waits and counts the waits it skips.
#[derive(Debug, Default)]
pub struct NoWait {
    /// The waits asked for so far.
    pub waits: usize,
}

impl Pacer for NoWait {
    fn wait(&mut self, _response_headers: &Headers) {
        self.waits += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_last_reply_repeats_and_authorization_is_redacted() {
        let transport = ScriptedTransport::new()
            .route(Method::Get, "u", Reply::new(500, ""))
            .route(Method::Get, "u", Reply::new(200, "ok"));
        let headers = [("Authorization", "Bearer t0k".to_owned())];
        let statuses: Vec<u16> = (0..3)
            .map(|_| transport.get("u", &headers).expect("routed").status)
            .collect();
        assert_eq!(statuses, [500, 200, 200]);
        let recorded = transport.requests();
        assert_eq!(recorded.len(), 3);
        assert_eq!(recorded[0].header("authorization"), Some(REDACTED));
    }

    #[test]
    fn a_request_without_a_route_fails_and_is_recorded() {
        let transport = ScriptedTransport::new();
        assert!(transport.delete("u", &[], b"").is_err());
        assert_eq!(transport.requests_with(Method::Delete).len(), 1);
    }
}
