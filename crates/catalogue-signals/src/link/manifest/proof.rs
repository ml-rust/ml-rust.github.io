//! The requests of one manifest proof.

use serde::Deserialize;

use super::candidates::manifest_candidates;
use super::declares::{MAX_MANIFEST, declares_crate};
use super::site::RepoSite;
use crate::http::{Answer, MAX_BODY, Paced, Pacer, Transport};
use crate::link::error::LinkError;

/// The answers that mean the repo, the branch or the file is not there to read.
const ABSENT: &[u16] = &[404, 409, 410, 451];

/// The most tree pages read. A longer tree is checked as far as these list it.
const MAX_TREE_PAGES: u32 = 10;

#[derive(Deserialize)]
struct RepoInfo {
    default_branch: Option<String>,
}

#[derive(Deserialize)]
struct TreeEntry {
    path: String,
    #[serde(rename = "type")]
    kind: String,
}

/// A tree answer: an object with a `tree` list on GitHub and Forgejo, a bare list on GitLab.
#[derive(Deserialize)]
#[serde(untagged)]
enum Tree {
    Wrapped { tree: Option<Vec<TreeEntry>> },
    Bare(Vec<TreeEntry>),
}

impl Tree {
    fn into_entries(self) -> Vec<TreeEntry> {
        match self {
            Self::Wrapped { tree } => tree.unwrap_or_default(),
            Self::Bare(entries) => entries,
        }
    }
}

/// The path of a manifest on the default branch of `site` that declares `crate_name`.
///
/// The proof reads the default branch name, then the recursive tree, then the manifests in the
/// order of [`manifest_candidates`], and stops at the first match. A truncated tree is checked as
/// far as it lists. Returns `None` when no read manifest matches, or when the repo, the branch or
/// the tree is absent.
///
/// # Errors
///
/// Returns a [`LinkError`] when no response arrives, a status other than 200 or an absent status
/// answers, a body breaks off, a listing is not the expected JSON, or the rate limit stays after
/// the retries.
pub(in crate::link) fn declaring_manifest(
    transport: &dyn Transport,
    site: &RepoSite<'_>,
    crate_name: &str,
    pacer: &mut dyn Pacer,
) -> Result<Option<String>, LinkError> {
    let mut paced = Paced::new(pacer);
    let Some(branch) = default_branch(transport, site, &mut paced)? else {
        return Ok(None);
    };
    let paths = tree_paths(transport, site, &branch, &mut paced)?;
    let headers = site.raw_headers();
    let limit = MAX_MANIFEST.saturating_add(1);
    for path in manifest_candidates(paths, crate_name) {
        let url = site.raw_url(&branch, &path);
        let answer = fetch(transport, site, &url, &headers, limit, &mut paced)?;
        if answer.is_some_and(|answer| declares_crate(&answer.body, crate_name)) {
            return Ok(Some(path));
        }
    }
    Ok(None)
}

/// The name of the default branch, when the repo has one.
fn default_branch(
    transport: &dyn Transport,
    site: &RepoSite<'_>,
    paced: &mut Paced<'_>,
) -> Result<Option<String>, LinkError> {
    let url = site.repo_url();
    let headers = site.json_headers();
    let Some(answer) = fetch(transport, site, &url, &headers, MAX_BODY, paced)? else {
        return Ok(None);
    };
    let info: RepoInfo = site.parse(&url, &answer.body)?;
    Ok(info.default_branch.filter(|name| !name.is_empty()))
}

/// The file paths of the tree of `branch`, over at most [`MAX_TREE_PAGES`] pages.
fn tree_paths(
    transport: &dyn Transport,
    site: &RepoSite<'_>,
    branch: &str,
    paced: &mut Paced<'_>,
) -> Result<Vec<String>, LinkError> {
    let headers = site.json_headers();
    let mut paths = Vec::new();
    for page in 1..=MAX_TREE_PAGES {
        let url = site.tree_url(branch, page);
        let Some(answer) = fetch(transport, site, &url, &headers, MAX_BODY, paced)? else {
            break;
        };
        let tree: Tree = site.parse(&url, &answer.body)?;
        paths.extend(
            tree.into_entries()
                .into_iter()
                .filter(|entry| entry.kind == "blob")
                .map(|entry| entry.path),
        );
        if !has_next_page(&answer) {
            break;
        }
    }
    Ok(paths)
}

/// Whether the answer names a next page. Only GitLab sends the header.
fn has_next_page(answer: &Answer) -> bool {
    answer
        .headers
        .get("x-next-page")
        .is_some_and(|value| !value.trim().is_empty())
}

