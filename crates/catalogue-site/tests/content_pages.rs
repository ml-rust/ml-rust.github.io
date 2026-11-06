//! The home, topic, entry and change pages show what the data and the report hold.

#[path = "common/build.rs"]
mod build;
#[path = "common/fixture.rs"]
mod fixture;

use std::collections::BTreeMap;

/// Markup that only a build with a report prints.
const TIER_MARKUP: [&str; 6] = [
    "tier-badge",
    "data-tier",
    "signal-table",
    "tier-counts",
    "class=\"panel why",
    "class=\"disclosure disclosure-card\" id=\"tier\"",
];

#[test]
fn the_home_page_lists_the_showcase_with_and_without_tiers() {
    for site in [tiered(), untiered()] {
        let home = &site["index.html"];
        assert!(home.contains("Maintained by the ML Rust organization"));
        assert!(home.contains("We pick this list by hand"));
        assert!(home.contains("<a class=\"tile-name\" href=\"/projects/beta/\">Beta</a>"));
        assert!(home.contains("A command line tool that runs models."));
        assert!(home.contains("<a href=\"/ml-rust/\">All 1 project from ML Rust"));
        assert!(!home.contains("<a class=\"tile-name\" href=\"/projects/alpha/\">"));
    }
}

#[test]
fn the_ml_rust_page_lists_only_org_affiliated_entries() {
    for site in [tiered(), untiered()] {
        let page = &site["ml-rust/index.html"];
        assert!(page.contains("<h1>ML Rust projects</h1>"));
        assert!(page.contains("href=\"/projects/beta/\""));
        assert!(!page.contains("href=\"/projects/alpha/\""));
    }
}

#[test]
fn every_page_describes_itself_to_search_engines() {
    let site = tiered();
    for (path, address) in [
        ("index.html", "https://ml-rust.github.io/"),
        (
            "topics/tensors/index.html",
            "https://ml-rust.github.io/topics/tensors/",
        ),
        (
            "projects/alpha/index.html",
            "https://ml-rust.github.io/projects/alpha/",
        ),
        ("search/index.html", "https://ml-rust.github.io/search/"),
    ] {
        let page = &site[path];
        assert!(
            page.contains("<meta name=\"description\" content=\""),
            "{path}"
        );
        assert!(
            page.contains(&format!("<link rel=\"canonical\" href=\"{address}\">")),
            "{path}"
        );
        assert!(!page.contains("noindex"), "{path}");
    }
    assert!(
        site["index.html"].contains(
            "<title>Rust machine learning libraries and apps · ML Rust catalogue</title>"
        )
    );
    assert!(site["index.html"].contains("application/ld+json"));
    let missing = &site["404.html"];
    assert!(missing.contains("<meta name=\"robots\" content=\"noindex\">"));
    assert!(!missing.contains("rel=\"canonical\""));
}

#[test]
fn every_page_shows_the_share_image_in_a_link_preview() {
    let site = tiered();
    for path in [
        "index.html",
        "topics/tensors/index.html",
        "projects/alpha/index.html",
        "404.html",
    ] {
        let page = &site[path];
        assert!(
            page.contains("<meta property=\"og:image\" content=\"https://ml-rust.github.io/assets/share-image."),
            "{path}"
        );
        assert!(
            page.contains("<meta property=\"og:image:width\" content=\"1200\">"),
            "{path}"
        );
        assert!(
            page.contains("<meta property=\"og:image:height\" content=\"630\">"),
            "{path}"
        );
        assert!(page.contains("<meta property=\"og:image:alt\" content=\"ML Rust catalogue: "));
        assert!(
            page.contains("<meta name=\"twitter:card\" content=\"summary_large_image\">"),
            "{path}"
        );
    }
}

