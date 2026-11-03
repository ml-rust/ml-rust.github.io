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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn header_lookup_ignores_case() {
        let headers = Headers::new([("ETag", "\"a\"")]);
        assert_eq!(headers.get("etag"), Some("\"a\""));
        assert_eq!(headers.get("content-length"), None);
    }
}
