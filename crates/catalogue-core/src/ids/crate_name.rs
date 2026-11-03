use std::fmt;

use serde::{Deserialize, Serialize};

use super::error::IdError;

const MAX_LEN: usize = 64;

/// A crates.io package name.
///
/// ASCII letters, digits, `-` and `_`. The first character is a letter. The
/// length is 1 to 64 characters.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct CrateName(String);

impl CrateName {
    /// Checks `value` and wraps it.
    pub fn new(value: impl Into<String>) -> Result<Self, IdError> {
        let value = value.into();
        check(&value)?;
        Ok(Self(value))
    }

    /// The crate name text.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The form crates.io compares: ASCII lowercase with `_` read as `-`.
    pub fn normalized(&self) -> String {
        canonical_crate_name(&self.0)
    }
}

/// The form crates.io compares: ASCII lowercase with `_` read as `-`.
pub fn canonical_crate_name(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    push_canonical_crate_name(&mut out, name);
    out
}

/// Appends the [`canonical_crate_name`] form of `name` to `out`.
pub fn push_canonical_crate_name(out: &mut String, name: &str) {
    out.extend(name.chars().map(|c| {
        if c == '_' {
            '-'
        } else {
            c.to_ascii_lowercase()
        }
    }));
}

fn check(value: &str) -> Result<(), IdError> {
    let err = |rule: &'static str| IdError::new("crate name", value, rule);
    if value.is_empty() || value.len() > MAX_LEN {
        return Err(err("length must be 1 to 64 characters"));
    }
    if !value.starts_with(|c: char| c.is_ascii_alphabetic()) {
        return Err(err("the first character must be an ASCII letter"));
    }
    let allowed = |b: u8| b.is_ascii_alphanumeric() || b == b'-' || b == b'_';
    if !value.bytes().all(allowed) {
        return Err(err("only ASCII letters, digits, `-` and `_` are allowed"));
    }
    Ok(())
}

impl TryFrom<String> for CrateName {
    type Error = IdError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<CrateName> for String {
    fn from(name: CrateName) -> Self {
        name.0
    }
}

impl fmt::Display for CrateName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_valid_names() {
        let long = "a".repeat(64);
        for ok in [
            "a",
            "serde",
            "serde_json",
            "tokio-util",
            "A1",
            long.as_str(),
        ] {
            assert!(CrateName::new(ok).is_ok(), "{ok}");
        }
    }

    #[test]
    fn normalized_folds_case_and_underscores() {
        let name = CrateName::new("Candle_Core-x").expect("name");
        assert_eq!(name.normalized(), "candle-core-x");
    }

    #[test]
    fn rejects_invalid_names() {
        let too_long = "a".repeat(65);
        for bad in [
            "",
            "1abc",
            "-abc",
            "_abc",
            "a.b",
            "a b",
            "é",
            too_long.as_str(),
        ] {
            assert!(CrateName::new(bad).is_err(), "{bad}");
        }
    }
}
