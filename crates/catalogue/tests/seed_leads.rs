#[path = "common/lead_transport.rs"]
mod lead_transport;

use std::fs;
use std::path::{Path, PathBuf};

use catalogue::{
    LeadSeed, Outcome, SeedError, build_lead_seed, load_catalogue, parse_leads, validate,
    write_drafts,
};
use catalogue_core::{CrateName, DescriptionSource, Entry, EntryKind, Topics};
use catalogue_signals::dump::{CrateRow, DumpMeta, DumpSnapshot, Owner, OwnerKind, VersionRow};
use catalogue_signals::http::{Headers, Pacer};
use catalogue_signals::seed::{ExistingIndex, LeadInputs, LeadPacers};
use jiff::Timestamp;
use lead_transport::Script;
use serde_json::json;

const TOKEN: &str = "s3cret-token";

/// The topics of the `valid` fixture, with terms.
const TOPICS: &str = r#"
[[topic]]
id = "ml"
name = "Machine learning"
keywords = ["machine-learning"]
terms = ["neural network"]

[[topic]]
id = "tensors"
name = "Tensors"
keywords = ["tensor"]
terms = ["tensor"]
"#;

fn at(text: &str) -> Timestamp {
    text.parse().expect("timestamp")
}

/// One crate of the test dump. It has one shared owner and carries the `tensor` keyword.
struct Spec {
    id: u32,
    name: &'static str,
    repository: &'static str,
    description: Option<&'static str>,
    has_lib: Option<bool>,
    has_bin: bool,
}

fn library(id: u32, name: &'static str, repository: &'static str) -> Spec {
    Spec {
        id,
        name,
        repository,
        description: Some("A crate."),
        has_lib: Some(true),
        has_bin: false,
    }
}

fn binary(id: u32, name: &'static str, repository: &'static str) -> Spec {
    Spec {
        has_lib: Some(false),
        has_bin: true,
        ..library(id, name, repository)
    }
}

fn snapshot(specs: &[Spec]) -> DumpSnapshot {
    let mut snapshot = DumpSnapshot {
        meta: DumpMeta {
            timestamp: at("2026-11-02T00:00:00Z"),
            crates_io_commit: String::new(),
            directory: String::new(),
        },
        crates: Default::default(),
        versions: Default::default(),
        default_version: Default::default(),
        edges: Vec::new(),
        default_requirements: Default::default(),
        owners: Default::default(),
        users: Default::default(),
        teams: Default::default(),
        categories: Default::default(),
        keywords: Default::default(),
        deleted: Vec::new(),
        downloads: Default::default(),
        warnings: Default::default(),
    };
    for spec in specs {
        snapshot.crates.insert(
            spec.id,
            CrateRow {
                name: spec.name.to_owned(),
                created_at: at("2025-01-01T00:00:00Z"),
                updated_at: at("2026-09-01T00:00:00Z"),
                description: spec.description.map(str::to_owned),
                homepage: None,
                repository: Some(spec.repository.to_owned()),
                deprecation: None,
                has_readme: true,
            },
        );
        snapshot.versions.insert(
            spec.id,
            VersionRow {
                crate_id: spec.id,
                num: "1.0.0".to_owned(),
                created_at: at("2025-02-01T12:00:00Z"),
                yanked: false,
                rust_version: None,
                edition: None,
                published_by: None,
                has_lib: spec.has_lib,
                has_bin: spec.has_bin,
            },
        );
        snapshot.default_version.insert(spec.id, spec.id);
        snapshot.keywords.insert(spec.id, vec!["tensor".to_owned()]);
        let owner = Owner {
            id: 1,
            kind: OwnerKind::User,
        };
        snapshot.owners.insert(spec.id, vec![owner]);
    }
    snapshot
}