#[test]
fn a_topic_description_holds_its_blurb_when_the_page_shows_a_summary() {
    let site = tiered();
    let page = &site["topics/ml/index.html"];
    assert!(page.contains(
        "<meta name=\"description\" content=\"Teaching programs from data. Find models and tools. Compare"
    ));
    assert!(
        page.contains("<p class=\"lede\">Projects that train or run machine learning models.</p>")
    );
}

#[test]
fn no_showcase_file_leaves_the_section_out() {
    let files: Vec<(String, String)> = fixture::files()
        .into_iter()
        .filter(|(path, _)| path != "showcase.toml")
        .collect();
    let site = fixture::site(&files, true);
    assert!(!site["index.html"].contains("ml-rust-projects"));
}

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
fn a_category_page_lists_its_entries_strongest_first() {
    let site = tiered();
    let tensors = &site["categories/tensor-libraries/index.html"];
    in_order(
        tensors,
        &[
            "<h1>Tensor libraries</h1>",
            "<p class=\"lede\">Libraries for multidimensional arrays and their math.</p>",
            "<tr class=\"rows-group\" id=\"tier-featured\">",
            "/projects/alpha/\">Alpha</a>",
            "<tr class=\"rows-group\" id=\"tier-archived\">",
            "/projects/gamma/\">Gamma</a>",
        ],
    );
    assert!(!tensors.contains("Delta</a>"), "{tensors}");
    let runtimes = &site["categories/inference-runtimes/index.html"];
    assert!(
        runtimes.contains("/projects/beta/\">Beta</a>"),
        "{runtimes}"
    );
}

#[test]
fn no_list_page_holds_an_install_command() {
    for site in [tiered(), fixture::site(&fixture::files(), false)] {
        let lists = site.iter().filter(|(path, _)| {
            path.as_str() == "index.html"
                || path.starts_with("featured/")
                || path.starts_with("ml-rust/")
                || path.starts_with("topics/")
                || path.starts_with("categories/")
        });
        for (path, page) in lists {
            for command in [
                "cargo add",
                "cargo install",
                "brew install",
                "docker pull",
                "row-install",
                "start-install",
                "data-install-command",
            ] {
                assert!(!page.contains(command), "{path} holds {command}");
            }
        }
    }
    let tensors = &tiered()["categories/tensor-libraries/index.html"];
    assert!(tensors.contains("class=\"start-card\""), "{tensors}");
}

#[test]
fn list_rows_hold_no_channel_link() {
    let site = tiered();
    for path in [
        "categories/tensor-libraries/index.html",
        "categories/inference-runtimes/index.html",
    ] {
        let page = &site[path];
        let start = page.find("<tr class=\"entry-row").expect("a row");
        let end = page.rfind("</tr>").expect("the end of a row");
        let page = &page[start..end];
        for channel in [
            "href=\"https://crates.io/crates/",
            "href=\"https://formulae.brew.sh/",
            "href=\"https://repology.org/",
            "href=\"https://hub.docker.com/",
        ] {
            assert!(!page.contains(channel), "{path} links {channel}");
        }
        assert!(!page.contains("\">Homebrew</a>"), "{path}");
        assert!(!page.contains("\">Repository</a>"), "{path}");
    }
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
            "This project replaces <a href=\"/projects/gamma/\">Gamma</a>.",
            "A demotion to Listed waits for approval.",
            "Fails: active dependents 9 below 11.",
            "Not measured: Active dependents as a share of the peak.",
            "<code>featured</code>: fails: active dependents 9 below 11",
            "<a href=\"https://crates.io/crates/alpha-extra\">alpha-extra</a>",
            "<h2 id=\"signals\">Measurements and sources</h2>",
            "<details class=\"disclosure disclosure-group\" id=\"sources\">",
            "<li>Description: <a href=\"https://crates.io/crates/alpha-crate\">the crates.io description</a></li>",
            "<tr><th scope=\"row\">Active dependents as a share of the peak</th><td class=\"not-measured\">Not measured</td><td></td><td></td><td>Yes</td></tr>",
            "<tr><th scope=\"row\">Latest release</th><td>2026-09-01</td><td><a href=\"https://crates.io/data-access\">crates.io database dump</a></td><td><time datetime=\"2026-11-03\">3 Nov 2026</time></td><td>Yes</td></tr>",
            "<tr><th scope=\"row\">GitHub stars</th><td>1200</td>",
            "<td>Yes — one part of the repository route</td></tr>",
            "The README claims GPU support that the code lacks.",
            "<a href=\"https://github.com/ml-rust/ml-rust.github.io/issues/11\">issue #11</a>",
            "<a href=\"https://github.com/ml-rust/ml-rust.github.io/issues/15\">#15</a></td><td>Maintainer statement</td><td>Maintained</td>",
            "<td>frank</td><td>bob</td><td>None</td>",
            "<a href=\"https://github.com/ml-rust/ml-rust.github.io/issues/11\">#11</a></td><td>Correctness</td>",
            &link_row(
                "Homepage",
                "https://alpha.example.org/",
                "alpha.example.org",
            ),
            &link_row(
                "crates.io",
                "https://crates.io/crates/alpha-crate",
                "crates.io/crates/alpha-crate",
            ),
        ],
    );
}

