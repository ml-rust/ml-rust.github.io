//! The requests of the manifest proof on each repo host.

use serde::de::DeserializeOwned;

use crate::github::{RepoRef, parse_json, rest_headers};
use crate::http::{PacedError, percent_encode};
use crate::link::error::LinkError;
use crate::link::forge::{ForgeHost, ForgeRepo};

/// The GitHub REST root of a repo.
const GITHUB_REPOS: &str = "https://api.github.com/repos";
/// The Forgejo API root of a repo on Codeberg.
const CODEBERG_REPOS: &str = "https://codeberg.org/api/v1/repos";
/// The GitLab API root of a project.
const GITLAB_PROJECTS: &str = "https://gitlab.com/api/v4/projects";

/// The media type of a GitHub JSON answer.
const GITHUB_JSON: &str = "application/vnd.github+json";
/// The media type that makes GitHub send a file's bytes.
const GITHUB_RAW: &str = "application/vnd.github.raw+json";

/// The tree entries per GitLab page.
const GITLAB_PAGE: u32 = 100;

/// The repo a manifest proof reads, with what the host needs to read it.
#[derive(Debug, Clone, Copy)]
pub(in crate::link) enum RepoSite<'a> {
    /// A repo on GitHub, read with a token.
    Github {
        /// The repo.
        repo: &'a RepoRef,
        /// The bearer token.
        token: &'a str,
    },
    /// A repo on another host, read with no token.
    Forge(&'a ForgeRepo),
}

impl RepoSite<'_> {
    /// The URL that describes the repo and names its default branch.
    pub(super) fn repo_url(&self) -> String {
        match self {
            Self::Github { repo, .. } => format!("{GITHUB_REPOS}/{}/{}", repo.owner, repo.name),
            Self::Forge(repo) => match repo.host {
                ForgeHost::Codeberg => format!("{CODEBERG_REPOS}/{}", repo.path),
                ForgeHost::Gitlab => {
                    format!("{GITLAB_PROJECTS}/{}", percent_encode(&repo.path))
                }
            },
        }
    }

    /// The URL of page `page`, from 1, of the recursive tree of `branch`.
    ///
    /// Only GitLab pages its tree. The other hosts answer the first page and stop.
    pub(super) fn tree_url(&self, branch: &str, page: u32) -> String {
        let root = self.repo_url();
        let branch = percent_encode(branch);
        match self {
            Self::Github { .. } => format!("{root}/git/trees/{branch}?recursive=1"),
            Self::Forge(repo) => match repo.host {
                ForgeHost::Codeberg => format!("{root}/git/trees/{branch}?recursive=true"),
                ForgeHost::Gitlab => format!(
                    "{root}/repository/tree?ref={branch}&recursive=true&per_page={GITLAB_PAGE}&page={page}"
                ),
            },
        }
    }

    /// The URL of the raw bytes of `path` on `branch`.
    pub(super) fn raw_url(&self, branch: &str, path: &str) -> String {
        let root = self.repo_url();
        let branch = percent_encode(branch);
        match self {
            Self::Github { .. } => {
                format!("{root}/contents/{}?ref={branch}", encoded_path(path))
            }
            Self::Forge(repo) => match repo.host {
                ForgeHost::Codeberg => format!("{root}/raw/{}?ref={branch}", encoded_path(path)),
                ForgeHost::Gitlab => format!(
                    "{root}/repository/files/{}/raw?ref={branch}",
                    percent_encode(path)
                ),
            },
        }
    }

    /// The headers of a JSON request. Only GitHub gets the token.
    pub(super) fn json_headers(&self) -> Vec<(&'static str, String)> {
        match self {
            Self::Github { token, .. } => rest_headers(token, GITHUB_JSON),
            Self::Forge(_) => Vec::new(),
        }
    }

    /// The headers of a raw file request. Only GitHub gets the token.
    pub(super) fn raw_headers(&self) -> Vec<(&'static str, String)> {
        match self {
            Self::Github { token, .. } => rest_headers(token, GITHUB_RAW),
            Self::Forge(_) => Vec::new(),
        }
    }

    /// The link error for a request that ended without a usable answer.
    pub(super) fn request_error(&self, error: PacedError) -> LinkError {
        match (self, error) {
            (Self::Github { .. }, error) => LinkError::Github(error.into()),
            (Self::Forge(_), PacedError::Http(error)) => LinkError::Http(error),
            (Self::Forge(_), source) => LinkError::Forge { source },
        }
    }

    /// Parses a JSON answer from `url`.
    pub(super) fn parse<T: DeserializeOwned>(
        &self,
        url: &str,
        body: &[u8],
    ) -> Result<T, LinkError> {
        match self {
            Self::Github { .. } => parse_json(url, body).map_err(LinkError::Github),
            Self::Forge(_) => serde_json::from_slice(body).map_err(|source| LinkError::ForgeJson {
                url: url.to_owned(),
                source,
            }),
        }
    }
}

