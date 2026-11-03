use std::fmt;

use jiff::civil;
use serde::de::value::MapAccessDeserializer;
use serde::de::{self, MapAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use thiserror::Error;
use toml::value::{Date as TomlDate, Datetime};

/// A calendar date without time or zone.
///
/// A TOML file writes it as a native date (`added = 2026-11-03`) or as a
/// quoted string (`added = "2026-11-03"`). Output always uses the native form.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Date(civil::Date);

/// A rejected date value.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum DateError {
    /// The value carries a time of day or a UTC offset.
    #[error("a date must not carry a time or an offset")]
    TimeOrOffset,
    /// The value is a time of day with no date part.
    #[error("expected a calendar date")]
    NoDate,
    /// The year, month and day do not form a real date.
    #[error("invalid calendar date: {0}")]
    Invalid(String),
}

impl Date {
    /// Creates a date from year, month and day.
    pub fn new(year: i16, month: i8, day: i8) -> Result<Self, DateError> {
        civil::Date::new(year, month, day)
            .map(Self)
            .map_err(|e| DateError::Invalid(e.to_string()))
    }

    /// The underlying civil date.
    pub fn civil(self) -> civil::Date {
        self.0
    }

    fn from_toml(value: Datetime) -> Result<Self, DateError> {
        if value.time.is_some() || value.offset.is_some() {
            return Err(DateError::TimeOrOffset);
        }
        let date = value.date.ok_or(DateError::NoDate)?;
        let out_of_range = || DateError::Invalid("component out of range".to_owned());
        let year = i16::try_from(date.year).map_err(|_| out_of_range())?;
        let month = i8::try_from(date.month).map_err(|_| out_of_range())?;
        let day = i8::try_from(date.day).map_err(|_| out_of_range())?;
        Self::new(year, month, day)
    }
}

impl fmt::Display for Date {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, f)
    }
}

impl Serialize for Date {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let year = u16::try_from(self.0.year()).map_err(serde::ser::Error::custom)?;
        let month = u8::try_from(self.0.month()).map_err(serde::ser::Error::custom)?;
        let day = u8::try_from(self.0.day()).map_err(serde::ser::Error::custom)?;
        Datetime {
            date: Some(TomlDate { year, month, day }),
            time: None,
            offset: None,
        }
        .serialize(serializer)
    }
}

struct DateVisitor;

impl<'de> Visitor<'de> for DateVisitor {
    type Value = Date;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("a calendar date such as 2026-11-03")
    }

    fn visit_str<E: de::Error>(self, value: &str) -> Result<Date, E> {
        let parsed = value.parse::<Datetime>().map_err(E::custom)?;
        Date::from_toml(parsed).map_err(E::custom)
    }

    fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<Date, A::Error> {
        let parsed = Datetime::deserialize(MapAccessDeserializer::new(map))?;
        Date::from_toml(parsed).map_err(de::Error::custom)
    }
}

impl<'de> Deserialize<'de> for Date {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_any(DateVisitor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    struct Wrap {
        d: Date,
    }

    fn expected() -> Wrap {
        Wrap {
            d: Date::new(2026, 11, 3).expect("valid date"),
        }
    }

    #[test]
    fn reads_a_native_toml_date() {
        let parsed: Wrap = toml::from_str("d = 2026-11-03").expect("native date");
        assert_eq!(parsed, expected());
    }

    #[test]
    fn reads_a_quoted_date() {
        let parsed: Wrap = toml::from_str("d = \"2026-11-03\"").expect("quoted date");
        assert_eq!(parsed, expected());
    }

    #[test]
    fn writes_a_native_toml_date() {
        let text = toml::to_string(&expected()).expect("serializes");
        assert_eq!(text, "d = 2026-11-03\n");
        let back: Wrap = toml::from_str(&text).expect("round trip");
        assert_eq!(back, expected());
    }

    #[test]
    fn rejects_values_with_a_time_or_offset() {
        for bad in [
            "d = 2026-11-03T10:00:00",
            "d = 2026-11-03T10:00:00Z",
            "d = 2026-11-03T10:00:00+08:00",
            "d = 10:00:00",
            "d = \"2026-11-03T10:00:00\"",
            "d = \"2026-11-03T10:00:00Z\"",
            "d = \"10:00:00\"",
        ] {
            assert!(toml::from_str::<Wrap>(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn rejects_impossible_dates_and_other_types() {
        for bad in [
            "d = 2026-02-30",
            "d = \"2026-13-01\"",
            "d = \"yesterday\"",
            "d = 5",
        ] {
            assert!(toml::from_str::<Wrap>(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn displays_as_iso_date() {
        assert_eq!(expected().d.to_string(), "2026-11-03");
    }
}
