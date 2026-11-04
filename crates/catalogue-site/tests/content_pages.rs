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
    "class=\"panel why",
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
    assert!(!site["changes/index.html"].contains("<table"));
    assert!(!site["changes/index.html"].contains("timeline-row"));
    let home = &site["index.html"];
    assert!(home.contains("Tiers are not published."), "{home}");
    assert!(!home.contains("id=\"featured\""), "{home}");
    assert!(!home.contains("/changes/\">All tier changes"), "{home}");
    assert!(
        home.contains("<span class=\"tile-meta\">4 projects</span>"),
        "{home}"
    );
    let ml = &site["topics/ml/index.html"];
    assert!(ml.contains("Entries are sorted by name."), "{ml}");
    assert!(!ml.contains("rows-group"), "{ml}");
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
            "<a class=\"tile\" href=\"/topics/ml/\">",
            "<span class=\"tile-name\">Machine learning</span>",
            "<span class=\"tile-meta\">4 projects</span>",
            "<span class=\"tile-top\">Alpha, Beta</span>",
            "Subtopics: <a href=\"/topics/tensors/\">Tensors</a>",
            "<h2 id=\"categories\">Or pick the kind of project</h2>",
            "<a class=\"tile tile-small\" href=\"/categories/inference-runtimes/\">",
            "<a class=\"tile tile-small\" href=\"/categories/tensor-libraries/\">",
            "<a class=\"tile tile-small\" href=\"/categories/vector-databases/\">",
            "<h2 id=\"featured\">Featured projects</h2>",
            "/projects/alpha/\">Alpha</a>",
            "<a href=\"/changes/\">All tier changes",
        ],
    );
    // Recently added leaves the home page, and changes reduce to the one latest line.
    assert!(!home.contains("id=\"recent\""), "{home}");
    assert!(!home.contains("First check"), "{home}");
    in_order(
        home,
        &[
            "<p class=\"change-line\" id=\"changes\">",
            "/projects/alpha/\">Alpha</a> moved from Listed to Featured.",
        ],
    );
    assert_eq!(home.matches("moved from").count(), 1, "{home}");
    // The first check keeps its line and its rows on the changes page.
    in_order(
        &site["changes/index.html"],
        &[
            "The first check on <time datetime=\"2026-09-01\">1 Sep 2026</time> placed 1 entry.",
            "<span class=\"tier-badge tier-listed\">Listed</span> <span class=\"tier-count\">1</span>",
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
            "Part of <a href=\"/topics/ml/\">Machine learning</a>.",
            "<h1>Tensors</h1>",
            "Libraries for n-dimensional arrays and the math on them.",
            "<a href=\"https://en.wikipedia.org/wiki/Tensor_(machine_learning)\">en.wikipedia.org</a>",
            "<tr class=\"rows-group\" id=\"tier-featured\">",
            "Featured</span> <span class=\"rows-group-count\">",
            "Alpha</a>",
            "<tr class=\"rows-group\" id=\"tier-new\">",
            "New</span> <span class=\"rows-group-count\">",
            "Delta</a>",
            "<tr class=\"rows-group\" id=\"tier-archived\">",
            "Archived</span> <span class=\"rows-group-count\">",
            "Gamma</a>",
        ],
    );
    assert!(!tensors.contains("id=\"tier-listed\""), "{tensors}");
    let ml = &site["topics/ml/index.html"];
    in_order(
        ml,
        &[
            "<p class=\"subtopics\" id=\"subtopics\">Subtopics: <a href=\"/topics/tensors/\">Tensors</a>",
            "<tr class=\"rows-group\" id=\"tier-listed\">",
            "Listed</span> <span class=\"rows-group-count\">",
            "Beta</a>",
        ],
    );
    assert!(
        !ml.contains("Delta</a>"),
        "a topic judgment moves Delta: {ml}"
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
            "<p class=\"lede\">Libraries for multidimensional arrays and their math.</p>",
            "<tr class=\"rows-group\" id=\"tier-featured\">",
            "/projects/alpha/\">Alpha</a>",
            "<code>cargo add alpha-crate</code> <a href=\"https://crates.io/crates/alpha-crate\">crates.io</a>",
            "<tr class=\"rows-group\" id=\"tier-archived\">",
            "/projects/gamma/\">Gamma</a>",
            "<code>cargo add gamma</code>",
        ],
    );
    assert!(!tensors.contains("Delta</a>"), "{tensors}");
    let runtimes = &site["categories/inference-runtimes/index.html"];
    in_order(
        runtimes,
        &[
            "/projects/beta/\">Beta</a>",
            "<code>brew install beta</code> <a href=\"https://formulae.brew.sh/formula/beta\">Homebrew</a>",
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
        runtimes.contains(
            "<p class=\"row-install\"><a href=\"https://github.com/ml-rust/beta\">Repository</a></p>"
        ),
        "{runtimes}"
    );
    assert!(!runtimes.contains("<code>"), "{runtimes}");
}

