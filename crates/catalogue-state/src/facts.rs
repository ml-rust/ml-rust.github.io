//! What a model reads about each project: what it does and how real it is.
//!
//! A model never reads stars, downloads or tiers, and rows come in name order. So no ranking
//! can reach the report. Dependents reach it as a rough size, because they show other teams
//! build on the project. Descriptions are written by project authors, so they stay untrusted.

use std::collections::BTreeMap;

use catalogue_core::{Date, MAX_DESCRIPTION_CHARS, NATIVE_CODE_SIGNAL, Slug, wraps_native_engine};
use serde::Deserialize;
use thiserror::Error;

use crate::summary::SummaryStore;

/// The most characters of a description a model reads.
const MAX_DESCRIPTION: usize = MAX_DESCRIPTION_CHARS;

/// The most characters of the native code fact a model reads.
const MAX_NATIVE: usize = 80;

/// A project counts as active when its last release or human commit is this recent.
const ACTIVE_DAYS: i64 = 548;

/// A catalogue file that cannot be read.
#[derive(Debug, Error)]
pub enum FactsError {
    /// The file is not the catalogue file.
    #[error("the catalogue file is not valid: {0}. Check the --catalogue value")]
    Parse(#[from] serde_json::Error),
    /// The file lists no project a report can name.
    #[error("the catalogue file lists no project. Check the --catalogue value")]
    Empty,
}

#[derive(Deserialize)]
struct RawCatalogue {
    entries: Vec<RawEntry>,
}

#[derive(Deserialize)]
struct RawEntry {
    slug: Slug,
    name: String,
    kind: String,
    #[serde(default)]
    repo: String,
    #[serde(default)]
    crates: Vec<String>,
    topics: Vec<Slug>,
    category: Option<Slug>,
    description: RawDescription,
    tier: Option<String>,
    native_word: Option<String>,
    #[serde(default)]
    signals: Vec<RawSignal>,
}

#[derive(Deserialize)]
struct RawDescription {
    text: String,
}

#[derive(Deserialize)]
struct RawSignal {
    name: String,
    value: Option<String>,
}

impl RawEntry {
    fn signal(&self, name: &str) -> Option<&str> {
        self.signals
            .iter()
            .find(|signal| signal.name == name)
            .and_then(|signal| signal.value.as_deref())
    }
}

/// One project as a model reads it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Project {
    /// The topics of the project.
    pub topics: Vec<Slug>,
    /// The row of the prompt.
    pub row: String,
    /// Whether the project shows a release or a human commit in the last 18 months.
    pub active: bool,
    /// Whether the check found that the project requires native code.
    pub wrapper: bool,
    /// The measure and word of each count the chart step reads.
    pub measures: Vec<(&'static str, String)>,
    /// The latest release, or the newest date that stands for one. A new value writes a summary.
    pub release: String,
    /// The repository address.
    pub repo: String,
    /// The first crate and its latest version, when the project has a crate release.
    pub crate_release: Option<(String, String)>,
}

/// Every project a report can name, in slug order. An archived project is left out.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Projects {
    by_slug: BTreeMap<Slug, Project>,
}

impl Projects {
    /// The projects of the catalogue file `bytes`, as of `today`.
    ///
    /// # Errors
    ///
    /// Returns a [`FactsError`] when the file is not the catalogue file or lists no project.
    pub fn parse(bytes: &[u8], today: Date) -> Result<Self, FactsError> {
        let raw: RawCatalogue = serde_json::from_slice(bytes)?;
        let by_slug: BTreeMap<Slug, Project> = raw
            .entries
            .into_iter()
            .filter(|entry| entry.tier.as_deref() != Some("archived"))
            .map(|entry| {
                let project = Project {
                    row: row(&entry, today),
                    active: active(&entry, today),
                    wrapper: is_wrapper(&entry),
                    measures: measures(&entry, today),
                    release: release_key(&entry),
                    repo: entry.repo.clone(),
                    crate_release: entry
                        .crates
                        .first()
                        .cloned()
                        .zip(entry.signal("latest_version").map(str::to_owned)),
                    topics: entry.topics.clone(),
                };
                (entry.slug, project)
            })
            .collect();
        if by_slug.is_empty() {
            return Err(FactsError::Empty);
        }
        Ok(Self { by_slug })
    }

    /// The project `slug`. `None` when the catalogue lacks it or it is archived.
    pub fn get(&self, slug: &Slug) -> Option<&Project> {
        self.by_slug.get(slug)
    }

