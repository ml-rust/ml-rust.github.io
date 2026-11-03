//! The request headers of a JSON call with a bearer key.

use catalogue_github::Secret;
use catalogue_signals::USER_AGENT;

/// The headers of a JSON `POST` that `key` authorises.
///
/// The list holds `Authorization`, `Content-Type`, `Accept` and the catalogue
/// `User-Agent`. The transport marks `Authorization` as sensitive.
pub(crate) fn json_bearer(key: &Secret) -> Vec<(&'static str, String)> {
    vec![
        ("Authorization", format!("Bearer {}", key.expose())),
        ("Content-Type", "application/json".to_owned()),
        ("Accept", "application/json".to_owned()),
        ("User-Agent", USER_AGENT.to_owned()),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_key_goes_only_into_authorization() {
        let headers = json_bearer(&Secret::new("k-1".to_owned()));
        let names: Vec<&str> = headers.iter().map(|(name, _)| *name).collect();
        assert_eq!(
            names,
            ["Authorization", "Content-Type", "Accept", "User-Agent"]
        );
        assert_eq!(headers[0].1, "Bearer k-1");
        assert!(headers[1..].iter().all(|(_, value)| !value.contains("k-1")));
    }
}
