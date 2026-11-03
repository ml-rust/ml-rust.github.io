//! `/404.html`: the page GitHub Pages serves for an unknown path.

use askama::Template;

use super::layout::Layout;

/// The not-found page.
#[derive(Debug, Template)]
#[template(path = "404.html")]
pub(crate) struct NotFoundPage {
    /// The shared layout.
    pub layout: Layout,
}

impl NotFoundPage {
    /// The not-found page in `layout`.
    pub(crate) fn new(layout: Layout) -> Self {
        Self { layout }
    }
}
