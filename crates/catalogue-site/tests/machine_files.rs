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
            "compare/index.html",
            "data/compare/index.json",
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
    assert_eq!(value["schema"], 2);
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
fn the_install_docs_and_why_fields_hold_what_the_entry_page_shows() {
    let site = tiered();
    let value = data_file(&site);
    let page = &site["projects/alpha/index.html"];
    let alpha = &value["entries"][0];
    let command = alpha["install"]["command"].as_str().expect("command");
    assert!(
        page.contains(&format!("data-install-command=\"{command}\"")),
        "{command}"
    );
    let docs = alpha["docs"].as_str().expect("docs");
    assert!(page.contains(&format!("href=\"{docs}\"")), "{docs}");
    let lacks = alpha["why"]["lacks"][0].as_str().expect("a lack");
    assert!(page.contains(lacks), "{lacks}");
    let beta = &value["entries"][1];
    assert_eq!(beta["docs"], Value::Null, "an app has no docs.rs link");
    assert_eq!(beta["install"]["command"], "brew install beta");
    let beta_page = &site["projects/beta/index.html"];
    let found = beta["why"]["found"][0].as_str().expect("a finding");
    assert!(beta_page.contains(found), "{found}");
}

/// The JSON of the compare file of `slug`.
fn compare_file(site: &BTreeMap<String, String>, slug: &str) -> Value {
    let path = format!("data/compare/{slug}.json");
    let text = site
        .get(&path)
        .unwrap_or_else(|| panic!("the build has no {path}"));
    serde_json::from_str(text).expect("valid JSON")
}

#[test]
fn each_entry_has_a_compare_file_and_a_line_in_the_index() {
    for site in [tiered(), untiered()] {
        let value = data_file(&site);
        let index = compare_file(&site, "index");
        let lines = index["entries"].as_array().expect("index entries");
        let entries = value["entries"].as_array().expect("entries");
        assert_eq!(lines.len(), entries.len());
        for (line, entry) in lines.iter().zip(entries) {
            assert_eq!(line["slug"], entry["slug"]);
            assert_eq!(line["name"], entry["name"]);
            assert_eq!(line.as_object().expect("line").len(), 2, "{line}");
            let slug = entry["slug"].as_str().expect("slug");
            assert_eq!(compare_file(&site, slug)["slug"], entry["slug"]);
        }
        let files = site
            .keys()
            .filter(|path| path.starts_with("data/compare/"))
            .count();
        assert_eq!(files, entries.len() + 1, "no other file in the directory");
    }
}

#[test]
fn a_compare_file_holds_the_same_values_as_its_record_in_the_data_file() {
    for site in [tiered(), untiered()] {
        let value = data_file(&site);
        for entry in value["entries"].as_array().expect("entries") {
            let slug = entry["slug"].as_str().expect("slug");
            let file = compare_file(&site, slug);
            let fields = file.as_object().expect("object");
            for (key, found) in fields {
                if key != "signals" {
                    assert_eq!(found, &entry[key], "{slug}: {key}");
                }
            }
            for key in [
                "slug",
                "name",
                "kind",
                "repo",
                "description",
                "install",
                "docs",
                "tier",
                "why",
                "native_word",
                "signals",
            ] {
                assert!(fields.contains_key(key), "{slug}: no {key}");
            }
            let all = entry["signals"].as_array().expect("signals");
            for signal in file["signals"].as_array().expect("signals") {
                assert!(all.contains(signal), "{slug}: {signal}");
                assert_ne!(signal["name"], "stars", "{slug}");
            }
        }
    }
}

#[test]
fn a_compare_file_holds_each_signal_the_page_reads() {
    let site = tiered();
    let file = compare_file(&site, "alpha");
    let names: Vec<&str> = file["signals"]
        .as_array()
        .expect("signals")
        .iter()
        .filter_map(|signal| signal["name"].as_str())
        .collect();
    for name in [
        "active_users",
        "last_release",
        "native_code",
        "license",
        "runs_on",
    ] {
        assert!(names.contains(&name), "no {name} in {names:?}");
    }
    assert_eq!(file["native_word"], "Required");
    let license = file["signals"]
        .as_array()
        .expect("signals")
        .iter()
        .find(|signal| signal["name"] == "license")
        .expect("the license signal");
    assert_eq!(license["source"]["url"], "https://crates.io/data-access");
    assert_eq!(license["fetched"], "2026-11-03");
    assert!(
        site["data/compare/alpha.json"].len() < site["data/catalogue.json"].len(),
        "the file is a part of the data file"
    );
}

#[test]
fn markup_in_a_name_stays_escaped_in_the_compare_files_and_parses_back() {
    let mut files = fixture::files();
    for (_, contents) in &mut files {
        *contents = contents.replace("name = \"Alpha\"", "name = '''<script>&'''");
    }
    let site = fixture::site(&files, true);
    for path in ["data/compare/alpha.json", "data/compare/index.json"] {
        assert!(!site[path].contains("<script"), "{path}");
        assert!(site[path].contains("\\u003cscript\\u003e\\u0026"), "{path}");
    }
    assert_eq!(compare_file(&site, "alpha")["name"], "<script>&");
}

#[test]
fn the_data_readme_names_the_compare_files() {
    let site = tiered();
    for part in ["data/compare/index.json", "data/compare/<slug>.json"] {
        assert!(site["data/README.txt"].contains(part), "no {part}");
    }
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
        "install",
        "docs",
        "tier",
        "why",
        "native_word",
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
