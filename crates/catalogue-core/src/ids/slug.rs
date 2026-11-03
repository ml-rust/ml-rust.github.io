use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use super::error::IdError;

const MAX_LEN: usize = 64;

/// A lowercase kebab-case identifier, 1 to 64 characters.
///
/// Matches `^[a-z0-9]+(-[a-z0-9]+)*$`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Slug(String);

impl Slug {
    /// Checks `value` and wraps it.
    pub fn new(value: impl Into<String>) -> Result<Self, IdError> {
        let value = value.into();
        check(&value)?;
        Ok(Self(value))
    }

    /// The slug text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

fn check(value: &str) -> Result<(), IdError> {
    let err = |rule: &'static str| IdError::new("slug", value, rule);
    if value.is_empty() || value.len() > MAX_LEN {
        return Err(err("length must be 1 to 64 characters"));
    }
    let allowed = |b: u8| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-';
    if !value.bytes().all(allowed) {
        return Err(err("only a-z, 0-9 and `-` are allowed"));
    }
    if value.starts_with('-') || value.ends_with('-') || value.contains("--") {
        return Err(err("`-` must sit between two words"));
    }
    Ok(())
}

impl TryFrom<String> for Slug {
    type Error = IdError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<Slug> for String {
    fn from(slug: Slug) -> Self {
        slug.0
    }
}

impl FromStr for Slug {
    type Err = IdError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::new(s)
    }
}

impl fmt::Display for Slug {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_valid_slugs() {
        let long = "a".repeat(64);
        for ok in ["a", "tensors", "gpu-compute", "x1-y2", "0", long.as_str()] {
            assert!(Slug::new(ok).is_ok(), "{ok}");
        }
    }

    #[test]
    fn rejects_invalid_slugs() {
        let too_long = "a".repeat(65);
        for bad in [
            "",
            "Upper",
            "under_score",
            "-a",
            "a-",
            "a--b",
            "a b",
            "é",
            too_long.as_str(),
        ] {
            assert!(Slug::new(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn serde_round_trip_checks_the_rule() {
        #[derive(Debug, PartialEq, Serialize, Deserialize)]
        struct Wrap {
            slug: Slug,
        }
        let parsed: Wrap = toml::from_str("slug = \"gpu-compute\"").expect("valid slug");
        assert_eq!(parsed.slug.as_str(), "gpu-compute");
        assert_eq!(
            toml::to_string(&parsed).expect("serializes"),
            "slug = \"gpu-compute\"\n"
        );
        assert!(toml::from_str::<Wrap>("slug = \"Bad\"").is_err());
    }

    #[test]
    fn displays_the_text() {
        assert_eq!(
            Slug::new("ok-1").map(|s| s.to_string()),
            Ok("ok-1".to_owned())
        );
    }
}