/// One row of the links card.
fn link_row(label: &str, href: &str, target: &str) -> String {
    format!(
        "<a class=\"link-row\" href=\"{href}\"><span class=\"link-label\">{label}</span><span class=\"link-target\">{target}</span></a>"
    )
}

/// The page text from the start of the `#signals` section to its closing tag.
fn signals_section(page: &str) -> &str {
    let start = page
        .find("aria-labelledby=\"signals\"")
        .expect("the signals section");
    let end = page[start..]
        .find("</section>")
        .expect("the end of the signals section");
    &page[start..start + end]
}

#[test]
fn the_description_source_appears_only_in_the_sources_disclosure() {
    let site = tiered();
    let alpha = &site["projects/alpha/index.html"];
    let source = "the crates.io description";
    assert_eq!(alpha.matches(source).count(), 1, "{alpha}");
    let sources = &alpha[alpha
        .find("id=\"sources\"")
        .expect("the Sources disclosure")..];
    let disclosure = &sources[..sources.find("</details>").expect("its end")];
    assert!(disclosure.contains(source), "{disclosure}");
    assert!(disclosure.contains("<span class=\"disclosure-title\">Sources</span>"));
    assert!(!alpha.contains("<p class=\"sources\">Source: "), "{alpha}");
    assert!(!alpha.contains("<p class=\"sources\">From "), "{alpha}");
    assert!(!alpha.contains("<p class=\"sources\">Sources: "), "{alpha}");
}

#[test]
fn no_fetch_date_prints_outside_the_signals_section() {
    let site = tiered();
    for path in ["projects/alpha/index.html", "projects/beta/index.html"] {
        let page = &site[path];
        let section = signals_section(page);
        assert!(section.contains("fetched"), "{path} lists no fetch date");
        let outside = page.replace(section, "");
        assert!(
            !outside.to_lowercase().contains("fetched"),
            "{path} prints a fetch date outside #signals"
        );
    }
}

#[test]
fn the_checked_cell_links_to_the_sources() {
    let site = tiered();
    let alpha = &site["projects/alpha/index.html"];
    assert!(alpha.contains("<a class=\"stat-link\" href=\"#signals\">Sources</a>"));
}

