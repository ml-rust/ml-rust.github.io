//! The rules, submit and not-found pages match the files in `tests/golden/` byte for byte.
//!
//! Set `CATALOGUE_UPDATE_GOLDEN=1` to write the current output to those files.

use std::env;
use std::fs;
use std::path::PathBuf;

mod common;

use common::{TOPICS, catalogue, pages, thresholds};

/// The environment variable that rewrites the golden files.
const UPDATE: &str = "CATALOGUE_UPDATE_GOLDEN";

/// Each built page and its golden file name.
const GOLDEN: [(&str, &str); 3] = [
    ("rules/index.html", "rules.html"),
    ("submit/index.html", "submit.html"),
    ("404.html", "404.html"),
];

fn golden_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("golden")
}

#[test]
fn pages_match_the_golden_files() {
    let site = pages(&catalogue(TOPICS, Some(&thresholds(Some("2026-11-03")))));
    let update = env::var(UPDATE).is_ok_and(|value| value == "1");
    for (page, name) in GOLDEN {
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
