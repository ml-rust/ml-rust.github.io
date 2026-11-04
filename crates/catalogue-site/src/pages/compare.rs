//! `/compare/`: the static shell of the side-by-side table. The script fills it from the data file.

use askama::Template;

use super::layout::Layout;

/// The compare page.
#[derive(Debug, Template)]
#[template(path = "compare.html")]
pub(crate) struct ComparePage {
    /// The shared layout.
    pub layout: Layout,
}

impl ComparePage {
    /// The compare page in `layout`.
    pub(crate) fn new(layout: Layout) -> Self {
        Self { layout }
    }
}
