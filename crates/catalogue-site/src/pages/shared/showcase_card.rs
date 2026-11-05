//! The hand-picked ML Rust projects of the home page, from `data/showcase.toml`.

use catalogue_core::Catalogue;

use super::listing::project_count;
use super::paths::project_href;
use crate::href::SafeHref;

/// One tile of `showcase_grid.html`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ShowcaseCard<'a> {
    /// The entry slug. It picks the icon.
    pub id: String,
    /// The entry name.
    pub name: &'a str,
    /// The sourced description of the entry.
    pub description: &'a str,
    /// The link to the entry page.
    pub href: SafeHref,
}

/// The tiles and the count of every ML Rust project.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ShowcaseSection<'a> {
    /// The tiles, in file order.
    pub cards: Vec<ShowcaseCard<'a>>,
    /// The count of every org-affiliated entry, as text.
    pub total: String,
}

/// The section of `catalogue`. `None` when `showcase.toml` lists no project.
pub(crate) fn showcase_section(catalogue: &Catalogue) -> Option<ShowcaseSection<'_>> {
    let cards: Vec<ShowcaseCard<'_>> = catalogue
        .showcase()
        .iter()
        .filter_map(|slug| {
            catalogue.entry(slug).map(|entry| ShowcaseCard {
                id: slug.as_str().to_owned(),
                name: &entry.name,
                description: &entry.description.text,
                href: project_href(slug),
            })
        })
        .collect();
    if cards.is_empty() {
        return None;
    }
    let total = catalogue
        .entries()
        .values()
        .filter(|entry| entry.org_affiliated)
        .count();
    Some(ShowcaseSection {
        cards,
        total: project_count(total),
    })
}
