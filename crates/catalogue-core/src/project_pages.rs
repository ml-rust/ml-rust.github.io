//! The public pages of an entry's crates and install channels.
//!
//! The check cites these pages as signal sources, and entry pages link them.

use url::Url;

use crate::entry::CRATES_IO_PAGE;

/// The start of a Homebrew formula page.
const HOMEBREW_PAGE: &str = "https://formulae.brew.sh/formula";

/// The start of a Repology project page.
const REPOLOGY_PAGE: &str = "https://repology.org/project";

/// The start of a Docker Hub image page.
const DOCKER_HUB_PAGE: &str = "https://hub.docker.com/r";

/// The crates.io page of crate `name`. `None` when the result is not a valid URL.
pub fn crate_page(name: &str) -> Option<Url> {
    Url::parse(&format!("{CRATES_IO_PAGE}{name}")).ok()
}

/// The Homebrew page of formula `formula`.
pub fn homebrew_page(formula: &str) -> Option<Url> {
    page_url(HOMEBREW_PAGE, &[formula])
}

/// The Repology versions page of project `project`.
pub fn repology_page(project: &str) -> Option<Url> {
    page_url(REPOLOGY_PAGE, &[project, "versions"])
}

/// The Docker Hub page of image `image`, such as `ml-rust/tool`.
///
/// Each `/`-separated part of the image name becomes one path segment.
pub fn docker_page(image: &str) -> Option<Url> {
    let parts: Vec<&str> = image.split('/').collect();
    page_url(DOCKER_HUB_PAGE, &parts)
}

/// `base` with each of `segments` appended as one percent-encoded path segment.
///
/// `None` when `base` is not a URL that takes path segments.
pub fn page_url(base: &str, segments: &[&str]) -> Option<Url> {
    let mut url = Url::parse(base).ok()?;
    url.path_segments_mut()
        .ok()?
        .pop_if_empty()
        .extend(segments);
    Some(url)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(url: Option<Url>) -> Option<String> {
        url.map(String::from)
    }

    #[test]
    fn each_channel_has_its_page() {
        assert_eq!(
            text(crate_page("numr")).as_deref(),
            Some("https://crates.io/crates/numr")
        );
        assert_eq!(
            text(homebrew_page("python@3.12")).as_deref(),
            Some("https://formulae.brew.sh/formula/python@3.12")
        );
        assert_eq!(
            text(repology_page("tool")).as_deref(),
            Some("https://repology.org/project/tool/versions")
        );
        assert_eq!(
            text(docker_page("ml-rust/tool")).as_deref(),
            Some("https://hub.docker.com/r/ml-rust/tool")
        );
    }

    #[test]
    fn a_segment_cannot_add_a_path_or_a_query() {
        assert_eq!(
            text(homebrew_page("a/b?c")).as_deref(),
            Some("https://formulae.brew.sh/formula/a%2Fb%3Fc")
        );
        assert_eq!(
            text(page_url("https://github.com", &["ml-rust", "tool"])).as_deref(),
            Some("https://github.com/ml-rust/tool")
        );
        assert_eq!(page_url("mailto:someone@example.com", &["x"]), None);
    }
}
