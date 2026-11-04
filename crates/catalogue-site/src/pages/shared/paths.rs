//! The paths of the data-built pages, and links to them.

use catalogue_core::{Catalogue, Category, Slug, Topic};
use url::Url;

use crate::href::SafeHref;

/// A link with its text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NamedLink<'a> {
    /// The link text.
    pub name: &'a str,
    /// The link target.
    pub href: SafeHref,
}

/// A link to an outside page, with the page's host as its text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EvidenceLink {
    /// The host of the page, or the whole link when it has no host.
    pub text: String,
    /// The link target. `#` for a link that is not `http` or `https`.
    pub href: SafeHref,
}

impl EvidenceLink {
    /// The link to `url`.
    pub(crate) fn of(url: &Url) -> Self {
        let text = url.host_str().unwrap_or(url.as_str()).to_owned();
        Self {
            text,
            href: SafeHref::from(url),
        }
    }

    /// The links to every page of `urls`, in order.
    pub(crate) fn all(urls: &[Url]) -> Vec<Self> {
        urls.iter().map(Self::of).collect()
    }

    /// The links to the evidence of the summary of `topic`. Empty without a summary.
    pub(crate) fn of_summary(topic: &Topic) -> Vec<Self> {
        topic
            .summary
            .as_ref()
            .map(|summary| Self::all(&summary.evidence))
            .unwrap_or_default()
    }
}

/// The file of the page of entry `slug`, below the site root.
pub(crate) fn project_file(slug: &Slug) -> String {
    format!("projects/{slug}/index.html")
}

/// The link to the page of entry `slug`.
pub(crate) fn project_href(slug: &Slug) -> SafeHref {
    SafeHref::new(&format!("/projects/{slug}/"))
}

/// The file of the page of topic `id`, below the site root.
pub(crate) fn topic_file(id: &Slug) -> String {
    format!("topics/{id}/index.html")
}

/// The link to the page of topic `id`.
pub(crate) fn topic_href(id: &Slug) -> SafeHref {
    SafeHref::new(&format!("/topics/{id}/"))
}

/// The file of the page of category `id`, below the site root.
pub(crate) fn category_file(id: &Slug) -> String {
    format!("categories/{id}/index.html")
}

/// The link to the page of category `id`.
pub(crate) fn category_href(id: &Slug) -> SafeHref {
    SafeHref::new(&format!("/categories/{id}/"))
}

/// The link to category `id` with the category's name, or `None` for an id the catalogue lacks.
pub(crate) fn category_link<'a>(catalogue: &'a Catalogue, id: &Slug) -> Option<NamedLink<'a>> {
    catalogue
        .categories()
        .iter()
        .find(|category| &category.id == id)
        .map(named_category)
}

fn named_category(category: &Category) -> NamedLink<'_> {
    NamedLink {
        name: &category.name,
        href: category_href(&category.id),
    }
}

/// The link to entry `slug` with the entry's name, or `None` for a slug the catalogue lacks.
pub(crate) fn entry_link<'a>(catalogue: &'a Catalogue, slug: &Slug) -> Option<NamedLink<'a>> {
    let entry = catalogue.entry(slug)?;
    Some(NamedLink {
        name: &entry.name,
        href: project_href(slug),
    })
}

/// The link to topic `id` with the topic's name, or `None` for an id the catalogue lacks.
pub(crate) fn topic_link<'a>(catalogue: &'a Catalogue, id: &Slug) -> Option<NamedLink<'a>> {
    let topic = catalogue.topics().iter().find(|topic| &topic.id == id)?;
    Some(NamedLink {
        name: &topic.name,
        href: topic_href(&topic.id),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn slug(text: &str) -> Slug {
        Slug::new(text).expect("slug")
    }

    #[test]
    fn page_files_and_links_agree() {
        assert_eq!(project_file(&slug("numr")), "projects/numr/index.html");
        assert_eq!(project_href(&slug("numr")).as_str(), "/projects/numr/");
        assert_eq!(topic_file(&slug("gpu")), "topics/gpu/index.html");
        assert_eq!(topic_href(&slug("gpu")).as_str(), "/topics/gpu/");
        assert_eq!(
            category_file(&slug("model-formats")),
            "categories/model-formats/index.html"
        );
        assert_eq!(
            category_href(&slug("model-formats")).as_str(),
            "/categories/model-formats/"
        );
    }

    #[test]
    fn an_evidence_link_shows_its_host_and_refuses_a_script() {
        let web = Url::parse("https://www.example.com/a?b=1").expect("url");
        let link = EvidenceLink::of(&web);
        assert_eq!(link.text, "www.example.com");
        assert_eq!(link.href.as_str(), "https://www.example.com/a?b=1");
        let script = Url::parse("javascript:alert(1)").expect("url");
        let link = EvidenceLink::of(&script);
        assert_eq!(link.text, "javascript:alert(1)");
        assert!(link.href.is_refused());
    }
}
