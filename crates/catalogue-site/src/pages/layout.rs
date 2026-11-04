//! The header, navigation and footer data that every page shares.

use crate::href::SafeHref;

/// The site name in the header and in every page title.
pub const SITE_NAME: &str = "Rust ML catalogue";

/// The address of the site, with no trailing slash, for files read away from the site.
pub const SITE_URL: &str = "https://ml-rust.github.io";

/// A section in the main navigation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NavItem {
    /// The topic list.
    Topics,
    /// The tier rules.
    Rules,
    /// The tier change log.
    Changes,
    /// The issue forms.
    Submit,
}

/// Every navigation item with its label and target, in display order.
const NAV: [(NavItem, &str, &str); 4] = [
    (NavItem::Topics, "Topics", "/topics/"),
    (NavItem::Rules, "Rules", "/rules/"),
    (NavItem::Changes, "Changes", "/changes/"),
    (NavItem::Submit, "Submit", "/submit/"),
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
    /// The link to the stylesheet.
    pub css: SafeHref,
    /// The main navigation.
    pub nav: Vec<NavLink>,
}

impl Layout {
    /// The layout of a page in section `current`, linking the stylesheet at `css`.
    pub(crate) fn new(css: SafeHref, current: Option<NavItem>) -> Self {
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
            css,
            nav,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_current_section_is_marked() {
        let layout = Layout::new(SafeHref::new("/assets/site.css"), Some(NavItem::Rules));
        let current: Vec<&str> = layout
            .nav
            .iter()
            .filter(|link| link.current)
            .map(|link| link.label)
            .collect();
        assert_eq!(current, ["Rules"]);
        let none = Layout::new(SafeHref::new("/assets/site.css"), None);
        assert!(none.nav.iter().all(|link| !link.current));
    }

    #[test]
    fn every_link_is_a_site_path_with_a_trailing_slash() {
        let layout = Layout::new(SafeHref::new("/assets/site.css"), None);
        let labels: Vec<&str> = layout.nav.iter().map(|link| link.label).collect();
        assert_eq!(labels, ["Topics", "Rules", "Changes", "Submit"]);
        for link in &layout.nav {
            assert!(!link.href.is_refused(), "{}", link.label);
            assert!(link.href.as_str().ends_with('/'), "{}", link.href);
        }
    }
}
