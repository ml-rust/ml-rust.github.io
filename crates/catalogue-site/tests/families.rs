//! Lists show one row per project family, strongest first, and pages link the family.

#[path = "common/build.rs"]
mod build;
#[path = "common/fixture.rs"]
mod fixture;

use std::collections::BTreeMap;

use catalogue_core::{EntryReport, Movement, Slug, Tier};
use catalogue_site::{SiteInputs, search_records};

/// The sentence the Featured lists show.
const MOST_DOWNLOADS_FIRST: &str = "Order: most downloads first, then name, with unmeasured values last. A library counts downloads over 30 days. An app counts binary downloads per release. An app with none counts its crate downloads over 30 days.";

const STRONGEST_FIRST: &str = "Order: strongest tier first, then most downloads, then name, with unmeasured values last. A library counts downloads over 30 days. An app counts binary downloads per release. An app with none counts its crate downloads over 30 days.";

/// A library on the `tensors` topic and in the tensor category, with `extra` as more fields.
fn extra_entry(name: &str, extra: &str) -> (String, String) {
    (
        format!("entries/{name}.toml"),
        format!(
            "{extra}name = \"{name}\"\nkind = \"library\"\nrepo = \"https://github.com/o/{name}\"\n\
             crates = [\"{name}\"]\ntopics = [\"tensors\"]\ncategory = \"tensor-libraries\"\n\
             added = 2026-08-01\n\n[description]\ntext = \"Extra.\"\nsource = \"human\"\n"
        ),
    )
}

fn record(tier: Tier) -> EntryReport {
    EntryReport {
        tier,
        stable: false,
        dormant: Some(false),
        demotion_pending: None,
        reasons: Vec::new(),
        unknown: Vec::new(),
        notes: Vec::new(),
        signals: Vec::new(),
        search_terms: Vec::new(),
        readme: None,
    }
}

/// A build over the fixture plus `extra` entries, each with its tier.
fn build_with(extra: &[(&str, &str, Tier)]) -> BTreeMap<String, String> {
    let mut files = fixture::files();
    files.extend(
        extra
            .iter()
            .map(|(name, fields, _)| extra_entry(name, fields)),
    );
    let (catalogue, mut report) = fixture::data(&files);
    for (name, _, tier) in extra {
        report
            .entries
            .insert(Slug::new(*name).expect("slug"), record(*tier));
    }
    build::render(&SiteInputs {
        catalogue: &catalogue,
        report: Some(&report),
        changes: &[],
        first_check: None,
        movement: &Movement::NeedsHistory,
    })
}

#[test]
fn a_list_shows_the_parent_with_its_related_count() {
    let site = build_with(&[("alpha-ext", "part_of = \"alpha\"\n", Tier::Listed)]);
    for page in [
        "topics/tensors/index.html",
        "categories/tensor-libraries/index.html",
    ] {
        let html = &site[page];
        assert!(html.contains(STRONGEST_FIRST), "{page}");
        assert!(
            html.contains(
                "<a class=\"entry-related\" href=\"/projects/alpha/\">and 1 related project</a>"
            ),
            "{page}: {html}"
        );
        assert!(!html.contains(">alpha-ext</a>"), "{page}: {html}");
    }
}

#[test]
fn a_child_page_links_up_and_the_parent_page_lists_its_children() {
    let site = build_with(&[("alpha-ext", "part_of = \"alpha\"\n", Tier::Listed)]);
    assert!(
        site["projects/alpha-ext/index.html"]
            .contains("<li>Part of <a href=\"/projects/alpha/\">Alpha</a>.</li>")
    );
    let alpha = &site["projects/alpha/index.html"];
    assert!(
        alpha.contains("<h2 id=\"related\">Related projects</h2>"),
        "{alpha}"
    );
    assert!(
        alpha.contains("<li><a href=\"/projects/alpha-ext/\">alpha-ext</a></li>"),
        "{alpha}"
    );
}

#[test]
fn a_child_whose_parent_is_not_in_the_list_keeps_its_own_row() {
    let site = build_with(&[("beta-ext", "part_of = \"beta\"\n", Tier::Listed)]);
    let tensors = &site["topics/tensors/index.html"];
    assert!(
        tensors.contains("/projects/beta-ext/\">beta-ext</a>"),
        "{tensors}"
    );
    assert!(!tensors.contains("related project"), "{tensors}");
}

#[test]
fn a_child_stays_searchable_with_its_own_tier() {
    let mut files = fixture::files();
    files.push(extra_entry("alpha-ext", "part_of = \"alpha\"\n"));
    let (catalogue, mut report) = fixture::data(&files);
    report.entries.insert(
        Slug::new("alpha-ext").expect("slug"),
        record(Tier::Archived),
    );
    let records = search_records(&catalogue, Some(&report));
    assert!(
        records
            .iter()
            .any(|record| record.url.contains("/projects/alpha-ext/")),
        "the child has no search record"
    );
    let site = build_with(&[("alpha-ext", "part_of = \"alpha\"\n", Tier::Archived)]);
    assert!(site["projects/alpha-ext/index.html"].contains("tier-archived"));
}

#[test]
fn the_home_page_shows_eight_families_and_links_the_full_list() {
    let names: Vec<String> = (0..9).map(|at| format!("extra-{at}")).collect();
    let mut extra: Vec<(&str, &str, Tier)> = names
        .iter()
        .map(|name| (name.as_str(), "", Tier::Featured))
        .collect();
    extra.push(("extra-0-child", "part_of = \"extra-0\"\n", Tier::Featured));
    let site = build_with(&extra);
    let home = &site["index.html"];
    let start = home.find("id=\"featured\"").expect("the Featured list");
    let end = home.find("id=\"changes\"").expect("the changes line");
    let block = &home[start..end];
    assert_eq!(block.matches("class=\"entry-row\"").count(), 8, "{block}");
    assert!(
        block.contains("<a href=\"/featured/\">All 11 Featured projects<span"),
        "{block}"
    );
    assert!(block.contains("and 1 related project"), "{block}");
    assert!(block.contains(MOST_DOWNLOADS_FIRST), "{block}");
    let full = &site["featured/index.html"];
    assert_eq!(full.matches("class=\"entry-row\"").count(), 10, "{full}");
    assert!(!full.contains(">extra-0-child</a>"), "{full}");
}

#[test]
fn no_list_prints_a_rank_or_a_composite() {
    let site = fixture::site(&fixture::files(), true);
    for (path, text) in site.iter().filter(|(path, _)| path.ends_with(".html")) {
        for word in ["Rank", "rank-", "Score:", "composite-"] {
            assert!(!text.contains(word), "{path} holds {word}");
        }
    }
}

#[test]
fn the_featured_page_says_when_tiers_are_not_published() {
    let site = fixture::site(&fixture::files(), false);
    let page = &site["featured/index.html"];
    assert!(page.contains("Tiers are not published"), "{page}");
    assert!(!page.contains("entry-row"), "{page}");
}