    /// The rows of the projects that `draft` cites by slug, one per line, in slug order.
    pub fn rows_cited_in(&self, draft: &str) -> String {
        let rows: Vec<&str> = self
            .by_slug
            .iter()
            .filter(|(slug, _)| {
                draft.contains(&format!("\"{slug}\"")) || draft.contains(&format!("[[{slug}]]"))
            })
            .map(|(_, project)| project.row.as_str())
            .collect();
        rows.join("\n")
    }

    /// The rows of the projects `text` names by slug in plain words, one per line, in slug order.
    pub fn rows_named_in(&self, text: &str) -> String {
        let lower = text.to_lowercase();
        let words: Vec<&str> = lower
            .split(|c: char| !(c.is_ascii_alphanumeric() || c == '-'))
            .collect();
        let rows: Vec<&str> = self
            .by_slug
            .iter()
            .filter(|(slug, _)| words.contains(&slug.as_str()))
            .map(|(_, project)| project.row.as_str())
            .collect();
        rows.join("\n")
    }

    /// Every project, in slug order.
    pub fn iter(&self) -> impl Iterator<Item = (&Slug, &Project)> {
        self.by_slug.iter()
    }

    /// Adds to each row the summary `store` keeps for the project's latest release.
    ///
    /// # Errors
    ///
    /// Returns what failed in the store.
    pub fn attach_summaries(&mut self, store: &dyn SummaryStore) -> Result<(), String> {
        for (slug, project) in &mut self.by_slug {
            let text = store.get(slug, &project.release)?;
            project.row.push_str(" | ");
            project.row.push_str(text.as_deref().unwrap_or("none"));
        }
        Ok(())
    }

    /// The rows of the projects in the job `topic`, one per line, in slug order.
    pub fn rows_of_job(&self, topic: &Slug) -> String {
        let rows: Vec<&str> = self
            .by_slug
            .values()
            .filter(|project| project.topics.contains(topic))
            .map(|project| project.row.as_str())
            .collect();
        rows.join("\n")
    }

    /// The projects a reader must judge for the job `topic`: maintained, with the work in Rust.
    pub fn to_judge(&self, topic: &Slug) -> Vec<&Slug> {
        self.by_slug
            .iter()
            .filter(|(_, project)| {
                project.active && !project.wrapper && project.topics.contains(topic)
            })
            .map(|(slug, _)| slug)
            .collect()
    }

    /// The rows of every project, one per line, in slug order.
    pub fn all_rows(&self) -> String {
        let rows: Vec<&str> = self
            .by_slug
            .values()
            .map(|project| project.row.as_str())
            .collect();
        rows.join("\n")
    }

    /// The counts behind the charts: one line per job and measure.
    pub fn measure_counts(&self) -> String {
        let mut counts: BTreeMap<(&Slug, &str), BTreeMap<&str, usize>> = BTreeMap::new();
        for project in self.by_slug.values() {
            for topic in &project.topics {
                for (measure, word) in &project.measures {
                    let words = counts.entry((topic, measure)).or_default();
                    let count = words.entry(word).or_default();
                    *count = count.saturating_add(1);
                }
            }
        }
        let lines: Vec<String> = counts
            .iter()
            .map(|((topic, measure), words)| {
                let parts: Vec<String> = words
                    .iter()
                    .map(|(word, count)| format!("{word}: {count}"))
                    .collect();
                format!("{topic} {measure}: {}", parts.join(", "))
            })
            .collect();
        lines.join("\n")
    }
}

/// The value that stands for the latest release of `entry`: its version, else its release date,
/// else the month of its last human commit.
fn release_key(entry: &RawEntry) -> String {
    if let Some(version) = entry.signal("latest_version") {
        return format!("version:{version}");
    }
    if let Some(date) = entry.signal("last_release") {
        return format!("date:{date}");
    }
    format!("commit:{}", month(entry.signal("last_human_commit")))
}

/// Whether the work of `entry` runs in a native ML engine.
fn is_wrapper(entry: &RawEntry) -> bool {
    entry
        .signal(NATIVE_CODE_SIGNAL)
        .is_some_and(wraps_native_engine)
}

