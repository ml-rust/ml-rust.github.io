#[path = "common/cache.rs"]
mod cache;
#[path = "common/dump_responses.rs"]
mod dump_responses;
#[path = "common/transport.rs"]
mod transport;

use std::fs;
use std::path::Path;

use cache::cache;
use catalogue_signals::fetch::{
    DumpState, FetchError, FetchOutcome, cached, dump_dir, dump_with, rustsec_dir, rustsec_with,
};
use transport::{FakeResponse, FakeTransport};

const URL: &str = "https://example.test/db-dump.tar.gz";
const BODY: &[u8] = b"0123456789";

fn read_state(cache_dir: &Path) -> DumpState {
    let json = fs::read(dump_dir(cache_dir).join("state.json")).expect("state file");
    serde_json::from_slice(&json).expect("state parses")
}

fn write_state(cache_dir: &Path, state: &DumpState) {
    let json = serde_json::to_vec(state).expect("state serializes");
    fs::write(dump_dir(cache_dir).join("state.json"), json).expect("state written");
}

fn state(etag: &str, complete: bool) -> DumpState {
    DumpState {
        etag: Some(etag.to_owned()),
        last_modified: None,
        content_length: Some(BODY.len() as u64),
        fetched_at: "2026-11-03T02:10:00Z".to_owned(),
        complete,
    }
}

fn prepare(cache_dir: &Path, file: &str, contents: &[u8]) {
    fs::create_dir_all(dump_dir(cache_dir)).expect("dump dir");
    fs::write(dump_dir(cache_dir).join(file), contents).expect("file written");
}

#[test]
fn fresh_download_stores_file_and_state() {
    let (_guard, dir) = cache();
    let transport = FakeTransport::new([FakeResponse::ok(BODY, "\"v1\"")]);
    let outcome = dump_with(&transport, URL, &dir).expect("downloads");
    let path = dump_dir(&dir).join("db-dump.tar.gz");
    assert_eq!(
        outcome,
        FetchOutcome::Downloaded {
            path: path.clone(),
            bytes: 10
        }
    );
    assert_eq!(fs::read(&path).expect("file"), BODY);
    assert!(!dump_dir(&dir).join("db-dump.tar.gz.partial").exists());
    let saved = read_state(&dir);
    assert_eq!(saved.etag.as_deref(), Some("\"v1\""));
    assert_eq!(saved.content_length, Some(10));
    assert!(saved.complete);
    assert!(saved.fetched_at.ends_with('Z'));
    assert!(transport.requests()[0].is_empty());
}

#[test]
fn unchanged_dump_answers_304_to_if_none_match() {
    let (_guard, dir) = cache();
    prepare(&dir, "db-dump.tar.gz", BODY);
    write_state(&dir, &state("\"v1\"", true));
    let transport = FakeTransport::new([FakeResponse::status(304)]);
    let outcome = dump_with(&transport, URL, &dir).expect("revalidates");
    assert_eq!(
        outcome,
        FetchOutcome::Unchanged {
            path: dump_dir(&dir).join("db-dump.tar.gz")
        }
    );
    assert_eq!(
        transport.requests()[0],
        [("If-None-Match".to_owned(), "\"v1\"".to_owned())]
    );
    assert_eq!(
        fs::read(dump_dir(&dir).join("db-dump.tar.gz")).expect("file"),
        BODY
    );
}

#[test]
fn changed_dump_replaces_the_complete_file() {
    let (_guard, dir) = cache();
    prepare(&dir, "db-dump.tar.gz", b"old");
    write_state(&dir, &state("\"v1\"", true));
    let transport = FakeTransport::new([FakeResponse::ok(BODY, "\"v2\"")]);
    let outcome = dump_with(&transport, URL, &dir).expect("downloads");
    assert!(matches!(
        outcome,
        FetchOutcome::Downloaded { bytes: 10, .. }
    ));
    assert_eq!(
        fs::read(dump_dir(&dir).join("db-dump.tar.gz")).expect("file"),
        BODY
    );
    assert_eq!(read_state(&dir).etag.as_deref(), Some("\"v2\""));
}

#[test]
fn partial_file_resumes_with_range_and_if_range() {
    let (_guard, dir) = cache();
    prepare(&dir, "db-dump.tar.gz.partial", &BODY[..4]);
    write_state(&dir, &state("\"v1\"", false));
    let transport = FakeTransport::new([FakeResponse::partial(&BODY[4..], "bytes 4-9/10")]);
    let outcome = dump_with(&transport, URL, &dir).expect("resumes");
    assert!(matches!(
        outcome,
        FetchOutcome::Downloaded { bytes: 10, .. }
    ));
    assert_eq!(
        fs::read(dump_dir(&dir).join("db-dump.tar.gz")).expect("file"),
        BODY
    );
    assert_eq!(
        transport.requests()[0],
        [
            ("Range".to_owned(), "bytes=4-".to_owned()),
            ("If-Range".to_owned(), "\"v1\"".to_owned())
        ]
    );
    assert!(read_state(&dir).complete);
}

#[test]
fn server_that_ignores_the_range_restarts_from_zero() {
    let (_guard, dir) = cache();
    prepare(&dir, "db-dump.tar.gz.partial", b"stale-bytes-longer");
    write_state(&dir, &state("\"v1\"", false));
    let transport = FakeTransport::new([FakeResponse::ok(BODY, "\"v2\"")]);
    dump_with(&transport, URL, &dir).expect("restarts");
    assert_eq!(
        fs::read(dump_dir(&dir).join("db-dump.tar.gz")).expect("file"),
        BODY
    );
    assert_eq!(read_state(&dir).etag.as_deref(), Some("\"v2\""));
}

