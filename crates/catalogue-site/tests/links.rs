//! Every internal link resolves to an emitted file or to a page this crate does not emit.

use std::collections::BTreeMap;

#[path = "common/build.rs"]
mod build;
mod common;
#[path = "common/fixture.rs"]
mod fixture;

use common::{TOPICS, catalogue, pages, thresholds};

/// Pages the site links to that this crate does not emit.
/// An entry that ends in `/` also covers every page below it.
const PENDING: [&str; 1] = ["/search/"];

/// The value of every `href`, `src` and `action` attribute in `html`.
fn targets(html: &str) -> Vec<&str> {
    let mut out = Vec::new();
    for attribute in [" href=\"", " src=\"", " action=\""] {
        let mut rest = html;
        while let Some(start) = rest.find(attribute) {
            let value = &rest[start + attribute.len()..];
            let end = value.find('"').unwrap_or(value.len());
            out.push(&value[..end]);
            rest = &value[end..];
        }
    }
    out
}

/// The emitted file that serves `path`, a site path without query or fragment.
fn served_file(path: &str) -> String {
    let file = path.trim_start_matches('/');
    if file.is_empty() || file.ends_with('/') {
        format!("{file}index.html")
    } else {
        file.to_owned()
    }
}

/// Whether `path` is a page this crate does not emit.
fn is_pending(path: &str) -> bool {
    PENDING.iter().any(|pending| {
        path == *pending || (*pending != "/" && path.starts_with(pending) && path.ends_with('/'))
    })
}

/// Every internal target that resolves to nothing, as `page: target`.
fn broken(site: &BTreeMap<String, String>) -> Vec<String> {
    let mut out = Vec::new();
    for (page, html) in site.iter().filter(|(path, _)| path.ends_with(".html")) {
        for target in targets(html) {
            if target.starts_with('#') || target.starts_with("https://") {
                continue;
            }
            let path = target.split(['?', '#']).next().unwrap_or(target);
            let resolved = path.starts_with('/')
                && !path.starts_with("//")
                && (site.contains_key(&served_file(path)) || is_pending(path));
            if !resolved {
                out.push(format!("{page}: {target}"));
            }
        }
    }
    out
}

#[test]
fn every_internal_link_resolves() {
    let site = pages(&catalogue(TOPICS, Some(&thresholds(Some("2026-11-03")))));
    let found = broken(&site);
    assert!(found.is_empty(), "broken links: {found:?}");
    let all: Vec<&str> = site.values().flat_map(|html| targets(html)).collect();
    assert!(all.contains(&"/rules/"), "{all:?}");
    assert!(all.contains(&"/submit/"), "{all:?}");
    assert!(
        all.iter().any(|target| target.starts_with("/assets/site.")),
        "{all:?}"
    );
    assert!(all.contains(&"/topics/tensors/"), "{all:?}");
}

#[test]
fn every_link_of_the_fixture_resolves_with_and_without_a_report() {
    for tiered in [true, false] {
        let site = fixture::site(&fixture::files(), tiered);
        let found = broken(&site);
        assert!(found.is_empty(), "tiered {tiered}: broken links: {found:?}");
        let all: Vec<&str> = site.values().flat_map(|html| targets(html)).collect();
        for target in [
            "/",
            "/topics/",
            "/topics/ml/",
            "/topics/tensors/",
            "/projects/alpha/",
            "/projects/beta/",
            "/projects/gamma/",
            "/projects/delta/",
            "/changes/",
        ] {
            assert!(
                all.contains(&target),
                "tiered {tiered}: no link to {target}"
            );
        }
    }
}

#[test]
fn the_check_rejects_links_to_missing_emitted_pages() {
    let mut site = BTreeMap::new();
    site.insert(
        "a.html".to_owned(),
        r##"<a href="/rules"></a><a href="/rules/"></a><a href="/nowhere/"></a><a href="/topics/x/"></a><a href="javascript:x"></a><a href="#main"></a>"##
            .to_owned(),
    );
    site.insert("rules/index.html".to_owned(), String::new());
    assert_eq!(
        broken(&site),
        [
            "a.html: /rules",
            "a.html: /nowhere/",
            "a.html: /topics/x/",
            "a.html: javascript:x"
        ]
    );
}

#[test]
fn the_skip_link_target_exists_on_every_page() {
    let site = pages(&catalogue(TOPICS, Some(&thresholds(None))));
    for (path, html) in site.iter().filter(|(path, _)| path.ends_with(".html")) {
        if html.contains("href=\"#main\"") {
            assert!(html.contains("<main id=\"main\""), "{path}");
        }
    }
}
