//! Built pages match the files in `tests/golden/` byte for byte.
//!
//! Set `CATALOGUE_UPDATE_GOLDEN=1` to write the current output to those files.

use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::PathBuf;

#[path = "common/build.rs"]
mod build;
mod common;
#[path = "common/fixture.rs"]
mod fixture;

use common::{TOPICS, catalogue, pages, thresholds};

/// The environment variable that rewrites the golden files.
const UPDATE: &str = "CATALOGUE_UPDATE_GOLDEN";

/// The pages of the one-entry build and their golden file names.
const STATIC_PAGES: [(&str, &str); 3] = [
    ("rules/index.html", "rules.html"),
    ("submit/index.html", "submit.html"),
    ("404.html", "404.html"),
];

/// The pages of the fixture build with a report and their golden file names.
const CONTENT_PAGES: [(&str, &str); 15] = [
    ("index.html", "index.html"),
    ("search/index.html", "search.html"),
    ("topics/tensors/index.html", "topic-tensors.html"),
    (
        "categories/tensor-libraries/index.html",
        "category-tensor-libraries.html",
    ),
    ("projects/alpha/index.html", "project-alpha.html"),
    ("projects/beta/index.html", "project-beta.html"),
    ("changes/index.html", "changes.html"),
    ("featured/index.html", "featured.html"),
    ("ml-rust/index.html", "ml-rust.html"),
    ("changes/feed.xml", "feed.xml"),
    ("data/catalogue.json", "catalogue.json"),
    ("data/compare/index.json", "compare-index.json"),
    ("data/compare/alpha.json", "compare-alpha.json"),
    ("llms.txt", "llms.txt"),
    ("sitemap.xml", "sitemap.xml"),
];

fn golden_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("golden")
}

/// Checks each page of `golden` in `site` against its golden file, or rewrites the file.
fn check(site: &BTreeMap<String, String>, golden: &[(&str, &str)]) {
    let update = env::var(UPDATE).is_ok_and(|value| value == "1");
    for &(page, name) in golden {
        let built = site
            .get(page)
            .unwrap_or_else(|| panic!("the build has no {page}"));
        let path = golden_dir().join(name);
        if update {
            fs::create_dir_all(golden_dir()).unwrap_or_else(|error| panic!("golden dir: {error}"));
            fs::write(&path, built).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
            continue;
        }
        let Ok(expected) = fs::read_to_string(&path) else {
            panic!(
                "{} is missing. Run the tests with {UPDATE}=1 to write it, then review it",
                path.display()
            );
        };
        assert!(
            *built == expected,
            "{page} differs from {}. Run the tests with {UPDATE}=1 to rewrite it, then review the diff",
            path.display()
        );
    }
}

#[test]
fn static_pages_match_the_golden_files() {
    let site = pages(&catalogue(TOPICS, Some(&thresholds(Some("2026-11-03")))));
    check(&site, &STATIC_PAGES);
}

#[test]
fn content_pages_match_the_golden_files() {
    check(&fixture::site(&fixture::files(), true), &CONTENT_PAGES);
}
