//! The home, topic, entry and change pages show what the data and the report hold.

#[path = "common/build.rs"]
mod build;
#[path = "common/fixture.rs"]
mod fixture;

use std::collections::BTreeMap;

/// Markup that only a build with a report prints.
const TIER_MARKUP: [&str; 5] = [
    "tier-badge",
    "data-tier",
    "signal-table",
    "tier-counts",
    "Why this tier",
];

fn tiered() -> BTreeMap<String, String> {
    fixture::site(&fixture::files(), true)
}

fn untiered() -> BTreeMap<String, String> {
    fixture::site(&fixture::files(), false)
}

/// Asserts that `page` holds each of `parts`, in order.
fn in_order(page: &str, parts: &[&str]) {
    let mut rest = page;
    for part in parts {
        let Some(at) = rest.find(part) else {
            panic!("missing {part:?} after the earlier parts in:\n{page}");
        };
        rest = &rest[at + part.len()..];
    }
}

#[test]
fn every_content_page_is_emitted() {
    let site = tiered();
    for path in [
        "index.html",
        "topics/index.html",
        "topics/ml/index.html",
        "topics/tensors/index.html",
        "categories/inference-runtimes/index.html",
        "categories/tensor-libraries/index.html",
        "categories/vector-databases/index.html",
        "projects/alpha/index.html",
        "projects/beta/index.html",
        "projects/gamma/index.html",
        "projects/delta/index.html",
        "changes/index.html",
    ] {
        assert!(site.contains_key(path), "no {path}");
    }
}

#[test]
fn without_a_report_no_page_shows_a_tier_or_a_signal() {
    let site = untiered();
    for (path, text) in site.iter().filter(|(path, _)| path.ends_with(".html")) {
        for markup in TIER_MARKUP {
            assert!(!text.contains(markup), "{path} shows {markup}");
        }
    }
    assert!(site["changes/index.html"].contains("Tiers are not published"));
    assert!(!site["changes/index.html"].contains("<table>"));
    let home = &site["index.html"];
    assert!(home.contains("Tiers are not published."), "{home}");
    assert!(!home.contains("id=\"featured\""), "{home}");
    assert!(!home.contains("/changes/\">All tier changes"), "{home}");
    assert!(
        home.contains("<p class=\"topic-count\">4 entries</p>"),
        "{home}"
    );
    let ml = &site["topics/ml/index.html"];
    assert!(ml.contains("Entries are sorted by name."), "{ml}");
    assert!(!ml.contains("<h3>"), "{ml}");
    assert!(site["projects/alpha/index.html"].contains("<h2 id=\"tier\">Tier</h2>"));
}

#[test]
fn the_home_page_lists_topics_featured_recent_entries_and_changes() {
    let site = tiered();
    let home = &site["index.html"];
    in_order(
        home,
        &[
            "<a href=\"/rules/\">published rules</a>",
            "<a href=\"/topics/ml/\">Machine learning</a>",
            "<li>Featured: 1</li>",
            "<li>Listed: 1</li>",
            "<li>New: 1</li>",
            "<li>Archived: 1</li>",
            "Subtopics: <a href=\"/topics/tensors/\">Tensors</a>",
            "<h2 id=\"categories\">Browse by category</h2>",
            "<a href=\"/categories/inference-runtimes/\">Inference runtimes</a>",
            "<a href=\"/categories/tensor-libraries/\">Tensor libraries</a>",
            "<a href=\"/categories/vector-databases/\">Vector databases</a>",
            "<h2 id=\"featured\">Featured</h2>",
            "/projects/alpha/\">Alpha</a>",
            "<h2 id=\"recent\">Recently added</h2>",
            "/projects/beta/\">Beta</a>",
            "/projects/delta/\">Delta</a>",
            "/projects/alpha/\">Alpha</a>",
            "/projects/gamma/\">Gamma</a>",
            "<h2 id=\"changes\">Recent tier changes</h2>",
            "/projects/alpha/\">Alpha</a></td><td>Listed</td><td>Featured</td>",
            "/projects/delta/\">Delta</a></td><td>First check</td><td>New</td>",
            "<td>omega</td><td>First check</td><td>Listed</td>",
            "<a href=\"/changes/\">All tier changes</a>",
        ],
    );
}

