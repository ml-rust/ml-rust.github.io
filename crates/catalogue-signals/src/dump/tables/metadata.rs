//! The dump's `metadata.json`.

use std::io::Read;

use jiff::Timestamp;
use serde_json::Value;

use crate::dump::error::DumpError;
use crate::dump::snapshot::DumpMeta;

/// Largest `metadata.json` the reader accepts, in bytes.
const MAX_BYTES: u64 = 64 * 1024;

/// Reads `metadata.json` from the archive directory `directory`.
pub(crate) fn read_metadata(input: impl Read, directory: String) -> Result<DumpMeta, DumpError> {
    let mut text = String::new();
    input
        .take(MAX_BYTES + 1)
        .read_to_string(&mut text)
        .map_err(|source| DumpError::Io {
            entry: format!("{directory}/metadata.json"),
            source,
        })?;
    if text.len() as u64 > MAX_BYTES {
        return Err(invalid(format!(
            "the file is larger than {MAX_BYTES} bytes"
        )));
    }
    let value: Value = serde_json::from_str(&text)
        .map_err(|error| invalid(format!("the file is not JSON: {error}")))?;
    let raw = field(&value, "timestamp")?;
    let timestamp = raw.parse::<Timestamp>().map_err(|error| {
        invalid(format!(
            "`timestamp` is `{raw}`, expected an RFC 3339 time such as `2026-11-03T02:00:17Z`: {error}"
        ))
    })?;
    Ok(DumpMeta {
        timestamp,
        crates_io_commit: field(&value, "crates_io_commit")?.to_owned(),
        directory,
    })
}

fn field<'v>(value: &'v Value, name: &str) -> Result<&'v str, DumpError> {
    value
        .get(name)
        .and_then(Value::as_str)
        .ok_or_else(|| invalid(format!("the file has no string field `{name}`")))
}

fn invalid(reason: String) -> DumpError {
    DumpError::Metadata { reason }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_timestamp_and_commit() {
        let json = r#"{"timestamp":"2026-11-03T02:00:17.123456Z","crates_io_commit":"abc123"}"#;
        let meta = read_metadata(json.as_bytes(), "2026-11-03-020017".to_owned()).expect("reads");
        assert_eq!(
            meta.timestamp,
            "2026-11-03T02:00:17.123456Z"
                .parse::<Timestamp>()
                .expect("ts")
        );
        assert_eq!(meta.crates_io_commit, "abc123");
        assert_eq!(meta.directory, "2026-11-03-020017");
    }

    #[test]
    fn rejects_missing_fields_and_bad_values() {
        for (json, needle) in [
            (r#"{"crates_io_commit":"abc"}"#, "`timestamp`"),
            (
                r#"{"timestamp":"2026-11-03T02:00:17Z"}"#,
                "`crates_io_commit`",
            ),
            (r#"{"timestamp":"soon","crates_io_commit":"abc"}"#, "`soon`"),
            ("not json", "not JSON"),
        ] {
            let error = read_metadata(json.as_bytes(), "d".to_owned()).expect_err(json);
            assert!(error.to_string().contains(needle), "{json}: {error}");
        }
    }

    #[test]
    fn rejects_oversized_files() {
        let json = format!(r#"{{"pad":"{}"}}"#, "x".repeat(70_000));
        let error = read_metadata(json.as_bytes(), "d".to_owned()).expect_err("rejects");
        assert!(error.to_string().contains("larger than"), "{error}");
    }
}
