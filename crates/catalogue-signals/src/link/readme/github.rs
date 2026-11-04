//! The README of a GitHub repo.

use serde::Deserialize;
use serde::de::Error as _;
use url::Url;

use super::base64;
use super::text::{RepoReadme, raw_limit};
use crate::github::{GithubError, RepoRef};
use crate::http::{Paced, Pacer, Transport};
use crate::link::error::LinkError;
use crate::link::manifest::{RepoSite, fetch};

/// The longest answer read. The API sends file text up to 1 MiB, in base64.
const MAX_ANSWER: u64 = 4 << 20;

/// The `encoding` of an answer that holds the file text.
const BASE64: &str = "base64";

/// The answer of `GET /repos/{owner}/{repo}/readme` in its JSON form. Unknown fields are ignored.
#[derive(Deserialize)]
struct ReadmeFile {
    /// The path of the file in the repo.
    path: String,
    /// The API link of the file. Its `ref` query names the branch.
    url: Option<String>,
    /// The page of the file.
    html_url: Url,
    /// How `content` is encoded. Anything but `base64` means the file is too big to hold.
    encoding: Option<String>,
    /// The file text.
    #[serde(default)]
    content: String,
}

impl ReadmeFile {
    /// The branch in the `ref` query of the API link, else in the page link.
    fn branch(&self) -> Option<String> {
        self.branch_in_query().or_else(|| self.branch_in_page())
    }

    fn branch_in_query(&self) -> Option<String> {
        let link = Url::parse(self.url.as_deref()?).ok()?;
        link.query_pairs()
            .find(|(name, value)| name == "ref" && !value.is_empty())
            .map(|(_, value)| value.into_owned())
    }

    /// The segments between `blob` and the file path of `/{owner}/{repo}/blob/{branch}/{path}`.
    fn branch_in_page(&self) -> Option<String> {
        let segments: Vec<&str> = self.html_url.path_segments()?.collect();
        let end = segments.len().checked_sub(self.path.split('/').count())?;
        let branch = segments.get(3..end)?;
        (!branch.is_empty()).then(|| branch.join("/"))
    }
}

/// The error for an answer of `url` that holds no usable README.
fn unusable(url: &str, what: &str) -> LinkError {
    LinkError::Github(GithubError::Json {
        url: url.to_owned(),
        source: serde_json::Error::custom(what),
    })
}

/// Fetches the README of `repo` on its default branch. `None` when GitHub holds none.
///
/// One request gets the path, the branch and the text. A file too big for that answer takes a
/// second request for its raw bytes, cut at [`MAX_README`](super::MAX_README). `token` is sent as a bearer token.
///
/// # Errors
///
/// Returns a [`LinkError`] when no response arrives, a status other than 200 or an absent status
/// answers, a body breaks off, the answer is not the expected JSON, or the rate limit stays after
/// the retries.
pub fn github_readme(
    transport: &dyn Transport,
    token: &str,
    repo: &RepoRef,
    pacer: &mut dyn Pacer,
) -> Result<Option<RepoReadme>, LinkError> {
    let site = RepoSite::Github { repo, token };
    let mut paced = Paced::new(pacer);
    let url = format!("{}/readme", site.repo_url());
    let headers = site.json_headers();
    let Some(answer) = fetch(transport, &site, &url, &headers, MAX_ANSWER, &mut paced)? else {
        return Ok(None);
    };
    let file: ReadmeFile = site.parse(&url, &answer.body)?;
    if file.path.is_empty() || file.path.chars().any(char::is_control) {
        return Err(unusable(&url, "the answer names no usable file path"));
    }
    let branch = file
        .branch()
        .ok_or_else(|| unusable(&url, "the answer names no branch"))?;
    if file.encoding.as_deref() == Some(BASE64) {
        let bytes = base64::decode(&file.content)
            .ok_or_else(|| unusable(&url, "the file text is not base64"))?;
        return Ok(Some(RepoReadme::new(
            &bytes,
            file.path,
            branch,
            file.html_url,
        )));
    }
    let raw = site.raw_url(&branch, &file.path);
    let Some(answer) = fetch(
        transport,
        &site,
        &raw,
        &site.raw_headers(),
        raw_limit(),
        &mut paced,
    )?
    else {
        return Ok(None);
    };
    Ok(Some(RepoReadme::new(
        &answer.body,
        file.path,
        branch,
        file.html_url,
    )))
}

