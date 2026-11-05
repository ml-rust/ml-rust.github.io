//! The interactive layer: which page loads which script, the row facts the scripts read, the
//! compare link, and the rules every script keeps.

#[path = "common/build.rs"]
mod build;
#[path = "common/fixture.rs"]
mod fixture;

use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

/// The start of a script tag, up to its source.
const SCRIPT_TAG: &str = "<script type=\"module\" src=\"";

/// The list script, the search script and the compare script, as a tag names each.
const LIST: &str = "/assets/list.";
const SEARCH: &str = "/search/search.";
const COMPARE: &str = "/assets/compare.";

/// DOM APIs that parse text as markup. No script uses them.
const MARKUP_APIS: [&str; 4] = [
    "innerHTML",
    "insertAdjacentHTML",
    "outerHTML",
    "document.write",
];

fn tiered() -> BTreeMap<String, String> {
    fixture::site(&fixture::files(), true)
}

fn untiered() -> BTreeMap<String, String> {
    fixture::site(&fixture::files(), false)
}

fn asset(name: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("assets")
        .join(name);
    fs::read_to_string(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

/// The source of each script tag of `page`, in load order.
fn scripts(page: &str) -> Vec<&str> {
    page.split(SCRIPT_TAG)
        .skip(1)
        .filter_map(|rest| rest.split('"').next())
        .collect()
}

/// The start of each source of `page`, one of the three stems, in load order.
fn script_stems(page: &str) -> Vec<&'static str> {
    scripts(page)
        .into_iter()
        .map(|src| {
            [LIST, SEARCH, COMPARE]
                .into_iter()
                .find(|stem| src.starts_with(stem))
                .unwrap_or_else(|| panic!("unknown script {src}"))
        })
        .collect()
}

/// The opening tag of the row of `slug` in `page`.
fn row_tag<'a>(page: &'a str, slug: &str) -> &'a str {
    let start = page
        .find(&format!("data-slug=\"{slug}\""))
        .unwrap_or_else(|| panic!("no row of {slug} in {page}"));
    let rest = &page[start..];
    &rest[..rest.find('>').expect("the end of the tag")]
}

#[test]
fn each_page_loads_only_the_scripts_it_uses() {
    for site in [tiered(), untiered()] {
        for (page, wanted) in [
            ("topics/tensors/index.html", vec![LIST]),
            ("categories/tensor-libraries/index.html", vec![LIST]),
            ("projects/alpha/index.html", vec![LIST]),
            ("search/index.html", vec![LIST, SEARCH]),
            ("compare/index.html", vec![LIST, COMPARE]),
            ("index.html", vec![]),
            ("topics/index.html", vec![]),
            ("rules/index.html", vec![]),
            ("changes/index.html", vec![]),
            ("submit/index.html", vec![]),
            ("404.html", vec![]),
        ] {
            assert_eq!(script_stems(&site[page]), wanted, "{page}");
        }
    }
    assert_eq!(script_stems(&tiered()["featured/index.html"]), vec![LIST]);
}

#[test]
fn a_page_with_nothing_for_the_list_script_does_not_load_it() {
    let site = tiered();
    assert!(
        scripts(&site["categories/vector-databases/index.html"]).is_empty(),
        "a category with no row has nothing to sort"
    );
    assert!(
        scripts(&untiered()["featured/index.html"]).is_empty(),
        "a Featured page with no tiers has no row"
    );
    // A project with a command and no table still loads it, for the copy button.
    assert!(!site["projects/beta/index.html"].contains("class=\"project-compare\""));
    assert_eq!(script_stems(&site["projects/beta/index.html"]), vec![LIST]);
    // A project with neither has no use for it.
    let mut files = fixture::files();
    for (_, contents) in &mut files {
        let channels = "[distribution]\nhomebrew = \"beta\"\ndocker = [\"ml-rust/beta\"]\nrepology = \"beta\"\n";
        *contents = contents.replace(channels, "");
    }
    let bare = fixture::site(&files, true);
    assert!(scripts(&bare["projects/beta/index.html"]).is_empty());
}

#[test]
fn each_script_tag_loads_the_emitted_file() {
    let site = tiered();
    for (page, file) in [
        ("topics/tensors/index.html", "list.js"),
        ("compare/index.html", "compare.js"),
        ("search/index.html", "search.js"),
    ] {
        let source = asset(file);
        let found = scripts(&site[page])
            .into_iter()
            .filter_map(|src| site.get(src.trim_start_matches('/')))
            .any(|emitted| *emitted == source);
        assert!(found, "{page} does not load {file}");
    }
}

