//! Four entries over two topics and three categories, with judgments, a tier report and tier changes.
//!
//! Every test file that includes this module uses each item.

use std::collections::BTreeMap;

use catalogue_core::{
    CONTRIBUTORS_SIGNAL, Catalogue, DEPENDENTS_SIGNAL, DataSources, Date, EntryReport, FirstSeen,
    LICENSE_SIGNAL, MeasuredMovement, Movement, NATIVE_CODE_SIGNAL, PYTHON_SIGNAL, REPORT_SCHEMA,
    RUNS_ON_EVIDENCE_SIGNAL, RUNS_ON_SIGNAL, ReasonRecord, Rule, STARS_SIGNAL, SignalFact, Slug,
    SourceRef, Tier, TierReport, UsersChange, never_counts,
};
use catalogue_site::{SiteInputs, TierChangeRow};
use url::Url;

use crate::build::{render, source, thresholds};

/// A top-level topic and a subtopic, each with a sourced summary.
const TOPICS: &str = r#"[[topic]]
id = "ml"
name = "Machine learning"

[topic.summary]
text = "Projects that train or run machine learning models."
evidence = ["https://en.wikipedia.org/wiki/Machine_learning"]

[[topic]]
id = "tensors"
name = "Tensors"
parent = "ml"

[topic.summary]
text = "Libraries for n-dimensional arrays and the math on them."
evidence = ["https://en.wikipedia.org/wiki/Tensor_(machine_learning)"]
"#;

/// Three categories. No entry names the last.
const CATEGORIES: &str = r#"[[category]]
id = "inference-runtimes"
name = "Inference runtimes"
blurb = "Programs and libraries that load a trained model and run it."

[[category]]
id = "tensor-libraries"
name = "Tensor libraries"
blurb = "Libraries for multidimensional arrays and their math."

[[category]]
id = "vector-databases"
name = "Vector databases"
blurb = "Databases that store vectors and find the ones nearest to a query."
"#;

/// Two backends and one crate that shows Python bindings.
const BACKENDS: &str = r#"python = ["pyo3"]

[[backend]]
id = "cuda"
name = "CUDA"
dependencies = ["cudarc"]
features = ["cuda"]

[[backend]]
id = "metal"
name = "Metal"
features = ["metal"]
"#;

/// An org member and an outside maintainer.
const MAINTAINERS: &str = r#"[[maintainer]]
login = "carol"
org_member = true
since = 2026-01-01

[[maintainer]]
login = "bob"
org_member = false
since = 2026-02-01
"#;

/// Two groups that Beta matches, one that Delta matches, and one that no entry matches.
const SYNONYMS: &str = r#"[[group]]
terms = ["ml", "machine learning"]

[[group]]
terms = ["cli", "command line", "command-line interface"]

[[group]]
terms = ["autodiff", "automatic differentiation"]

[[group]]
terms = ["ocr", "optical character recognition"]
"#;

/// A library with two crates and a homepage, described by its crate.
const ALPHA: &str = r#"name = "Alpha"
kind = "library"
repo = "https://github.com/ml-rust/alpha"
homepage = "https://alpha.example.org/"
crates = ["alpha-crate", "alpha-extra"]
topics = ["tensors"]
category = "tensor-libraries"
added = 2026-10-01

[description]
text = "A tensor library."
source = "crate"
source_url = "https://crates.io/crates/alpha-crate"
"#;

/// An org-affiliated app with every install channel, described by its README.
const BETA: &str = r#"name = "Beta"
kind = "app"
repo = "https://github.com/ml-rust/beta"
crates = []
topics = ["ml"]
category = "inference-runtimes"
org_affiliated = true
added = 2026-11-03

[description]
text = "A command line tool that runs models."
source = "readme"
source_url = "https://github.com/ml-rust/beta#readme"

[distribution]
homebrew = "beta"
docker = ["ml-rust/beta"]
repology = "beta"
"#;

/// A library that Alpha replaces.
const GAMMA: &str = r#"name = "Gamma"
kind = "library"
repo = "https://github.com/someone/gamma"
crates = ["gamma"]
topics = ["tensors"]
category = "tensor-libraries"
added = 2026-09-15

