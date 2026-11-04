//! Whether GitHub lists Rust among the languages of a repo.

use std::collections::BTreeMap;

use serde::de::IgnoredAny;

use super::decide::RUST;
use crate::github::{GithubError, RepoRef, parse_json, rest_headers};
use crate::http::{Paced, Pacer, Transport, percent_encode};

/// The repos endpoint of the GitHub API.
const ENDPOINT: &str = "https://api.github.com/repos";

/// Reads the language list of `repo` with one REST request.
///
/// A 404 answer reads as no Rust. `token` is sent as a bearer token. A 403 or 429 answer with a
/// wait hint is retried after the `pacer` waits.
///
/// # Errors
///
/// Returns a [`GithubError`] when no response arrives, the status is not 200 or 404, the body is
/// not a JSON object, or the rate limit stays after the retries.
pub(super) fn lists_rust(
    transport: &dyn Transport,
    token: &str,
    repo: &RepoRef,
    pacer: &mut dyn Pacer,
) -> Result<bool, GithubError> {
    let url = format!(
        "{ENDPOINT}/{}/{}/languages",
        percent_encode(&repo.owner),
        percent_encode(&repo.name)
    );
    let headers = rest_headers(token, "application/vnd.github+json");
    let mut paced = Paced::new(pacer);
    let Some(body) = paced.fetch_optional(&url, || transport.get(&url, &headers))? else {
        return Ok(false);
    };
    let languages: BTreeMap<String, IgnoredAny> = parse_json(&url, &body)?;
    Ok(languages.keys().any(|language| is_rust_name(language)))
}

fn is_rust_name(language: &str) -> bool {
    language.eq_ignore_ascii_case(RUST)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_rust_name_ignores_case_and_nothing_else() {
        assert!(is_rust_name("Rust"));
        assert!(is_rust_name("RUST"));
        assert!(!is_rust_name("Rusty"));
        assert!(!is_rust_name("Shell"));
    }
}
