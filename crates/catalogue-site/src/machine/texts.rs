//! The plain text files: `llms.txt`, `robots.txt` and `data/README.txt`.

use catalogue_core::Catalogue;

use super::catalogue_file::{CATALOGUE_FILE, CATALOGUE_SCHEMA, DATA_LICENSE};
use crate::build::OutputFile;
use crate::pages::{SITE_NAME, SITE_URL, category_href, topic_href};

/// `name` on one line, with no markup. These files are plain text with no escaping.
fn plain_name(name: &str) -> String {
    name.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .replace(['<', '>'], "")
}

fn text_file(path: &str, text: String) -> OutputFile {
    OutputFile {
        path: path.to_owned(),
        bytes: text.into_bytes(),
    }
}

/// `llms.txt`: where a script or an agent finds the data, the rules, the topics and the
/// categories. The lists come from the data files.
pub(crate) fn llms_file(catalogue: &Catalogue) -> OutputFile {
    let mut lines = vec![
        format!("# {SITE_NAME}"),
        String::new(),
        "Rust projects for machine learning: libraries on crates.io, and apps.".to_owned(),
        "Tiers come from published rules over raw signals. Each published claim links its source."
            .to_owned(),
        String::new(),
        "## Data".to_owned(),
        format!("- Every entry as JSON: {SITE_URL}/{CATALOGUE_FILE}"),
        format!("- What the JSON fields mean: {SITE_URL}/data/README.txt"),
        format!("- Tier rules: {SITE_URL}/rules/"),
        format!("- Atom feed of changes: {SITE_URL}/changes/feed.xml"),
        format!("- License of the data: {DATA_LICENSE}"),
        String::new(),
        "## Topics".to_owned(),
    ];
    lines.extend(catalogue.topics().iter().map(|topic| {
        let href = topic_href(&topic.id);
        format!("- {}: {SITE_URL}{href}", plain_name(&topic.name))
    }));
    lines.push(String::new());
    lines.push("## Categories".to_owned());
    lines.extend(catalogue.categories().iter().map(|category| {
        let href = category_href(&category.id);
        format!("- {}: {SITE_URL}{href}", plain_name(&category.name))
    }));
    lines.push(String::new());
    text_file("llms.txt", lines.join("\n"))
}

/// `robots.txt`: every page may be read, and the sitemap lists them.
pub(crate) fn robots_file() -> OutputFile {
    text_file(
        "robots.txt",
        format!("User-agent: *\nAllow: /\nSitemap: {SITE_URL}/sitemap.xml\n"),
    )
}

/// `data/README.txt`: the fields of the data file and its license.
pub(crate) fn data_readme_file() -> OutputFile {
    text_file(
        "data/README.txt",
        format!(
            "\
{SITE_NAME}: data file

File: {SITE_URL}/{CATALOGUE_FILE}
License: {DATA_LICENSE}. Credit the catalogue and link {SITE_URL}/.
Rules: {SITE_URL}/rules/

The file holds what the pages show, and nothing they hide.
No field is a score. A tier comes from published thresholds over raw signals.

Top level
- schema: the file format version, now {CATALOGUE_SCHEMA}. A change to any field raises it.
- generated: the run date of the check behind the tiers. Null when tiers are not published.
- license: the license of the data.
- thresholds_version: the thresholds version the check applied. Null when tiers are not published.
- entries: every entry, sorted by slug.

Each entry
- slug: the id of the entry. The page is {SITE_URL}/projects/<slug>/.
- name: the display name.
- kind: library or app.
- repo: the source repository.
- crates: the published crates, the primary crate first.
- topics: the topic ids, after merged judgments.
- category: the category id, or null.
- part_of: the slug of the bigger project this entry belongs to, or null.
- description: text, source (readme, crate, human or repo) and source_url, or null.
- install: how to get the project, as command (or null), text and url, as the entry page shows it.
- docs: the docs link the entry page shows, or null.
- tier: featured, listed, new or archived. Null when tiers are not published.
- why: the sentences of the Why block of the entry page, as verdict, lead, meets, lacks and found. The verdict is the one line that says why the entry has its tier. Null when tiers are not published.
- native_word: the word a list prints for the native code fact: Required, Optional, None found, Build tool or Not measured.
- reasons: each rule outcome, as rule and detail.
- signals: every signal of the check.

Each signal
- name: the stable id. active_users is the raw id of Active dependents: crates from other owners that depend on this project in a current release, where one owner counts at most once. new_adopters is the raw id of New dependents (365 days): the same count, limited to dependencies that began in the last 365 days.
- label: the name the page shows.
- value: the raw value as text, or null when not measured.
- source: label and url of the page that holds the value. Null when not measured.
- fetched: the date of the data behind the value. Null when not measured.
- counts_toward_tier: whether a tier rule reads the value. A rule reads stars, forks and contributors only together, on the repository route. Stars alone never qualify a project.

Smaller files for the compare page
- {SITE_URL}/data/compare/index.json: slug and name of every entry, sorted by slug.
- {SITE_URL}/data/compare/<slug>.json: one entry, holding the same values as its record in the file above. It leaves out crates, topics, category, part_of and reasons, and it holds only the signals the compare page shows.
"
        ),
    )
}

#[cfg(test)]
mod tests {
    use catalogue_core::{DataSources, SourceFile};

    use super::*;

    fn source(path: &str, contents: &str) -> SourceFile {
        SourceFile {
            path: path.to_owned(),
            contents: contents.to_owned(),
        }
    }

    fn data() -> Catalogue {
        Catalogue::load(DataSources {
            topics: Some(source(
                "topics.toml",
                "[[topic]]\nid = \"zeta\"\nname = \"Zeta <b>\"\n\n[[topic]]\nid = \"alpha\"\nname = \"Alpha\"\n",
            )),
            categories: Some(source(
                "categories.toml",
                "[[category]]\nid = \"tools\"\nname = \"Tools\"\nblurb = \"Some tools.\"\n",
            )),
            ..DataSources::default()
        })
        .expect("valid data")
    }

    #[test]
    fn llms_points_at_the_data_and_lists_topics_and_categories_from_the_data() {
        let file = llms_file(&data());
        assert_eq!(file.path, "llms.txt");
        let text = String::from_utf8(file.bytes).expect("UTF-8");
        for part in [
            "https://ml-rust.github.io/data/catalogue.json",
            "https://ml-rust.github.io/rules/",
            "- Zeta b: https://ml-rust.github.io/topics/zeta/\n- Alpha: https://ml-rust.github.io/topics/alpha/",
            "- Tools: https://ml-rust.github.io/categories/tools/",
        ] {
            assert!(text.contains(part), "missing {part} in {text}");
        }
        assert!(!text.contains("<b>"), "{text}");
    }

    #[test]
    fn robots_names_the_sitemap() {
        let text = String::from_utf8(robots_file().bytes).expect("UTF-8");
        assert_eq!(
            text,
            "User-agent: *\nAllow: /\nSitemap: https://ml-rust.github.io/sitemap.xml\n"
        );
    }

    #[test]
    fn the_readme_names_the_license_and_the_fields() {
        let file = data_readme_file();
        assert_eq!(file.path, "data/README.txt");
        let text = String::from_utf8(file.bytes).expect("UTF-8");
        for part in ["CC-BY-4.0", "part_of:", "counts_toward_tier:", "schema:"] {
            assert!(text.contains(part), "missing {part}");
        }
    }
}
