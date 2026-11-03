use url::Url;

use crate::ids::{is_github_repo_name, normalize_repo_url};

/// The GitHub owner of the ml-rust organization, in lowercase.
const ORG_OWNER: &str = "ml-rust";

/// The repository link as `https://<host>/<owner>/<repo>`, and whether it is a GitHub repository of `ml-rust`.
///
/// Any https host is accepted. Errors with the skip reason for a missing
/// repository, a link that is not https, or a path that is not an owner and a name.
pub fn repo_link(raw: Option<&str>) -> Result<(Url, bool), String> {
    let raw = raw
        .map(str::trim)
        .filter(|link| !link.is_empty())
        .ok_or_else(|| "no repository".to_owned())?;
    if !raw.to_ascii_lowercase().starts_with("https://") {
        return Err(format!("repository `{raw}` is not an https link"));
    }
    let normalized = normalize_repo_url(raw)
        .ok_or_else(|| format!("repository `{raw}` has no owner and name"))?;
    let parts: Vec<&str> = normalized.split('/').collect();
    let [host, owner, name] = parts.as_slice() else {
        return Err(format!("repository `{raw}` has no owner and name"));
    };
    if !is_github_repo_name(owner) || !is_github_repo_name(name) {
        return Err(format!("repository `{raw}` is not an owner and name"));
    }
    let url = Url::parse(&format!("https://{normalized}"))
        .map_err(|_| format!("repository `{raw}` is not a valid link"))?;
    Ok((url, *host == "github.com" && *owner == ORG_OWNER))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repo_links_normalise() {
        for raw in [
            "https://github.com/Foo/Bar",
            "https://github.com/foo/bar.git",
            "https://www.github.com/foo/bar/tree/main/sub",
            " https://github.com/foo/bar/ ",
        ] {
            let (url, org) = repo_link(Some(raw)).expect(raw);
            assert_eq!(url.as_str(), "https://github.com/foo/bar", "{raw}");
            assert!(!org);
        }
    }

    #[test]
    fn other_hosts_are_accepted_and_never_org_affiliated() {
        let (url, org) = repo_link(Some("https://GitLab.com/ML-Rust/X.git")).expect("gitlab");
        assert_eq!(url.as_str(), "https://gitlab.com/ml-rust/x");
        assert!(!org);
        assert_eq!(
            repo_link(Some("https://github.com.evil.io/a/b"))
                .expect("other host")
                .0
                .as_str(),
            "https://github.com.evil.io/a/b"
        );
    }

    #[test]
    fn org_owner_is_affiliated_in_any_case() {
        for raw in [
            "https://github.com/ml-rust/x",
            "https://github.com/ML-Rust/x",
        ] {
            assert!(repo_link(Some(raw)).expect(raw).1, "{raw}");
        }
        assert!(
            !repo_link(Some("https://github.com/ml-rust-fork/x"))
                .expect("fork")
                .1
        );
    }

    #[test]
    fn unusable_repositories_give_a_reason() {
        let reason = |raw: Option<&str>| repo_link(raw).expect_err("skipped");
        assert_eq!(reason(None), "no repository");
        assert_eq!(reason(Some("  ")), "no repository");
        assert!(reason(Some("http://github.com/a/b")).contains("not an https link"));
        assert!(reason(Some("git://github.com/a/b")).contains("not an https link"));
        assert!(reason(Some("git@github.com:a/b.git")).contains("not an https link"));
        assert!(reason(Some("https://github.com/a")).contains("no owner and name"));
        assert!(reason(Some("https://github.com/a/b c")).contains("owner and name"));
    }
}
