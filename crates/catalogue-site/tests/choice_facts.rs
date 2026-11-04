//! Entry pages and rows show the facts a person reads to choose a project.

#[path = "common/build.rs"]
mod build;
#[path = "common/fixture.rs"]
mod fixture;

use std::collections::BTreeMap;

const ALPHA: &str = "projects/alpha/index.html";
const BETA: &str = "projects/beta/index.html";
const TOPIC: &str = "topics/tensors/index.html";
const CATEGORY: &str = "categories/tensor-libraries/index.html";

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
            panic!("no {part} after the parts before it in:\n{page}");
        };
        rest = &rest[at + part.len()..];
    }
}

#[test]
fn the_stat_strip_and_the_compatibility_block_state_the_facts_in_the_stated_order() {
    let site = tiered();
    in_order(
        &site[ALPHA],
        &[
            "<dt>Downloads (30 days)</dt><dd><span class=\"stat-value\">1,840,052</span>",
            "<dt>Dependents</dt><dd><span class=\"stat-value\">12</span>",
            "<dt>Version</dt><dd><span class=\"stat-value\">0.4.2</span>",
            "<dt>Released</dt>",
            "<time class=\"stat-sub\" datetime=\"2026-09-01\">1 Sep 2026</time>",
            "<dt>Checked</dt>",
            "<h2 id=\"compatibility\">",
            "<dt>Runs on</dt>",
            "CUDA, Metal",
            "<p class=\"compat-lines-label\">What shows it</p>",
            "<li>CUDA: alpha-crate feature cuda (dependency cudarc)</li>",
            "<li>Metal: alpha-extra feature metal</li>",
            "<dt>Native code</dt>",
            "Required",
            "<li>Binds native code: openssl-sys</li>",
            "<dt>Python bindings</dt>",
            "Has Python bindings",
            "<li>alpha-extra needs Python</li>",
            "<dt>License</dt>",
            "Apache-2.0 OR MIT",
            "<dt>Dependents</dt>",
            ">12<",
            "<dt>Contributors</dt>",
            ">38<",
        ],
    );
    let page = &site[ALPHA];
    let compat = &page[page.find("id=\"compatibility\"").expect("the block")..];
    let compat = &compat[..compat.find("</section>").expect("its end")];
    assert!(!compat.contains("compat-source"), "{compat}");
    assert!(!compat.contains("fetched"), "{compat}");
}

#[test]
fn the_sources_disclosure_lists_every_source_of_the_compatibility_facts_with_its_date() {
    let site = tiered();
    let page = &site[ALPHA];
    let from = &page[page.find("id=\"sources\"").expect("the disclosure")..];
    let list = &from[..from.find("</details>").expect("its end")];
    for source in [
        "<a href=\"https://crates.io/data-access\">crates.io database dump</a>, fetched <time datetime=\"2026-11-03\">3 Nov 2026</time>",
        "<a href=\"https://github.com/ml-rust/alpha/graphs/contributors\">GitHub contributors</a>, fetched <time datetime=\"2026-11-03\">3 Nov 2026</time>",
    ] {
        assert_eq!(list.matches(source).count(), 1, "{source} in {list}");
    }
}

#[test]
fn no_gpu_feature_stays_in_the_words_of_the_check_on_the_page() {
    let site = tiered();
    let page = &site["projects/delta/index.html"];
    assert!(page.contains("No GPU feature found"), "{page}");
    assert!(!page.contains("CPU only"), "{page}");
}

#[test]
fn an_unknown_value_reads_not_measured_and_an_app_installs_from_its_channel() {
    let site = tiered();
    let page = &site[BETA];
    let row = |label: &str, value: &str| {
        format!("<dt>{label}</dt>\n<dd class=\"compat-value\"><span class=\"compat-word{value}")
    };
    assert!(page.contains(&row("License", "\">None stated")), "{page}");
    assert!(
        page.contains(&row("Contributors", " fact-quiet\">Not measured")),
        "{page}"
    );
    in_order(
        page,
        &[
            &row("Crate facts", " fact-quiet\">Not measured"),
            "Runs on, Native code, Python bindings and Dependents read crates on crates.io. This entry lists no crate.",
        ],
    );
    assert!(!page.contains("<dt>Runs on</dt>"), "{page}");
    assert!(!page.contains("<dt>Native code</dt>"), "{page}");
    in_order(
        page,
        &[
            "<dt>Downloads per release</dt><dd><span class=\"stat-value\">1,500</span>",
            "<dt>Dependents</dt><dd><span class=\"stat-value fact-quiet\">Not applicable</span>",
        ],
    );
    assert!(!page.contains("<dt>Active dependents</dt>"), "{page}");
    assert!(
        page.contains("<code data-install-command=\"brew install beta\">brew install beta</code>"),
        "{page}"
    );
    let delta = &site["projects/delta/index.html"];
    assert!(
        delta.contains(
            "<dt>Downloads (30 days)</dt><dd><span class=\"stat-value fact-quiet\">Not measured</span>"
        ),
        "{delta}"
    );
}

