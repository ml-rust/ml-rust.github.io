//! The README of a repo on a host other than GitHub.

use url::Url;

use super::text::{RepoReadme, raw_limit};
use crate::http::{Paced, Pacer, Transport, percent_encode};
use crate::link::error::LinkError;
use crate::link::forge::{ForgeHost, ForgeRepo};
use crate::link::manifest::{RepoSite, default_branch, fetch, root_files};

/// The file names that hold a README, first choice first. Compared without regard to case.
const NAMES: [&str; 4] = ["README.md", "README.mdx", "README.markdown", "README"];

/// The first of `paths` with a name in [`NAMES`], in the order of [`NAMES`].
fn pick_readme(paths: &[String]) -> Option<&String> {
    NAMES
        .iter()
        .find_map(|name| paths.iter().find(|path| path.eq_ignore_ascii_case(name)))
}

/// The page of `path` on `branch` of `repo`. `None` when the parts make no URL.
fn file_page(repo: &ForgeRepo, branch: &str, path: &str) -> Option<Url> {
    let branch = encoded_path(branch);
    let path = encoded_path(path);
    let host = repo.host.name();
    let route = match repo.host {
        ForgeHost::Codeberg => "src/branch",
        ForgeHost::Gitlab => "-/blob",
    };
    Url::parse(&format!(
        "https://{host}/{}/{route}/{branch}/{path}",
        repo.path
    ))
    .ok()
}

/// Encodes each segment of `path` and keeps the separators.
fn encoded_path(path: &str) -> String {
    path.split('/')
        .map(percent_encode)
        .collect::<Vec<_>>()
        .join("/")
}

/// Fetches the README of `repo` on its default branch. No request carries a token.
///
/// The fetch reads the default branch name, then the root folder, then the raw file, cut at
/// [`MAX_README`](super::MAX_README). Returns `None` when the repo, the branch or the file is absent, or the root
/// folder holds no README.
///
/// # Errors
///
/// Returns a [`LinkError`] when no response arrives, a status other than 200 or an absent status
/// answers, a body breaks off, a listing is not the expected JSON, or the rate limit stays after
/// the retries.
pub fn forge_readme(
    transport: &dyn Transport,
    repo: &ForgeRepo,
    pacer: &mut dyn Pacer,
) -> Result<Option<RepoReadme>, LinkError> {
    let site = RepoSite::Forge(repo);
    let mut paced = Paced::new(pacer);
    let Some(branch) = default_branch(transport, &site, &mut paced)? else {
        return Ok(None);
    };
    let files = root_files(transport, &site, &branch, &mut paced)?;
    let Some(path) = pick_readme(&files) else {
        return Ok(None);
    };
    let url = site.raw_url(&branch, path);
    let headers = site.raw_headers();
    let Some(answer) = fetch(transport, &site, &url, &headers, raw_limit(), &mut paced)? else {
        return Ok(None);
    };
    let Some(source) = file_page(repo, &branch, path) else {
        return Ok(None);
    };
    Ok(Some(RepoReadme::new(
        &answer.body,
        path.clone(),
        branch,
        source,
    )))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::link::test_host::{Host, NoWait, Reply};

    fn owned(names: &[&str]) -> Vec<String> {
        names.iter().map(|&name| name.to_owned()).collect()
    }

    fn run(link: &str, host: &Host) -> Option<RepoReadme> {
        let repo = ForgeRepo::parse(link).expect("repo");
        forge_readme(host, &repo, &mut NoWait).expect("runs")
    }

    #[test]
    fn the_first_name_wins_in_any_case_and_a_nested_file_stays_out() {
        let names = owned(&["readme", "ReadMe.MD", "docs/README.md"]);
        assert_eq!(pick_readme(&names).map(String::as_str), Some("ReadMe.MD"));
        let names = owned(&["README.rst", "README.txt", "docs/README.md"]);
        assert_eq!(pick_readme(&names), None);
    }

    #[test]
    fn codeberg_reads_the_root_folder_then_the_raw_file_with_no_token() {
        let root = "https://codeberg.org/api/v1/repos/a/b";
        let host = Host::default()
            .route(root, Reply::ok(r#"{"default_branch":"dev"}"#))
            .route(
                &format!("{root}/git/trees/dev"),
                Reply::ok(
                    r#"{"tree":[{"path":"src","type":"tree"},{"path":"README.md","type":"blob"}]}"#,
                ),
            )
            .route(&format!("{root}/raw/README.md?ref=dev"), Reply::ok("# A"));
        let found = run("https://codeberg.org/a/b", &host).expect("a readme");
        assert_eq!(found.text, "# A");
        assert_eq!(found.path, "README.md");
        assert_eq!(found.branch, "dev");
        assert_eq!(
            found.source.as_str(),
            "https://codeberg.org/a/b/src/branch/dev/README.md"
        );
        assert!(host.headers().iter().all(Vec::is_empty));
    }

    #[test]
    fn gitlab_reads_the_root_folder_then_the_raw_file_with_no_token() {
        let root = "https://gitlab.com/api/v4/projects/g%2Fb";
        let host = Host::default()
            .route(root, Reply::ok(r#"{"default_branch":"main"}"#))
            .route(
                &format!("{root}/repository/tree?ref=main&per_page=100&page=1"),
                Reply::ok(r#"[{"path":"readme.markdown","type":"blob"}]"#),
            )
            .route(
                &format!("{root}/repository/files/readme.markdown/raw?ref=main"),
                Reply::ok("# A"),
            );
        let found = run("https://gitlab.com/g/b", &host).expect("a readme");
        assert_eq!(
            found.source.as_str(),
            "https://gitlab.com/g/b/-/blob/main/readme.markdown"
        );
        assert!(host.headers().iter().all(Vec::is_empty));
    }

    #[test]
    fn a_root_folder_with_no_readme_is_no_readme() {
        let root = "https://codeberg.org/api/v1/repos/a/b";
        let host = Host::default()
            .route(root, Reply::ok(r#"{"default_branch":"main"}"#))
            .route(
                &format!("{root}/git/trees/main"),
                Reply::ok(r#"{"tree":[{"path":"Cargo.toml","type":"blob"}]}"#),
            );
        assert_eq!(run("https://codeberg.org/a/b", &host), None);
    }

    #[test]
    fn an_absent_repo_is_no_readme() {
        assert_eq!(run("https://codeberg.org/a/b", &Host::default()), None);
    }
}