#[test]
fn the_compare_page_is_complete_with_no_script_and_is_listed() {
    let site = tiered();
    let page = &site["compare/index.html"];
    assert!(page.contains("<h1>Compare projects</h1>"), "{page}");
    assert!(page.contains("class=\"lede\""), "{page}");
    let start = page.find("<noscript>").expect("noscript");
    let end = page.find("</noscript>").expect("noscript end");
    assert!(page[start..end].contains("<a href=\"/topics/\">"), "{page}");
    assert!(
        site["sitemap.xml"].contains("<loc>https://ml-rust.github.io/compare/</loc>"),
        "the sitemap lists the page"
    );
    assert!(
        site["index.html"].contains("<a href=\"/compare/\">Compare projects</a>"),
        "the footer links the page"
    );
}

#[test]
fn a_row_holds_each_fact_the_scripts_read() {
    let site = tiered();
    let page = &site["topics/tensors/index.html"];
    let alpha = row_tag(page, "alpha");
    for part in [
        "data-name=\"Alpha\"",
        "data-kind-name=\"Library\"",
        "data-tier-name=\"Featured\"",
        "data-downloads=\"1840052\"",
        "data-released=\"2026-09-01\"",
        "data-gpu=\"CUDA|Metal\"",
        "data-native=\"Required\"",
        "data-license-family=\"Permissive\"",
        "data-python=\"Python bindings\"",
        "data-dormancy=\"active\"",
    ] {
        assert!(alpha.contains(part), "no {part} in {alpha}");
    }
}

#[test]
fn a_found_nothing_value_and_a_missing_value_stay_two_answers() {
    let site = tiered();
    let page = &site["topics/tensors/index.html"];
    let delta = row_tag(page, "delta");
    assert!(delta.contains("data-gpu=\"Unknown\""), "{delta}");
    assert!(delta.contains("data-native=\"Not measured\""), "{delta}");
    assert!(delta.contains("data-python=\"Not measured\""), "{delta}");
    assert!(!delta.contains("data-downloads="), "{delta}");
    assert!(!delta.contains("data-dependents="), "{delta}");
    let gamma = row_tag(page, "gamma");
    for part in [
        "data-gpu=\"Not measured\"",
        "data-native=\"Not measured\"",
        "data-license-family=\"Not measured\"",
        "data-python=\"Not measured\"",
    ] {
        assert!(gamma.contains(part), "no {part} in {gamma}");
    }
    assert!(!gamma.contains("data-released="), "{gamma}");
}

#[test]
fn a_build_with_no_tiers_prints_no_tier_and_no_measured_fact() {
    let site = untiered();
    let alpha = row_tag(&site["topics/tensors/index.html"], "alpha");
    assert!(!alpha.contains("data-tier"), "{alpha}");
    assert!(!alpha.contains("data-downloads"), "{alpha}");
    for part in [
        "data-gpu=\"Not measured\"",
        "data-native=\"Not measured\"",
        "data-license-family=\"Not measured\"",
        "data-python=\"Not measured\"",
        "data-dormancy=\"not-measured\"",
    ] {
        assert!(alpha.contains(part), "no {part} in {alpha}");
    }
}

#[test]
fn a_family_row_says_how_many_projects_it_stands_for() {
    let mut files = fixture::files();
    for (_, contents) in &mut files {
        *contents = contents.replace(
            "name = \"Gamma\"\n",
            "name = \"Gamma\"\npart_of = \"alpha\"\n",
        );
    }
    let site = fixture::site(&files, true);
    let page = &site["categories/tensor-libraries/index.html"];
    assert!(
        row_tag(page, "alpha").contains("data-projects=\"2\""),
        "{page}"
    );
    assert!(!page.contains("data-slug=\"gamma\""), "{page}");
    let alone = &site["topics/ml/index.html"];
    assert!(!alone.contains("data-projects=\"1\""), "{alone}");
}

#[test]
fn a_hostile_name_stays_escaped_in_its_data_attribute() {
    let mut files = fixture::files();
    for (_, contents) in &mut files {
        *contents = contents.replace("name = \"Alpha\"", "name = '''\"><script>x</script>'''");
    }
    let site = fixture::site(&files, true);
    let page = &site["topics/tensors/index.html"];
    assert!(
        page.contains("data-name=\"&#34;&#62;&#60;script&#62;x&#60;/script&#62;\""),
        "{page}"
    );
}

