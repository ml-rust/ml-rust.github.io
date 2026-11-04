//! Decides every URL in a README: where it goes, or whether it stays out.

use url::Url;

use crate::href::SafeHref;

/// A forge that serves file pages and raw files.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Forge {
    Github,
    Codeberg,
    Gitlab,
}

/// Where the relative URLs of a README resolve. Every part is percent-encoded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ReadmeBase {
    forge: Forge,
    /// `owner/repo`, or the group path on GitLab.
    repo: String,
    /// The default branch.
    branch: String,
    /// The segments of the folder that holds the README.
    folder: Vec<String>,
}

/// What a URL becomes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Resolved {
    /// The target when the URL is a link.
    pub(super) link: SafeHref,
    /// The source when the URL is an image. `None` when an image cannot load from it.
    pub(super) image: Option<SafeHref>,
}

impl ReadmeBase {
    /// The base of the README at `path` on `branch`, with its file page at `source`.
    ///
    /// Returns `None` for a page on any other host, or a page link with too few segments.
    pub(super) fn of(source: &Url, path: &str, branch: &str) -> Option<Self> {
        let forge = match source.host_str()? {
            "github.com" => Forge::Github,
            "codeberg.org" => Forge::Codeberg,
            "gitlab.com" => Forge::Gitlab,
            _ => return None,
        };
        let segments = source.path_segments()?;
        let repo: Vec<&str> = match forge {
            Forge::Github | Forge::Codeberg => segments.take(2).collect(),
            Forge::Gitlab => segments.take_while(|segment| *segment != "-").collect(),
        };
        if repo.len() < 2 {
            return None;
        }
        let mut folder: Vec<String> = path.split('/').map(encode_segment).collect();
        folder.pop();
        Some(Self {
            forge,
            repo: repo.join("/"),
            branch: branch
                .split('/')
                .map(encode_segment)
                .collect::<Vec<_>>()
                .join("/"),
            folder,
        })
    }

    /// The file page of the encoded `path`.
    fn file_page(&self, path: &str) -> String {
        let Self { repo, branch, .. } = self;
        match self.forge {
            Forge::Github => format!("https://github.com/{repo}/blob/{branch}/{path}"),
            Forge::Codeberg => format!("https://codeberg.org/{repo}/src/branch/{branch}/{path}"),
            Forge::Gitlab => format!("https://gitlab.com/{repo}/-/blob/{branch}/{path}"),
        }
    }

    /// The raw file of the encoded `path`.
    fn raw_file(&self, path: &str) -> String {
        let Self { repo, branch, .. } = self;
        match self.forge {
            Forge::Github => format!("https://raw.githubusercontent.com/{repo}/{branch}/{path}"),
            Forge::Codeberg => format!("https://codeberg.org/{repo}/raw/branch/{branch}/{path}"),
            Forge::Gitlab => format!("https://gitlab.com/{repo}/-/raw/{branch}/{path}"),
        }
    }
}

/// What the README URL `raw` becomes. `None` turns a link to plain text and removes an image.
///
/// Whitespace and control characters at either end go first. A backslash or a control character
/// inside refuses the URL. So does a fragment on its own, because no heading has an anchor.
pub(super) fn resolve(raw: &str, base: &ReadmeBase) -> Option<Resolved> {
    let url = raw.trim_matches(|c: char| c.is_whitespace() || c.is_control());
    if url.is_empty() || url.starts_with('#') || url.chars().any(|c| c == '\\' || c.is_control()) {
        return None;
    }
    if let Some(rest) = url.strip_prefix("//") {
        return absolute(&format!("https://{rest}"), base, true);
    }
    match scheme_of(url) {
        Some(scheme) if scheme.eq_ignore_ascii_case("https") => absolute(url, base, true),
        Some(scheme) if scheme.eq_ignore_ascii_case("http") => absolute(url, base, false),
        Some(_) => None,
        None => relative(url, base),
    }
}

