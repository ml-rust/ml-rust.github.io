//! The category tiles: one per category, with its count.

use catalogue_core::Catalogue;

use super::listing::project_count;
use super::paths::category_href;
use crate::href::SafeHref;

/// One tile of `category_tile.html`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CategoryCard<'a> {
    /// The category id. It picks the icon.
    pub id: String,
    /// The category name.
    pub name: &'a str,
    /// The sentence that defines the category.
    pub blurb: &'a str,
    /// The link to the category page.
    pub href: SafeHref,
    /// The project count as text.
    pub count: String,
}

/// One tile per category, in file order.
pub(crate) fn category_cards(catalogue: &Catalogue) -> Vec<CategoryCard<'_>> {
    catalogue
        .categories()
        .iter()
        .map(|category| CategoryCard {
            id: category.id.as_str().to_owned(),
            name: &category.name,
            blurb: &category.blurb,
            href: category_href(&category.id),
            count: project_count(catalogue.entries_in_category(&category.id).len()),
        })
        .collect()
}
