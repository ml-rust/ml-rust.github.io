//! Evidence link classes: a GitHub page the API can read, another page, or a refused link.

use catalogue_core::{GithubLogin, is_github_repo_name};
use url::Url;

/// The hosts of GitHub pages, matched exactly after ASCII lowercasing.
const GITHUB_HOSTS: [&str; 2] = ["github.com", "www.github.com"];

/// The host of raw GitHub files.
const RAW_HOST: &str = "raw.githubusercontent.com";

/// The longest GitHub repo name.
const MAX_REPO_NAME: usize = 100;

/// A repo named by owner and name, both checked against GitHub's name rules.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GithubRepo {
    /// The owner login.
    pub owner: String,
    /// The repo name.
    pub name: String,
}

/// A GitHub page the bot reads through the REST API.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GithubTarget {
    /// `github.com/{owner}/{repo}`: the README on the default branch.
    Readme {
        /// The repo.
        repo: GithubRepo,
    },
    /// `github.com/{owner}/{repo}/issues/{n}` or `/pull/{n}`: the title and body.
    Issue {
        /// The repo.
        repo: GithubRepo,
        /// The issue or PR number.
        number: u64,
    },
    /// `github.com/{owner}/{repo}/blob/{ref}/{path}` or
    /// `raw.githubusercontent.com/{owner}/{repo}/{ref}/{path}`: the raw file.
    File {
        /// The repo.
        repo: GithubRepo,
        /// The branch, tag or commit: the first segment after `blob`.
        git_ref: String,
        /// The decoded path segments. None is empty, `.` or `..`, or holds a slash.
        path: Vec<String>,
    },
}

/// How the bot reads one evidence link.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LinkClass {
    /// A GitHub page, read through the GitHub API.
    GitHub(GithubTarget),
    /// Another http or https page, read through a fetch service. The bot host never fetches it.
    Other,
    /// A link the bot never reads: another scheme, a user part, or a malformed GitHub path.
    Unsupported,
}

/// The class of `url`.
///
/// Only an https link with no user part and no port, on an exact GitHub host,
/// is a GitHub page. The host comes from the parsed URL alone.
pub fn classify(url: &Url) -> LinkClass {
    if !matches!(url.scheme(), "http" | "https")
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return LinkClass::Unsupported;
    }
    let Some(host) = url.host_str().map(str::to_ascii_lowercase) else {
        return LinkClass::Unsupported;
    };
    let github = GITHUB_HOSTS.contains(&host.as_str());
    let raw = host == RAW_HOST;
    if !(github || raw) || url.scheme() != "https" || url.port().is_some() {
        return LinkClass::Other;
    }
    let Some(segments) = decoded_segments(url) else {
        return LinkClass::Unsupported;
    };
    let target = if raw {
        raw_target(&segments)
    } else {
        page_target(&segments)
    };
    match target {
        Target::Found(target) => LinkClass::GitHub(target),
        Target::Unknown => LinkClass::Other,
        Target::Malformed => LinkClass::Unsupported,
    }
}

/// The result of matching a GitHub path.
enum Target {
    /// A page the API reads.
    Found(GithubTarget),
    /// A clean path of another shape, such as a wiki page.
    Unknown,
    /// A path in a known shape whose owner, name, number or ref breaks the rules.
    Malformed,
}

/// A `github.com` path as a target.
fn page_target(segments: &[String]) -> Target {
    let found = match segments {
        [owner, name] => {
            let name = name.strip_suffix(".git").unwrap_or(name);
            repo(owner, name).map(|repo| GithubTarget::Readme { repo })
        }
        [owner, name, kind, number] if kind == "issues" || kind == "pull" => repo(owner, name)
            .and_then(|repo| {
                issue_number(number).map(|number| GithubTarget::Issue { repo, number })
            }),
        [owner, name, blob, git_ref, path @ ..] if blob == "blob" && !path.is_empty() => {
            file(owner, name, git_ref, path)
        }
        _ => return Target::Unknown,
    };
    found.map_or(Target::Malformed, Target::Found)
}

