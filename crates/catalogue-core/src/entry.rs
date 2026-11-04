use serde::{Deserialize, Serialize};
use url::Url;

use crate::date::Date;
use crate::ids::{CrateName, Slug};
use crate::project_pages::crate_page;

/// The start of a crate page on crates.io.
pub const CRATES_IO_PAGE: &str = "https://crates.io/crates/";

/// Longest allowed description, in characters.
pub const MAX_DESCRIPTION_CHARS: usize = 300;

/// Whether an entry is a library or an application.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EntryKind {
    /// A crate other projects depend on.
    Library,
    /// A program people run.
    App,
}

impl EntryKind {
    /// Every kind, libraries first.
    pub const ALL: [Self; 2] = [Self::Library, Self::App];

    /// The lowercase name used in data files.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Library => "library",
            Self::App => "app",
        }
    }
}

/// Where a description comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DescriptionSource {
    /// Taken from the project README. Requires `source_url`.
    Readme,
    /// The crate's own crates.io description. Requires `source_url`, the crate page.
    Crate,
    /// Written by a person.
    Human,
    /// The GitHub repository description. Requires `source_url`, the entry's repo.
    Repo,
}

/// The short text that describes an entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Description {
    /// The description, 1 to 300 characters.
    pub text: String,
    /// Where the text comes from.
    pub source: DescriptionSource,
    /// The page the text was taken from. Required for `readme`, `crate` and `repo`.
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
        if self.source == DescriptionSource::Crate && self.source_url.is_none() {
            out.push("a `crate` description requires `source_url`".to_owned());
        }
        if self.source == DescriptionSource::Repo && self.source_url.is_none() {
            out.push("a `repo` description requires `source_url`".to_owned());
        }
        out
    }

    /// Lists the rules a `repo` description breaks against the repo of its entry.
    ///
    /// The `source_url` must equal `repo`.
    fn repo_source_problems(&self, repo: &Url) -> Vec<String> {
        match self.source_url.as_ref() {
            Some(url) if self.source == DescriptionSource::Repo && url != repo => vec![format!(
                "a `repo` description needs `source_url` equal to the entry's repo `{repo}`, found `{url}`"
            )],
            _ => Vec::new(),
        }
    }

    /// Lists the rules a `crate` description breaks against the crates of its entry.
    ///
    /// The `source_url` must be `https://crates.io/crates/<name>` for one of `crates`.
    fn crate_source_problems(&self, crates: &[CrateName]) -> Vec<String> {
        let Some(url) = self
            .source_url
            .as_ref()
            .filter(|_| self.source == DescriptionSource::Crate)
        else {
            return Vec::new();
        };
        let matches = crates
            .iter()
            .any(|name| crate_page(name.as_str()).as_ref() == Some(url));
        if matches {
            Vec::new()
        } else {
            vec![format!(
                "a `crate` description needs `source_url` `{CRATES_IO_PAGE}<crate>` for one of the entry's crates, found `{url}`"
            )]
        }
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
    /// Category id: what the project is. At most one.
    pub category: Option<Slug>,
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
        if let Some(homepage) = self.homepage.as_ref().filter(|url| url.scheme() != "https") {
            out.push(format!("homepage `{homepage}` must use https"));
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
        out.extend(self.description.crate_source_problems(&self.crates));
        out.extend(self.description.repo_source_problems(&self.repo));
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
    fn kind_names_match_the_data_files() {
        for kind in [EntryKind::Library, EntryKind::App] {
            let text = format!("kind = \"{}\"", kind.as_str());
            let parsed: toml::Table = toml::from_str(&text).expect("toml");
            let value = parsed.get("kind").cloned().expect("kind");
            assert_eq!(value.try_into::<EntryKind>().expect("kind"), kind);
        }
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
    fn non_https_homepage_is_reported() {
        let text = format!("homepage = \"javascript:alert(1)\"\n{ALPHA}");
        assert!(has(
            &problems(&text),
            "homepage `javascript:alert(1)` must use https"
        ));
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

    fn crate_source(crates: &str, name: &str) -> String {
        let text = ALPHA
            .replace("crates = [\"alpha-crate\"]", crates)
            .replace("source = \"human\"", "source = \"crate\"");
        format!("{text}source_url = \"{name}\"\n")
    }

    #[test]
    fn crate_description_needs_a_crate_page_of_the_entry() {
        let crates = "crates = [\"alpha-crate\"]";
        let missing = ALPHA.replace("source = \"human\"", "source = \"crate\"");
        assert!(has(
            &problems(&missing),
            "`crate` description requires `source_url`"
        ));
        let page = crate_source(crates, "https://crates.io/crates/alpha-crate");
        assert!(problems(&page).is_empty());
        for bad in [
            "https://crates.io/crates/other-crate",
            "https://github.com/ml-rust/alpha",
            "http://crates.io/crates/alpha-crate",
        ] {
            let found = problems(&crate_source(crates, bad));
            assert!(has(&found, "needs `source_url`"), "{bad}: {found:?}");
        }
    }

    #[test]
    fn repo_description_needs_the_repo_of_the_entry() {
        let text = ALPHA.replace("source = \"human\"", "source = \"repo\"");
        assert!(has(
            &problems(&text),
            "`repo` description requires `source_url`"
        ));
        let same = format!("{text}source_url = \"https://github.com/ml-rust/alpha\"\n");
        assert!(problems(&same).is_empty());
        for bad in [
            "https://github.com/ml-rust/beta",
            "https://github.com/ml-rust/alpha#readme",
            "https://crates.io/crates/alpha-crate",
        ] {
            let found = problems(&format!("{text}source_url = \"{bad}\"\n"));
            assert!(has(&found, "equal to the entry's repo"), "{bad}: {found:?}");
        }
    }

    #[test]
    fn crate_page_may_name_any_listed_crate() {
        let crates = "crates = [\"alpha-crate\", \"alpha-extra\"]";
        let extra = crate_source(crates, "https://crates.io/crates/alpha-extra");
        assert!(problems(&extra).is_empty());
        let unlisted = crate_source(crates, "https://crates.io/crates/alpha");
        assert!(!problems(&unlisted).is_empty());
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
