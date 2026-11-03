//! The `reqwest` implementation of [`Transport`].

use std::time::Duration;

use reqwest::blocking::{Client, RequestBuilder};
use reqwest::header::{AUTHORIZATION, HeaderValue};

use super::error::HttpError;
use super::headers::Headers;
use super::transport::{Response, Transport};
use crate::agent::USER_AGENT;

/// Seconds to wait for a connection.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(30);
/// Seconds to wait for the response head and for each body read. The body as a
/// whole has no limit.
const READ_TIMEOUT: Duration = Duration::from_secs(120);

/// A blocking HTTPS client that sends the crate `User-Agent` on every request.
///
/// It sends the headers a call passes and no others, so an `Authorization`
/// header goes out only when the caller lists it.
#[derive(Debug, Clone)]
pub struct ReqwestTransport {
    client: Client,
}

impl ReqwestTransport {
    /// Builds the client.
    ///
    /// # Errors
    ///
    /// Returns [`HttpError::Client`] when the TLS backend cannot start.
    pub fn new() -> Result<Self, HttpError> {
        Self::with_timeout(READ_TIMEOUT)
    }

    /// Builds the client with `timeout` for the response head and for each body read.
    ///
    /// A slow API, such as a model provider, passes a longer wait than the default.
    ///
    /// # Errors
    ///
    /// Returns [`HttpError::Client`] when the TLS backend cannot start.
    pub fn with_timeout(timeout: Duration) -> Result<Self, HttpError> {
        let client = Client::builder()
            .user_agent(USER_AGENT)
            .connect_timeout(CONNECT_TIMEOUT)
            .timeout(timeout)
            .build()
            .map_err(|error| HttpError::Client {
                reason: error.to_string(),
            })?;
        Ok(Self { client })
    }

    /// Adds `headers` to `request`, sends it and wraps the response.
    fn send(
        &self,
        mut request: RequestBuilder,
        url: &str,
        headers: &[(&str, String)],
    ) -> Result<Response, HttpError> {
        for (name, value) in headers {
            request = request.header(*name, header_value(name, value)?);
        }
        let response = request.send().map_err(|error| HttpError::Request {
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

impl Transport for ReqwestTransport {
    fn get(&self, url: &str, headers: &[(&str, String)]) -> Result<Response, HttpError> {
        self.send(self.client.get(url), url, headers)
    }

    fn post(
        &self,
        url: &str,
        headers: &[(&str, String)],
        body: &[u8],
    ) -> Result<Response, HttpError> {
        self.send(self.client.post(url).body(body.to_vec()), url, headers)
    }

    fn patch(
        &self,
        url: &str,
        headers: &[(&str, String)],
        body: &[u8],
    ) -> Result<Response, HttpError> {
        self.send(self.client.patch(url).body(body.to_vec()), url, headers)
    }

    fn delete(
        &self,
        url: &str,
        headers: &[(&str, String)],
        body: &[u8],
    ) -> Result<Response, HttpError> {
        let request = self.client.delete(url);
        let request = if body.is_empty() {
            request
        } else {
            request.body(body.to_vec())
        };
        self.send(request, url, headers)
    }
}

/// Builds a header value. An `Authorization` value is marked sensitive, so the
/// client never prints it in debug output.
fn header_value(name: &str, value: &str) -> Result<HeaderValue, HttpError> {
    let mut built = HeaderValue::from_str(value).map_err(|_| HttpError::InvalidHeader {
        name: name.to_owned(),
    })?;
    if name.eq_ignore_ascii_case(AUTHORIZATION.as_str()) {
        built.set_sensitive(true);
    }
    Ok(built)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn authorization_values_are_sensitive() {
        let value = header_value("Authorization", "Bearer secret").expect("valid value");
        assert!(value.is_sensitive());
        assert!(!format!("{value:?}").contains("secret"));
    }

    #[test]
    fn other_values_are_not_sensitive() {
        let value = header_value("Accept", "application/json").expect("valid value");
        assert!(!value.is_sensitive());
    }

    #[test]
    fn invalid_values_name_the_header_only() {
        let error = header_value("Authorization", "Bearer bad\nsecret").expect_err("rejected");
        let text = error.to_string();
        assert!(text.contains("`Authorization`"));
        assert!(!text.contains("secret"));
    }
}