#[test]
fn a_page_without_tiers_still_links_the_description_source() {
    let site = untiered();
    let alpha = &site["projects/alpha/index.html"];
    in_order(
        alpha,
        &[
            "<h2 id=\"signals\">Sources</h2>",
            "id=\"sources\"",
            "<span class=\"disclosure-summary\">1 source</span>",
            "<li>Description: <a href=\"https://crates.io/crates/alpha-crate\">the crates.io description</a></li>",
        ],
    );
    assert!(!alpha.contains("signal-table"), "{alpha}");
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
            "It passes the Featured gates and waits for outside approval.",
            "A featured-approval judgment from a maintainer outside the organization.",
            "Not measured: Repology families.",
            "<li>Homebrew returned no install count.</li>",
            "<code>featured-needs-outside-approval</code>",
            "<li>Description: <a href=\"https://github.com/ml-rust/beta#readme\">the project README</a></li>",
            "<td>Yes — one part of the repository route</td>",
            "No correction applies to this entry.",
            "<td>Kind</td><td>Product</td>",
            &link_row(
                "Homebrew",
                "https://formulae.brew.sh/formula/beta",
                "formulae.brew.sh/formula/beta",
            ),
            &link_row(
                "Docker Hub",
                "https://hub.docker.com/r/ml-rust/beta",
                "hub.docker.com/r/ml-rust/beta",
            ),
            &link_row(
                "Repology",
                "https://repology.org/project/beta/versions",
                "repology.org/project/beta/versions",
            ),
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
        state: None,
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
fn the_main_column_comes_first_and_the_links_card_ends_with_the_install_line() {
    let site = tiered();
    in_order(
        &site["projects/alpha/index.html"],
        &[
            "<header class=\"project-head\">",
            "</header>",
            "<div class=\"project-layout\">",
            "<h2 id=\"compatibility\">",
            "<h2 id=\"also-in\">",
            "<details class=\"disclosure disclosure-card\" id=\"tier\">",
            "<span class=\"disclosure-title\">Why Featured</span>",
            "</details>",
            "<h2 id=\"signals\">",
            "<h2 id=\"corrections\">",
            "<aside class=\"project-action\"",
            ">Links</h2>",
            &link_row(
                "Documentation",
                "https://docs.rs/alpha-crate",
                "docs.rs/alpha-crate",
            ),
            &link_row(
                "Repository",
                "https://github.com/ml-rust/alpha",
                "github.com/ml-rust/alpha",
            ),
            "<p class=\"install-label\">Install</p>",
            "<code data-install-command=\"cargo add alpha-crate\">cargo add alpha-crate</code>",
            "</aside>",
        ],
    );
}

#[test]
fn a_project_with_no_install_command_leaves_the_install_part_out() {
    let site = tiered();
    for page in site.iter().filter_map(|(path, page)| {
        (path.starts_with("projects/") && !page.contains("data-install-command")).then_some(page)
    }) {
        assert!(!page.contains("install-label"), "{page}");
        assert!(!page.contains("holds no install command"), "{page}");
    }
}

#[test]
fn the_why_block_is_collapsed_with_the_lead_as_its_summary() {
    let site = tiered();
    let beta = &site["projects/beta/index.html"];
    in_order(
        beta,
        &[
            "<details class=\"disclosure disclosure-card\" id=\"tier\">",
            "<span class=\"disclosure-title\">Why Listed</span><span class=\"disclosure-summary\">It passes the Featured gates and waits for outside approval.</span></summary>",
            "</details>",
        ],
    );
    assert!(!beta.contains("class=\"panel why"), "{beta}");
    assert_eq!(beta.matches("id=\"tier\"").count(), 1, "{beta}");
}

#[test]
fn an_archived_page_leads_with_the_reason_and_steps_the_links_back() {
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
            "<div class=\"project-layout\">",
            "<h2 id=\"compatibility\">",
            "<h2 id=\"also-in\">",
            "id=\"sources\"",
            "<a href=\"https://github.com/someone/gamma\">GitHub</a>",
            "<aside class=\"project-action project-action-quiet\"",
            ">For existing users</h2>",
            &link_row("Documentation", "https://docs.rs/gamma", "docs.rs/gamma"),
        ],
    );
    assert!(!gamma.contains(">Links</h2>"), "{gamma}");
    assert!(!gamma.contains("disclosure-card\" id=\"tier\""), "{gamma}");
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
