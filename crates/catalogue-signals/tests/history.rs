#[path = "common/cache.rs"]
mod cache;

use std::collections::BTreeMap;
use std::fs;

use cache::cache;
use catalogue_core::{Date, EntryKind, Tier};
use catalogue_signals::history::{
    Dated, HistoryError, NetworkValues, RepoValues, Row, Snapshot, read_all, write,
};
use catalogue_signals::link::LinkCheck;
use flate2::Compression;
use flate2::write::GzEncoder;
use std::io::Write;

fn date(year: i16, month: i8, day: i8) -> Date {
    Date::new(year, month, day).expect("date")
}

fn row(tier: Tier, docker_pulls: Option<u64>) -> Row {
    Row {
        kind: EntryKind::App,
        tier,
        dead_now: tier == Tier::Archived,
        dormant: Some(tier == Tier::Archived),
        active_users: Some(2.5),
        docker_pulls,
        network: None,
    }
}

fn snapshot(on: Date, rows: &[(&str, Row)]) -> Snapshot {
    Snapshot {
        date: on,
        rows: rows
            .iter()
            .map(|(slug, row)| ((*slug).to_owned(), row.clone()))
            .collect::<BTreeMap<_, _>>(),
    }
}

fn gzip(text: &str) -> Vec<u8> {
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(text.as_bytes()).expect("encode");
    encoder.finish().expect("finish")
}

fn lines(path: &std::path::Path) -> Vec<String> {
    use std::io::Read;
    let bytes = fs::read(path).expect("read file");
    let mut text = String::new();
    flate2::read::GzDecoder::new(bytes.as_slice())
        .read_to_string(&mut text)
        .expect("gunzip");
    text.lines().map(str::to_owned).collect()
}

#[test]
fn a_snapshot_round_trips_through_its_dated_file() {
    let (_guard, dir) = cache();
    let on = date(2026, 11, 3);
    let written = snapshot(
        on,
        &[
            ("zeta", row(Tier::Featured, Some(10))),
            ("alpha", row(Tier::Archived, None)),
        ],
    );
    let path = write(&dir, &written).expect("write");
    assert_eq!(path, dir.join("2026-11-03.jsonl.gz"));
    assert_eq!(
        lines(&path),
        [
            r#"{"slug":"alpha","kind":"app","tier":"archived","dead_now":true,"dormant":true,"active_users":2.5,"docker_pulls":null}"#,
            r#"{"slug":"zeta","kind":"app","tier":"featured","dead_now":false,"dormant":false,"active_users":2.5,"docker_pulls":10}"#,
        ]
    );
    let history = read_all(&dir).expect("read");
    let later = date(2026, 11, 4);
    assert_eq!(history.previous_tier("alpha", later), Some(Tier::Archived));
    assert_eq!(history.previous_tier("zeta", later), Some(Tier::Featured));
    assert_eq!(
        history,
        catalogue_signals::history::History::new(vec![written])
    );
}

