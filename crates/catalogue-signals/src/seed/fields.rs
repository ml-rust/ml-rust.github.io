//! The entry fields that derive from crate metadata.

use std::collections::BTreeSet;

use catalogue_core::{MAX_DESCRIPTION_CHARS, Slug, Topics, normalize_repo_url};
use url::Url;

/// The GitHub owner of the ml-rust organization, in lowercase.
const ORG_OWNER: &str = "ml-rust";

/// The description text: trimmed, with every run of whitespace collapsed to one space.
///
/// Errors with the skip reason for a missing, blank or over-long description.
pub(super) fn description_text(raw: Option<&str>) -> Result<String, String> {
    let words: Vec<&str> = raw.unwrap_or_default().split_whitespace().collect();
    let text = words.join(" ");
    if text.is_empty() {
        return Err("no description".to_owned());
    }
    if text.chars().count() > MAX_DESCRIPTION_CHARS {
        return Err(format!(
            "description is longer than {MAX_DESCRIPTION_CHARS} characters"
        ));
    }
    Ok(text)
}

/// The repository link as `https://<host>/<owner>/<repo>`, and whether it is a GitHub repository of `ml-rust`.
///
/// Any https host is accepted. Errors with the skip reason for a missing
/// repository, a link that is not https, or a path that is not an owner and a name.
pub(super) fn repo_link(raw: Option<&str>) -> Result<(Url, bool), String> {
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
    if !is_repo_name(owner) || !is_repo_name(name) {
        return Err(format!("repository `{raw}` is not an owner and name"));
    }
    let url = Url::parse(&format!("https://{normalized}"))
        .map_err(|_| format!("repository `{raw}` is not a valid link"))?;
    Ok((url, *host == "github.com" && *owner == ORG_OWNER))
}

/// The key that groups the crates of one repository: the normalised `host/owner/repo`.
///
/// `None` when the link is not an https link with an owner and a name.
pub(super) fn repo_key(raw: Option<&str>) -> Option<String> {
    let (url, _) = repo_link(raw).ok()?;
    Some(url.as_str().trim_start_matches("https://").to_owned())
}

/// The homepage when it is an https link.
pub(super) fn https_link(raw: Option<&str>) -> Option<Url> {
    let url = Url::parse(raw?.trim()).ok()?;
    (url.scheme() == "https" && url.host_str().is_some()).then_some(url)
}

/// The ids of the topics that map to a crate's keywords or categories, sorted.
///
/// A topic keyword matches an equal keyword, ignoring case. A topic category
/// matches an equal category slug or a slug that extends it with `::`.
pub(super) fn matching_topics(
    topics: &Topics,
    keywords: &[String],
    categories: &[String],
) -> Vec<Slug> {
    let keywords: BTreeSet<String> = keywords.iter().map(|k| k.to_lowercase()).collect();
    let found: BTreeSet<&Slug> = topics
        .topic
        .iter()
        .filter(|topic| {
            topic.keywords.iter().any(|k| keywords.contains(k))
                || topic
                    .categories
                    .iter()
                    .any(|c| categories.iter().any(|slug| in_category(c, slug)))
        })
        .map(|topic| &topic.id)
        .collect();
    found.into_iter().cloned().collect()
}

/// Whether `slug` equals `category` or extends it with `::`.
fn in_category(category: &str, slug: &str) -> bool {
    slug == category
        || slug
            .strip_prefix(category)
            .is_some_and(|rest| rest.starts_with("::"))
}

fn is_repo_name(value: &str) -> bool {
    !value.is_empty()
        && value
            .bytes()
            .all(|b| matches!(b, b'a'..=b'z' | b'0'..=b'9' | b'.' | b'_' | b'-'))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn topics(text: &str) -> Topics {
        toml::from_str(text).expect("topics parse")
    }

    fn ids(found: Vec<Slug>) -> Vec<String> {
        found.iter().map(ToString::to_string).collect()
    }

    #[test]
    fn description_collapses_whitespace() {
        assert_eq!(
            description_text(Some("  A  tensor\n\tlibrary. ")),
            Ok("A tensor library.".to_owned())
        );
    }

    #[test]
    fn description_limit_is_300_characters() {
        let limit = "é".repeat(300);
        assert_eq!(description_text(Some(&limit)), Ok(limit.clone()));
        let over = "x".repeat(301);
        let error = description_text(Some(&over)).expect_err("over the limit");
        assert!(error.contains("longer than 300"), "{error}");
    }

    #[test]
    fn missing_or_blank_description_is_an_error() {
        assert_eq!(description_text(None), Err("no description".to_owned()));
        assert_eq!(
            description_text(Some(" \n ")),
            Err("no description".to_owned())
        );
    }

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
            assert_eq!(repo_key(Some(raw)).as_deref(), Some("github.com/foo/bar"));
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
        assert_eq!(repo_key(Some("http://github.com/a/b")), None);
        assert_eq!(repo_key(None), None);
    }

    #[test]
    fn homepage_must_be_https() {
        assert_eq!(
            https_link(Some("https://example.com/x")).map(|u| u.to_string()),
            Some("https://example.com/x".to_owned())
        );
        assert_eq!(https_link(Some("http://example.com")), None);
        assert_eq!(https_link(Some("not a link")), None);
        assert_eq!(https_link(Some("")), None);
        assert_eq!(https_link(None), None);
    }

    const TOPICS: &str = r#"
[[topic]]
id = "nlp"
name = "NLP"
keywords = ["nlp", "text"]

[[topic]]
id = "foundations"
name = "Foundations"
keywords = ["tensor"]
categories = ["science::math"]

[[topic]]
id = "other"
name = "Other"
"#;

    #[test]
    fn keywords_match_topics_sorted_by_id() {
        let words = vec!["Text".to_owned(), "tensor".to_owned(), "zip".to_owned()];
        assert_eq!(
            ids(matching_topics(&topics(TOPICS), &words, &[])),
            ["foundations", "nlp"]
        );
    }

    #[test]
    fn categories_match_equal_and_extending_slugs() {
        let found = |slug: &str| ids(matching_topics(&topics(TOPICS), &[], &[slug.to_owned()]));
        assert_eq!(found("science::math"), ["foundations"]);
        assert_eq!(found("science::math::linear"), ["foundations"]);
        assert!(found("science::mathematics").is_empty());
        assert!(found("science").is_empty());
    }

    #[test]
    fn keyword_and_category_of_one_topic_match_it_once() {
        let words = vec!["tensor".to_owned()];
        let cats = vec!["science::math".to_owned()];
        assert_eq!(
            ids(matching_topics(&topics(TOPICS), &words, &cats)),
            ["foundations"]
        );
    }

    #[test]
    fn no_match_gives_no_topic() {
        let words = vec!["zip".to_owned()];
        assert!(matching_topics(&topics(TOPICS), &words, &[]).is_empty());
    }
}