#[test]
fn only_the_list_tables_are_marked_for_the_list_script() {
    let site = tiered();
    assert!(
        site["topics/tensors/index.html"]
            .contains("<table class=\"rows\" data-sortable data-filterable>")
    );
    assert!(
        site["categories/tensor-libraries/index.html"].contains("data-sortable data-filterable")
    );
    assert!(site["featured/index.html"].contains("data-sortable data-filterable"));
    let project = &site["projects/alpha/index.html"];
    assert!(project.contains("data-sortable>"), "{project}");
    assert!(!project.contains("data-filterable"), "{project}");
    assert!(!site["search/index.html"].contains("data-sortable"));
}

#[test]
fn a_page_with_no_script_holds_no_control_that_needs_one() {
    let site = tiered();
    for (path, page) in &site {
        if !path.ends_with(".html") {
            continue;
        }
        for class in [
            "sort-button",
            "row-compare",
            "list-filters",
            "copy-button",
            "compare-tray",
        ] {
            assert!(!page.contains(class), "{path} prints {class}");
        }
    }
}

#[test]
fn the_compare_link_of_a_project_page_covers_the_first_rows_of_the_table() {
    let site = tiered();
    let page = &site["projects/alpha/index.html"];
    assert!(
        page.contains("<a href=\"/compare/?p=alpha,delta,gamma\">Compare these"),
        "{page}"
    );
    assert!(!site["projects/beta/index.html"].contains("Compare these"));
}

#[test]
fn the_scripts_keep_the_rules_of_the_page() {
    for name in ["list.js", "compare.js"] {
        let source = asset(name);
        for api in MARKUP_APIS {
            assert!(!source.contains(api), "{name} uses {api}");
        }
        assert!(source.contains("const COMPARE_LIMIT = 4;"), "{name}");
        assert!(!source.contains("import "), "{name} imports a module");
        assert!(!source.contains("<script"), "{name} writes a script tag");
    }
    let compare = asset("compare.js");
    assert!(compare.contains("const DATA_DIR = \"/data/compare/\";"));
    assert!(
        !compare.contains("catalogue.json"),
        "the page never fetches the whole data file"
    );
    assert_eq!(
        compare.matches("await fetch(").count(),
        1,
        "one same-origin fetch"
    );
    assert!(
        compare.contains("SLUG.test(slug)"),
        "only a safe slug reaches a file path"
    );
    assert!(compare.contains("url.protocol === \"https:\" && url.hostname"));
    assert!(
        !asset("list.js").contains("fetch("),
        "the list script fetches nothing"
    );
}

#[test]
fn the_compare_page_holds_no_install_command_and_keeps_the_channel_link() {
    let compare = asset("compare.js");
    assert!(!compare.contains("installCell"));
    assert!(!compare.contains("label: \"Install\""));
    assert!(!compare.contains("data-install-command"));
    assert!(!compare.contains("entry.install?.command"));
    assert!(compare.contains("add(channel, safeLink(entry.install?.url));"));
    assert!(
        asset("list.js").contains("code[data-install-command]"),
        "the project page keeps its copy button"
    );
}

#[test]
fn the_links_row_is_last_and_never_counts_as_a_difference() {
    let compare = asset("compare.js");
    let row = compare
        .lines()
        .position(|line| line.contains("{ label: \"Links\", cell: linksCell, plain: true },"))
        .expect("the Links row");
    let next = compare.lines().nth(row + 1).expect("the next line");
    assert_eq!(next, "];", "the Links row ends the table");
    assert!(!compare.contains("sourcesCell"), "no Sources row");
    assert!(
        compare.contains("Each project page lists the source and date of every value."),
        "one note stands for the sources"
    );
    assert!(
        compare.contains("!fact.plain && new Set("),
        "no Differs mark"
    );
    assert!(
        compare.contains("else if (!fact.plain)"),
        "the toggle keeps it"
    );
}

#[test]
fn no_template_holds_an_inline_handler_or_an_inline_script() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("templates");
    for entry in fs::read_dir(&dir).expect("templates") {
        let path = entry.expect("entry").path();
        if path.extension().is_none_or(|ext| ext != "html") {
            continue;
        }
        let text = fs::read_to_string(&path).expect("template");
        for needle in [" onclick=", " onchange=", " onload=", "<script>"] {
            assert!(!text.contains(needle), "{}: {needle}", path.display());
        }
    }
}