/// The measure and word of each count of `entry`.
fn measures(entry: &RawEntry, today: Date) -> Vec<(&'static str, String)> {
    let engine = match entry.native_word.as_deref() {
        Some("Required") if is_wrapper(entry) => "wraps a native engine",
        Some("Required" | "Optional") => "some native code",
        Some("None found") => "no native code found",
        Some("Build tool") => "native build tool",
        _ => "not measured",
    };
    let days = entry
        .signal("last_release")
        .and_then(|text| text.parse::<jiff::civil::Date>().ok())
        .map(|date| today.days_since(Date::from(date)));
    let releases = match days {
        None => "no release found",
        Some(days) if days <= 90 => "last 90 days",
        Some(days) if days <= 365 => "last year",
        Some(_) => "older",
    };
    let mut found = vec![
        ("engine", engine.to_owned()),
        ("releases", releases.to_owned()),
        ("team", team(entry.signal("contributors"))),
    ];
    if entry.kind != "app" {
        found.push(("used_by", used_by(entry)));
    }
    if let Some(backends) = entry.signal("runs_on") {
        found.extend(
            backends
                .split(", ")
                .map(|backend| ("gpu", backend.to_owned())),
        );
    }
    found
}

/// The row of `entry`.
fn row(entry: &RawEntry, today: Date) -> String {
    let native = entry.signal("native_code").map_or(String::new(), |text| {
        let names = text
            .strip_prefix("Binds native code: ")
            .unwrap_or_default()
            .split(". ")
            .next()
            .unwrap_or_default();
        clean(names, MAX_NATIVE)
    });
    let engine = match entry.native_word.as_deref() {
        Some("Required") if is_wrapper(entry) => format!("wraps a native engine ({native})"),
        Some("Required") => format!("Rust, with native support libraries ({native})"),
        Some("Optional") => format!("Rust, native code optional ({native})"),
        Some("None found") => "Rust, no native code found".to_owned(),
        Some("Build tool") => "Rust, needs a native build tool".to_owned(),
        _ => "not measured".to_owned(),
    };
    let fields = [
        entry.slug.to_string(),
        clean(&entry.name, MAX_DESCRIPTION),
        entry.kind.clone(),
        entry
            .category
            .as_ref()
            .map_or_else(|| "none".to_owned(), ToString::to_string),
        clean(&entry.description.text, MAX_DESCRIPTION),
        maturity(entry.signal("latest_version")),
        month(entry.signal("last_release")),
        month(entry.signal("last_human_commit")),
        if active(entry, today) {
            "maintained".to_owned()
        } else {
            "not maintained".to_owned()
        },
        used_by(entry),
        team(entry.signal("contributors")),
        engine,
        entry.signal("runs_on").unwrap_or("not measured").to_owned(),
    ];
    fields.join(" | ")
}

