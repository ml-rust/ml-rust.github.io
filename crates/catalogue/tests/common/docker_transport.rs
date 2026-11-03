//! A transport that answers the requests of an online `catalogue tiers` run with Docker Hub counts.

use std::io::Cursor;

use catalogue_signals::http::{Headers, HttpError, Response, Transport};

/// The Docker Hub repository URL of the `ml-rust/gamma` image.
pub const GAMMA_URL: &str = "https://hub.docker.com/v2/repositories/ml-rust/gamma/";

/// Serves a pull count at [`GAMMA_URL`], a missing repo for GraphQL and 404 for the rest.
pub struct DockerRouter {
    pull_count: u64,
}

impl DockerRouter {
    /// A router whose Docker Hub repo holds `pull_count` pulls.
    pub fn new(pull_count: u64) -> Self {
        Self { pull_count }
    }
}

fn reply(status: u16, body: &str) -> Result<Response, HttpError> {
    Ok(Response {
        status,
        headers: Headers::default(),
        body: Box::new(Cursor::new(body.as_bytes().to_vec())),
    })
}

impl Transport for DockerRouter {
    fn get(&self, url: &str, _headers: &[(&str, String)]) -> Result<Response, HttpError> {
        if url == GAMMA_URL {
            return reply(200, &format!(r#"{{"pull_count":{}}}"#, self.pull_count));
        }
        reply(404, "")
    }

    fn post(
        &self,
        _url: &str,
        _headers: &[(&str, String)],
        _body: &[u8],
    ) -> Result<Response, HttpError> {
        reply(200, r#"{"data":{"repository":null}}"#)
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
