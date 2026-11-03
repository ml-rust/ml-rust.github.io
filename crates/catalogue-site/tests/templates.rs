//! No template or source file turns HTML escaping off.

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
        let found = escape_breaks(&text, &patterns);
        assert!(
            found.is_empty(),
            "{} turns escaping off: {found:?}",
            path.display()
        );
    }
}
