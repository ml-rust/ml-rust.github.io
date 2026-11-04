//! `/categories/<id>/`: one category with its blurb and entries.

use askama::Template;
use catalogue_core::Category;

use super::layout::Layout;
use super::shared::{EntryGroup, group_install_rows, movement_note, order_text};
use crate::inputs::SiteInputs;

/// The page of one category.
#[derive(Debug, Template)]
#[template(path = "category.html")]
pub(crate) struct CategoryPage<'a> {
    /// The shared layout.
    pub layout: Layout,
    /// The category name.
    pub name: &'a str,
    /// The sentence that defines the category.
    pub blurb: &'a str,
    /// The entries, strongest tier first, each row leading with how to get the project.
    pub groups: Vec<EntryGroup<'a>>,
    /// The sentence that states the order of the list.
    pub order: &'static str,
    /// The line shown when the history cannot measure movement.
    pub movement_note: Option<&'static str>,
}

impl<'a> CategoryPage<'a> {
    /// The page of `category` in `layout`.
    pub(crate) fn new(layout: Layout, inputs: &SiteInputs<'a>, category: &'a Category) -> Self {
        let slugs = inputs.catalogue.entries_in_category(&category.id);
        Self {
            layout,
            name: &category.name,
            blurb: &category.blurb,
            groups: group_install_rows(inputs, &slugs),
            order: order_text(inputs.report),
            movement_note: movement_note(inputs),
        }
    }
}