/// Sends one paced GET and turns its error into a link error.
fn fetch(
    transport: &dyn Transport,
    site: &RepoSite<'_>,
    url: &str,
    headers: &[(&'static str, String)],
    limit: u64,
    paced: &mut Paced<'_>,
) -> Result<Option<Answer>, LinkError> {
    paced
        .fetch_capped(url, || transport.get(url, headers), ABSENT, limit)
        .map_err(|error| site.request_error(error))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::github::{GithubError, RepoRef};
    use crate::link::forge::ForgeRepo;
    use crate::link::manifest::candidates::MAX_MANIFESTS;
    use crate::link::test_host::{Host, NoWait, Reply};

    const CODEBERG: &str = "https://codeberg.org/api/v1/repos/a/b";
    const GITLAB: &str = "https://gitlab.com/api/v4/projects/a%2Fb";
    const GITHUB: &str = "https://api.github.com/repos/a/b";

    fn repo_json() -> Reply {
        Reply::ok(r#"{"default_branch":"main"}"#)
    }

    fn tree_json(paths: &[&str]) -> String {
        let entries: Vec<String> = paths
            .iter()
            .map(|path| format!(r#"{{"path":"{path}","type":"blob"}}"#))
            .collect();
        format!(r#"{{"tree":[{}],"truncated":false}}"#, entries.join(","))
    }

    fn manifest(name: &str) -> Reply {
        Reply::ok(&format!("[package]\nname = \"{name}\"\n"))
    }

    fn codeberg(host: &Host, crate_name: &str) -> Result<Option<String>, LinkError> {
        let repo = ForgeRepo::parse("https://codeberg.org/a/b").expect("repo");
        declaring_manifest(host, &RepoSite::Forge(&repo), crate_name, &mut NoWait)
    }

    fn codeberg_tree() -> String {
        format!("{CODEBERG}/git/trees/main?recursive=true")
    }

    fn codeberg_raw(path: &str) -> String {
        format!("{CODEBERG}/raw/{path}?ref=main")
    }

    #[test]
    fn a_root_manifest_matches_and_stops_the_reads() {
        let host = Host::default()
            .route(CODEBERG, repo_json())
            .route(
                &codeberg_tree(),
                Reply::ok(&tree_json(&["Cargo.toml", "b/Cargo.toml"])),
            )
            .route(&codeberg_raw("Cargo.toml"), manifest("whisper-rs"));
        let found = codeberg(&host, "whisper-rs").expect("proves");
        assert_eq!(found.as_deref(), Some("Cargo.toml"));
        assert_eq!(host.urls().len(), 3);
        assert!(host.headers().iter().all(Vec::is_empty));
    }

    #[test]
    fn a_nested_manifest_matches_across_dash_and_underscore() {
        let host = Host::default()
            .route(CODEBERG, repo_json())
            .route(
                &codeberg_tree(),
                Reply::ok(&tree_json(&["Cargo.toml", "crates/my_crate/Cargo.toml"])),
            )
            .route(&codeberg_raw("Cargo.toml"), Reply::ok("[workspace]\n"))
            .route(
                &codeberg_raw("crates/my_crate/Cargo.toml"),
                manifest("my_crate"),
            );
        let found = codeberg(&host, "my-crate").expect("proves");
        assert_eq!(found.as_deref(), Some("crates/my_crate/Cargo.toml"));
    }

    #[test]
    fn a_directory_entry_is_not_read() {
        let tree = r#"{"tree":[{"path":"Cargo.toml","type":"tree"}]}"#;
        let host = Host::default()
            .route(CODEBERG, repo_json())
            .route(&codeberg_tree(), Reply::ok(tree));
        assert_eq!(codeberg(&host, "x").expect("runs"), None);
        assert_eq!(host.urls().len(), 2);
    }

    #[test]
    fn the_reads_stop_at_the_cap() {
        let paths: Vec<String> = (0..MAX_MANIFESTS + 3)
            .map(|index| format!("d{index:03}/Cargo.toml"))
            .collect();
        let refs: Vec<&str> = paths.iter().map(String::as_str).collect();
        let mut host = Host::default()
            .route(CODEBERG, repo_json())
            .route(&codeberg_tree(), Reply::ok(&tree_json(&refs)));
        for path in &paths {
            host = host.route(&codeberg_raw(path), manifest("other"));
        }
        assert_eq!(codeberg(&host, "x").expect("runs"), None);
        assert_eq!(host.urls().len(), 2 + MAX_MANIFESTS);
    }

    #[test]
    fn an_oversize_manifest_does_not_match_and_the_next_one_is_read() {
        let oversize = format!("[package]\nname = \"x\"\n{}", "\n".repeat(1 << 20));
        let host = Host::default()
            .route(CODEBERG, repo_json())
            .route(
                &codeberg_tree(),
                Reply::ok(&tree_json(&["Cargo.toml", "x/Cargo.toml"])),
            )
            .route(&codeberg_raw("Cargo.toml"), Reply::ok(&oversize))
            .route(&codeberg_raw("x/Cargo.toml"), manifest("x"));
        let found = codeberg(&host, "x").expect("proves");
        assert_eq!(found.as_deref(), Some("x/Cargo.toml"));
    }

    #[test]
    fn a_manifest_that_is_gone_or_unparsable_does_not_match() {
        let host = Host::default()
            .route(CODEBERG, repo_json())
            .route(
                &codeberg_tree(),
                Reply::ok(&tree_json(&["Cargo.toml", "a/Cargo.toml"])),
            )
            .route(&codeberg_raw("Cargo.toml"), Reply::ok("[package"));
        assert_eq!(codeberg(&host, "x").expect("runs"), None);
    }

    #[test]
    fn a_truncated_tree_still_checks_what_it_lists() {
        let tree = r#"{"tree":[{"path":"Cargo.toml","type":"blob"}],"truncated":true}"#;
        let host = Host::default()
            .route(CODEBERG, repo_json())
            .route(&codeberg_tree(), Reply::ok(tree))
            .route(&codeberg_raw("Cargo.toml"), manifest("x"));
        let found = codeberg(&host, "x").expect("proves");
        assert_eq!(found.as_deref(), Some("Cargo.toml"));
    }

    #[test]
    fn an_absent_repo_or_an_empty_one_finds_nothing() {
        let host = Host::default();
        assert_eq!(codeberg(&host, "x").expect("runs"), None);
        assert_eq!(host.urls(), [CODEBERG]);

        let host = Host::default().route(CODEBERG, Reply::ok(r#"{"default_branch":null}"#));
        assert_eq!(codeberg(&host, "x").expect("runs"), None);
        assert_eq!(host.urls(), [CODEBERG]);

        let host = Host::default()
            .route(CODEBERG, repo_json())
            .route(&codeberg_tree(), Reply::status(409));
        assert_eq!(codeberg(&host, "x").expect("runs"), None);
    }

    #[test]
    fn a_server_error_or_a_bad_listing_is_an_error() {
        let host = Host::default().route(CODEBERG, Reply::status(503));
        let error = codeberg(&host, "x").expect_err("rejected");
        assert!(matches!(error, LinkError::Forge { .. }), "{error}");
        assert!(error.to_string().contains(CODEBERG), "{error}");

        let host = Host::default().route(CODEBERG, Reply::ok("<html>"));
        let error = codeberg(&host, "x").expect_err("rejected");
        assert!(matches!(error, LinkError::ForgeJson { .. }), "{error}");

        let host = Host::default()
            .route(CODEBERG, repo_json())
            .route(&codeberg_tree(), Reply::ok(&tree_json(&["Cargo.toml"])))
            .route(&codeberg_raw("Cargo.toml"), Reply::status(500));
        let error = codeberg(&host, "x").expect_err("rejected");
        assert!(matches!(error, LinkError::Forge { .. }), "{error}");
    }

    #[test]
    fn no_response_stays_a_transport_error() {
        let host = Host::default().route(CODEBERG, Reply::Refused);
        let error = codeberg(&host, "x").expect_err("rejected");
        assert!(matches!(error, LinkError::Http(_)), "{error}");
    }

    #[test]
    fn gitlab_reads_a_bare_list_over_its_pages() {
        let page = |number: u32| {
            format!("{GITLAB}/repository/tree?ref=main&recursive=true&per_page=100&page={number}")
        };
        let first = r#"[{"path":"docs","type":"tree"},{"path":"docs/a.md","type":"blob"}]"#;
        let second = r#"[{"path":"Cargo.toml","type":"blob"}]"#;
        let host = Host::default()
            .route(GITLAB, repo_json())
            .route(&page(1), Reply::ok(first).header("X-Next-Page", "2"))
            .route(&page(2), Reply::ok(second).header("X-Next-Page", ""))
            .route(
                &format!("{GITLAB}/repository/files/Cargo.toml/raw?ref=main"),
                manifest("x"),
            );
        let repo = ForgeRepo::parse("https://gitlab.com/a/b").expect("repo");
        let found =
            declaring_manifest(&host, &RepoSite::Forge(&repo), "x", &mut NoWait).expect("proves");
        assert_eq!(found.as_deref(), Some("Cargo.toml"));
        assert_eq!(host.urls().len(), 4);
    }

    #[test]
    fn github_sends_the_token_and_a_rejected_token_stays_a_github_error() {
        let repo = RepoRef::parse("a/b").expect("repo");
        let site = RepoSite::Github {
            repo: &repo,
            token: "t0k",
        };
        let host = Host::default()
            .route(GITHUB, repo_json())
            .route(
                &format!("{GITHUB}/git/trees/main?recursive=1"),
                Reply::ok(&tree_json(&["Cargo.toml"])),
            )
            .route(
                &format!("{GITHUB}/contents/Cargo.toml?ref=main"),
                manifest("x"),
            );
        let found = declaring_manifest(&host, &site, "x", &mut NoWait).expect("proves");
        assert_eq!(found.as_deref(), Some("Cargo.toml"));
        assert!(host.headers().iter().all(|headers| {
            headers
                .iter()
                .any(|(name, value)| name == "Authorization" && value == "Bearer t0k")
        }));

        let host = Host::default().route(GITHUB, Reply::status(401));
        let error = declaring_manifest(&host, &site, "x", &mut NoWait).expect_err("rejected");
        assert!(
            matches!(
                error,
                LinkError::Github(GithubError::Status { status: 401, .. })
            ),
            "{error}"
        );
    }
}
