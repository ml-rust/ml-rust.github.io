//! The entry fields that derive from crate metadata.

use std::collections::BTreeSet;

use catalogue_core::{Slug, Topics, repo_link};
use url::Url;

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
    fn repo_key_is_the_normalised_host_owner_and_name() {
        for raw in [
            "https://github.com/Foo/Bar",
            "https://github.com/foo/bar.git",
            "https://www.github.com/foo/bar/tree/main/sub",
            " https://github.com/foo/bar/ ",
        ] {
            assert_eq!(repo_key(Some(raw)).as_deref(), Some("github.com/foo/bar"));
        }
    }

    #[test]
    fn unusable_repositories_have_no_key() {
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