#[test]
fn partial_file_that_is_already_whole_commits_without_a_request() {
    let (_guard, dir) = cache();
    prepare(&dir, "db-dump.tar.gz.partial", BODY);
    write_state(&dir, &state("\"v1\"", false));
    let transport = FakeTransport::new([]);
    let outcome = dump_with(&transport, URL, &dir).expect("commits");
    assert!(matches!(
        outcome,
        FetchOutcome::Downloaded { bytes: 10, .. }
    ));
    assert!(transport.requests().is_empty());
    assert!(read_state(&dir).complete);
}

#[test]
fn short_body_is_an_error_and_the_next_run_resumes() {
    let (_guard, dir) = cache();
    let transport = FakeTransport::new([FakeResponse::ok(&BODY[..6], "\"v1\"").announcing(10)]);
    let error = dump_with(&transport, URL, &dir).expect_err("short body");
    assert!(matches!(
        error,
        FetchError::Incomplete {
            received: 6,
            expected: 10,
            ..
        }
    ));
    let message = error.to_string();
    assert!(message.contains('6') && message.contains("10"), "{message}");
    assert!(message.contains("Rerun to resume"), "{message}");
    assert!(!dump_dir(&dir).join("db-dump.tar.gz").exists());
    assert!(!read_state(&dir).complete);

    let transport = FakeTransport::new([FakeResponse::partial(&BODY[6..], "bytes 6-9/10")]);
    dump_with(&transport, URL, &dir).expect("resumes");
    assert_eq!(
        fs::read(dump_dir(&dir).join("db-dump.tar.gz")).expect("file"),
        BODY
    );
    assert_eq!(
        transport.requests()[0][0],
        ("Range".to_owned(), "bytes=6-".to_owned())
    );
}

#[test]
fn unexpected_status_names_the_url_and_the_status() {
    let (_guard, dir) = cache();
    let transport = FakeTransport::new([FakeResponse::status(503)]);
    let error = dump_with(&transport, URL, &dir).expect_err("bad status");
    assert!(matches!(error, FetchError::Status { status: 503, .. }));
    let message = error.to_string();
    assert!(
        message.contains(URL) && message.contains("503"),
        "{message}"
    );
}

#[test]
fn range_start_that_differs_from_the_offset_is_an_error() {
    let (_guard, dir) = cache();
    prepare(&dir, "db-dump.tar.gz.partial", &BODY[..4]);
    write_state(&dir, &state("\"v1\"", false));
    let transport = FakeTransport::new([FakeResponse::partial(&BODY[2..], "bytes 2-9/10")]);
    let error = dump_with(&transport, URL, &dir).expect_err("bad range");
    assert!(matches!(error, FetchError::ContentRange { offset: 4, .. }));
}

#[test]
fn cached_keeps_each_directory_and_file_name_apart() {
    let (_guard, dir) = cache();
    let other = dir.join("other");
    let transport = FakeTransport::new([FakeResponse::ok(BODY, "\"a1\"")]);
    let outcome = cached(&transport, URL, &other, "thing.bin").expect("downloads");
    assert_eq!(
        outcome,
        FetchOutcome::Downloaded {
            path: other.join("thing.bin"),
            bytes: 10
        }
    );
    assert_eq!(fs::read(other.join("thing.bin")).expect("file"), BODY);
    assert!(other.join("state.json").exists());
    assert!(!other.join("thing.bin.partial").exists());
    assert!(!dump_dir(&dir).exists());

    let transport = FakeTransport::new([FakeResponse::status(304)]);
    let outcome = cached(&transport, URL, &other, "thing.bin").expect("revalidates");
    assert_eq!(
        outcome,
        FetchOutcome::Unchanged {
            path: other.join("thing.bin")
        }
    );
    assert_eq!(
        transport.requests()[0],
        [("If-None-Match".to_owned(), "\"a1\"".to_owned())]
    );
}

#[test]
fn rustsec_download_lands_under_its_own_directory() {
    let (_guard, dir) = cache();
    let transport = FakeTransport::new([FakeResponse::ok(BODY, "\"r1\"")]);
    rustsec_with(&transport, URL, &dir).expect("downloads");
    assert_eq!(
        fs::read(rustsec_dir(&dir).join("advisory-db.tar.gz")).expect("file"),
        BODY
    );
    assert!(!dump_dir(&dir).exists());
}

#[test]
fn a_body_without_content_length_downloads_and_is_not_resumed() {
    let (_guard, dir) = cache();
    let mut response = FakeResponse::ok(BODY, "\"v1\"");
    response
        .headers
        .retain(|(name, _)| !name.eq_ignore_ascii_case("content-length"));
    let transport = FakeTransport::new([response]);
    let outcome = dump_with(&transport, URL, &dir).expect("downloads");
    assert!(matches!(
        outcome,
        FetchOutcome::Downloaded { bytes: 10, .. }
    ));
    assert_eq!(read_state(&dir).content_length, Some(10));

    prepare(&dir, "db-dump.tar.gz.partial", &BODY[..4]);
    let mut interrupted = state("\"v1\"", false);
    interrupted.content_length = None;
    write_state(&dir, &interrupted);
    let transport = FakeTransport::new([FakeResponse::ok(BODY, "\"v2\"")]);
    dump_with(&transport, URL, &dir).expect("restarts");
    assert!(transport.requests()[0].is_empty());
}
