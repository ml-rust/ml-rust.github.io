//! The header, navigation and footer data that every page shares.

use super::meta::PageMeta;
use crate::href::SafeHref;

/// The site name in the header and in every page title.
pub const SITE_NAME: &str = "ML Rust catalogue";

/// The address of the site, with no trailing slash, for files read away from the site.
pub const SITE_URL: &str = "https://ml-rust.github.io";

/// A section in the main navigation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NavItem {
    /// The jobs and categories, and every page under them.
    Explore,
    /// The tier rules.
    Rules,
    /// The tier change log.
    Changes,
    /// The issue forms.
    Submit,
}

/// Every navigation item with its label and target, in display order.
const NAV: [(NavItem, &str, &str); 4] = [
    (NavItem::Explore, "Explore", "/topics/"),
    (NavItem::Rules, "Rules", "/rules/"),
    (NavItem::Changes, "Changes", "/changes/"),
    (NavItem::Submit, "Add project", "/submit/"),
];

/// One navigation link.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NavLink {
    /// The link text.
    pub label: &'static str,
    /// The link target.
    pub href: SafeHref,
    /// Whether the link points to the page that shows it.
    pub current: bool,
}

/// The data `base.html` reads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Layout {
    /// [`SITE_NAME`].
    pub site_name: &'static str,
    /// The title, description and address of the page.
    pub meta: PageMeta,
    /// The link to the stylesheet.
    pub css: SafeHref,
    /// The link to the favicon.
    pub favicon: SafeHref,
    /// The main navigation.
    pub nav: Vec<NavLink>,
    /// The scripts of the page, in load order. Empty on a page with no script.
    pub scripts: Vec<SafeHref>,
}

impl Layout {
    /// The layout of a page in section `current`, linking the stylesheet at `css` and the favicon
    /// at `favicon`.
    pub(crate) fn new(
        css: SafeHref,
        favicon: SafeHref,
        current: Option<NavItem>,
        meta: PageMeta,
    ) -> Self {
        let nav = NAV
            .iter()
            .map(|&(item, label, href)| NavLink {
                label,
                href: SafeHref::new(href),
                current: current == Some(item),
            })
            .collect();
        Self {
            site_name: SITE_NAME,
            meta,
            css,
            favicon,
            nav,
            scripts: Vec::new(),
        }
    }

    /// The layout with the module script at `src` added after the ones it holds.
    pub(crate) fn with_script(mut self, src: SafeHref) -> Self {
        self.scripts.push(src);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn css() -> SafeHref {
        SafeHref::new("/assets/site.css")
    }

    fn favicon() -> SafeHref {
        SafeHref::new("/assets/favicon.svg")
    }

    #[test]
    fn only_the_current_section_is_marked() {
        let layout = Layout::new(css(), favicon(), Some(NavItem::Rules), PageMeta::rules());
        let current: Vec<&str> = layout
            .nav
            .iter()
            .filter(|link| link.current)
            .map(|link| link.label)
            .collect();
        assert_eq!(current, ["Rules"]);
        let none = Layout::new(css(), favicon(), None, PageMeta::home());
        assert!(none.nav.iter().all(|link| !link.current));
    }

    #[test]
    fn every_link_is_a_site_path_with_a_trailing_slash() {
        let layout = Layout::new(css(), favicon(), None, PageMeta::home());
        let labels: Vec<&str> = layout.nav.iter().map(|link| link.label).collect();
        assert_eq!(labels, ["Explore", "Rules", "Changes", "Add project"]);
        for link in &layout.nav {
            assert!(!link.href.is_refused(), "{}", link.label);
            assert!(link.href.as_str().ends_with('/'), "{}", link.href);
        }
    }
}