/// A `raw.githubusercontent.com` path as a target.
fn raw_target(segments: &[String]) -> Target {
    match segments {
        [owner, name, git_ref, path @ ..] if !path.is_empty() => {
            file(owner, name, git_ref, path).map_or(Target::Malformed, Target::Found)
        }
        _ => Target::Malformed,
    }
}

fn file(owner: &str, name: &str, git_ref: &str, path: &[String]) -> Option<GithubTarget> {
    let repo = repo(owner, name)?;
    is_ref(git_ref).then(|| GithubTarget::File {
        repo,
        git_ref: git_ref.to_owned(),
        path: path.to_vec(),
    })
}

/// The repo when `owner` is a GitHub login and `name` a GitHub repo name.
fn repo(owner: &str, name: &str) -> Option<GithubRepo> {
    let name_ok = name.len() <= MAX_REPO_NAME && is_github_repo_name(name);
    (name_ok && GithubLogin::new(owner).is_ok()).then(|| GithubRepo {
        owner: owner.to_owned(),
        name: name.to_owned(),
    })
}

/// An issue number: ASCII digits, not zero.
fn issue_number(text: &str) -> Option<u64> {
    if text.is_empty() || !text.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    text.parse().ok().filter(|number| *number > 0)
}

/// A ref segment: printable ASCII with no space.
fn is_ref(text: &str) -> bool {
    !text.is_empty() && text.bytes().all(|b| b.is_ascii_graphic())
}

/// The decoded path segments, without one trailing empty segment.
///
/// `None` when a segment is empty, `.` or `..`, holds a slash, a backslash or
/// a control character after decoding, or is not valid percent-encoded UTF-8.
fn decoded_segments(url: &Url) -> Option<Vec<String>> {
    let mut raw: Vec<&str> = url.path_segments()?.collect();
    if raw.last().is_some_and(|last| last.is_empty()) {
        raw.pop();
    }
    raw.into_iter()
        .map(|segment| {
            let decoded = percent_decode(segment)?;
            let clean = !decoded.is_empty()
                && decoded != "."
                && decoded != ".."
                && !decoded
                    .chars()
                    .any(|c| matches!(c, '/' | '\\') || c.is_control());
            clean.then_some(decoded)
        })
        .collect()
}

/// `text` with each `%XX` escape decoded. `None` for a broken escape or bytes that are not UTF-8.
fn percent_decode(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut at = 0;
    while let Some(&byte) = bytes.get(at) {
        if byte == b'%' {
            let high = hex(*bytes.get(at + 1)?)?;
            let low = hex(*bytes.get(at + 2)?)?;
            out.push((high << 4) | low);
            at += 3;
        } else {
            out.push(byte);
            at += 1;
        }
    }
    String::from_utf8(out).ok()
}