/// The GraphQL answer for an existing repo.
fn found(name: &str, language: &str, description: Option<&str>, topics: &[&str]) -> String {
    let nodes: Vec<_> = topics
        .iter()
        .map(|topic| json!({"topic": {"name": topic}}))
        .collect();
    json!({"data": {"repository": {
        "nameWithOwner": name,
        "isArchived": false,
        "isFork": false,
        "description": description,
        "homepageUrl": null,
        "repositoryTopics": {"nodes": nodes},
        "primaryLanguage": {"name": language},
        "licenseInfo": {"spdxId": "MIT"},
    }}})
    .to_string()
}

/// A Rust repo whose description maps to the `ml` topic.
fn rust_repo(name: &str) -> String {
    found(name, "Rust", Some("Trains a neural network."), &[])
}

fn fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("valid")
}

/// Holds nothing back and counts the waits.
#[derive(Default)]
struct Counting {
    waits: u32,
}

impl Pacer for Counting {
    fn wait(&mut self, _response_headers: &Headers) {
        self.waits += 1;
    }
}

#[derive(Default)]
struct Waits {
    github: Counting,
    homebrew: Counting,
    docker: Counting,
    repology: Counting,
}

fn run_counting(
    data: &Path,
    list: &str,
    script: &Script,
    specs: &[Spec],
    waits: &mut Waits,
) -> Result<LeadSeed, SeedError> {
    let catalogue = load_catalogue(data).expect("reads").expect("valid");
    let topics: Topics = toml::from_str(TOPICS).expect("topics parse");
    let existing = ExistingIndex::from_catalogue(&catalogue);
    let dump = snapshot(specs);
    let inputs = LeadInputs {
        snapshot: &dump,
        topics: &topics,
        existing: &existing,
        min_active_users: 2.0,
    };
    let leads = parse_leads(Path::new("leads.txt"), list).expect("the list parses");
    let mut pacers = LeadPacers {
        github: &mut waits.github,
        homebrew: &mut waits.homebrew,
        docker: &mut waits.docker,
        repology: &mut waits.repology,
    };
    build_lead_seed(&inputs, &leads, script, TOKEN, &mut pacers)
}

fn try_run(list: &str, script: &Script, specs: &[Spec]) -> Result<LeadSeed, SeedError> {
    run_counting(&fixture(), list, script, specs, &mut Waits::default())
}

fn run(list: &str, script: &Script, specs: &[Spec]) -> LeadSeed {
    try_run(list, script, specs).expect("the run ends")
}

fn only_entry(seed: &LeadSeed) -> &Entry {
    let [(_, entry)] = seed.drafts.entries.as_slice() else {
        panic!("one draft expected:\n{}", seed.text);
    };
    entry
}

fn skip_reason<'a>(seed: &'a LeadSeed, name: &str) -> &'a str {
    seed.drafts
        .skipped
        .iter()
        .find(|(skipped, _)| skipped == name)
        .map_or_else(
            || panic!("`{name}` is not skipped:\n{}", seed.text),
            |(_, reason)| reason.as_str(),
        )
}

fn crates(entry: &Entry) -> Vec<&str> {
    entry.crates.iter().map(CrateName::as_str).collect()
}

