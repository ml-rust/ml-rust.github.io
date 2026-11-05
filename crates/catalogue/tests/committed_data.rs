//! The `data/` directory in the repository loads and passes every check.

use std::collections::BTreeMap;
use std::path::Path;

use catalogue::{load_catalogue, parse_leads};
use catalogue_core::{Catalogue, Movement};
use catalogue_site::{OutputFile, SITE_URL, SiteInputs, build_site};

/// The longest `<title>`, in characters, that a search result shows whole.
const MAX_TITLE: usize = 65;

/// The committed data directory, loaded and checked.
fn committed_catalogue() -> Catalogue {
    let data = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
    load_catalogue(&data)
        .expect("the data directory reads")
        .expect("the data directory is valid")
}

/// Every file of a build over `catalogue` with no tier report, keyed by path.
fn built_files(catalogue: &Catalogue) -> BTreeMap<String, OutputFile> {
    let inputs = SiteInputs {
        catalogue,
        report: None,
        changes: &[],
        first_check: None,
        movement: &Movement::NeedsHistory,
    };
    build_site(&inputs)
        .expect("the site builds")
        .into_iter()
        .map(|file| (file.path.clone(), file))
        .collect()
}

/// The text of `file` as UTF-8.
fn text(file: &OutputFile) -> &str {
    std::str::from_utf8(&file.bytes).expect("a page is UTF-8")
}

/// `escaped` with the template escapes undone, so that its length is what a reader sees.
fn unescaped(escaped: &str) -> String {
    escaped
        .replace("&#60;", "<")
        .replace("&#62;", ">")
        .replace("&#34;", "\"")
        .replace("&#39;", "'")
        .replace("&#38;", "&")
}

/// The text between the first `start` in `page` and the next `end`, unescaped.
fn between(page: &str, start: &str, end: &str) -> String {
    let (_, rest) = page.split_once(start).expect("the page holds the start");
    let (found, _) = rest.split_once(end).expect("the page holds the end");
    unescaped(found)
}

/// The words of `text` in lower case, without punctuation.
fn words(text: &str) -> Vec<String> {
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|word| !word.is_empty())
        .map(str::to_lowercase)
        .collect()
}

/// Checks that the description of `page` holds `definition` whole and says more than `name`.
fn check_description(page: &str, name: &str, definition: &str) {
    let description = between(page, "<meta name=\"description\" content=\"", "\">");
    assert!(description.contains(definition), "{description}");
    assert!(!description.contains('…'), "{description}");
    assert!(
        !words(definition).starts_with(&words(name)),
        "`{definition}` opens by repeating `{name}`"
    );
    let named = words(name);
    let added = words(definition)
        .into_iter()
        .filter(|word| !named.contains(word))
        .count();
    assert!(added >= 5, "`{definition}` adds too little to `{name}`");
}

#[test]
fn every_built_page_title_fits_a_search_result() {
    let catalogue = committed_catalogue();
    let files = built_files(&catalogue);
    let mut pages = 0;
    for (path, file) in files.iter().filter(|(path, _)| path.ends_with(".html")) {
        let title = between(text(file), "<title>", "</title>");
        assert!(
            title.chars().count() <= MAX_TITLE,
            "{path}: `{title}` is over {MAX_TITLE} characters"
        );
        if !path.starts_with("projects/") {
            assert!(!title.contains("Rust ML"), "{path}: {title}");
        }
        pages += 1;
    }
    assert!(pages > catalogue.entries().len());
}

#[test]
fn every_project_title_keeps_the_whole_project_name() {
    let catalogue = committed_catalogue();
    let files = built_files(&catalogue);
    assert!(!catalogue.entries().is_empty());
    for (slug, entry) in catalogue.entries() {
        let page = &files[&format!("projects/{slug}/index.html")];
        let title = between(text(page), "<title>", "</title>");
        assert!(title.starts_with(&entry.name), "{slug}: {title}");
        assert!(!title.contains('…'), "{slug}: {title}");
    }
}

