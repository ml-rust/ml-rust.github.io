use std::fmt;

use super::error::IdError;

/// The length of a full git commit SHA in hex digits.
const LEN: usize = 40;

/// A full git commit SHA: 40 lowercase hex digits.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CommitSha(String);

impl CommitSha {
    /// Checks `value` and wraps it. Uppercase digits are rejected, never folded.
    pub fn new(value: impl Into<String>) -> Result<Self, IdError> {
        let value = value.into();
        let valid = value.len() == LEN
            && value
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte));
        if valid {
            Ok(Self(value))
        } else {
            Err(IdError::new(
                "commit SHA",
                &value,
                "a commit SHA is 40 lowercase hex digits",
            ))
        }
    }

    /// The SHA text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for CommitSha {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_40_lowercase_hex_digits_pass() {
        let sha = "0123456789abcdef".repeat(3);
        let sha = &sha[..40];
        assert_eq!(CommitSha::new(sha).expect("valid").as_str(), sha);
        for bad in [
            sha.to_ascii_uppercase(),
            sha[..39].to_owned(),
            format!("{sha}0"),
            format!("g{}", &sha[1..]),
            String::new(),
        ] {
            assert!(CommitSha::new(bad.as_str()).is_err(), "{bad}");
        }
    }
}
