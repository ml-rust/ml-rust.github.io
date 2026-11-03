#[path = "common/cache.rs"]
mod cache;

use std::fs;

use std::path::Path;

use cache::cache;
use catalogue_core::Date;
use catalogue_signals::link::{CrateRelease, LinkCheck, cached_check, links_dir, store_check};

const SHA: &str = "a866b336f14aa57a07f0d0be9f8762746e64ecb4";

fn release(name: &str, version: &str) -> CrateRelease {
    CrateRelease {
        name: name.to_owned(),
        version: version.to_owned(),
    }
}

fn checked() -> Date {
    Date::new(2026, 11, 2).expect("date")
}

/// The cached result for `release` and `repo`, without the check date.
fn hit(dir: &Path, release: &CrateRelease, repo: &str) -> Option<LinkCheck> {
    cached_check(dir, release, repo)
        .expect("reads")
        .map(|outcome| outcome.check)
}

fn verified() -> LinkCheck {
    LinkCheck::Verified {
        sha1: SHA.to_owned(),
        dirty: true,
    }
}

#[test]
fn a_stored_result_is_read_back_for_the_same_repo() {
    let (_guard, dir) = cache();
    let serde = release("serde", "1.0.228");
    assert_eq!(hit(&dir, &serde, "serde-rs/serde"), None);
    store_check(&dir, &serde, "serde-rs/serde", &verified(), checked()).expect("stores");
    let found = cached_check(&dir, &serde, "serde-rs/serde")
        .expect("reads")
        .expect("hit");
    assert_eq!(found.check, verified());
    assert!(found.cached);
    assert_eq!(found.checked, checked());
}

#[test]
fn the_file_holds_the_repo_and_the_result_under_the_canonical_name() {
    let (_guard, dir) = cache();
    store_check(
        &dir,
        &release("Serde_Json", "1.0.1"),
        "Serde-RS/Json",
        &LinkCheck::NoVcsInfo,
        checked(),
    )
    .expect("stores");
    let text = fs::read_to_string(links_dir(&dir).join("serde-json-1.0.1@serde-rs__json.json"))
        .expect("file");
    let value: serde_json::Value = serde_json::from_str(&text).expect("json");
    assert_eq!(value["rule"], 2);
    assert_eq!(value["repo"], "serde-rs/json");
    assert_eq!(value["result"]["kind"], "no-vcs-info");
    assert_eq!(value["checked"], "2026-11-02");
}

#[test]
fn another_repo_another_version_or_another_crate_misses() {
    let (_guard, dir) = cache();
    let serde = release("serde", "1.0.228");
    store_check(&dir, &serde, "serde-rs/serde", &verified(), checked()).expect("stores");
    assert_eq!(hit(&dir, &serde, "fork/serde"), None);
    let newer = release("serde", "1.0.229");
    assert_eq!(hit(&dir, &newer, "serde-rs/serde"), None);
    let other = release("serde_json", "1.0.228");
    assert_eq!(hit(&dir, &other, "serde-rs/serde"), None);
}

#[test]
fn the_repo_compares_without_regard_to_case() {
    let (_guard, dir) = cache();
    let serde = release("serde", "1.0.228");
    store_check(&dir, &serde, "serde-rs/serde", &verified(), checked()).expect("stores");
    assert_eq!(hit(&dir, &serde, "Serde-RS/Serde"), Some(verified()));
}

#[test]
fn a_new_result_replaces_the_old_one_per_repo_and_leaves_no_temporary_file() {
    let (_guard, dir) = cache();
    let serde = release("serde", "1.0.228");
    store_check(&dir, &serde, "a/b", &verified(), checked()).expect("first");
    store_check(&dir, &serde, "a/b", &LinkCheck::NoVcsInfo, checked()).expect("replaces");
    store_check(&dir, &serde, "c/d", &verified(), checked()).expect("other repo");
    assert_eq!(hit(&dir, &serde, "a/b"), Some(LinkCheck::NoVcsInfo));
    assert_eq!(hit(&dir, &serde, "c/d"), Some(verified()));
    let mut files: Vec<String> = fs::read_dir(links_dir(&dir))
        .expect("dir")
        .map(|entry| {
            entry
                .expect("entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    files.sort();
    assert_eq!(
        files,
        ["serde-1.0.228@a__b.json", "serde-1.0.228@c__d.json"]
    );
}

#[test]
fn a_file_that_is_not_a_record_misses() {
    let (_guard, dir) = cache();
    let serde = release("serde", "1.0.228");
    fs::create_dir_all(links_dir(&dir)).expect("dir");
    fs::write(
        links_dir(&dir).join("serde-1.0.228@a__b.json"),
        r#"{"repo":"a/b"}"#,
    )
    .expect("file");
    assert_eq!(hit(&dir, &serde, "a/b"), None);
}

#[test]
fn a_file_without_a_rule_an_older_rule_or_a_check_date_misses() {
    let (_guard, dir) = cache();
    let serde = release("serde", "1.0.228");
    fs::create_dir_all(links_dir(&dir)).expect("dir");
    let path = links_dir(&dir).join("serde-1.0.228@a__b.json");
    let result = format!(r#""result":{{"kind":"verified","sha1":"{SHA}","dirty":false}}"#);
    let dated = r#""checked":"2026-11-02""#;
    for record in [
        format!(r#"{{"repo":"a/b",{result},{dated}}}"#),
        format!(r#"{{"rule":1,"repo":"a/b",{result},{dated}}}"#),
        format!(r#"{{"rule":2,"repo":"a/b",{result}}}"#),
    ] {
        fs::write(&path, record).expect("file");
        assert_eq!(hit(&dir, &serde, "a/b"), None);
    }
    fs::write(
        &path,
        format!(r#"{{"rule":2,"repo":"a/b",{result},{dated}}}"#),
    )
    .expect("file");
    let found = cached_check(&dir, &serde, "a/b")
        .expect("reads")
        .expect("hit");
    assert_eq!(found.checked, checked());
}