#[test]
fn the_signal_table_keeps_the_precise_value_and_each_fact_shows_its_source() {
    let site = tiered();
    let page = &site[ALPHA];
    assert!(page.contains("<td>9.36</td>"), "{page}");
    in_order(
        page,
        &[
            "<th scope=\"row\">Contributors</th><td>38</td>",
            "https://github.com/ml-rust/alpha/graphs/contributors",
            "<td>Yes — one part of the repository route</td>",
        ],
    );
}

#[test]
fn rows_on_topic_and_category_pages_show_the_facts_as_whole_numbers() {
    let site = tiered();
    // The home page's Featured rows compare the same facts.
    for page in [TOPIC, CATEGORY, "index.html"] {
        in_order(
            &site[page],
            &[
                "<tr class=\"entry-row\" data-slug=\"alpha\"",
                "data-downloads=\"1840052\"",
                "data-dependents=\"12\"",
                "data-stars=\"1200\"",
                "<td class=\"cell-downloads\" data-label=\"Downloads\"><span class=\"fact\" title=\"1,840,052 downloads over 30 days\"><span aria-hidden=\"true\">1.8M</span><span class=\"visually-hidden\">1,840,052 downloads over 30 days</span></span></td>",
                "<td class=\"cell-dependents\" data-label=\"Dependents\"><span class=\"fact\">12</span></td>",
                "<td class=\"cell-stars\" data-label=\"Stars\"><span class=\"fact\">1,200</span></td>",
                "<td class=\"cell-release\" data-label=\"Latest release\"><span class=\"release-version\">0.4.2</span><time class=\"release-date\" datetime=\"2026-09-01\">1 Sep 2026</time></td>",
                "<td class=\"cell-gpu\" data-label=\"Runs on (GPU)\"><span class=\"fact\">CUDA, Metal</span></td>",
                "<td class=\"cell-license\" data-label=\"License\"><span class=\"fact\">Apache-2.0 OR MIT</span></td>",
            ],
        );
    }
}

#[test]
fn the_entry_page_shows_the_repository_row_below_the_stat_strip() {
    let site = tiered();
    in_order(
        &site[ALPHA],
        &[
            "<dt>Checked</dt>",
            "<dl class=\"stat-strip stat-strip-repo\">",
            "<dt>Stars</dt><dd><span class=\"stat-value\">1,200</span>",
            "<dt>Forks</dt><dd><span class=\"stat-value\">80</span>",
            "<dt>Contributors</dt><dd><span class=\"stat-value\">38</span>",
            "<dt>Commits</dt><dd><span class=\"stat-value\">4,200</span>",
            "<h2 id=\"compatibility\">",
        ],
    );
    in_order(
        &site[BETA],
        &[
            "<dt>Stars</dt><dd><span class=\"stat-value\">40</span>",
            "<dt>Forks</dt><dd><span class=\"stat-value fact-quiet\">Not measured</span>",
        ],
    );
}

#[test]
fn the_measurements_hold_a_repository_group() {
    let site = tiered();
    in_order(
        &site[ALPHA],
        &[
            "id=\"signals-repository\"",
            "1,200 stars, 80 forks, 38 contributors, 4,200 commits. Stars alone never qualify a project.",
            "<th scope=\"row\">GitHub forks</th><td>80</td>",
            "<th scope=\"row\">Commits on the default branch</th><td>4200</td>",
            "<td>No</td>",
        ],
    );
}

#[test]
fn the_tier_explainer_of_a_list_defines_active_dependents() {
    let site = tiered();
    assert!(
        site[TOPIC].contains(
            "Active dependents: crates from other owners that depend on this project in a current release. One owner counts at most once."
        ),
        "{}",
        site[TOPIC]
    );
}