/// Encodes each segment of `path` and keeps the separators.
fn encoded_path(path: &str) -> String {
    path.split('/')
        .map(percent_encode)
        .collect::<Vec<_>>()
        .join("/")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn forge(link: &str) -> ForgeRepo {
        ForgeRepo::parse(link).expect(link)
    }

    #[test]
    fn github_urls_use_the_rest_api() {
        let repo = RepoRef::parse("apache/arrow-rs").expect("repo");
        let site = RepoSite::Github {
            repo: &repo,
            token: "t0k",
        };
        assert_eq!(
            site.repo_url(),
            "https://api.github.com/repos/apache/arrow-rs"
        );
        assert_eq!(
            site.tree_url("main", 1),
            "https://api.github.com/repos/apache/arrow-rs/git/trees/main?recursive=1"
        );
        assert_eq!(
            site.raw_url("main", "arrow/Cargo.toml"),
            "https://api.github.com/repos/apache/arrow-rs/contents/arrow/Cargo.toml?ref=main"
        );
    }

    #[test]
    fn only_github_requests_carry_the_token() {
        let repo = RepoRef::parse("a/b").expect("repo");
        let site = RepoSite::Github {
            repo: &repo,
            token: "t0k",
        };
        let bearer = ("Authorization", "Bearer t0k".to_owned());
        assert!(site.json_headers().contains(&bearer));
        assert!(site.raw_headers().contains(&bearer));
        for link in ["https://codeberg.org/a/b", "https://gitlab.com/a/b"] {
            let repo = forge(link);
            let site = RepoSite::Forge(&repo);
            assert!(site.json_headers().is_empty(), "{link}");
            assert!(site.raw_headers().is_empty(), "{link}");
        }
    }

    #[test]
    fn forgejo_urls_follow_its_api() {
        let repo = forge("https://codeberg.org/sarah-quinones/faer");
        let site = RepoSite::Forge(&repo);
        assert_eq!(
            site.repo_url(),
            "https://codeberg.org/api/v1/repos/sarah-quinones/faer"
        );
        assert_eq!(
            site.tree_url("main", 1),
            "https://codeberg.org/api/v1/repos/sarah-quinones/faer/git/trees/main?recursive=true"
        );
        assert_eq!(
            site.raw_url("main", "faer/Cargo.toml"),
            "https://codeberg.org/api/v1/repos/sarah-quinones/faer/raw/faer/Cargo.toml?ref=main"
        );
    }

    #[test]
    fn gitlab_urls_encode_the_project_and_the_file_path() {
        let repo = forge("https://gitlab.com/group/sub/proj");
        let site = RepoSite::Forge(&repo);
        assert_eq!(
            site.repo_url(),
            "https://gitlab.com/api/v4/projects/group%2Fsub%2Fproj"
        );
        assert_eq!(
            site.tree_url("main", 3),
            "https://gitlab.com/api/v4/projects/group%2Fsub%2Fproj/repository/tree?ref=main&recursive=true&per_page=100&page=3"
        );
        assert_eq!(
            site.raw_url("main", "crates/a/Cargo.toml"),
            "https://gitlab.com/api/v4/projects/group%2Fsub%2Fproj/repository/files/crates%2Fa%2FCargo.toml/raw?ref=main"
        );
    }

    #[test]
    fn a_branch_and_a_path_from_the_host_stay_inside_their_url_part() {
        let repo = forge("https://codeberg.org/a/b");
        let site = RepoSite::Forge(&repo);
        assert_eq!(
            site.tree_url("feat/x?y", 1),
            "https://codeberg.org/api/v1/repos/a/b/git/trees/feat%2Fx%3Fy?recursive=true"
        );
        assert_eq!(
            site.raw_url("m&n", "a b/Cargo.toml"),
            "https://codeberg.org/api/v1/repos/a/b/raw/a%20b/Cargo.toml?ref=m%26n"
        );
    }
}