[description]
text = "An older tensor library."
source = "human"
"#;

/// A library that a topic judgment moves from `ml` to `tensors`.
const DELTA: &str = r#"name = "Delta"
kind = "library"
repo = "https://github.com/someone/delta"
crates = ["delta"]
topics = ["ml"]
added = 2026-11-02

[description]
text = "A small automatic differentiation crate."
source = "human"
"#;

/// A correctness judgment on Alpha.
const CORRECTNESS: &str = r#"entry = "alpha"
issue = "https://github.com/ml-rust/ml-rust.github.io/issues/11"
reporter = "dave"
approver = "carol"
date = 2026-10-20
evidence = ["https://github.com/ml-rust/alpha/issues/3"]

[judgment]
type = "correctness"
finding = "The README claims GPU support that the code lacks."
"#;

/// Alpha replaces Gamma.
const SUPERSEDES: &str = r#"entry = "gamma"
issue = "https://github.com/ml-rust/ml-rust.github.io/issues/12"
reporter = "erin"
approver = "carol"
date = 2026-10-25
evidence = ["https://github.com/someone/gamma/issues/9"]

[judgment]
type = "supersedes"
by = "alpha"
"#;

/// Beta is a product. An outside maintainer approves it, as Beta is org affiliated.
const KIND: &str = r#"entry = "beta"
issue = "https://github.com/ml-rust/ml-rust.github.io/issues/13"
reporter = "dave"
approver = "bob"
date = 2026-11-03
evidence = ["https://github.com/ml-rust/beta#readme"]

[judgment]
type = "kind"
value = "product"
"#;

/// Delta moves to the `tensors` topic.
const TOPIC: &str = r#"entry = "delta"
issue = "https://github.com/ml-rust/ml-rust.github.io/issues/14"
reporter = "erin"
approver = "carol"
date = 2026-11-03
evidence = ["https://github.com/someone/delta/blob/main/README.md"]

[judgment]
type = "topic"
topics = ["tensors"]
"#;

/// The owner of Alpha states that it is maintained.
const MAINTAINER: &str = r#"entry = "alpha"
issue = "https://github.com/ml-rust/ml-rust.github.io/issues/15"
reporter = "frank"
approver = "bob"
date = 2026-11-01

[judgment]
type = "maintainer"
status = "maintained"
"#;

/// Every data file of the fixture as `(path, contents)`, without the thresholds.
pub fn files() -> Vec<(String, String)> {
    [
        ("topics.toml", TOPICS),
        ("categories.toml", CATEGORIES),
        ("backends.toml", BACKENDS),
        ("maintainers.toml", MAINTAINERS),
        ("synonyms.toml", SYNONYMS),
        ("entries/alpha.toml", ALPHA),
        ("entries/beta.toml", BETA),
        ("entries/gamma.toml", GAMMA),
        ("entries/delta.toml", DELTA),
        ("judgments/11.toml", CORRECTNESS),
        ("judgments/12.toml", SUPERSEDES),
        ("judgments/13.toml", KIND),
        ("judgments/14.toml", TOPIC),
        ("judgments/15.toml", MAINTAINER),
    ]
    .into_iter()
    .map(|(path, contents)| (path.to_owned(), contents.to_owned()))
    .collect()
}

/// Every file of a build over `files` and calibrated thresholds, keyed by path, as UTF-8 text.
///
/// `tiered` adds the tier report. The tier changes and the movement go in either way.
pub fn site(files: &[(String, String)], tiered: bool) -> BTreeMap<String, String> {
    site_with(files, tiered, &movement())
}

/// The build of [`site`], with `movement` in place of the fixture's movement.
pub fn site_with(
    files: &[(String, String)],
    tiered: bool,
    movement: &Movement,
) -> BTreeMap<String, String> {
    let (catalogue, report) = data(files);
    let changes = changes();
    render(&SiteInputs {
        catalogue: &catalogue,
        report: tiered.then_some(&report),
        changes: &changes,
        first_check: Some(first_check()),
        movement,
    })
}

/// The checked catalogue of `files` with calibrated thresholds, and the tier report over it.
pub fn data(files: &[(String, String)]) -> (Catalogue, TierReport) {
    (load(files), report())
}

