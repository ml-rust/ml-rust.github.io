//! Parsing the TOML front matter of an advisory file.

use jiff::civil;
use serde::Deserialize;

use super::advisory::{Advisory, AdvisoryKind};

/// An advisory with the notes the parser raises while reading it.
pub(super) struct Parsed {
    pub(super) advisory: Advisory,
    pub(super) notes: Vec<String>,
}

#[derive(Deserialize)]
struct Document {
    advisory: RawAdvisory,
    #[serde(default)]
    versions: RawVersions,
}

#[derive(Deserialize)]
struct RawAdvisory {
    id: String,
    package: String,
    date: toml::Value,
    informational: Option<String>,
    withdrawn: Option<toml::Value>,
    url: Option<String>,
}

#[derive(Default, Deserialize)]
struct RawVersions {
    #[serde(default)]
    patched: Vec<String>,
    #[serde(default)]
    unaffected: Vec<String>,
}

/// The text of the first ```` ```toml ```` fenced block in `text`.
pub(super) fn toml_block(text: &str) -> Result<&str, &'static str> {
    let mut offset = 0;
    let mut start = None;
    for line in text.split_inclusive('\n') {
        let next = offset + line.len();
        let trimmed = line.trim_end();
        match start {
            None if trimmed.trim_start().eq_ignore_ascii_case("```toml") => start = Some(next),
            Some(begin) if trimmed.trim_start() == "```" => {
                return text
                    .get(begin..offset)
                    .ok_or("the TOML block is not valid text");
            }
            _ => {}
        }
        offset = next;
    }
    match start {
        Some(_) => Err("the TOML block has no closing fence"),
        None => Err("the file has no ```toml block"),
    }
}

/// Parses the front matter of the advisory file `text`.
pub(super) fn parse(text: &str) -> Result<Parsed, String> {
    let block = toml_block(text).map_err(str::to_owned)?;
    let document: Document = toml::from_str(block).map_err(|error| error.to_string())?;
    let raw = document.advisory;
    let date = parse_date("date", &raw.date)?;
    let withdrawn = raw
        .withdrawn
        .as_ref()
        .map(|value| parse_date("withdrawn", value))
        .transpose()?;
    let mut notes = Vec::new();
    let kind = match raw.informational.as_deref() {
        None => AdvisoryKind::Vulnerability,
        Some("unmaintained") => AdvisoryKind::Unmaintained,
        Some("unsound") => AdvisoryKind::Unsound,
        Some("notice") => AdvisoryKind::Notice,
        Some(other) => {
            notes.push(format!(
                "unknown informational value `{other}`, read as notice"
            ));
            AdvisoryKind::Notice
        }
    };
    let advisory = Advisory {
        id: raw.id,
        package: raw.package,
        date,
        kind,
        withdrawn,
        patched: document.versions.patched,
        unaffected: document.versions.unaffected,
        url: raw.url,
    };
    Ok(Parsed { advisory, notes })
}

/// Reads a date written as a quoted string or as a native TOML date.
fn parse_date(field: &str, value: &toml::Value) -> Result<civil::Date, String> {
    let text = match value {
        toml::Value::String(text) => text.clone(),
        toml::Value::Datetime(datetime) if datetime.time.is_none() && datetime.offset.is_none() => {
            datetime.to_string()
        }
        other => other.to_string(),
    };
    text.parse()
        .map_err(|_| format!("`{field}` is `{text}`, expected a date as YYYY-MM-DD"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_the_first_toml_block() {
        let text = "```toml\n[advisory]\nid = \"x\"\n```\n\n```toml\nignored\n```\n";
        assert_eq!(toml_block(text), Ok("[advisory]\nid = \"x\"\n"));
        let crlf = "```toml\r\na = 1\r\n```\r\n";
        assert_eq!(toml_block(crlf), Ok("a = 1\r\n"));
    }

    #[test]
    fn missing_or_open_blocks_are_errors() {
        assert!(toml_block("# title\n").is_err());
        assert!(toml_block("```toml\na = 1\n").is_err());
        assert!(toml_block("```rust\na\n```\n").is_err());
    }

    #[test]
    fn a_bad_date_names_the_field_and_value() {
        let text = "```toml\n[advisory]\nid = \"a\"\npackage = \"p\"\ndate = \"2021-13-45\"\n```\n";
        let error = parse(text).err().unwrap_or_default();
        assert!(error.contains("`date`"), "{error}");
        assert!(error.contains("2021-13-45"), "{error}");
    }

    #[test]
    fn native_and_quoted_dates_read_the_same() {
        let quoted =
            "```toml\n[advisory]\nid = \"a\"\npackage = \"p\"\ndate = \"2021-07-07\"\n```\n";
        let native = "```toml\n[advisory]\nid = \"a\"\npackage = \"p\"\ndate = 2021-07-07\nwithdrawn = 2022-01-02\n```\n";
        let quoted = parse(quoted).expect("quoted date").advisory;
        let native = parse(native).expect("native date").advisory;
        assert_eq!(quoted.date, civil::date(2021, 7, 7));
        assert_eq!(native.date, civil::date(2021, 7, 7));
        assert_eq!(native.withdrawn, Some(civil::date(2022, 1, 2)));
    }

    #[test]
    fn a_datetime_with_a_time_is_rejected() {
        let text =
            "```toml\n[advisory]\nid = \"a\"\npackage = \"p\"\ndate = 2021-07-07T10:00:00Z\n```\n";
        let error = parse(text).err().unwrap_or_default();
        assert!(error.contains("`date`"), "{error}");
    }
}
