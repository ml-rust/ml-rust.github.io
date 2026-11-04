//! The feed, the data file and the text files come from the same inputs as the pages.

#[path = "common/build.rs"]
mod build;
#[path = "common/fixture.rs"]
mod fixture;

use std::collections::BTreeMap;

use serde_json::Value;

const AUTODISCOVERY: &str = "<link rel=\"alternate\" type=\"application/atom+xml\" title=\"Tier changes and new entries\" href=\"/changes/feed.xml\">";

fn tiered() -> BTreeMap<String, String> {
    fixture::site(&fixture::files(), true)
}

fn untiered() -> BTreeMap<String, String> {
    fixture::site(&fixture::files(), false)
}

fn data_file(site: &BTreeMap<String, String>) -> Value {
    serde_json::from_str(&site["data/catalogue.json"]).expect("valid JSON")
}

#[test]
fn every_file_is_emitted_with_and_without_a_report() {
    for site in [tiered(), untiered()] {
        for path in [
            "changes/feed.xml",
            "data/catalogue.json",
            "data/README.txt",
            "llms.txt",
            "sitemap.xml",
            "robots.txt",
            "featured/index.html",
        ] {
            assert!(site.contains_key(path), "no {path}");
        }
    }
}

#[test]
fn two_builds_give_the_same_bytes() {
    assert_eq!(tiered(), tiered());
    assert_eq!(untiered(), untiered());
}

#[test]
fn every_page_names_the_feed() {
    for (path, text) in tiered().iter().filter(|(path, _)| path.ends_with(".html")) {
        assert!(text.contains(AUTODISCOVERY), "{path}");
    }
}

#[test]
fn the_feed_lists_changes_newest_first_with_one_first_check_item() {
    let site = tiered();
    let feed = &site["changes/feed.xml"];
    let order = [
        "<title>Alpha: Listed to Featured</title>",
        "<title>New entry: Delta</title>",
        "<title>Gamma: Listed to Archived</title>",
        "<title>First check: 1 entry: Listed 1</title>",
    ];
    let mut rest = feed.as_str();
    for title in order {
        let at = rest
            .find(title)
            .unwrap_or_else(|| panic!("missing {title} in {feed}"));
        rest = &rest[at..];
    }
    assert_eq!(feed.matches("<entry>").count(), 4, "{feed}");
    assert!(!feed.contains("omega"), "{feed}");
    assert!(
        feed.contains("<updated>2026-11-04T00:00:00Z</updated>\n<author>"),
        "{feed}"
    );
}

#[test]
fn the_feed_of_a_build_without_tiers_holds_no_item() {
    let site = untiered();
    assert_eq!(site["changes/feed.xml"].matches("<entry>").count(), 0);
}

#[test]
fn the_data_file_matches_the_report_and_the_effective_topics() {
    let value = data_file(&tiered());
    assert_eq!(value["schema"], 1);
    assert_eq!(value["generated"], "2026-11-04");
    assert_eq!(value["license"], "CC-BY-4.0");
    let entries = value["entries"].as_array().expect("entries");
    let slugs: Vec<&str> = entries
        .iter()
        .filter_map(|entry| entry["slug"].as_str())
        .collect();
    assert_eq!(slugs, ["alpha", "beta", "delta", "gamma"]);
    let alpha = &entries[0];
    assert_eq!(alpha["tier"], "featured");
    assert_eq!(alpha["crates"][1], "alpha-extra");
    assert_eq!(alpha["category"], "tensor-libraries");
    assert_eq!(alpha["part_of"], Value::Null);
    assert_eq!(
        alpha["description"]["source_url"],
        "https://crates.io/crates/alpha-crate"
    );
    assert_eq!(alpha["reasons"][0]["rule"], "expected-interval");
    let delta = &entries[2];
    assert_eq!(
        delta["topics"][0], "tensors",
        "a topic judgment moves Delta"
    );
}

#[test]
fn the_data_file_holds_no_field_the_pages_hide() {
    let value = data_file(&tiered());
    let allowed_top = [
        "schema",
        "generated",
        "license",
        "thresholds_version",
        "entries",
    ];
    let top = value.as_object().expect("object");
    assert!(
        top.keys().all(|key| allowed_top.contains(&key.as_str())),
        "{top:?}"
    );
    let allowed_entry = [
        "slug",
        "name",
        "kind",
        "repo",
        "crates",
        "topics",
        "category",
        "part_of",
        "description",
        "tier",
        "reasons",
        "signals",
    ];
    for entry in value["entries"].as_array().expect("entries") {
        let fields = entry.as_object().expect("object");
        assert!(
            fields
                .keys()
                .all(|key| allowed_entry.contains(&key.as_str())),
            "{fields:?}"
        );
        for signal in entry["signals"].as_array().expect("signals") {
            if signal["value"].is_null() {
                assert!(signal["source"].is_null(), "{signal}");
                assert!(signal["fetched"].is_null(), "{signal}");
            }
        }
    }
}

#[test]
fn the_data_file_of_a_build_without_tiers_holds_no_tier() {
    let value = data_file(&untiered());
    assert_eq!(value["generated"], Value::Null);
    for entry in value["entries"].as_array().expect("entries") {
        assert_eq!(entry["tier"], Value::Null);
        assert_eq!(entry["signals"].as_array().expect("signals").len(), 0);
    }
}

#[test]
fn llms_lists_the_topics_and_categories_of_the_data() {
    let site = tiered();
    let llms = &site["llms.txt"];
    for part in [
        "https://ml-rust.github.io/data/catalogue.json",
        "https://ml-rust.github.io/rules/",
        "- Machine learning: https://ml-rust.github.io/topics/ml/",
        "- Tensors: https://ml-rust.github.io/topics/tensors/",
        "- Vector databases: https://ml-rust.github.io/categories/vector-databases/",
    ] {
        assert!(llms.contains(part), "missing {part} in {llms}");
    }
}

#[test]
fn the_sitemap_lists_every_page_and_robots_points_at_it() {
    let site = tiered();
    let sitemap = &site["sitemap.xml"];
    for url in [
        "<loc>https://ml-rust.github.io/</loc>",
        "<loc>https://ml-rust.github.io/featured/</loc>",
        "<loc>https://ml-rust.github.io/projects/alpha/</loc>",
    ] {
        assert!(sitemap.contains(url), "missing {url} in {sitemap}");
    }
    assert!(!sitemap.contains("404"), "{sitemap}");
    assert!(site["robots.txt"].contains("Sitemap: https://ml-rust.github.io/sitemap.xml"));
}

#[test]
fn the_data_readme_names_the_license() {
    let site = tiered();
    assert!(site["data/README.txt"].contains("License: CC-BY-4.0"));
}
