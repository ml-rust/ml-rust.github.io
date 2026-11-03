//! Shared fixtures for unit tests.

use crate::catalogue::Catalogue;
use crate::load::{DataSources, SourceFile};
use crate::registry::Thresholds;

/// A library entry. Ends inside the `[description]` table.
pub(crate) const ALPHA: &str = r#"name = "Alpha"
kind = "library"
repo = "https://github.com/ml-rust/alpha"
crates = ["alpha-crate"]
topics = ["tensors"]
added = 2026-11-03

[description]
text = "A tensor library."
source = "human"
"#;

/// An org-affiliated app. Ends inside the `[distribution]` table.
pub(crate) const BETA: &str = r#"name = "Beta"
kind = "app"
repo = "https://github.com/ml-rust/beta"
crates = []
topics = ["tensors"]
org_affiliated = true
added = "2026-11-03"

[description]
text = "A command line tool."
source = "human"

[distribution]
homebrew = "beta"
docker = ["ml-rust/beta"]
"#;

/// A correctness judgment on `alpha`, issue 7.
pub(crate) const JUDGMENT: &str = r#"entry = "alpha"
issue = "https://github.com/ml-rust/ml-rust.github.io/issues/7"
reporter = "dave"
approver = "carol"
date = 2026-11-03
evidence = ["https://example.com/proof"]

[judgment]
type = "correctness"
finding = "The README claims support the code lacks."
"#;

/// Judgment `number` on `entry`, of type `supersedes` by `by`.
pub(crate) fn supersedes_judgment(number: u32, entry: &str, by: &str) -> String {
    format!(
        "{}[judgment]\ntype = \"supersedes\"\nby = \"{by}\"\n",
        judgment_head(number, entry)
    )
}

/// Judgment 7 on `alpha`, of type `category` with `topics` as a TOML array.
pub(crate) fn category_judgment(topics: &str) -> String {
    format!(
        "{}[judgment]\ntype = \"category\"\ntopics = {topics}\n",
        judgment_head(7, "alpha")
    )
}

fn judgment_head(number: u32, entry: &str) -> String {
    let head = JUDGMENT.split("[judgment]").next().unwrap_or_default();
    head.replace("entry = \"alpha\"", &format!("entry = \"{entry}\""))
        .replace("/issues/7", &format!("/issues/{number}"))
}

/// The starting thresholds, uncalibrated.
pub(crate) const THRESHOLDS: &str = r#"version = 1

[new]
max_age_days = 180
min_active_users = 2.0

[library_featured]
min_active_users = 10.0
min_new_adopters = 5.0
min_former_glory = 0.5

[app_featured]
min_signals = 2
binary_downloads = 1000
homebrew_on_request_365d = 500
repology_families = 2
docker_pull_growth_90d = 10000

[dormancy]
min_days = 365
dead_rot_signals = 2
dead_consecutive_checks = 2

[interval]
patch_gt3_major_ge1 = 700
patch_gt3 = 450
patch_gt0 = 300
major_ge1 = 200
minor_gt3 = 140
other = 80
days_per_release_cap = 50
downloads_per_month_double = 50000
active_users_double = 10.0

[rot]
unanswered_days = 270
maintainer_silence_days = 365
advisory_grace_days = 60
lost_glory_below = 0.5

[finished]
min_former_glory = 0.8
min_new_adopters = 1.0
min_binary_downloads_per_release = 200
"#;

/// [`THRESHOLDS`] parsed.
pub(crate) fn thresholds() -> Thresholds {
    match toml::from_str(THRESHOLDS) {
        Ok(parsed) => parsed,
        Err(error) => panic!("THRESHOLDS fixture does not parse: {error}"),
    }
}

const MAINTAINERS: &str = r#"[[maintainer]]
login = "carol"
org_member = true
since = 2026-01-01

[[maintainer]]
login = "bob"
org_member = false
since = 2026-02-01
"#;

const TOPICS: &str = r#"[[topic]]
id = "ml"
name = "Machine learning"

[[topic]]
id = "tensors"
name = "Tensors"
parent = "ml"
"#;

const SYNONYMS: &str = r#"[[group]]
terms = ["gpu", "cuda"]
"#;

pub(crate) fn file(path: &str, contents: &str) -> SourceFile {
    SourceFile {
        path: path.to_owned(),
        contents: contents.to_owned(),
    }
}

/// A data set that passes every rule.
pub(crate) fn valid() -> DataSources {
    DataSources {
        entries: vec![
            file("entries/alpha.toml", ALPHA),
            file("entries/beta.toml", BETA),
        ],
        judgments: vec![file("judgments/7.toml", JUDGMENT)],
        maintainers: Some(file("maintainers.toml", MAINTAINERS)),
        topics: Some(file("topics.toml", TOPICS)),
        synonyms: Some(file("synonyms.toml", SYNONYMS)),
        discovery: None,
        thresholds: None,
    }
}

/// Replaces the file at `path`, or adds it when absent.
///
/// A path that contains `entries/` or `judgments/` goes to that list.
/// `maintainers.toml`, `topics.toml`, `discovery.toml` and `thresholds.toml` replace the optional
/// file of that name.
/// Any other path replaces the synonyms file.
pub(crate) fn put(sources: &mut DataSources, path: &str, contents: &str) {
    let list = if path.contains("entries/") {
        &mut sources.entries
    } else if path.contains("judgments/") {
        &mut sources.judgments
    } else {
        let slot = match path {
            "maintainers.toml" => &mut sources.maintainers,
            "topics.toml" => &mut sources.topics,
            "discovery.toml" => &mut sources.discovery,
            "thresholds.toml" => &mut sources.thresholds,
            _ => &mut sources.synonyms,
        };
        *slot = Some(file(path, contents));
        return;
    };
    match list.iter_mut().find(|f| f.path == path) {
        Some(existing) => existing.contents = contents.to_owned(),
        None => list.push(file(path, contents)),
    }
}

/// Every diagnostic from loading `sources`, one `path: message` string each.
pub(crate) fn errors(sources: DataSources) -> Vec<String> {
    match Catalogue::load(sources) {
        Ok(_) => Vec::new(),
        Err(diags) => diags.iter().map(ToString::to_string).collect(),
    }
}

/// Asserts that loading fails with a diagnostic that contains `needle`.
pub(crate) fn expect_error(sources: DataSources, needle: &str) {
    let found = errors(sources);
    assert!(
        found.iter().any(|message| message.contains(needle)),
        "no diagnostic contains `{needle}`: {found:?}"
    );
}