#[test]
fn an_app_row_names_its_download_unit_and_has_no_dependents() {
    let site = tiered();
    let page = &site["topics/ml/index.html"];
    in_order(
        page,
        &[
            "data-slug=\"beta\"",
            "data-downloads=\"1500\"",
            "<td class=\"cell-downloads\" data-label=\"Downloads\"><span class=\"fact\" title=\"1,500 binary downloads per release\"><span aria-hidden=\"true\">1.5k per release</span>",
            "<td class=\"cell-dependents\" data-label=\"Dependents\"><span class=\"fact fact-quiet\">Not applicable</span></td>",
        ],
    );
}

#[test]
fn a_row_says_whether_native_code_is_required_and_keeps_the_exact_fact() {
    let site = tiered();
    let native = "Binds native code: openssl-sys";
    let cell = format!(
        "<td class=\"cell-native\" data-label=\"Native code\"><span class=\"fact\" title=\"{native}\"><span aria-hidden=\"true\">Required</span><span class=\"visually-hidden\">{native}</span></span></td>"
    );
    assert!(site[TOPIC].contains(&cell), "{}", site[TOPIC]);
}

#[test]
fn python_bindings_show_as_a_tag_beside_the_name_with_the_exact_fact() {
    let site = tiered();
    let page = &site[TOPIC];
    in_order(
        page,
        &[
            "/projects/alpha/\">Alpha</a>",
            "<span class=\"row-tag\" title=\"Has Python bindings: alpha-extra needs Python\">Python bindings</span>",
            "<p class=\"row-description\">",
        ],
    );
    let cell = &page[page.find("class=\"cell-native\"").expect("a native cell")..];
    let cell = &cell[..cell.find("</td>").expect("the end of the cell")];
    assert!(!cell.contains("Python"), "{cell}");
}

#[test]
fn a_build_without_a_report_reads_not_measured_and_shows_no_tier() {
    let site = untiered();
    let page = &site[ALPHA];
    assert!(
        page.contains(
            "<dt>License</dt>\n<dd class=\"compat-value\"><span class=\"compat-word fact-quiet\">Not measured"
        ),
        "{page}"
    );
    assert!(!page.contains("tier-badge"), "{page}");
    assert!(!page.contains("stat-strip"), "{page}");
    assert!(page.contains(">cargo add alpha-crate</code>"), "{page}");
    in_order(
        &site[TOPIC],
        &[
            "<td class=\"cell-gpu\" data-label=\"Runs on (GPU)\"><span class=\"fact fact-quiet\">Not measured</span></td>",
            "<td class=\"cell-license\" data-label=\"License\"><span class=\"fact fact-quiet\">Not measured</span></td>",
        ],
    );
}

#[test]
fn the_section_compares_with_one_link_and_names_a_single_job_once() {
    let site = tiered();
    in_order(
        &site[ALPHA],
        &[
            "<h2 id=\"also-in\">Compare with similar projects</h2>",
            "beside the projects that share a topic with it",
            "<a class=\"row-name\" href=\"/projects/alpha/\" aria-current=\"page\">Alpha</a> <span class=\"row-tag\">This page</span>",
            "<a class=\"row-name\" href=\"/projects/delta/\">Delta</a>",
            "<a class=\"row-name\" href=\"/projects/gamma/\">Gamma</a>",
            "<a href=\"/compare/?p=alpha,delta,gamma\">Compare these",
            "<a href=\"/topics/tensors/\">All projects in Tensors",
        ],
    );
    assert!(
        !site[ALPHA].contains("Also in"),
        "every row comes from one job: {}",
        site[ALPHA]
    );
    let page = &site[BETA];
    assert!(
        !page.contains("id=\"also-in\""),
        "no other entry is in its topic: {page}"
    );
}

#[test]
fn the_rules_page_says_these_facts_never_count_and_how_each_is_measured() {
    let site = tiered();
    let page = &site["rules/index.html"];
    assert!(page.contains("They never count toward a tier."), "{page}");
    assert!(
        page.contains("<strong>Commits</strong> The total commits on the default branch."),
        "{page}"
    );
    for label in [
        "License",
        "Runs on",
        "Native code",
        "Python bindings",
        "Dependents",
        "Contributors",
    ] {
        assert!(
            page.contains(&format!("<strong>{label}</strong>")),
            "{label}"
        );
    }
}