#[test]
fn no_topic_or_category_description_merely_repeats_its_name() {
    let catalogue = committed_catalogue();
    let files = built_files(&catalogue);
    assert!(!catalogue.topics().is_empty());
    assert!(!catalogue.categories().is_empty());
    for topic in catalogue.topics() {
        let page = text(&files[&format!("topics/{}/index.html", topic.id)]);
        check_description(page, &topic.name, &topic.blurb);
    }
    for category in catalogue.categories() {
        let page = text(&files[&format!("categories/{}/index.html", category.id)]);
        check_description(page, &category.name, &category.blurb);
    }
}

#[test]
fn a_committed_topic_page_with_no_summary_opens_with_its_blurb() {
    let catalogue = committed_catalogue();
    let files = built_files(&catalogue);
    let plain: Vec<_> = catalogue
        .topics()
        .iter()
        .filter(|topic| topic.summary.is_none())
        .collect();
    assert!(!plain.is_empty());
    for topic in plain {
        let page = text(&files[&format!("topics/{}/index.html", topic.id)]);
        let lede = between(page, "<p class=\"lede\">", "</p>");
        assert_eq!(lede, topic.blurb);
    }
}

#[test]
fn the_share_image_address_resolves_to_a_built_png() {
    let catalogue = committed_catalogue();
    let files = built_files(&catalogue);
    let home = text(&files["index.html"]);
    let url = between(home, "<meta property=\"og:image\" content=\"", "\">");
    let path = url
        .strip_prefix(SITE_URL)
        .and_then(|path| path.strip_prefix('/'))
        .expect("the address is on the site");
    let image = files.get(path).expect("the build holds the image");
    assert!(image.bytes.starts_with(b"\x89PNG\r\n\x1a\n"), "{path}");
}

#[test]
fn the_committed_data_directory_is_valid() {
    let data = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
    let catalogue = load_catalogue(&data)
        .expect("the data directory reads")
        .expect("the data directory is valid");
    let backends = catalogue.backends();
    assert!(!backends.backend.is_empty());
    assert!(!backends.python.is_empty());
    assert!(!backends.not_native.is_empty());
    assert!(!backends.native_build.is_empty());
}

#[test]
fn every_committed_showcase_project_has_an_icon() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let catalogue = load_catalogue(&root.join("data"))
        .expect("the data directory reads")
        .expect("the data directory is valid");
    assert!(!catalogue.showcase().is_empty());
    let templates = root.join("crates/catalogue-site/templates");
    let chooser = std::fs::read_to_string(templates.join("icon_showcase.html"))
        .expect("the icon chooser reads");
    for slug in catalogue.showcase() {
        let file = format!("icons/org-{slug}.svg");
        assert!(chooser.contains(&file), "{file} is not in the chooser");
        assert!(templates.join(&file).is_file(), "{file} is missing");
    }
}

#[test]
fn the_committed_lead_list_parses_and_holds_what_belongs() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/leads.txt");
    let text = std::fs::read_to_string(&path).expect("the lead list reads");
    let leads: Vec<String> = parse_leads(&path, &text)
        .expect("the lead list parses")
        .iter()
        .map(ToString::to_string)
        .collect();
    for name in [
        "axp",
        "blazr",
        "boostr",
        "compressr",
        "nexar",
        "numr",
        "oxidizr",
        "regexr",
        "solvr",
        "splintr",
        "treeboost",
        "venus",
        "vidr",
        "voxr",
    ] {
        let lead = format!("ml-rust/{name}");
        assert_eq!(
            leads.iter().filter(|listed| **listed == lead).count(),
            1,
            "{lead}"
        );
    }
    for name in ["ml-rust/fluxbench", "ml-rust/cargo-reclaim"] {
        assert!(!leads.iter().any(|lead| lead == name), "{name}");
    }
}
