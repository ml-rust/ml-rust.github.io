//! Writes a [`Date`] as a plain `YYYY-MM-DD` string, for JSON output.
//!
//! [`Date`]'s own `Serialize` writes a native TOML date, which JSON has no
//! form for. Use these functions with `#[serde(serialize_with = ...)]`.
//! [`Date`]'s `Deserialize` reads the plain string back.

use serde::Serializer;

use crate::date::Date;

/// Writes `date` as a plain `YYYY-MM-DD` string.
///
/// # Errors
///
/// Returns the serializer's error.
pub fn plain<S: Serializer>(date: &Date, serializer: S) -> Result<S::Ok, S::Error> {
    serializer.collect_str(date)
}

/// Writes `date` as a plain `YYYY-MM-DD` string, or as none.
///
/// # Errors
///
/// Returns the serializer's error.
pub fn plain_option<S: Serializer>(date: &Option<Date>, serializer: S) -> Result<S::Ok, S::Error> {
    match date {
        Some(date) => serializer.collect_str(date),
        None => serializer.serialize_none(),
    }
}

#[cfg(test)]
mod tests {
    use serde::{Deserialize, Serialize};

    use super::*;

    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    struct Wrap {
        #[serde(serialize_with = "plain")]
        d: Date,
        #[serde(serialize_with = "plain_option")]
        o: Option<Date>,
    }

    #[test]
    fn writes_quoted_dates_that_read_back() {
        let date = Date::new(2026, 11, 3).expect("date");
        let wrap = Wrap {
            d: date,
            o: Some(date),
        };
        let text = toml::to_string(&wrap).expect("serializes");
        assert_eq!(text, "d = \"2026-11-03\"\no = \"2026-11-03\"\n");
        assert_eq!(toml::from_str::<Wrap>(&text).expect("parses"), wrap);
    }
}