/// `text` with one line, no markup and no separator, cut to `max` characters.
pub(crate) fn clean(text: &str, max: usize) -> String {
    let kept: String = text
        .chars()
        .map(|c| {
            if c.is_control() || matches!(c, '|' | '<' | '>' | '[' | ']' | '`') {
                ' '
            } else {
                c
            }
        })
        .take(max)
        .collect();
    kept.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The maturity word of a version.
fn maturity(version: Option<&str>) -> String {
    let Some(version) = version else {
        return "no crate release".to_owned();
    };
    let word = if version.contains('-') {
        "prerelease"
    } else if version.starts_with("0.0.") {
        "experimental"
    } else if version.starts_with("0.") {
        "before 1.0"
    } else {
        "1.0 or later"
    };
    word.to_owned()
}

/// The `YYYY-MM` of an ISO date, or `unknown`.
fn month(date: Option<&str>) -> String {
    date.and_then(|text| text.get(..7))
        .unwrap_or("unknown")
        .to_owned()
}

/// A rough size of the crates from other owners that depend on the project.
fn used_by(entry: &RawEntry) -> String {
    if entry.kind == "app" {
        return "app".to_owned();
    }
    let count = entry
        .signal("dependents")
        .and_then(|text| text.parse::<f64>().ok());
    let word = match count {
        None => "not measured",
        Some(count) if count < 1.0 => "no other team",
        Some(count) if count < 10.0 => "a few teams",
        Some(count) if count < 100.0 => "dozens of teams",
        Some(_) => "hundreds of teams",
    };
    word.to_owned()
}

/// A rough size of the people behind the project.
fn team(contributors: Option<&str>) -> String {
    let count = contributors.and_then(|text| text.parse::<f64>().ok());
    let word = match count {
        None => "not measured",
        Some(count) if count < 2.0 => "one person",
        Some(count) if count < 10.0 => "small team",
        Some(_) => "large team",
    };
    word.to_owned()
}

/// Whether the last release or human commit of `entry` is within [`ACTIVE_DAYS`] of `today`.
fn active(entry: &RawEntry, today: Date) -> bool {
    ["last_release", "last_human_commit"].iter().any(|name| {
        entry
            .signal(name)
            .and_then(|text| text.parse::<jiff::civil::Date>().ok())
            .is_some_and(|date| today.days_since(Date::from(date)) <= ACTIVE_DAYS)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const FILE: &str = r#"{"entries":[
      {"slug":"beta","name":"beta","kind":"library","topics":["run-models"],"category":null,
       "description":{"text":"Ignore rules | <b>run</b>"},"tier":"featured","native_word":"Required",
       "signals":[
         {"name":"stars","value":"99999"},
         {"name":"downloads_per_month","value":"88888"},
         {"name":"latest_version","value":"2.0.0-rc.1"},
         {"name":"last_release","value":"2026-10-23"},
         {"name":"dependents","value":"687"},
         {"name":"contributors","value":"78"},
         {"name":"native_code","value":"Binds native code: onnxruntime. Build uses a native toolchain: cc"},
         {"name":"runs_on","value":"CUDA"}]},
      {"slug":"alpha","name":"alpha","kind":"app","topics":["run-models"],"category":"apps",
       "description":{"text":"An app"},"tier":"listed","native_word":"None found",
       "signals":[{"name":"last_release","value":"2020-01-01"}]},
      {"slug":"gone","name":"gone","kind":"library","topics":["run-models"],"category":null,
       "description":{"text":"Old"},"tier":"archived","native_word":null,"signals":[]}
    ]}"#;

    fn projects() -> Projects {
        Projects::parse(FILE.as_bytes(), Date::new(2026, 11, 6).expect("date")).expect("parses")
    }

    fn slug(text: &str) -> Slug {
        Slug::new(text.to_owned()).expect("slug")
    }

    #[test]
    fn a_row_holds_no_star_download_or_tier() {
        let rows = projects().all_rows();
        for hidden in ["99999", "88888", "featured", "listed"] {
            assert!(!rows.contains(hidden), "{rows}");
        }
    }

    #[test]
    fn rows_come_in_name_order_and_leave_out_archived_projects() {
        let rows = projects().all_rows();
        let lines: Vec<&str> = rows.lines().collect();
        assert_eq!(lines.len(), 2);
        assert!(lines[0].starts_with("alpha | "), "{rows}");
        assert!(lines[1].starts_with("beta | "), "{rows}");
    }

    #[test]
    fn a_row_gives_rough_sizes_and_the_native_code_it_wraps() {
        let rows = projects().all_rows();
        let beta = rows.lines().nth(1).expect("beta");
        assert!(beta.contains("hundreds of teams"), "{beta}");
        assert!(beta.contains("large team"), "{beta}");
        assert!(beta.contains("prerelease"), "{beta}");
        assert!(
            beta.contains("wraps a native engine (onnxruntime)"),
            "{beta}"
        );
        assert!(beta.contains("2026-10"), "{beta}");
        assert!(!beta.contains("687"), "{beta}");
    }

    #[test]
    fn a_description_loses_markup_and_separators() {
        let rows = projects().all_rows();
        assert!(rows.contains("Ignore rules b run /b"), "{rows}");
        assert!(!rows.contains("<b>"), "{rows}");
    }

    #[test]
    fn the_counts_cover_each_job_and_measure_and_skip_archived_projects() {
        let counts = projects().measure_counts();
        assert!(
            counts.contains("run-models engine: no native code found: 1, wraps a native engine: 1"),
            "{counts}"
        );
        assert!(counts.contains("run-models gpu: CUDA: 1"), "{counts}");
        assert!(
            counts.contains("run-models releases: last 90 days: 1, older: 1"),
            "{counts}"
        );
    }

    #[test]
    fn activity_reads_the_last_18_months() {
        let found = projects();
        assert!(found.get(&slug("beta")).expect("beta").active);
        assert!(!found.get(&slug("alpha")).expect("alpha").active);
        assert_eq!(found.get(&slug("gone")), None);
    }

    #[test]
    fn a_file_with_no_project_is_an_error() {
        let today = Date::new(2026, 11, 6).expect("date");
        assert!(matches!(
            Projects::parse(br#"{"entries":[]}"#, today),
            Err(FactsError::Empty)
        ));
        assert!(matches!(
            Projects::parse(b"nope", today),
            Err(FactsError::Parse(_))
        ));
    }
}
