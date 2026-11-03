//! Helpers shared by the integration tests.

use std::fs;
use std::num::NonZeroU64;
use std::path::PathBuf;

use catalogue_bot::IssueMeta;
use catalogue_core::GithubLogin;

/// The directory of the fixture bodies.
pub fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
}

/// The text of the fixture body `name`, such as `submit.md`.
pub fn body(name: &str) -> String {
    let path = fixtures().join(name);
    fs::read_to_string(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

/// Issue `number`, opened by `author`.
pub fn meta(number: u64, author: &str) -> IssueMeta {
    IssueMeta {
        number: NonZeroU64::new(number).expect("non-zero issue number"),
        author: GithubLogin::new(author).expect("valid login"),
    }
}