/// The checked catalogue of `files`, with a thresholds file calibrated on 2026-11-03.
fn load(files: &[(String, String)]) -> Catalogue {
    let thresholds = thresholds(Some("2026-11-03"));
    let mut sources = DataSources {
        thresholds: Some(source("thresholds.toml", &thresholds)),
        ..DataSources::default()
    };
    for (path, contents) in files {
        let file = source(path, contents);
        if path.starts_with("entries/") {
            sources.entries.push(file);
        } else if path.starts_with("judgments/") {
            sources.judgments.push(file);
        } else if path == "topics.toml" {
            sources.topics = Some(file);
        } else if path == "categories.toml" {
            sources.categories = Some(file);
        } else if path == "backends.toml" {
            sources.backends = Some(file);
        } else if path == "maintainers.toml" {
            sources.maintainers = Some(file);
        } else if path == "synonyms.toml" {
            sources.synonyms = Some(file);
        } else {
            panic!("the fixture has no place for {path}");
        }
    }
    Catalogue::load(sources)
        .unwrap_or_else(|diagnostics| panic!("fixture is invalid:\n{diagnostics}"))
}

fn day(month: i8, day: i8) -> Date {
    Date::new(2026, month, day).expect("date")
}

fn slug(text: &str) -> Slug {
    Slug::new(text).expect("slug")
}

fn reason(rule: Rule, detail: &str) -> ReasonRecord {
    ReasonRecord {
        rule,
        detail: detail.to_owned(),
    }
}

/// A fact named `name`. A value comes from `source` at `url`, fetched on 2026-11-03.
fn signal(name: &str, label: &str, value: Option<&str>, source: (&str, &str)) -> SignalFact {
    let (source_label, url) = source;
    SignalFact {
        name: name.to_owned(),
        label: label.to_owned(),
        value: value.map(str::to_owned),
        source: value.map(|_| SourceRef::new(source_label, Url::parse(url).expect("url"))),
        fetched: value.map(|_| day(11, 3)),
        counts_toward_tier: !never_counts(name),
    }
}

/// The source of every crates.io dump value.
const DUMP: (&str, &str) = ("crates.io database dump", "https://crates.io/data-access");

fn record(tier: Tier, reasons: Vec<ReasonRecord>, signals: Vec<SignalFact>) -> EntryReport {
    EntryReport {
        tier,
        stable: false,
        dormant: Some(false),
        demotion_pending: None,
        reasons,
        unknown: Vec::new(),
        notes: Vec::new(),
        signals,
    }
}

