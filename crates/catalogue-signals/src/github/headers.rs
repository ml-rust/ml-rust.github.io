//! The request headers of the GitHub REST API.

/// The API version every REST request pins.
const API_VERSION: &str = "2022-11-28";

/// The headers of a REST request: `Accept` set to `accept`, the pinned
/// `X-GitHub-Api-Version` and `Authorization` as a bearer `token`.
pub(crate) fn rest_headers(token: &str, accept: &str) -> Vec<(&'static str, String)> {
    vec![
        ("Accept", accept.to_owned()),
        ("X-GitHub-Api-Version", API_VERSION.to_owned()),
        ("Authorization", format!("Bearer {token}")),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_headers_carry_the_media_type_the_version_and_the_bearer_token() {
        assert_eq!(
            rest_headers("t0k", "application/vnd.github+json"),
            [
                ("Accept", "application/vnd.github+json".to_owned()),
                ("X-GitHub-Api-Version", "2022-11-28".to_owned()),
                ("Authorization", "Bearer t0k".to_owned()),
            ]
        );
    }
}