/// The value of one hex digit in either case.
fn hex(byte: u8) -> Option<u8> {
    char::from(byte)
        .to_digit(16)
        .and_then(|digit| u8::try_from(digit).ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn class(text: &str) -> LinkClass {
        classify(&Url::parse(text).expect(text))
    }

    fn gh(owner: &str, name: &str) -> GithubRepo {
        GithubRepo {
            owner: owner.to_owned(),
            name: name.to_owned(),
        }
    }

    #[test]
    fn repo_links_read_the_readme() {
        for text in [
            "https://github.com/o/r",
            "https://www.github.com/o/r/",
            "HTTPS://GITHUB.COM/o/r",
            "https://github.com/o/r.git",
            "https://github.com:443/o/r?tab=readme#top",
        ] {
            assert_eq!(
                class(text),
                LinkClass::GitHub(GithubTarget::Readme { repo: gh("o", "r") }),
                "{text}"
            );
        }
    }

    #[test]
    fn issue_and_pull_links_read_the_issue() {
        assert_eq!(
            class("https://github.com/rust-lang/rust/issues/12"),
            LinkClass::GitHub(GithubTarget::Issue {
                repo: gh("rust-lang", "rust"),
                number: 12
            })
        );
        assert_eq!(
            class("https://github.com/o/r/pull/7"),
            LinkClass::GitHub(GithubTarget::Issue {
                repo: gh("o", "r"),
                number: 7
            })
        );
        for text in [
            "https://github.com/o/r/issues/0",
            "https://github.com/o/r/issues/+1",
            "https://github.com/o/r/issues/99999999999999999999999",
        ] {
            assert_eq!(class(text), LinkClass::Unsupported, "{text}");
        }
    }

    #[test]
    fn blob_and_raw_links_read_the_file() {
        let expected = LinkClass::GitHub(GithubTarget::File {
            repo: gh("o", "r"),
            git_ref: "main".to_owned(),
            path: vec!["docs".to_owned(), "a b.md".to_owned()],
        });
        assert_eq!(
            class("https://github.com/o/r/blob/main/docs/a%20b.md"),
            expected
        );
        assert_eq!(
            class("https://raw.githubusercontent.com/o/r/main/docs/a%20b.md"),
            expected
        );
    }

    #[test]
    fn lookalike_hosts_and_user_parts_are_not_github() {
        assert_eq!(
            class("https://github.com.evil.example/o/r"),
            LinkClass::Other
        );
        assert_eq!(class("https://evilgithub.com/o/r"), LinkClass::Other);
        assert_eq!(
            class("https://github.com@evil.example/x"),
            LinkClass::Unsupported
        );
        assert_eq!(class("https://user@github.com/o/r"), LinkClass::Unsupported);
        assert_eq!(class("https://u:p@example.com/"), LinkClass::Unsupported);
    }

    #[test]
    fn ports_plain_http_and_other_schemes_are_not_github() {
        assert_eq!(class("https://github.com:8443/o/r"), LinkClass::Other);
        assert_eq!(class("http://github.com/o/r"), LinkClass::Other);
        assert_eq!(class("ftp://github.com/o/r"), LinkClass::Unsupported);
        assert_eq!(class("javascript:alert(1)"), LinkClass::Unsupported);
        assert_eq!(class("https://example.com/uses"), LinkClass::Other);
    }

    #[test]
    fn encoded_slashes_dot_segments_and_bad_names_are_refused() {
        for text in [
            "https://github.com/o/r/blob/main/a%2Fb",
            "https://github.com/o/r/blob/main/a%5Cb",
            "https://github.com/o/r/blob/main/a%00b",
            "https://github.com/o/r/blob/main/%ZZ",
            "https://github.com/o/r/blob/main/a//b",
            "https://github.com/o%2Fx/r",
            "https://github.com/-o/r",
            "https://github.com/o/r%20s",
            "https://raw.githubusercontent.com/o/r/main",
        ] {
            assert_eq!(class(text), LinkClass::Unsupported, "{text}");
        }
        assert_eq!(
            class("https://github.com/o/r/blob/main/x/../../../../etc/passwd"),
            LinkClass::Other,
            "the parser resolves dot segments, so the path no longer has the blob shape"
        );
        assert_eq!(
            class("https://github.com/o/r/blob/main/%2e%2e/x"),
            LinkClass::Other,
            "an encoded dot segment resolves like a plain one"
        );
    }

    #[test]
    fn other_github_shapes_go_to_the_fetch_service() {
        assert_eq!(class("https://github.com/o/r/wiki/Home"), LinkClass::Other);
        assert_eq!(class("https://github.com/features"), LinkClass::Other);
    }

    #[test]
    fn percent_decoding_reads_both_cases_and_refuses_broken_escapes() {
        assert_eq!(percent_decode("a%2fb%2F").as_deref(), Some("a/b/"));
        assert_eq!(percent_decode("%C3%A9").as_deref(), Some("é"));
        assert_eq!(percent_decode("%"), None);
        assert_eq!(percent_decode("%4"), None);
        assert_eq!(percent_decode("%FF"), None);
    }
}