#[test]
fn a_topic_page_groups_its_entries_by_tier() {
    let site = tiered();
    let tensors = &site["topics/tensors/index.html"];
    in_order(
        tensors,
        &[
            "<h1>Tensors</h1>",
            "Part of <a href=\"/topics/ml/\">Machine learning</a>.",
            "Libraries for n-dimensional arrays and the math on them.",
            "<a href=\"https://en.wikipedia.org/wiki/Tensor_(machine_learning)\">en.wikipedia.org</a>",
            "<h3>Featured</h3>",
            "Alpha</a>",
            "<h3>New</h3>",
            "Delta</a>",
            "<h3>Archived</h3>",
            "Gamma</a>",
        ],
    );
    assert!(!tensors.contains("<h3>Listed</h3>"), "{tensors}");
    let ml = &site["topics/ml/index.html"];
    in_order(
        ml,
        &[
            "<h2 id=\"subtopics\">Subtopics</h2>",
            "<a href=\"/topics/tensors/\">Tensors</a>",
            "<h3>Listed</h3>",
            "Beta</a>",
        ],
    );
    assert!(
        !ml.contains("Delta</a>"),
        "a category judgment moves Delta: {ml}"
    );
}

#[test]
fn a_category_page_lists_its_entries_strongest_first_with_how_to_get_each() {
    let site = tiered();
    let tensors = &site["categories/tensor-libraries/index.html"];
    in_order(
        tensors,
        &[
            "<h1>Tensor libraries</h1>",
            "<p>Libraries for multidimensional arrays and their math.</p>",
            "<h3>Featured</h3>",
            "<code>cargo add alpha-crate</code> <a href=\"https://crates.io/crates/alpha-crate\">crates.io</a>",
            "/projects/alpha/\">Alpha</a>",
            "<h3>Archived</h3>",
            "<code>cargo add gamma</code>",
            "/projects/gamma/\">Gamma</a>",
        ],
    );
    assert!(!tensors.contains("Delta</a>"), "{tensors}");
    let runtimes = &site["categories/inference-runtimes/index.html"];
    in_order(
        runtimes,
        &[
            "<code>brew install beta</code> <a href=\"https://formulae.brew.sh/formula/beta\">Homebrew</a>",
            "/projects/beta/\">Beta</a>",
        ],
    );
}

#[test]
fn an_app_without_a_channel_leads_with_its_repository() {
    let mut files = fixture::files();
    for (path, contents) in &mut files {
        if path == "entries/beta.toml"
            && let Some(at) = contents.find("[distribution]")
        {
            contents.truncate(at);
        }
    }
    let site = fixture::site(&files, false);
    let runtimes = &site["categories/inference-runtimes/index.html"];
    assert!(
        runtimes.contains("<p><a href=\"https://github.com/ml-rust/beta\">Repository</a></p>"),
        "{runtimes}"
    );
    assert!(!runtimes.contains("<code>"), "{runtimes}");
}

#[test]
fn a_category_with_no_entries_says_so_and_the_home_page_stays_silent_about_counts() {
    for site in [tiered(), untiered()] {
        let empty = &site["categories/vector-databases/index.html"];
        in_order(
            empty,
            &[
                "<h1>Vector databases</h1>",
                "<p>Databases that store vectors and find the ones nearest to a query.</p>",
                "<p>No entry is in this category yet.</p>",
            ],
        );
        let home = &site["index.html"];
        let start = home.find("id=\"categories\"").expect("the category block");
        let block = &home[start..];
        let block = &block[block.find("<ul").expect("the list")..];
        let block = &block[..block.find("</ul>").expect("the end of the list")];
        assert!(block.contains("Vector databases</a>"), "{block}");
        assert!(!block.contains(char::is_numeric), "{block}");
    }
}

#[test]
fn a_build_without_a_report_lists_a_category_by_name() {
    let site = untiered();
    let tensors = &site["categories/tensor-libraries/index.html"];
    in_order(
        tensors,
        &[
            "Entries are sorted by name.",
            "/projects/alpha/\">Alpha</a>",
            "/projects/gamma/\">Gamma</a>",
        ],
    );
    assert!(!tensors.contains("<h3>"), "{tensors}");
}

#[test]
fn an_entry_page_links_its_category_beside_its_topics() {
    let site = tiered();
    in_order(
        &site["projects/alpha/index.html"],
        &[
            "Topics: <a href=\"/topics/tensors/\">Tensors</a>",
            "Category: <a href=\"/categories/tensor-libraries/\">Tensor libraries</a>",
            "<h2 id=\"description\">Description</h2>",
        ],
    );
    assert!(!site["projects/delta/index.html"].contains("Category: <a"));
}