#[test]
fn writing_a_date_twice_replaces_the_file_and_leaves_no_temporary_file() {
    let (_guard, dir) = cache();
    let on = date(2026, 11, 3);
    write(&dir, &snapshot(on, &[("a", row(Tier::Listed, None))])).expect("first");
    let path = write(&dir, &snapshot(on, &[("b", row(Tier::New, None))])).expect("second");
    let later = date(2026, 11, 4);
    let history = read_all(&dir).expect("read");
    assert_eq!(history.previous_tier("a", later), None);
    assert_eq!(history.previous_tier("b", later), Some(Tier::New));
    let names: Vec<String> = fs::read_dir(&dir)
        .expect("list")
        .map(|item| {
            item.expect("item")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    assert_eq!(names, ["2026-11-03.jsonl.gz"]);
    assert!(path.is_file());
}

#[test]
fn writing_creates_a_missing_directory_and_a_missing_directory_reads_empty() {
    let (_guard, dir) = cache();
    let nested = dir.join("a").join("b");
    assert!(read_all(&nested).expect("absent reads empty").is_empty());
    write(&nested, &snapshot(date(2026, 1, 2), &[])).expect("write");
    assert!(!read_all(&nested).expect("read").is_empty());
}

#[test]
fn files_whose_stem_is_not_a_date_are_ignored() {
    let (_guard, dir) = cache();
    for name in [
        "notes.jsonl.gz",
        "2026-13-01.jsonl.gz",
        "2026-1-1.jsonl.gz",
        "20261103.jsonl.gz",
        "2026-11-03.jsonl",
        "2026-11-03.jsonl.gz.tmp",
        ".2026-11-03.jsonl.gz.tmp",
        "README.md",
    ] {
        fs::write(dir.join(name), b"not gzip").expect("write junk");
    }
    fs::create_dir(dir.join("2026-11-05.jsonl.gz")).expect("directory");
    write(
        &dir,
        &snapshot(date(2026, 11, 3), &[("a", row(Tier::Listed, None))]),
    )
    .expect("write");
    let history = read_all(&dir).expect("junk is ignored");
    assert_eq!(
        history.previous_tier("a", date(2026, 11, 4)),
        Some(Tier::Listed)
    );
}

#[test]
fn a_corrupt_file_is_an_error_that_names_its_path() {
    let (_guard, dir) = cache();
    let path = dir.join("2026-11-03.jsonl.gz");
    fs::write(&path, b"not gzip").expect("write");
    let error = read_all(&dir).expect_err("corrupt");
    assert!(
        matches!(&error, HistoryError::Corrupt { line: None, .. }),
        "{error}"
    );
    let message = error.to_string();
    assert!(message.contains(&path.display().to_string()), "{message}");
    assert!(message.contains("Restore the file from git"), "{message}");
}

#[test]
fn a_line_without_dormant_reads_as_unknown() {
    let (_guard, dir) = cache();
    let old = r#"{"slug":"a","kind":"app","tier":"listed","dead_now":false,"active_users":null,"docker_pulls":null}"#;
    fs::write(dir.join("2026-11-01.jsonl.gz"), gzip(&format!("{old}\n"))).expect("write");
    let history = read_all(&dir).expect("read");
    let rows = &history.snapshots()[0].rows;
    assert_eq!(rows.get("a").map(|row| row.dormant), Some(None));
}

#[test]
fn a_bad_line_names_its_number() {
    let (_guard, dir) = cache();
    let good = r#"{"slug":"a","kind":"app","tier":"listed","dead_now":false,"active_users":null,"docker_pulls":null}"#;
    for (name, body) in [
        ("2026-11-01", format!("{good}\n{{\"slug\":\"b\"}}\n")),
        ("2026-11-02", format!("{good}\n{good}\n")),
        (
            "2026-11-03",
            format!("{}\n", good.replace("\"a\"", "\"Bad Slug\"")),
        ),
        (
            "2026-11-04",
            format!("{}\n", good.replace("listed", "gold")),
        ),
    ] {
        let path = dir.join(format!("{name}.jsonl.gz"));
        fs::write(&path, gzip(&body)).expect("write");
        let error = read_all(&dir).expect_err("corrupt");
        let HistoryError::Corrupt {
            line, path: named, ..
        } = &error
        else {
            panic!("expected corrupt: {error}");
        };
        assert_eq!(*named, path, "{name}");
        assert!(line.is_some(), "{name}: {error}");
        fs::remove_file(&path).expect("remove");
    }
}

fn with_network() -> Row {
    let on = date(2026, 10, 30);
    Row {
        network: Some(NetworkValues {
            facts: Some(Dated {
                date: on,
                of: "ml-rust/alpha".to_owned(),
                value: RepoValues::Found {
                    archived: false,
                    stars: 12,
                    last_human_commit: Some(date(2026, 9, 1)),
                    oldest_unanswered_days: None,
                    maintainer_replies_365: 7,
                },
            }),
            link: Some(Dated {
                date: on,
                of: "alpha 1.0.0 in ml-rust/alpha".to_owned(),
                value: LinkCheck::NoVcsInfo,
            }),
            docker: Some(Dated {
                date: on,
                of: "ml-rust/alpha".to_owned(),
                value: 5_000,
            }),
            ..NetworkValues::default()
        }),
        ..row(Tier::Listed, None)
    }
}

#[test]
fn network_values_round_trip_through_the_file() {
    let (_guard, dir) = cache();
    let written = snapshot(date(2026, 11, 3), &[("alpha", with_network())]);
    write(&dir, &written).expect("write");
    assert_eq!(
        read_all(&dir).expect("read"),
        catalogue_signals::history::History::new(vec![written])
    );
}

#[test]
fn a_line_without_network_values_reads_as_none_and_writes_back_the_same() {
    let (_guard, dir) = cache();
    let line = r#"{"slug":"alpha","kind":"app","tier":"listed","dead_now":false,"dormant":null,"active_users":null,"docker_pulls":null}"#;
    fs::write(dir.join("2026-11-02.jsonl.gz"), gzip(&format!("{line}\n"))).expect("write");
    let history = read_all(&dir).expect("read");
    let [old] = history.snapshots() else {
        panic!("expected one snapshot");
    };
    assert_eq!(
        old.rows.get("alpha").map(|row| row.network.is_none()),
        Some(true)
    );
    let path = write(&dir, old).expect("write");
    assert_eq!(lines(&path), [line]);
}

#[test]
fn the_same_rows_give_the_same_bytes() {
    let (_guard, first) = cache();
    let (_other, second) = cache();
    let rows = [
        ("zeta", with_network()),
        ("alpha", with_network()),
        ("mid", row(Tier::New, Some(3))),
    ];
    let reversed: Vec<(&str, Row)> = rows.iter().rev().cloned().collect();
    let on = date(2026, 11, 3);
    let a = write(&first, &snapshot(on, &rows)).expect("write");
    let b = write(&second, &snapshot(on, &reversed)).expect("write");
    assert_eq!(fs::read(a).expect("read"), fs::read(b).expect("read"));
}