#[cfg(test)]
mod tests {
    use super::super::MAX_README;
    use super::*;
    use crate::link::test_host::{Host, NoWait, Reply};

    const README: &str = "https://api.github.com/repos/a/b/readme";

    fn run(host: &Host) -> Result<Option<RepoReadme>, LinkError> {
        let repo = RepoRef::parse("a/b").expect("repo");
        github_readme(host, "t0k", &repo, &mut NoWait)
    }

    fn answer(path: &str, branch: &str) -> String {
        format!(
            r#"{{"path":"{path}","url":"https://api.github.com/repos/A/B/contents/{path}?ref={branch}",
                "html_url":"https://github.com/A/B/blob/{branch}/{path}","encoding":"base64","content":"IyBB\nCg==\n"}}"#
        )
    }

    #[test]
    fn the_json_form_gives_the_text_the_path_the_branch_and_the_source() {
        let host = Host::default().route(README, Reply::ok(&answer("docs/README.md", "dev")));
        let found = run(&host).expect("runs").expect("a readme");
        assert_eq!(found.text, "# A\n");
        assert_eq!(found.path, "docs/README.md");
        assert_eq!(found.branch, "dev");
        assert!(!found.truncated);
        assert_eq!(
            found.source.as_str(),
            "https://github.com/A/B/blob/dev/docs/README.md"
        );
    }

    #[test]
    fn the_request_carries_the_json_media_type_and_the_token() {
        let host = Host::default().route(README, Reply::ok(&answer("README.md", "main")));
        run(&host).expect("runs");
        let headers = &host.headers()[0];
        assert!(headers.contains(&(
            "Accept".to_owned(),
            "application/vnd.github+json".to_owned()
        )));
        assert!(headers.contains(&("Authorization".to_owned(), "Bearer t0k".to_owned())));
    }

    #[test]
    fn a_branch_with_a_slash_reads_from_the_page_link_when_the_api_link_has_no_ref() {
        let body = r#"{"path":"README.md","url":"https://api.github.com/repos/a/b/contents/README.md",
            "html_url":"https://github.com/a/b/blob/feat/x/README.md","encoding":"base64","content":""}"#;
        let host = Host::default().route(README, Reply::ok(body));
        let found = run(&host).expect("runs").expect("a readme");
        assert_eq!(found.branch, "feat/x");
    }

    #[test]
    fn a_404_is_no_readme() {
        assert_eq!(run(&Host::default()).expect("runs"), None);
    }

    #[test]
    fn a_server_error_is_an_error() {
        let host = Host::default().route(README, Reply::status(500));
        let error = run(&host).expect_err("status");
        assert!(error.to_string().contains("500"), "{error}");
    }

    #[test]
    fn bad_base64_is_an_error_that_names_the_url() {
        let body = r#"{"path":"README.md","html_url":"https://github.com/a/b/blob/main/README.md","encoding":"base64","content":"!"}"#;
        let host = Host::default().route(README, Reply::ok(body));
        let error = run(&host).expect_err("base64");
        assert!(error.to_string().contains(README), "{error}");
    }

    #[test]
    fn a_file_too_big_for_the_answer_takes_its_raw_bytes_up_to_the_cap() {
        let body = r#"{"path":"README.md","html_url":"https://github.com/a/b/blob/main/README.md","encoding":"none","content":""}"#;
        let raw = "https://api.github.com/repos/a/b/contents/README.md?ref=main";
        let host = Host::default()
            .route(README, Reply::ok(body))
            .route(raw, Reply::bytes(vec![b'a'; MAX_README + 100]));
        let found = run(&host).expect("runs").expect("a readme");
        assert!(found.truncated);
        assert_eq!(found.text.len(), MAX_README);
        let headers = &host.headers()[1];
        assert!(headers.contains(&(
            "Accept".to_owned(),
            "application/vnd.github.raw+json".to_owned()
        )));
    }
}
