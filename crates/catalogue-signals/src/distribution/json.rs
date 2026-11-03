//! Parsing helpers shared by the distribution lookups.

use jiff::Timestamp;
use serde::de::DeserializeOwned;

use super::error::DistributionError;

/// Parses a response body as JSON.
pub(super) fn parse_json<T: DeserializeOwned>(
    api: &'static str,
    url: &str,
    bytes: &[u8],
) -> Result<T, DistributionError> {
    serde_json::from_slice(bytes).map_err(|source| DistributionError::Json {
        api,
        url: url.to_owned(),
        source,
    })
}

/// Parses an RFC 3339 timestamp from the field `field`.
pub(super) fn parse_timestamp(
    api: &'static str,
    url: &str,
    field: &'static str,
    text: &str,
) -> Result<Timestamp, DistributionError> {
    text.parse().map_err(|_| DistributionError::BadField {
        api,
        url: url.to_owned(),
        field,
        value: text.to_owned(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_timestamp_parses_and_a_bad_one_names_the_field() {
        let ok = parse_timestamp("A", "u", "last_updated", "2026-11-03T04:52:35.923567Z");
        assert!(ok.is_ok());
        let error = parse_timestamp("A", "u", "last_updated", "soon").expect_err("bad");
        assert!(error.to_string().contains("`last_updated`"), "{error}");
    }

    #[test]
    fn bad_json_names_the_api_and_the_url() {
        let error = parse_json::<Vec<u8>>("A", "https://x", b"<html>").expect_err("bad");
        assert!(
            error
                .to_string()
                .starts_with("A: the response from https://x"),
            "{error}"
        );
    }
}