/// A script that knows `acme/tool` on every channel.
fn tool_on_every_channel() -> Script {
    Script::default()
        .repo("acme/tool", 200, &rust_repo("acme/tool"))
        .url(
            "formulae.brew.sh",
            200,
            r#"{"urls":{"head":{"url":"https://github.com/acme/tool.git"}}}"#,
        )
        .url("repology.org", 200, r#"[{"repo":"debian_12"}]"#)
        .url("hub.docker.com", 200, r#"{"pull_count":7}"#)
}

#[test]
fn an_app_without_a_crate_drafts_and_passes_validation() {
    let script = tool_on_every_channel();
    let seed = run("acme/tool\n", &script, &[]);
    let entry = only_entry(&seed);
    assert_eq!(entry.kind, EntryKind::App);
    assert_eq!(entry.name, "tool");
    assert_eq!(entry.repo.as_str(), "https://github.com/acme/tool");
    assert!(entry.crates.is_empty());
    assert_eq!(entry.description.text, "Trains a neural network.");
    assert_eq!(entry.description.source, DescriptionSource::Repo);
    assert_eq!(entry.description.source_url.as_ref(), Some(&entry.repo));
    assert_eq!(entry.added.to_string(), "2026-11-02");
    let channels = entry.distribution.as_ref().expect("channels");
    assert_eq!(channels.homebrew.as_deref(), Some("tool"));
    assert_eq!(channels.repology.as_deref(), Some("tool"));
    assert_eq!(channels.docker, ["acme/tool"]);
    assert!(
        seed.text.contains(
            "apps: 1\ntool\t0.00\tml\tlead:acme/tool\tbasis:no-crate\tbelow-floor\tlicense:MIT\t\
             homebrew:tool\trepology:tool:name-only\tdocker:acme/tool\n"
        ),
        "{}",
        seed.text
    );
    assert_eq!(
        script.requests().first().map(String::as_str),
        Some("POST acme/tool")
    );
    assert_eq!(script.requests().len(), 4, "{:?}", script.requests());
    assert!(!script.requests().iter().any(|r| r.contains("crates.io")));

    let data = data_copy("seed-leads-app");
    let paths = write_drafts(&data, &seed.drafts).expect("writes");
    assert_eq!(paths, [data.join("entries").join("tool.toml")]);
    let text = fs::read_to_string(&paths[0]).expect("reads the draft");
    assert!(text.contains("source = \"repo\""), "{text}");
    match validate(&data).expect("reads") {
        Outcome::Valid(summary) => {
            assert_eq!(summary.to_string(), "ok: 4 entries, 7 judgments, 2 topics");
        }
        Outcome::Invalid(diagnostics) => panic!("the draft is invalid:\n{diagnostics}"),
    }
}

#[test]
fn a_channel_that_does_not_match_stays_empty() {
    let script = Script::default()
        .repo("acme/tool", 200, &rust_repo("acme/tool"))
        .url(
            "formulae.brew.sh",
            200,
            r#"{"homepage":"https://other.example","urls":{"head":{"url":"https://github.com/other/tool.git"}}}"#,
        )
        .url("repology.org", 200, "[]");
    let seed = run("acme/tool\n", &script, &[]);
    let entry = only_entry(&seed);
    assert_eq!(entry.distribution, None);
    assert!(
        seed.text.contains("homebrew-unmatched:tool"),
        "{}",
        seed.text
    );
    assert!(seed.warnings.is_empty(), "{:?}", seed.warnings);
}

#[test]
fn a_failed_channel_lookup_stays_empty_and_warns() {
    let script = Script::default()
        .repo("acme/tool", 200, &rust_repo("acme/tool"))
        .url("formulae.brew.sh", 500, "")
        .url("hub.docker.com", 200, r#"{"pull_count":7}"#);
    let seed = run("acme/tool\n", &script, &[]);
    let channels = only_entry(&seed).distribution.as_ref().expect("docker");
    assert_eq!(channels.homebrew, None);
    assert_eq!(channels.docker, ["acme/tool"]);
    assert!(
        seed.text.contains("homebrew:lookup-failed"),
        "{}",
        seed.text
    );
    let [warning] = seed.warnings.as_slice() else {
        panic!("one warning expected: {:?}", seed.warnings);
    };
    assert!(warning.contains("acme/tool"), "{warning}");
    assert!(warning.contains("status 500"), "{warning}");
}

#[test]
fn a_library_family_drafts_without_a_floor_and_says_so() {
    const LINK: &str = "https://github.com/acme/tool";
    let script = Script::default().repo("acme/tool", 200, &rust_repo("acme/tool"));
    let seed = run(
        "acme/tool\n",
        &script,
        &[library(1, "tool-core", LINK), binary(2, "tool-cli", LINK)],
    );
    let entry = only_entry(&seed);
    assert_eq!(entry.kind, EntryKind::Library);
    assert_eq!(crates(entry), ["tool-core", "tool-cli"]);
    assert_eq!(entry.description.source, DescriptionSource::Crate);
    assert_eq!(entry.distribution, None);
    assert!(
        seed.text.starts_with(
            "drafts: 1. A lead bypasses the discovery rules and the active-user floor."
        ),
        "{}",
        seed.text
    );
    assert!(
        seed.text.contains(
            "libraries: 1\ntool\t0.00\ttensors\tlead:acme/tool\tbasis:has_lib\tbelow-floor\tlicense:MIT\n"
        ),
        "{}",
        seed.text
    );
    assert!(
        seed.text
            .contains("skipped: 0. A lead bypasses the discovery rules"),
        "{}",
        seed.text
    );
    assert_eq!(
        script.requests(),
        ["POST acme/tool"],
        "a library asks no channel"
    );
}

#[test]
fn a_family_of_binaries_makes_an_app_that_lists_them() {
    const LINK: &str = "https://github.com/acme/tool";
    let script = tool_on_every_channel();
    let seed = run("acme/tool\n", &script, &[binary(1, "tool", LINK)]);
    let entry = only_entry(&seed);
    assert_eq!(entry.kind, EntryKind::App);
    assert_eq!(crates(entry), ["tool"]);
    assert_eq!(entry.description.source, DescriptionSource::Repo);
    assert!(
        seed.text.contains("lead:acme/tool\tbasis:has_lib"),
        "{}",
        seed.text
    );
}

#[test]
fn an_unmarked_crate_gives_a_labelled_guess() {
    const LINK: &str = "https://github.com/acme/tool";
    let script = Script::default().repo("acme/tool", 200, &rust_repo("acme/tool"));
    let unmarked = Spec {
        has_lib: None,
        ..library(1, "tool-cli", LINK)
    };
    let seed = run("acme/tool\n", &script, &[unmarked]);
    assert_eq!(only_entry(&seed).kind, EntryKind::App);
    assert!(seed.text.contains("basis:name-guess"), "{}", seed.text);
}

#[test]
fn a_redirect_uses_the_canonical_repository() {
    let script = Script::default().repo("old-owner/thing", 200, &rust_repo("New-Owner/thing"));
    let seed = run(
        "old-owner/thing\n",
        &script,
        &[library(1, "thing", "https://github.com/old-owner/thing")],
    );
    let entry = only_entry(&seed);
    assert_eq!(entry.kind, EntryKind::Library);
    assert_eq!(entry.repo.as_str(), "https://github.com/new-owner/thing");
    assert!(seed.text.contains("lead:old-owner/thing"), "{}", seed.text);
}

#[test]
fn a_redirect_finds_the_crates_of_the_canonical_repository() {
    let script = Script::default().repo("old-owner/thing", 200, &rust_repo("new-owner/thing"));
    let seed = run(
        "old-owner/thing\n",
        &script,
        &[library(1, "thing", "https://github.com/new-owner/thing")],
    );
    assert_eq!(crates(only_entry(&seed)), ["thing"]);
}

#[test]
fn two_leads_of_one_repository_make_one_draft() {
    let answer = rust_repo("new-owner/thing");
    let script = Script::default()
        .repo("old-owner/thing", 200, &answer)
        .repo("new-owner/thing", 200, &answer);
    let seed = run("old-owner/thing\nnew-owner/thing\n", &script, &[]);
    assert_eq!(only_entry(&seed).name, "thing");
    assert_eq!(
        skip_reason(&seed, "new-owner/thing"),
        "resolves to `new-owner/thing`, the same repository as lead `old-owner/thing`"
    );
}

#[test]
fn a_repeated_line_is_fetched_once() {
    let script = Script::default().repo("acme/tool", 200, &rust_repo("acme/tool"));
    let seed = run("acme/tool\nAcme/Tool\n", &script, &[]);
    assert_eq!(seed.drafts.entries.len(), 1);
    assert!(
        skip_reason(&seed, "Acme/Tool").contains("repeats"),
        "{}",
        seed.text
    );
    let posts = script.requests();
    assert_eq!(posts.iter().filter(|r| r.starts_with("POST")).count(), 1);
}

#[test]
fn a_gone_repository_is_skipped_with_a_reason() {
    let script = Script::default();
    let seed = run("acme/gone\n", &script, &[]);
    assert!(seed.drafts.entries.is_empty());
    assert_eq!(
        skip_reason(&seed, "acme/gone"),
        "GitHub holds no repository of this name"
    );
}

#[test]
fn a_fork_is_skipped_with_a_reason() {
    let body = rust_repo("acme/copy").replace(r#""isFork":false"#, r#""isFork":true"#);
    let script = Script::default().repo("acme/copy", 200, &body);
    let seed = run("acme/copy\n", &script, &[]);
    assert_eq!(skip_reason(&seed, "acme/copy"), "the repository is a fork");
}

#[test]
fn a_repo_in_another_language_needs_a_rust_named_crate_family() {
    let answer = found("acme/engine", "C++", Some("A tensor engine."), &[]);
    let script = Script::default().repo("acme/engine", 200, &answer);
    let seed = run("acme/engine\n", &script, &[]);
    let reason = skip_reason(&seed, "acme/engine");
    assert!(
        reason.contains("`C++`") && reason.contains("Rust-named"),
        "{reason}"
    );

    let script = Script::default().repo("acme/engine", 200, &answer);
    let family = [library(1, "engine-sys", "https://github.com/acme/engine")];
    let seed = run("acme/engine\n", &script, &family);
    assert_eq!(crates(only_entry(&seed)), ["engine-sys"]);
    assert!(seed.text.contains("language:C++"), "{}", seed.text);
}

#[test]
fn an_entry_on_the_repository_is_reported_as_listed() {
    let script = Script::default().repo("ml-rust/alpha", 200, &rust_repo("ml-rust/alpha"));
    let seed = run("ml-rust/alpha\n", &script, &[]);
    assert!(seed.drafts.entries.is_empty());
    assert!(
        seed.text
            .contains("listed: 1\nml-rust/alpha\tentry `alpha` holds the repository\n"),
        "{}",
        seed.text
    );
}

#[test]
fn a_redirect_to_an_entry_is_reported_as_listed() {
    let script = Script::default().repo("old-owner/alpha", 200, &rust_repo("ml-rust/alpha"));
    let seed = run("old-owner/alpha\n", &script, &[]);
    assert!(
        seed.text.contains("listed: 1\nold-owner/alpha\t"),
        "{}",
        seed.text
    );
}

#[test]
fn a_family_crate_in_an_entry_is_reported_as_listed() {
    let script = Script::default().repo("acme/beta", 200, &rust_repo("acme/beta"));
    let family = [library(1, "beta-crate", "https://github.com/acme/beta")];
    let seed = run("acme/beta\n", &script, &family);
    assert!(
        seed.text
            .contains("acme/beta\tcrate `beta-crate` is in an existing entry"),
        "{}",
        seed.text
    );
}

#[test]
fn a_repository_with_no_topic_is_skipped() {
    let answer = found(
        "acme/threads",
        "Rust",
        Some("A data parallelism library."),
        &["parallelism", "cloud"],
    );
    let script = Script::default().repo("acme/threads", 200, &answer);
    let seed = run("acme/threads\n", &script, &[]);
    assert_eq!(skip_reason(&seed, "acme/threads"), "no topic matches");
    assert_eq!(script.requests().len(), 1, "a skip asks no channel");
}

#[test]
fn a_github_topic_maps_a_repository() {
    let answer = found("acme/tool", "Rust", Some("A tool."), &["tensor"]);
    let script = Script::default().repo("acme/tool", 200, &answer);
    let seed = run("acme/tool\n", &script, &[]);
    assert!(seed.text.contains("tool\t0.00\ttensors\t"), "{}", seed.text);
}

#[test]
fn a_repository_without_a_description_is_skipped() {
    let answer = found("acme/tool", "Rust", None, &["tensor"]);
    let script = Script::default().repo("acme/tool", 200, &answer);
    let seed = run("acme/tool\n", &script, &[]);
    assert_eq!(skip_reason(&seed, "acme/tool"), "no description");
}

#[test]
fn a_rejected_token_stops_the_run_with_the_fatal_error() {
    let script = Script::everything(401);
    let error = try_run("acme/tool\nacme/other\n", &script, &[]).expect_err("rejected");
    assert!(matches!(error, SeedError::Github(_)), "{error}");
    assert_eq!(error.exit_code(), 1);
    let message = error.to_string();
    assert!(message.contains("status 401"), "{message}");
    assert!(
        message.contains("set GITHUB_TOKEN to a valid token"),
        "{message}"
    );
    assert!(!message.contains(TOKEN), "{message}");
    assert_eq!(script.requests().len(), 1, "no later request");
}

#[test]
fn a_failed_fetch_for_one_lead_does_not_stop_the_others() {
    let script = Script::default()
        .repo("acme/broken", 500, "")
        .repo(
            "acme/rejected",
            200,
            r#"{"data":null,"errors":[{"message":"forbidden"}]}"#,
        )
        .repo("acme/tool", 200, &rust_repo("acme/tool"));
    let seed = run("acme/broken\nacme/rejected\nacme/tool\n", &script, &[]);
    assert_eq!(only_entry(&seed).name, "tool");
    let broken = skip_reason(&seed, "acme/broken");
    assert!(
        broken.starts_with("fetch failed: ") && broken.contains("status 500"),
        "{broken}"
    );
    let rejected = skip_reason(&seed, "acme/rejected");
    assert!(rejected.contains("forbidden"), "{rejected}");
}

#[test]
fn one_pacer_holds_back_each_api() {
    let script = Script::default()
        .repo("acme/one", 200, &rust_repo("acme/one"))
        .repo("acme/two", 200, &rust_repo("acme/two"));
    let mut waits = Waits::default();
    let seed = run_counting(&fixture(), "acme/one\nacme/two\n", &script, &[], &mut waits)
        .expect("the run ends");
    assert_eq!(seed.drafts.entries.len(), 2);
    assert_eq!(
        (
            waits.github.waits,
            waits.homebrew.waits,
            waits.docker.waits,
            waits.repology.waits
        ),
        (1, 1, 1, 1)
    );
}

#[test]
fn two_drafts_with_one_slug_keep_the_first() {
    let script = Script::default()
        .repo("one/tool", 200, &rust_repo("one/tool"))
        .repo("two/tool", 200, &rust_repo("two/tool"));
    let seed = run("one/tool\ntwo/tool\n", &script, &[]);
    assert_eq!(
        only_entry(&seed).repo.as_str(),
        "https://github.com/one/tool"
    );
    assert_eq!(
        skip_reason(&seed, "two/tool"),
        "slug `tool` is already drafted for `one/tool`"
    );
}

/// A fresh copy of the `valid` fixture under the build directory.
fn data_copy(name: &str) -> PathBuf {
    let target = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    if target.exists() {
        fs::remove_dir_all(&target).expect("clears the old copy");
    }
    copy_dir(&fixture(), &target);
    target
}

fn copy_dir(from: &Path, to: &Path) {
    fs::create_dir_all(to).expect("creates the directory");
    for item in fs::read_dir(from).expect("reads the directory") {
        let item = item.expect("reads the item");
        let path = to.join(item.file_name());
        if item.path().is_dir() {
            copy_dir(&item.path(), &path);
        } else {
            fs::copy(item.path(), path).expect("copies the file");
        }
    }
}
