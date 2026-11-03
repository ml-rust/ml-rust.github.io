/// The form that compares two repository links: `host/owner/repo` in lowercase.
///
/// Reads `https://`, `http://`, `git://` and `ssh://` links and the
/// `git@host:owner/repo` form. Drops the scheme, a user part, a leading `www.`,
/// the query, the fragment, a `.git` suffix, trailing slashes and every path
/// segment after the repository. Returns `None` when the link has no host,
/// owner and repository.
pub fn normalize_repo_url(url: &str) -> Option<String> {
    let lowered = url.trim().to_lowercase();
    let without_tail = lowered.split(['?', '#']).next().unwrap_or_default();
    let rest = match without_tail.split_once("://") {
        Some((_, rest)) => rest.to_owned(),
        None => without_tail.replacen(':', "/", 1),
    };
    let mut segments = rest.split('/').filter(|segment| !segment.is_empty());
    let host = segments.next()?;
    let host = host.rsplit('@').next().unwrap_or(host);
    let host = host.strip_prefix("www.").unwrap_or(host);
    let owner = segments.next()?;
    let repo = segments.next()?;
    let repo = repo.strip_suffix(".git").unwrap_or(repo);
    if host.is_empty() || repo.is_empty() {
        return None;
    }
    Some(format!("{host}/{owner}/{repo}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_case_git_suffix_and_extra_path() {
        assert_eq!(
            normalize_repo_url("https://github.com/Owner/Repo.git/tree/main/x").as_deref(),
            Some("github.com/owner/repo")
        );
    }

    #[test]
    fn strips_www_slash_query_and_fragment() {
        for url in [
            "https://www.github.com/owner/repo/",
            "http://github.com/owner/repo?tab=readme#top",
            "git://github.com/owner/repo.git",
            "ssh://git@github.com/owner/repo.git",
            "git@github.com:owner/repo.git",
            "  HTTPS://GitHub.com/Owner/Repo  ",
            "github.com/owner/repo",
        ] {
            assert_eq!(
                normalize_repo_url(url).as_deref(),
                Some("github.com/owner/repo"),
                "{url}"
            );
        }
    }

    #[test]
    fn keeps_other_hosts() {
        assert_eq!(
            normalize_repo_url("https://gitlab.com/g/p").as_deref(),
            Some("gitlab.com/g/p")
        );
    }

    #[test]
    fn rejects_links_without_owner_and_repo() {
        for url in [
            "",
            "https://github.com",
            "https://github.com/owner",
            "https:///a/b",
        ] {
            assert_eq!(normalize_repo_url(url), None, "{url}");
        }
    }
}
