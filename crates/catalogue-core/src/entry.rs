use serde::{Deserialize, Serialize};
use url::Url;

use crate::date::Date;
use crate::ids::{CrateName, Slug};

/// Longest allowed description, in characters.
const MAX_DESCRIPTION_CHARS: usize = 300;

/// Whether an entry is a library or an application.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EntryKind {
    /// A crate other projects depend on.
    Library,
    /// A program people run.
    App,
}

/// Where a description comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DescriptionSource {
    /// Taken from the project README. Requires `source_url`.
    Readme,
    /// Written by a person.
    Human,
}

/// The short text that describes an entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Description {
    /// The description, 1 to 300 characters.
    pub text: String,
    /// Where the text comes from.
    pub source: DescriptionSource,
    /// The page the text was taken from. Required for `readme`.
    pub source_url: Option<Url>,
}

impl Description {
    /// Lists every rule this description breaks.
    pub fn problems(&self) -> Vec<String> {
        let mut out = Vec::new();
        if self.text.trim().is_empty() {
            out.push("description text must not be empty".to_owned());
        }
        if self.text.chars().count() > MAX_DESCRIPTION_CHARS {
            out.push(format!(
                "description text is longer than {MAX_DESCRIPTION_CHARS} characters"
            ));
        }
        if self.source == DescriptionSource::Readme && self.source_url.is_none() {
            out.push("a `readme` description requires `source_url`".to_owned());
        }
        out
    }
}

/// How an application is installed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Distribution {
    /// Homebrew formula name.
    pub homebrew: Option<String>,
    /// Container image names.
    #[serde(default)]
    pub docker: Vec<String>,
    /// Repology project name.
    pub repology: Option<String>,
}

/// One project in the catalogue.
///
/// The slug comes from the file name `data/entries/<slug>.toml`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Entry {
    /// Display name.
    pub name: String,
    /// Library or application.
    pub kind: EntryKind,
    /// Source repository. Uses https.
    pub repo: Url,
    /// Project home page.
    pub homepage: Option<Url>,
    /// Published crates. A library lists at least one.
    pub crates: Vec<CrateName>,
    /// Topic ids. At least one.
    pub topics: Vec<Slug>,
    /// Short description.
    pub description: Description,
    /// Whether the ml-rust organization maintains the project.
    #[serde(default)]
    pub org_affiliated: bool,
    /// Install channels. Applications only.
    pub distribution: Option<Distribution>,
    /// Date the entry joined the catalogue.
    pub added: Date,
}

impl Entry {
    /// Lists every rule this entry breaks on its own, without other files.
    pub fn problems(&self) -> Vec<String> {
        let mut out = Vec::new();
        if self.name.trim().is_empty() {
            out.push("name must not be empty".to_owned());
        }
        if self.repo.scheme() != "https" {
            out.push(format!("repo `{}` must use https", self.repo));
        }
        if self.topics.is_empty() {
            out.push("an entry needs at least one topic".to_owned());
        }
        if self.kind == EntryKind::Library {
            if self.crates.is_empty() {
                out.push("a library needs at least one crate".to_owned());
            }
            if self.distribution.is_some() {
                out.push("only an app can have `distribution`".to_owned());
            }
        }
        out.extend(self.description.problems());
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{ALPHA, BETA};

    fn parse(text: &str) -> Result<Entry, toml::de::Error> {
        toml::from_str(text)
    }

    fn problems(text: &str) -> Vec<String> {
        parse(text).expect("entry parses").problems()
    }

    fn has(problems: &[String], needle: &str) -> bool {
        problems.iter().any(|p| p.contains(needle))
    }

    #[test]
    fn valid_library_and_app_pass() {
        assert!(problems(ALPHA).is_empty());
        assert!(problems(BETA).is_empty());
    }

    #[test]
    fn reads_both_date_forms() {
        let quoted = ALPHA.replace("added = 2026-11-03", "added = \"2026-11-03\"");
        assert_eq!(
            parse(&quoted).expect("quoted"),
            parse(ALPHA).expect("native")
        );
    }

    #[test]
    fn unknown_fields_are_rejected() {
        assert!(parse(&format!("bogus = 1\n{ALPHA}")).is_err());
        let nested = format!("{ALPHA}bogus = 1\n");
        assert!(parse(&nested).is_err());
    }

    #[test]
    fn empty_name_is_reported() {
        let text = ALPHA.replace("name = \"Alpha\"", "name = \" \"");
        assert!(has(&problems(&text), "name must not be empty"));
    }

    #[test]
    fn http_repo_is_reported() {
        let text = ALPHA.replace(
            "https://github.com/ml-rust/alpha",
            "http://example.com/alpha",
        );
        assert!(has(&problems(&text), "must use https"));
    }

    #[test]
    fn library_without_crates_is_reported() {
        let text = ALPHA.replace("crates = [\"alpha-crate\"]", "crates = []");
        assert!(has(&problems(&text), "at least one crate"));
    }

    #[test]
    fn app_without_crates_passes() {
        assert!(BETA.contains("crates = []"));
        assert!(problems(BETA).is_empty());
    }

    #[test]
    fn entry_without_topics_is_reported() {
        let text = ALPHA.replace("topics = [\"tensors\"]", "topics = []");
        assert!(has(&problems(&text), "at least one topic"));
    }

    #[test]
    fn library_with_distribution_is_reported() {
        let text = format!("{ALPHA}\n[distribution]\nhomebrew = \"alpha\"\n");
        assert!(has(&problems(&text), "only an app can have `distribution`"));
    }

    #[test]
    fn readme_description_needs_a_source_url() {
        let text = ALPHA.replace("source = \"human\"", "source = \"readme\"");
        assert!(has(&problems(&text), "requires `source_url`"));
        let with_url = format!("{text}source_url = \"https://github.com/ml-rust/alpha#readme\"\n");
        assert!(problems(&with_url).is_empty());
    }

    #[test]
    fn description_length_is_limited() {
        let long = "x".repeat(301);
        let text = ALPHA.replace("A tensor library.", &long);
        assert!(has(&problems(&text), "longer than 300"));
        let limit = "x".repeat(300);
        assert!(problems(&ALPHA.replace("A tensor library.", &limit)).is_empty());
        let empty = ALPHA.replace("A tensor library.", "");
        assert!(has(&problems(&empty), "must not be empty"));
    }

    #[test]
    fn distribution_defaults_to_empty_docker() {
        let dist = parse(BETA)
            .expect("app")
            .distribution
            .expect("distribution");
        assert_eq!(dist.homebrew.as_deref(), Some("beta"));
        let minimal = BETA.replace("docker = [\"ml-rust/beta\"]\n", "");
        let dist = parse(&minimal)
            .expect("app")
            .distribution
            .expect("distribution");
        assert!(dist.docker.is_empty());
    }
}
