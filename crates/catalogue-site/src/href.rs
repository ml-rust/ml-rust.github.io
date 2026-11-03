//! Links that are safe to put in an `href`.

use std::fmt;

use url::Url;

/// The href of a refused link.
const REFUSED: &str = "#";

/// A link target that is an `http` or `https` URL, or a path on this site.
///
/// Any other text becomes `#`. `Display` writes the target. Templates still
/// HTML-escape it, because the type is not marked safe.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SafeHref(String);

impl SafeHref {
    /// The link to `raw`, or `#` when `raw` is neither a site path nor an `http`
    /// or `https` URL with a host.
    ///
    /// A site path starts with one `/` and holds no backslash, whitespace or
    /// control character. A URL comes back in its normalised form.
    pub fn new(raw: &str) -> Self {
        if is_site_path(raw) {
            return Self(raw.to_owned());
        }
        match Url::parse(raw) {
            Ok(url) if is_web_url(&url) => Self(url.into()),
            _ => Self(REFUSED.to_owned()),
        }
    }

    /// The link target.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Whether the input was refused and the target is `#`.
    pub fn is_refused(&self) -> bool {
        self.0 == REFUSED
    }
}

impl From<&Url> for SafeHref {
    fn from(url: &Url) -> Self {
        Self::new(url.as_str())
    }
}

impl fmt::Display for SafeHref {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Whether `raw` is a path on this site.
fn is_site_path(raw: &str) -> bool {
    raw.starts_with('/')
        && !raw.starts_with("//")
        && !raw
            .chars()
            .any(|c| c == '\\' || c.is_whitespace() || c.is_control())
}

/// Whether `url` is an `http` or `https` URL with a host.
fn is_web_url(url: &Url) -> bool {
    matches!(url.scheme(), "http" | "https") && url.host().is_some()
}

#[cfg(test)]
mod tests {
    use askama::Template;

    use super::*;

    #[derive(Template)]
    #[template(source = r#"<a href="{{ href }}">{{ text }}</a>"#, ext = "html")]
    struct Link<'a> {
        href: SafeHref,
        text: &'a str,
    }

    #[test]
    fn web_urls_pass_in_normalised_form() {
        assert_eq!(
            SafeHref::new("https://crates.io/crates/numr").as_str(),
            "https://crates.io/crates/numr"
        );
        assert_eq!(
            SafeHref::new("HTTP://Example.com").as_str(),
            "http://example.com/"
        );
    }

    #[test]
    fn site_paths_pass_unchanged() {
        for path in ["/", "/rules/", "/topics/inference/", "/search/?q=gpu"] {
            assert_eq!(SafeHref::new(path).as_str(), path);
        }
    }

    #[test]
    fn every_other_target_is_refused() {
        for raw in [
            "javascript:alert(1)",
            "JavaScript:alert(1)",
            " javascript:alert(1)",
            "data:text/html,<script>alert(1)</script>",
            "vbscript:msgbox",
            "mailto:someone@example.com",
            "ftp://example.com/",
            "//evil.example/",
            "/\\evil.example/",
            "/a b",
            "/\tjavascript:alert(1)",
            "rules/",
            "",
            "https://",
            "#main",
        ] {
            let href = SafeHref::new(raw);
            assert!(href.is_refused(), "{raw:?} gave {href}");
            assert_eq!(href.as_str(), "#");
        }
    }

    #[test]
    fn from_url_applies_the_same_check() {
        let web = Url::parse("https://github.com/ml-rust").expect("url");
        assert_eq!(SafeHref::from(&web).as_str(), "https://github.com/ml-rust");
        let script = Url::parse("javascript:alert(1)").expect("url");
        assert!(SafeHref::from(&script).is_refused());
    }

    #[test]
    fn a_template_escapes_the_text_and_refuses_a_script_link() {
        let html = Link {
            href: SafeHref::new("javascript:alert(1)"),
            text: "<script>\"'&",
        }
        .render()
        .expect("renders");
        assert_eq!(html, r##"<a href="#">&#60;script&#62;&#34;&#39;&#38;</a>"##);
    }

    #[test]
    fn a_template_escapes_quotes_and_ampersands_in_a_web_link() {
        let html = Link {
            href: SafeHref::new("https://example.com/?a=1&b=\"x\""),
            text: "x",
        }
        .render()
        .expect("renders");
        assert!(!html.contains("\"x\""), "{html}");
        assert!(html.contains("a=1&#38;b="), "{html}");
    }
}
