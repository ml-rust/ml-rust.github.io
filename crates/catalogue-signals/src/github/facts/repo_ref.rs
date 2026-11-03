//! The `owner/name` form of a repo.

use std::fmt;

use thiserror::Error;

/// A repo named by owner and name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepoRef {
    /// The owner login.
    pub owner: String,
    /// The repo name.
    pub name: String,
}

/// An argument that is not `owner/name`.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error(
    "`{input}` is not a repo in the form owner/name. Pass it as owner/name, for example rust-lang/rustlings"
)]
pub struct RepoRefError {
    /// The rejected argument.
    pub input: String,
}

impl RepoRef {
    /// Parses `owner/name`.
    ///
    /// An owner holds ASCII letters, digits and `-`. A name holds ASCII
    /// letters, digits, `-`, `_` and `.`, and is neither `.` nor `..`.
    ///
    /// # Errors
    ///
    /// Returns [`RepoRefError`] for any other input.
    pub fn parse(input: &str) -> Result<Self, RepoRefError> {
        let rejected = || RepoRefError {
            input: input.to_owned(),
        };
        let (owner, name) = input.split_once('/').ok_or_else(rejected)?;
        let owner_ok = !owner.is_empty()
            && owner
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-');
        let name_ok = !name.is_empty()
            && name != "."
            && name != ".."
            && name
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'));
        if owner_ok && name_ok {
            Ok(Self {
                owner: owner.to_owned(),
                name: name.to_owned(),
            })
        } else {
            Err(rejected())
        }
    }
}

impl fmt::Display for RepoRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.owner, self.name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_valid_pair_parses_and_displays() {
        let repo = RepoRef::parse("rust-lang/rustlings.rs_2").expect("parses");
        assert_eq!(repo.owner, "rust-lang");
        assert_eq!(repo.name, "rustlings.rs_2");
        assert_eq!(repo.to_string(), "rust-lang/rustlings.rs_2");
    }

    #[test]
    fn bad_inputs_are_rejected_with_the_input_named() {
        for input in [
            "", "x", "/", "a/", "/b", "a/b/c", "a b/c", "a/b c", "a_b/c", "a/..", "a/.", "é/x",
            "a/b?x=1",
        ] {
            let error = RepoRef::parse(input).expect_err(input);
            assert_eq!(error.input, input);
            assert!(error.to_string().contains(&format!("`{input}`")), "{error}");
        }
    }
}