/// The scheme at the start of `url`. A `/`, `?` or `#` before the first `:` means it has none.
fn scheme_of(url: &str) -> Option<&str> {
    let (scheme, _) = url.split_once(':')?;
    let mut chars = scheme.chars();
    let starts = chars.next().is_some_and(|c| c.is_ascii_alphabetic());
    (starts && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.')))
        .then_some(scheme)
}

/// An absolute URL as an `https` link, and as an image when `https_input` holds and its host serves
/// the content of the repo's own forge.
fn absolute(text: &str, base: &ReadmeBase, https_input: bool) -> Option<Resolved> {
    let mut url = Url::parse(text).ok()?;
    url.host_str()?;
    if !url.username().is_empty() || url.password().is_some() {
        return None;
    }
    url.set_scheme("https").ok()?;
    let link = checked(url.as_str())?;
    let image = if https_input {
        forge_image(&url, base.forge)
    } else {
        None
    };
    Some(Resolved { link, image })
}

/// The source of an absolute image that loads from `forge`. `None` for every other host, so a
/// visitor loads nothing from a third party.
fn forge_image(url: &Url, forge: Forge) -> Option<SafeHref> {
    let host = url.host_str()?;
    match (forge, host) {
        (Forge::Github, "raw.githubusercontent.com" | "user-images.githubusercontent.com")
        | (Forge::Codeberg, "codeberg.org")
        | (Forge::Gitlab, "gitlab.com") => checked(url.as_str()),
        (Forge::Github, "github.com") => github_image(url),
        _ => None,
    }
}

/// The source of an image under `github.com`: an attachment as it is, a `blob` page as its raw file.
fn github_image(url: &Url) -> Option<SafeHref> {
    let segments: Vec<&str> = url.path_segments()?.collect();
    match segments.as_slice() {
        ["user-attachments", ..] => checked(url.as_str()),
        [owner, repo, "blob", rest @ ..] if rest.len() >= 2 => checked(&format!(
            "https://raw.githubusercontent.com/{owner}/{repo}/{}",
            rest.join("/")
        )),
        _ => None,
    }
}

/// A relative URL against the folder of the README, or the repo root for a leading `/`.
fn relative(url: &str, base: &ReadmeBase) -> Option<Resolved> {
    let (url, fragment) = split_off(url, '#');
    let (path, query) = split_off(url, '?');
    let mut segments: Vec<String> = if path.starts_with('/') {
        Vec::new()
    } else {
        base.folder.clone()
    };
    for segment in path.split('/') {
        match dots_of(segment) {
            Some(0) => {}
            Some(_) => {
                segments.pop()?;
            }
            None => segments.push(encode_segment(segment)),
        }
    }
    if segments.is_empty() {
        return None;
    }
    let mut target = segments.join("/");
    if let Some(query) = query {
        target.push('?');
        target.push_str(&encode_part(query, b"/?"));
    }
    if let Some(fragment) = fragment {
        target.push('#');
        target.push_str(&encode_part(fragment, b"/?"));
    }
    Some(Resolved {
        link: checked(&base.file_page(&target))?,
        image: checked(&base.raw_file(&target)),
    })
}

/// `text` before the first `separator`, and the text after it.
fn split_off(text: &str, separator: char) -> (&str, Option<&str>) {
    match text.split_once(separator) {
        Some((before, after)) => (before, Some(after)),
        None => (text, None),
    }
}

/// How a path segment moves: `Some(0)` stays in place, `Some(1)` goes up, `None` names a file.
///
/// A URL parser reads `%2e` as a dot, so an encoded dot counts as a dot.
fn dots_of(segment: &str) -> Option<u8> {
    match segment.to_ascii_lowercase().replace("%2e", ".").as_str() {
        "" | "." => Some(0),
        ".." => Some(1),
        _ => None,
    }
}

/// `raw` as a link target. `None` when [`SafeHref`] refuses it.
fn checked(raw: &str) -> Option<SafeHref> {
    let href = SafeHref::new(raw);
    (!href.is_refused()).then_some(href)
}

/// Encodes one path segment.
fn encode_segment(segment: &str) -> String {
    encode_part(segment, b"")
}

/// Percent-encodes `text`, keeping the bytes a URL part carries as they are and the bytes of
/// `extra`. A `%` with two hex digits is already encoded and stays.
fn encode_part(text: &str, extra: &[u8]) -> String {
    let bytes = text.as_bytes();
    let mut out = String::with_capacity(text.len());
    for (at, &byte) in bytes.iter().enumerate() {
        let encoded = byte == b'%'
            && bytes
                .get(at + 1..at + 3)
                .is_some_and(|digits| digits.iter().all(u8::is_ascii_hexdigit));
        let kept = byte.is_ascii_alphanumeric()
            || matches!(
                byte,
                b'-' | b'.'
                    | b'_'
                    | b'~'
                    | b'!'
                    | b'$'
                    | b'&'
                    | b'('
                    | b')'
                    | b'*'
                    | b'+'
                    | b','
                    | b';'
                    | b'='
                    | b':'
                    | b'@'
            )
            || extra.contains(&byte);
        if encoded || kept {
            out.push(char::from(byte));
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base_of(source: &str, path: &str) -> ReadmeBase {
        let source = Url::parse(source).expect("source");
        ReadmeBase::of(&source, path, "main").expect("base")
    }

    fn github() -> ReadmeBase {
        base_of("https://github.com/o/r/blob/main/README.md", "README.md")
    }

    fn nested() -> ReadmeBase {
        base_of(
            "https://github.com/o/r/blob/main/docs/guide/README.md",
            "docs/guide/README.md",
        )
    }

    fn codeberg() -> ReadmeBase {
        base_of(
            "https://codeberg.org/o/r/src/branch/main/README.md",
            "README.md",
        )
    }

    fn gitlab() -> ReadmeBase {
        base_of(
            "https://gitlab.com/g/s/r/-/blob/main/README.md",
            "README.md",
        )
    }

    fn link(url: &str, base: &ReadmeBase) -> Option<String> {
        resolve(url, base).map(|found| found.link.as_str().to_owned())
    }

    fn image(url: &str, base: &ReadmeBase) -> Option<String> {
        resolve(url, base).and_then(|found| found.image.map(|src| src.as_str().to_owned()))
    }

    #[test]
    fn a_refused_scheme_gives_nothing() {
        for url in [
            "javascript:alert(1)",
            " JaVaScRiPt:alert(1)",
            "\tjavascript:alert(1)\n",
            "java\tscript:alert(1)",
            "java\nscript:alert(1)",
            "data:image/svg+xml,<svg/>",
            "DATA:text/html,x",
            "vbscript:x",
            "file:///etc/passwd",
            "mailto:a@b.c",
            "ftp://example.org/a",
            "x-custom:a",
            "",
            "   ",
            "#anchor",
            " #anchor",
            "https:\\\\example.org",
            "\\\\example.org",
            "a\\b.md",
            "https://user:pw@example.org/",
            "https://",
        ] {
            assert_eq!(link(url, &github()), None, "{url:?}");
            assert_eq!(image(url, &github()), None, "{url:?}");
        }
    }

    #[test]
    fn absolute_links_read_as_https() {
        for (url, want) in [
            ("https://example.org/a?b=1#c", "https://example.org/a?b=1#c"),
            ("HTTPS://Example.org/a", "https://example.org/a"),
            ("http://example.org/a", "https://example.org/a"),
            ("HTTP://example.org/a", "https://example.org/a"),
            ("//evil.example", "https://evil.example/"),
            ("//host/x", "https://host/x"),
            ("  https://example.org/  ", "https://example.org/"),
        ] {
            assert_eq!(link(url, &github()).as_deref(), Some(want), "{url}");
        }
    }

    #[test]
    fn relative_links_go_to_the_file_page_of_the_forge() {
        let root = "https://github.com/o/r/blob/main";
        for (url, want) in [
            ("a.md", format!("{root}/a.md")),
            ("./a.md", format!("{root}/a.md")),
            ("/a.md", format!("{root}/a.md")),
            ("docs/a.md", format!("{root}/docs/a.md")),
            ("a b.md", format!("{root}/a%20b.md")),
            ("a%20b.md", format!("{root}/a%20b.md")),
            ("a%zz.md", format!("{root}/a%25zz.md")),
            ("é.md", format!("{root}/%C3%A9.md")),
            ("a.md?x=1#y", format!("{root}/a.md?x=1#y")),
            ("a.md?x=a b", format!("{root}/a.md?x=a%20b")),
            ("a.md#\"y", format!("{root}/a.md#%22y")),
        ] {
            assert_eq!(
                link(url, &github()).as_deref(),
                Some(want.as_str()),
                "{url}"
            );
        }
    }

    #[test]
    fn relative_links_start_from_the_folder_of_the_readme() {
        let root = "https://github.com/o/r/blob/main";
        for (url, want) in [
            ("./a.md", format!("{root}/docs/guide/a.md")),
            ("../a.md", format!("{root}/docs/a.md")),
            ("../../a.md", format!("{root}/a.md")),
            ("/a.md", format!("{root}/a.md")),
            ("sub/../a.md", format!("{root}/docs/guide/a.md")),
        ] {
            assert_eq!(
                link(url, &nested()).as_deref(),
                Some(want.as_str()),
                "{url}"
            );
        }
    }

    #[test]
    fn a_climb_past_the_root_gives_nothing_in_any_spelling() {
        for (url, base) in [
            ("../a.md", github()),
            ("../../../x", nested()),
            ("/../a.md", nested()),
            ("%2e%2e/a.md", github()),
            ("%2E%2E/a.md", github()),
            (".%2e/a.md", github()),
            ("%2e./a.md", github()),
            ("./", github()),
            ("/", github()),
            ("?x=1", github()),
        ] {
            assert_eq!(link(url, &base), None, "{url}");
        }
    }

    #[test]
    fn each_forge_has_its_file_page_and_raw_file() {
        for (base, page, raw) in [
            (
                github(),
                "https://github.com/o/r/blob/main/a.png",
                "https://raw.githubusercontent.com/o/r/main/a.png",
            ),
            (
                codeberg(),
                "https://codeberg.org/o/r/src/branch/main/a.png",
                "https://codeberg.org/o/r/raw/branch/main/a.png",
            ),
            (
                gitlab(),
                "https://gitlab.com/g/s/r/-/blob/main/a.png",
                "https://gitlab.com/g/s/r/-/raw/main/a.png",
            ),
        ] {
            assert_eq!(link("a.png", &base).as_deref(), Some(page));
            assert_eq!(image("./a.png", &base).as_deref(), Some(raw));
        }
    }

    #[test]
    fn a_branch_keeps_its_slash_and_encodes_the_rest() {
        let source = Url::parse("https://github.com/o/r/blob/x/README.md").expect("source");
        let base = ReadmeBase::of(&source, "README.md", "feat/a b").expect("base");
        assert_eq!(
            link("a.md", &base).as_deref(),
            Some("https://github.com/o/r/blob/feat/a%20b/a.md")
        );
    }

    #[test]
    fn a_blob_image_converts_to_the_raw_file() {
        for (url, want) in [
            (
                "https://github.com/o/r/blob/main/a.png",
                "https://raw.githubusercontent.com/o/r/main/a.png",
            ),
            (
                "https://github.com/other/repo/blob/v1/docs/a.png?raw=true",
                "https://raw.githubusercontent.com/other/repo/v1/docs/a.png",
            ),
        ] {
            assert_eq!(image(url, &github()).as_deref(), Some(want), "{url}");
        }
    }

    #[test]
    fn absolute_images_load_only_from_the_content_hosts_of_the_forge() {
        for url in [
            "https://raw.githubusercontent.com/o/r/main/a.png",
            "https://user-images.githubusercontent.com/1/a.png",
            "https://github.com/user-attachments/assets/a",
        ] {
            assert_eq!(image(url, &github()).as_deref(), Some(url), "{url}");
        }
        assert!(
            image(
                "https://codeberg.org/o/r/raw/branch/main/a.png",
                &codeberg()
            )
            .is_some()
        );
        assert!(image("https://gitlab.com/g/r/-/raw/main/a.png", &gitlab()).is_some());
        for url in [
            "https://img.shields.io/badge/a-b-green",
            "https://docs.rs/numr/badge.svg",
            "https://crates.io/x.svg",
            "https://ci.example.org/badge.svg",
            "https://private-user-images.githubusercontent.com/1/a.png",
            "https://camo.githubusercontent.com/a",
            "https://github.com/o/r/actions/workflows/ci.yml/badge.svg",
            "https://github.com/o/r/raw/main/a.png",
            "https://raw.githubusercontent.com.evil.example/a.png",
            "https://codeberg.org/o/r/raw/branch/main/a.png",
            "https://gitlab.com/g/r/-/raw/main/a.png",
            "http://raw.githubusercontent.com/o/r/main/a.png",
            "//evil.example/a.png",
        ] {
            assert_eq!(image(url, &github()), None, "{url}");
        }
        assert_eq!(
            image(
                "https://raw.githubusercontent.com/o/r/main/a.png",
                &codeberg()
            ),
            None
        );
    }

    #[test]
    fn a_page_on_another_host_has_no_base() {
        let source = Url::parse("https://example.org/o/r/README.md").expect("source");
        assert_eq!(ReadmeBase::of(&source, "README.md", "main"), None);
    }
}
