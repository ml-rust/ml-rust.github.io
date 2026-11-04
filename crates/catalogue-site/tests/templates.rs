//! No template or source file turns HTML escaping off. One audited type prints README HTML.

use std::fs;
use std::path::{Path, PathBuf};

/// Text that turns escaping off, after whitespace is removed.
const ESCAPE_OFF: [&str; 5] = [
    "|safe",
    "safe(",
    "filtersafe",
    "escape=\"none\"",
    "(\"none\")",
];

/// Rust items that mark a value as already escaped.
const SAFE_TYPES: [&str; 3] = ["HtmlSafe", "MarkupDisplay", "filters::Safe"];

/// The one file that marks a type as already escaped, relative to the crate.
const README_HTML: &str = "src/pages/project/readme/html.rs";

fn crate_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// Every file below `dir`, sorted.
fn files(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut pending = vec![dir.to_path_buf()];
    while let Some(next) = pending.pop() {
        for entry in
            fs::read_dir(&next).unwrap_or_else(|error| panic!("{}: {error}", next.display()))
        {
            let path = entry.expect("entry").path();
            if path.is_dir() {
                pending.push(path);
            } else {
                out.push(path);
            }
        }
    }
    out.sort();
    out
}

/// Every pattern in `patterns` that `text` holds once whitespace is removed.
fn escape_breaks(text: &str, patterns: &[&str]) -> Vec<String> {
    let squeezed: String = text.chars().filter(|c| !c.is_whitespace()).collect();
    patterns
        .iter()
        .filter(|pattern| squeezed.contains(*pattern))
        .map(|pattern| (*pattern).to_owned())
        .collect()
}

#[test]
fn the_lint_finds_each_way_to_turn_escaping_off() {
    for text in [
        "{{ x|safe }}",
        "{{ x | safe }}",
        "{{ safe(x) }}",
        "{% filter safe %}x{% endfilter %}",
        r#"#[template(path = "a.html", escape = "none")]"#,
        r#"{{ x|escape("none") }}"#,
        r#"{{ x|e("none") }}"#,
    ] {
        assert!(!escape_breaks(text, &ESCAPE_OFF).is_empty(), "{text}");
    }
    assert!(escape_breaks("{{ x }} {{ safety }}", &ESCAPE_OFF).is_empty());
}

#[test]
fn no_template_turns_escaping_off() {
    let templates = files(&crate_dir().join("templates"));
    assert!(!templates.is_empty(), "no templates found");
    for path in templates {
        let text =
            fs::read_to_string(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
        let found = escape_breaks(&text, &ESCAPE_OFF);
        assert!(
            found.is_empty(),
            "{} turns escaping off: {found:?}",
            path.display()
        );
    }
}

#[test]
fn no_source_file_turns_escaping_off() {
    let patterns: Vec<&str> = ESCAPE_OFF.iter().chain(&SAFE_TYPES).copied().collect();
    for path in files(&crate_dir().join("src")) {
        let text =
            fs::read_to_string(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
        let mut found = escape_breaks(&text, &patterns);
        if path == crate_dir().join(README_HTML) {
            found.retain(|pattern| pattern != "HtmlSafe");
        }
        assert!(
            found.is_empty(),
            "{} turns escaping off: {found:?}",
            path.display()
        );
    }
}

#[test]
fn the_readme_html_type_is_the_only_html_safe_impl_and_only_its_file_builds_it() {
    for path in files(&crate_dir().join("src")) {
        let text =
            fs::read_to_string(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
        let squeezed: String = text.chars().filter(|c| !c.is_whitespace()).collect();
        if path == crate_dir().join(README_HTML) {
            assert_eq!(
                squeezed.matches("HtmlSafe").count(),
                1,
                "{} names the marker more than once",
                path.display()
            );
            assert!(
                squeezed.contains("implaskama::filters::HtmlSafeforReadmeHtml{}"),
                "{} marks another type",
                path.display()
            );
        } else {
            assert!(
                !squeezed.contains("HtmlSafe"),
                "{} names the marker",
                path.display()
            );
            assert!(
                !squeezed.contains("ReadmeHtml("),
                "{} builds the type",
                path.display()
            );
        }
    }
}

#[test]
fn a_template_prints_the_readme_html_only_in_the_readme_section() {
    for path in files(&crate_dir().join("templates")) {
        let text =
            fs::read_to_string(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
        if text.contains("readme.head") || text.contains("{{ rest }}") {
            assert!(
                path.ends_with("project_readme.html"),
                "{} prints README HTML",
                path.display()
            );
        }
    }
}
