//! The HTTP seam of the download.

use std::io::Read;

use super::error::FetchError;

/// Response headers, matched without regard to case.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Headers(Vec<(String, String)>);

impl Headers {
    /// Builds the headers from name and value pairs.
    pub fn new<N, V>(pairs: impl IntoIterator<Item = (N, V)>) -> Self
    where
        N: Into<String>,
        V: Into<String>,
    {
        Self(
            pairs
                .into_iter()
                .map(|(name, value)| (name.into(), value.into()))
                .collect(),
        )
    }

    /// The first value for `name`, or `None` when the header is absent.
    pub fn get(&self, name: &str) -> Option<&str> {
        self.0
            .iter()
            .find(|(candidate, _)| candidate.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }
}

/// One HTTP response with a streaming body.
pub struct Response {
    /// The HTTP status code.
    pub status: u16,
    /// The response headers.
    pub headers: Headers,
    /// The body, read as it arrives.
    pub body: Box<dyn Read>,
}

/// Sends GET requests. Redirects are followed inside the implementation.
pub trait Transport {
    /// Fetches `url` with the extra request `headers` as name and value pairs.
    ///
    /// # Errors
    ///
    /// Returns [`FetchError::Request`] when no response arrives.
    fn get(&self, url: &str, headers: &[(&str, String)]) -> Result<Response, FetchError>;
}

/// Parses `bytes <start>-<end>/<total>` into the start and the total.
pub(super) fn parse_content_range(value: &str) -> Option<(u64, u64)> {
    let range = value.trim().strip_prefix("bytes ")?;
    let (span, total) = range.split_once('/')?;
    let (start, _end) = span.split_once('-')?;
    Some((start.parse().ok()?, total.parse().ok()?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn header_lookup_ignores_case() {
        let headers = Headers::new([("ETag", "\"a\"")]);
        assert_eq!(headers.get("etag"), Some("\"a\""));
        assert_eq!(headers.get("content-length"), None);
    }

    #[test]
    fn content_range_gives_start_and_total() {
        assert_eq!(parse_content_range("bytes 100-199/200"), Some((100, 200)));
    }

    #[test]
    fn content_range_rejects_unknown_total_and_other_units() {
        assert_eq!(parse_content_range("bytes 100-199/*"), None);
        assert_eq!(parse_content_range("items 1-2/3"), None);
    }
}