#[test]
fn a_category_with_no_entries_says_so_and_the_home_page_counts_zero() {
    for site in [tiered(), untiered()] {
        let empty = &site["categories/vector-databases/index.html"];
        in_order(
            empty,
            &[
                "<h1>Vector databases</h1>",
                "<p class=\"lede\">Databases that store vectors and find the ones nearest to a query.</p>",
                "<p>No project is in this category yet.</p>",
            ],
        );
        let home = &site["index.html"];
        let start = home.find("id=\"categories\"").expect("the category block");
        let block = &home[start..];
        let block = &block[block.find("<ul").expect("the list")..];
        let block = &block[..block.find("</ul>").expect("the end of the list")];
        in_order(
            block,
            &[
                "<a class=\"tile tile-small\" href=\"/categories/vector-databases/\">",
                "<span class=\"tile-name\">Vector databases</span>",
                "<span class=\"tile-meta\">0 projects</span>",
            ],
        );
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
    assert!(!tensors.contains("rows-group"), "{tensors}");
}

#[test]
fn an_entry_page_links_its_category_beside_its_topics() {
    let site = tiered();
    in_order(
        &site["projects/alpha/index.html"],
        &[
            "<p class=\"project-description\">A tensor library.</p>",
            "Jobs: <a href=\"/topics/tensors/\">Tensors</a>",
            "Category: <a href=\"/categories/tensor-libraries/\">Tensor libraries</a>",
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
            "Source: <a href=\"https://crates.io/crates/alpha-crate\">the crates.io description</a>.",
            "This project replaces <a href=\"/projects/gamma/\">Gamma</a>.",
            "Homepage: <a href=\"https://alpha.example.org/\">",
            "crates.io: <a href=\"https://crates.io/crates/alpha-crate\">alpha-crate</a>",
            "A demotion to Listed waits for approval.",
            "Fails: active dependents 9 below 11.",
            "Not measured: Active dependents as a share of the peak.",
            "<code>featured</code>: fails: active dependents 9 below 11",
            "<a href=\"https://crates.io/crates/alpha-extra\">alpha-extra</a>",
            "<tr><th scope=\"row\">Active dependents as a share of the peak</th><td class=\"not-measured\">Not measured</td><td></td><td></td><td>Yes</td></tr>",
            "<tr><th scope=\"row\">Latest release</th><td>2026-09-01</td><td><a href=\"https://crates.io/data-access\">crates.io database dump</a></td><td><time datetime=\"2026-11-03\">3 Nov 2026</time></td><td>Yes</td></tr>",
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
            "Source: <a href=\"https://github.com/ml-rust/beta#readme\">the project README</a>.",
            "The ml-rust organization maintains this project. It passes the same tier rules as every other entry. A maintainer outside the organization approves it for Featured.",
            "Homebrew: <a href=\"https://formulae.brew.sh/formula/beta\">beta</a>",
            "Docker Hub: <a href=\"https://hub.docker.com/r/ml-rust/beta\">ml-rust/beta</a>",
            "Repology: <a href=\"https://repology.org/project/beta/versions\">beta</a>",
            "It passes the Featured gates and waits for outside approval.",
            "A featured-approval judgment from a maintainer outside the organization.",
            "Not measured: Repology families.",
            "<li>Homebrew returned no install count.</li>",
            "<code>featured-needs-outside-approval</code>",
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
        delta.contains("<td>Topic</td><td>Topics: <a href=\"/topics/tensors/\">Tensors</a></td>"),
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
            "<a href=\"/changes/feed.xml\">",
            "<h3 class=\"timeline-date\"><time datetime=\"2026-11-04\">4 Nov 2026</time></h3>",
            "<a class=\"timeline-name\" href=\"/projects/alpha/\">Alpha</a>",
            "<span class=\"tier-badge tier-listed\">Listed</span>",
            "<span class=\"tier-badge tier-featured\">Featured</span>",
            "<a class=\"timeline-name\" href=\"/projects/delta/\">Delta</a>",
            "<span class=\"row-tag\">First check</span>",
            "<span class=\"tier-badge tier-new\">New</span>",
            "<h3 class=\"timeline-date\"><time datetime=\"2026-10-28\">28 Oct 2026</time></h3>",
            "<a class=\"timeline-name\" href=\"/projects/gamma/\">Gamma</a>",
            "<span class=\"tier-badge tier-listed\">Listed</span>",
            "<span class=\"tier-badge tier-archived\">Archived</span>",
            "The first check on <time datetime=\"2026-09-01\">1 Sep 2026</time> placed 1 entry.",
        ],
    );
    assert!(!changes.contains(">omega</"), "{changes}");
}

#[test]
fn with_only_the_first_check_the_change_page_says_nothing_changed() {
    let (catalogue, report) = fixture::data(&fixture::files());
    let changes = [catalogue_site::TierChangeRow {
        date: fixture::first_check(),
        slug: catalogue_core::Slug::new("alpha").expect("slug"),
        from: None,
        to: catalogue_core::Tier::Listed,
    }];
    let site = build::render(&catalogue_site::SiteInputs {
        catalogue: &catalogue,
        report: Some(&report),
        changes: &changes,
        first_check: Some(fixture::first_check()),
        movement: &catalogue_core::Movement::NeedsHistory,
    });
    let page = &site["changes/index.html"];
    assert!(
        page.contains("No tier has changed since the first check."),
        "{page}"
    );
    assert!(!page.contains("timeline-row"), "{page}");
    assert!(
        page.contains("It is no change, because every entry appears on it."),
        "{page}"
    );
    assert!(!page.contains("No tier change is recorded."), "{page}");
}

#[test]
fn the_action_comes_right_after_the_header_with_a_hook_on_the_install_line() {
    let site = tiered();
    in_order(
        &site["projects/alpha/index.html"],
        &[
            "<header class=\"project-head\">",
            "</header>",
            "<aside class=\"project-action\"",
            "Use Alpha",
            "<code data-install-command=\"cargo add alpha-crate\">cargo add alpha-crate</code>",
            "<a class=\"button button-primary\" href=\"https://docs.rs/alpha-crate\">Documentation</a>",
            "<a class=\"button\" href=\"https://github.com/ml-rust/alpha\">Repository</a>",
            "<h2 id=\"tier\">",
            "<h2 id=\"compatibility\">",
            "<h2 id=\"also-in\">",
            "<h2 id=\"signals\">",
            "<h2 id=\"corrections\">",
        ],
    );
}

#[test]
fn an_archived_page_leads_with_the_reason_and_the_other_projects_and_steps_the_install_back() {
    let site = tiered();
    let gamma = &site["projects/gamma/index.html"];
    in_order(
        gamma,
        &[
            "</header>",
            "class=\"panel why why-archived\"",
            "Why Archived",
            "The repository is archived.",
            "<a href=\"/projects/alpha/\">Alpha</a> replaces this project.",
            "<a href=\"https://github.com/someone/gamma\">GitHub</a>",
            "<h2 id=\"also-in\">",
            "<aside class=\"project-action project-action-quiet\"",
            "For existing users",
            "<a class=\"button\" href=\"https://docs.rs/gamma\">Documentation</a>",
            "<h2 id=\"compatibility\">",
        ],
    );
    assert!(!gamma.contains("Use Gamma"), "{gamma}");
    assert!(!gamma.contains("button-primary"), "{gamma}");
    assert_eq!(gamma.matches("id=\"tier\"").count(), 1, "{gamma}");
    assert_eq!(gamma.matches("replaces this project").count(), 1, "{gamma}");
}

#[test]
fn each_project_page_has_one_h1_and_no_empty_comparison() {
    let site = tiered();
    for slug in ["alpha", "beta", "gamma", "delta"] {
        let page = &site[&format!("projects/{slug}/index.html")];
        assert_eq!(page.matches("<h1").count(), 1, "{slug}");
    }
    let beta = &site["projects/beta/index.html"];
    assert!(!beta.contains("rows-compact"), "{beta}");
}

#[test]
fn the_not_found_page_offers_search_and_the_job_tiles() {
    let site = tiered();
    in_order(
        &site["404.html"],
        &[
            "<h1 id=\"not-found-title\">Page not found</h1>",
            "<form class=\"hero-search\" action=\"/search/\" method=\"get\" role=\"search\">",
            "<a class=\"tile\" href=\"/topics/ml/\">",
        ],
    );
    assert_eq!(site["404.html"].matches("role=\"search\"").count(), 1);
}
