//! The `reqwest` implementation of [`Transport`].

use std::time::Duration;

use reqwest::blocking::Client;

use super::error::FetchError;
use super::transport::{Headers, Response, Transport};
use crate::agent::USER_AGENT;

/// Seconds to wait for a connection.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(30);
/// Seconds to wait for the response head and for each body read. The body as a
/// whole has no limit.
const READ_TIMEOUT: Duration = Duration::from_secs(120);

/// A blocking HTTPS client that sends [`USER_AGENT`] on every request.
#[derive(Debug, Clone)]
pub struct ReqwestTransport {
    client: Client,
}

impl ReqwestTransport {
    /// Builds the client.
    ///
    /// # Errors
    ///
    /// Returns [`FetchError::Client`] when the TLS backend cannot start.
    pub fn new() -> Result<Self, FetchError> {
        let client = Client::builder()
            .user_agent(USER_AGENT)
            .connect_timeout(CONNECT_TIMEOUT)
            .timeout(READ_TIMEOUT)
            .build()
            .map_err(|error| FetchError::Client {
                reason: error.to_string(),
            })?;
        Ok(Self { client })
    }
}

impl Transport for ReqwestTransport {
    fn get(&self, url: &str, headers: &[(&str, String)]) -> Result<Response, FetchError> {
        let mut request = self.client.get(url);
        for (name, value) in headers {
            request = request.header(*name, value);
        }
        let response = request.send().map_err(|error| FetchError::Request {
            url: url.to_owned(),
            source: Box::new(error),
        })?;
        let status = response.status().as_u16();
        let headers = Headers::new(response.headers().iter().filter_map(|(name, value)| {
            value
                .to_str()
                .ok()
                .map(|value| (name.as_str().to_owned(), value.to_owned()))
        }));
        Ok(Response {
            status,
            headers,
            body: Box::new(response),
        })
    }
}
