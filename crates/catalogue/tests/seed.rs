use std::fs;
use std::path::{Path, PathBuf};

use catalogue::{Outcome, SeedError, build_seed, load_catalogue, validate, write_drafts};
use catalogue_core::{Categories, Topics};
use catalogue_signals::discover::Rules;
use catalogue_signals::dump::{CrateRow, DumpMeta, DumpSnapshot, VersionRow};
use catalogue_signals::seed::{ExistingIndex, Mappings};
use jiff::Timestamp;

/// Path of the `valid` fixture data directory.
fn fixture(case: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(case)
}

fn at(text: &str) -> Timestamp {
    text.parse().expect("timestamp")
}

/// Two crates tagged `machine-learning`: `gizmo-ml` has a GitHub repository, `nogit-ml` has none.
fn snapshot() -> DumpSnapshot {
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
        details: Default::default(),
        owners: Default::default(),
        users: Default::default(),
        teams: Default::default(),
        categories: Default::default(),
        keywords: Default::default(),
        deleted: Vec::new(),
        downloads: Default::default(),
        warnings: Default::default(),
    };
    for (id, name, repository) in [
        (1, "gizmo-ml", Some("https://github.com/someone/gizmo-ml")),
        (2, "nogit-ml", None),
    ] {
        snapshot.crates.insert(
            id,
            CrateRow {
                name: name.to_owned(),
                created_at: at("2025-01-01T00:00:00Z"),
                updated_at: at("2026-09-01T00:00:00Z"),
                description: Some("Learns\n  from data.".to_owned()),
                homepage: Some("https://gizmo.example.com".to_owned()),
                repository: repository.map(str::to_owned),
                deprecation: None,
                has_readme: true,
            },
        );
        snapshot.versions.insert(
            id,
            VersionRow {
                crate_id: id,
                num: "1.0.0".to_owned(),
                created_at: at("2025-02-01T12:00:00Z"),
                yanked: false,
                rust_version: None,
                edition: None,
                published_by: None,
                has_lib: Some(true),
                has_bin: false,
            },
        );
        snapshot.default_version.insert(id, id);
        snapshot
            .keywords
            .insert(id, vec!["machine-learning".to_owned()]);
    }
    snapshot
}

fn rules() -> Rules {
    Rules {
        keywords: vec!["machine-learning".to_owned()],
        ..Rules::default()
    }
}

/// A fresh copy of the `valid` fixture under the build directory.
fn data_copy(name: &str) -> PathBuf {
    let target = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    if target.exists() {
        fs::remove_dir_all(&target).expect("clears the old copy");
    }
    copy_dir(&fixture("valid"), &target);
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

fn seed(data: &Path) -> catalogue::Seed {
    seed_snapshot(data, &snapshot())
}

fn seed_snapshot(data: &Path, snapshot: &DumpSnapshot) -> catalogue::Seed {
    let catalogue = load_catalogue(data).expect("reads").expect("valid");
    let topics = Topics {
        topic: catalogue.topics().to_vec(),
    };
    let categories = Categories {
        category: catalogue.categories().to_vec(),
    };
    let mappings = Mappings {
        topics: &topics,
        categories: &categories,
    };
    let existing = ExistingIndex::from_catalogue(&catalogue);
    build_seed(snapshot, &rules(), &mappings, &existing, 0.0)
}

#[test]
fn drafted_files_pass_validation() {
    let data = data_copy("seed-valid");
    let seeded = seed(&data);
    assert_eq!(
        seeded.text,
        "drafts: 1\ngizmo-ml\t0.00\tml\nskipped: 1\nnogit-ml\tno repository"
    );
    let paths = write_drafts(&data, &seeded.drafts).expect("writes");
    assert_eq!(paths, [data.join("entries").join("gizmo-ml.toml")]);
    let text = fs::read_to_string(&paths[0]).expect("reads the draft");
    assert!(text.contains("source = \"crate\""), "{text}");
    assert!(text.contains("added = 2026-11-02"), "{text}");
    match validate(&data).expect("reads") {
        Outcome::Valid(summary) => {
            assert_eq!(summary.to_string(), "ok: 4 entries, 7 judgments, 2 topics");
        }
        Outcome::Invalid(diagnostics) => panic!("drafts are invalid:\n{diagnostics}"),
    }
    let catalogue = load_catalogue(&data).expect("reads").expect("valid");
    let entry = catalogue
        .entries()
        .iter()
        .find(|(slug, _)| slug.as_str() == "gizmo-ml")
        .map(|(_, entry)| entry)
        .expect("the draft is listed");
    assert_eq!(entry.description.text, "Learns from data.");
    assert_eq!(entry.repo.as_str(), "https://github.com/someone/gizmo-ml");
    assert_eq!(entry.category, None);
}

#[test]
fn a_draft_with_a_matching_category_writes_it_and_passes_validation() {
    let data = data_copy("seed-category");
    let mut snap = snapshot();
    snap.keywords
        .insert(1, vec!["machine-learning".to_owned(), "tensor".to_owned()]);
    let seeded = seed_snapshot(&data, &snap);
    let paths = write_drafts(&data, &seeded.drafts).expect("writes");
    let text = fs::read_to_string(&paths[0]).expect("reads the draft");
    assert!(text.contains("category = \"tensor-libraries\""), "{text}");
    assert!(matches!(validate(&data).expect("reads"), Outcome::Valid(_)));
}

#[test]
fn a_listed_crate_is_not_drafted_again() {
    let data = data_copy("seed-again");
    let first = seed(&data);
    write_drafts(&data, &first.drafts).expect("writes");
    let second = seed(&data);
    assert_eq!(
        second.text,
        "drafts: 0\nskipped: 1\nnogit-ml\tno repository"
    );
}

#[test]
fn writing_refuses_to_overwrite_and_names_the_file() {
    let data = data_copy("seed-overwrite");
    let seeded = seed(&data);
    let path = data.join("entries").join("gizmo-ml.toml");
    fs::write(&path, "keep me").expect("plants a file");
    let error = write_drafts(&data, &seeded.drafts).expect_err("refuses");
    assert!(matches!(error, SeedError::Exists { .. }), "{error}");
    assert!(error.to_string().contains("gizmo-ml.toml"), "{error}");
    assert_eq!(fs::read_to_string(&path).expect("reads"), "keep me");
}

#[test]
fn nothing_to_write_creates_nothing() {
    let data = data_copy("seed-empty");
    fs::remove_dir_all(data.join("entries")).expect("removes the entries");
    let seeded = build_seed(
        &snapshot(),
        &Rules::default(),
        &Mappings {
            topics: &Topics::default(),
            categories: &Categories::default(),
        },
        &ExistingIndex::default(),
        0.0,
    );
    assert!(
        write_drafts(&data, &seeded.drafts)
            .expect("writes")
            .is_empty()
    );
    assert!(!data.join("entries").exists());
}