/// The report of a check on 2026-11-04 over the fixture.
fn report() -> TierReport {
    let alpha_repo = ("GitHub", "https://github.com/ml-rust/alpha");
    let mut alpha = record(
        Tier::Featured,
        vec![
            reason(Rule::ExpectedInterval, "expected interval 80 days"),
            reason(Rule::Featured, "fails: active users 9 below 11"),
            reason(
                Rule::DemotionPending,
                "stays Featured until a demotion judgment is approved",
            ),
        ],
        vec![
            signal("last_release", "Latest release", Some("2026-09-01"), DUMP),
            signal("active_users", "Active users", Some("9.36"), DUMP),
            signal("former_glory", "Former glory", None, DUMP),
            signal(STARS_SIGNAL, "GitHub stars", Some("1200"), alpha_repo),
            signal(LICENSE_SIGNAL, "License", Some("Apache-2.0 OR MIT"), DUMP),
            signal(RUNS_ON_SIGNAL, "Runs on", Some("CUDA, Metal"), DUMP),
            signal(
                RUNS_ON_EVIDENCE_SIGNAL,
                "What shows each backend",
                Some(
                    "CUDA: alpha-crate feature cuda (dependency cudarc); Metal: alpha-extra feature metal",
                ),
                DUMP,
            ),
            signal(
                NATIVE_CODE_SIGNAL,
                "Native code",
                Some("Binds native code: openssl-sys"),
                DUMP,
            ),
            signal(
                PYTHON_SIGNAL,
                "Python bindings",
                Some("Has Python bindings: alpha-extra needs Python"),
                DUMP,
            ),
            signal(DEPENDENTS_SIGNAL, "Dependents", Some("12"), DUMP),
            signal(
                CONTRIBUTORS_SIGNAL,
                "Contributors",
                Some("38"),
                (
                    "GitHub contributors",
                    "https://github.com/ml-rust/alpha/graphs/contributors",
                ),
            ),
        ],
    );
    alpha.demotion_pending = Some(Tier::Listed);
    alpha.unknown = vec!["former_glory".to_owned()];
    let beta_releases = (
        "GitHub releases",
        "https://github.com/ml-rust/beta/releases",
    );
    let mut beta = record(
        Tier::Listed,
        vec![
            reason(Rule::Finished, "dormant 400 days with no rot signal"),
            reason(
                Rule::FeaturedNeedsOutsideApproval,
                "passes the Featured gates with no featured-approval judgment",
            ),
        ],
        vec![
            signal(
                "binary_downloads_per_release",
                "Binary downloads per release",
                Some("1500"),
                beta_releases,
            ),
            signal("repology_families", "Repology families", None, DUMP),
            signal(LICENSE_SIGNAL, "License", Some("None stated"), DUMP),
            signal(RUNS_ON_SIGNAL, "Runs on", None, DUMP),
            signal(
                STARS_SIGNAL,
                "GitHub stars",
                Some("40"),
                ("GitHub", "https://github.com/ml-rust/beta"),
            ),
        ],
    );
    beta.stable = true;
    beta.dormant = Some(true);
    beta.unknown = vec!["repology_families".to_owned()];
    beta.notes = vec!["Homebrew returned no install count.".to_owned()];
    let gamma = record(
        Tier::Archived,
        vec![reason(Rule::InstantArchive, "the repository is archived")],
        vec![signal(
            "repo_archived",
            "Repository archived",
            Some("yes"),
            ("GitHub", "https://github.com/someone/gamma"),
        )],
    );
    let delta = record(
        Tier::New,
        vec![reason(Rule::New, "first release 30 days ago, under 180")],
        vec![
            signal("first_release", "First release", Some("2026-10-05"), DUMP),
            signal(LICENSE_SIGNAL, "License", Some("MIT"), DUMP),
            signal(
                RUNS_ON_SIGNAL,
                "Runs on",
                Some("No GPU feature found"),
                DUMP,
            ),
        ],
    );
    let entries = [
        ("alpha", alpha),
        ("beta", beta),
        ("gamma", gamma),
        ("delta", delta),
    ]
    .into_iter()
    .map(|(name, record)| (slug(name), record))
    .collect();
    TierReport {
        schema: REPORT_SCHEMA,
        generated: day(11, 4),
        thresholds_version: 3,
        calibrated: Some(day(11, 3)),
        entries,
    }
}

/// The date of the oldest snapshot behind [`changes`].
pub fn first_check() -> Date {
    day(9, 1)
}

/// The movement of a check on 2026-11-04: Delta is new, Alpha rises and Beta falls.
/// `omega` is an entry the catalogue no longer holds.
pub fn movement() -> Movement {
    let users = |name: &str, from, to| UsersChange {
        slug: slug(name),
        from,
        to,
    };
    Movement::Measured(MeasuredMovement {
        newest: day(11, 4),
        baseline: day(10, 5),
        new: vec![FirstSeen {
            slug: slug("delta"),
            date: day(11, 2),
        }],
        rising: vec![users("alpha", 5.0, 9.36), users("omega", 1.0, 90.0)],
        falling: vec![users("beta", 40.0, 20.5)],
    })
}

/// Tier changes, newest first. `omega` is an entry the catalogue no longer holds.
pub fn changes() -> Vec<TierChangeRow> {
    let change = |month, on, name: &str, from, to| TierChangeRow {
        date: day(month, on),
        slug: slug(name),
        from,
        to,
    };
    vec![
        change(11, 4, "alpha", Some(Tier::Listed), Tier::Featured),
        change(11, 4, "delta", None, Tier::New),
        change(10, 28, "gamma", Some(Tier::Listed), Tier::Archived),
        change(9, 1, "omega", None, Tier::Listed),
    ]
}