#[test]
fn an_entry_page_shows_every_signal_with_its_source() {
    let site = tiered();
    let alpha = &site["projects/alpha/index.html"];
    in_order(
        alpha,
        &[
            "<span class=\"tier-badge tier-featured\">Featured</span>",
            "This project replaces <a href=\"/projects/gamma/\">Gamma</a>.",
            "Source: <a href=\"https://crates.io/crates/alpha-crate\">the crates.io description</a>.",
            "Homepage: <a href=\"https://alpha.example.org/\">",
            "crates.io: <a href=\"https://crates.io/crates/alpha-crate\">alpha-crate</a>",
            "crates.io: <a href=\"https://crates.io/crates/alpha-extra\">alpha-extra</a>",
            "<code>featured</code>: fails: active users 9 below 11",
            "A demotion to Listed waits for approval.",
            "Not measured: Former glory.",
            "<tr><th scope=\"row\">Latest release</th><td>2026-09-01</td><td><a href=\"https://crates.io/data-access\">crates.io database dump</a></td><td><time datetime=\"2026-11-03\">2026-11-03</time></td><td>Yes</td></tr>",
            "<tr><th scope=\"row\">Former glory</th><td class=\"not-measured\">Not measured</td><td></td><td></td><td>Yes</td></tr>",
            "<tr><th scope=\"row\">GitHub stars</th><td>1200</td>",
            "<td>No — stars never count toward a tier</td></tr>",
            "The README claims GPU support that the code lacks.",
            "<a href=\"https://github.com/ml-rust/ml-rust.github.io/issues/11\">issue #11</a>",
            "<a href=\"https://github.com/ml-rust/ml-rust.github.io/issues/15\">#15</a></td><td>Maintainer statement</td><td>Maintained</td>",
            "<td>frank</td><td>bob</td><td>None</td>",
            "<a href=\"https://github.com/ml-rust/ml-rust.github.io/issues/11\">#11</a></td><td>Correctness</td>",
        ],
    );
    assert!(!alpha.contains("banner-org"), "{alpha}");
}

#[test]
fn an_org_affiliated_app_shows_the_notice_and_its_install_channels() {
    let site = tiered();
    let beta = &site["projects/beta/index.html"];
    in_order(
        beta,
        &[
            "<span class=\"entry-kind\">App</span> <span class=\"entry-label\">Product</span>",
            "<span class=\"tier-badge tier-listed\">Listed</span> <span class=\"stable-label\">Stable</span>",
            "The ml-rust organization maintains this project. It passes the same tier rules as every other entry. A maintainer outside the organization approves it for Featured.",
            "Source: <a href=\"https://github.com/ml-rust/beta#readme\">the project README</a>.",
            "Homebrew: <a href=\"https://formulae.brew.sh/formula/beta\">beta</a>",
            "Docker Hub: <a href=\"https://hub.docker.com/r/ml-rust/beta\">ml-rust/beta</a>",
            "Repology: <a href=\"https://repology.org/project/beta/versions\">beta</a>",
            "<code>featured-needs-outside-approval</code>",
            "Not measured: Repology families.",
            "<li>Homebrew returned no install count.</li>",
            "<td>No — stars never count toward a tier</td>",
            "No correction applies to this entry.",
            "<td>Kind</td><td>Product</td>",
        ],
    );
    let gamma = &site["projects/gamma/index.html"];
    assert!(
        gamma.contains("<a href=\"/projects/alpha/\">Alpha</a> replaces this project."),
        "{gamma}"
    );
    assert!(
        gamma.contains(
            "<td>Supersedes</td><td>Replaced by <a href=\"/projects/alpha/\">Alpha</a></td>"
        ),
        "{gamma}"
    );
    let delta = &site["projects/delta/index.html"];
    assert!(
        delta
            .contains("<td>Category</td><td>Topics: <a href=\"/topics/tensors/\">Tensors</a></td>"),
        "{delta}"
    );
}

#[test]
fn the_change_page_lists_every_change_newest_first() {
    let site = tiered();
    let changes = &site["changes/index.html"];
    in_order(
        changes,
        &[
            "2026-11-04</time></td><td><a href=\"/projects/alpha/\">Alpha</a>",
            "2026-11-04</time></td><td><a href=\"/projects/delta/\">Delta</a>",
            "2026-10-28</time></td><td><a href=\"/projects/gamma/\">Gamma</a></td><td>Listed</td><td>Archived</td>",
            "2026-09-01</time></td><td>omega</td>",
        ],
    );
}
