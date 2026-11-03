//! Parsers for the psql CSV field forms in the dump.

use jiff::tz::Offset;
use jiff::{Timestamp, civil};

/// Parses a psql `timestamptz` such as `2026-05-27 05:23:25.60157+00`.
///
/// The form is `YYYY-MM-DD HH:MM:SS[.fraction]±HH[:MM]`, with 1 to 9 fraction digits.
pub(crate) fn timestamp(text: &str) -> Option<Timestamp> {
    let bytes = text.as_bytes();
    let date = date_bytes(bytes.get(..10)?)?;
    let time = bytes.get(10..)?;
    let (&b' ', time) = time.split_first()? else {
        return None;
    };
    let hour = two_digits(time.get(0..2)?)?;
    let minute = two_digits(time.get(3..5)?)?;
    let second = two_digits(time.get(6..8)?)?;
    if time.get(2) != Some(&b':') || time.get(5) != Some(&b':') {
        return None;
    }
    let mut rest = time.get(8..)?;
    let mut nanosecond = 0;
    if let Some(fraction) = rest.strip_prefix(b".") {
        let len = fraction.iter().take_while(|b| b.is_ascii_digit()).count();
        if len == 0 || len > 9 {
            return None;
        }
        nanosecond = digits(fraction.get(..len)?)?;
        for _ in len..9 {
            nanosecond *= 10;
        }
        rest = fraction.get(len..)?;
    }
    let offset = offset(rest)?;
    let clock = civil::Time::new(hour, minute, second, nanosecond).ok()?;
    offset.to_timestamp(date.to_datetime(clock)).ok()
}

/// Parses a bare `YYYY-MM-DD` date.
pub(crate) fn date(text: &str) -> Option<civil::Date> {
    let bytes = text.as_bytes();
    if bytes.len() != 10 {
        return None;
    }
    date_bytes(bytes)
}

/// Parses a psql boolean: `t` or `f`.
pub(crate) fn boolean(text: &str) -> Option<bool> {
    match text {
        "t" => Some(true),
        "f" => Some(false),
        _ => None,
    }
}

/// Parses a non-negative decimal integer with no sign.
pub(crate) fn unsigned(text: &str) -> Option<u64> {
    if text.is_empty() || !text.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    text.parse().ok()
}

/// Parses `±HH` or `±HH:MM` with nothing after it.
fn offset(text: &[u8]) -> Option<Offset> {
    let (&sign, rest) = text.split_first()?;
    let sign = match sign {
        b'+' => 1,
        b'-' => -1,
        _ => return None,
    };
    let (hours, minutes) = match rest {
        [h1, h2] => (two_digits(&[*h1, *h2])?, 0),
        [h1, h2, b':', m1, m2] => (two_digits(&[*h1, *h2])?, two_digits(&[*m1, *m2])?),
        _ => return None,
    };
    if minutes >= 60 {
        return None;
    }
    let seconds = sign * (i32::from(hours) * 3600 + i32::from(minutes) * 60);
    Offset::from_seconds(seconds).ok()
}

/// Parses the first ten bytes as `YYYY-MM-DD`.
fn date_bytes(bytes: &[u8]) -> Option<civil::Date> {
    if bytes.len() != 10 || bytes.get(4) != Some(&b'-') || bytes.get(7) != Some(&b'-') {
        return None;
    }
    let year = i16::try_from(digits(bytes.get(0..4)?)?).ok()?;
    let month = two_digits(bytes.get(5..7)?)?;
    let day = two_digits(bytes.get(8..10)?)?;
    civil::Date::new(year, month, day).ok()
}

fn two_digits(bytes: &[u8]) -> Option<i8> {
    if bytes.len() != 2 {
        return None;
    }
    i8::try_from(digits(bytes)?).ok()
}

/// Parses 1 to 9 ASCII digits.
fn digits(bytes: &[u8]) -> Option<i32> {
    if bytes.is_empty() || bytes.len() > 9 {
        return None;
    }
    bytes.iter().try_fold(0i32, |acc, &b| {
        b.is_ascii_digit().then(|| acc * 10 + i32::from(b - b'0'))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ts(text: &str) -> Timestamp {
        text.parse().expect("RFC 3339 timestamp")
    }

    #[test]
    fn timestamp_with_hour_offset() {
        assert_eq!(
            timestamp("2026-05-27 05:23:25+00"),
            Some(ts("2026-05-27T05:23:25Z"))
        );
        assert_eq!(
            timestamp("2026-05-27 05:23:25+08"),
            Some(ts("2026-05-26T21:23:25Z"))
        );
        assert_eq!(
            timestamp("2026-05-27 05:23:25-05:30"),
            Some(ts("2026-05-27T10:53:25Z"))
        );
    }

    #[test]
    fn timestamp_fractions_of_every_length() {
        for (text, expected) in [
            ("2026-05-27 05:23:25.6+00", "2026-05-27T05:23:25.6Z"),
            ("2026-05-27 05:23:25.60+00", "2026-05-27T05:23:25.6Z"),
            ("2026-05-27 05:23:25.601+00", "2026-05-27T05:23:25.601Z"),
            ("2026-05-27 05:23:25.6015+00", "2026-05-27T05:23:25.6015Z"),
            ("2026-05-27 05:23:25.60157+00", "2026-05-27T05:23:25.60157Z"),
            (
                "2026-05-27 05:23:25.601573+00",
                "2026-05-27T05:23:25.601573Z",
            ),
            (
                "2026-05-27 05:23:25.601573123+00",
                "2026-05-27T05:23:25.601573123Z",
            ),
        ] {
            assert_eq!(timestamp(text), Some(ts(expected)), "{text}");
        }
    }

    #[test]
    fn timestamp_rejects_garbage() {
        for bad in [
            "",
            "garbage",
            "2026-05-27",
            "2026-05-27 05:23:25",
            "2026-05-27T05:23:25+00",
            "2026-05-27 05:23:25Z",
            "2026-05-27 05:23:25.+00",
            "2026-05-27 05:23:25.6015731234+00",
            "2026-05-27 05:23:25+0",
            "2026-05-27 05:23:25+00:6",
            "2026-05-27 05:23:25+00:60",
            "2026-05-27 05:23:25+00x",
            "2026-02-30 05:23:25+00",
            "2026-05-27 25:23:25+00",
            "2026-05-27 05-23-25+00",
            "2026-5-27 05:23:25+00",
            "+026-05-27 05:23:25+00",
            "2026-05-27 05:23:25 +00",
        ] {
            assert_eq!(timestamp(bad), None, "{bad}");
        }
    }

    #[test]
    fn dates() {
        assert_eq!(date("2026-11-03"), Some(civil::date(2026, 11, 3)));
        for bad in [
            "",
            "2026-11-3",
            "2026-11-03 ",
            "2026/11/03",
            "2026-13-01",
            "-026-11-03",
        ] {
            assert_eq!(date(bad), None, "{bad}");
        }
    }

    #[test]
    fn booleans() {
        assert_eq!(boolean("t"), Some(true));
        assert_eq!(boolean("f"), Some(false));
        for bad in ["", "true", "T", "1", "yes"] {
            assert_eq!(boolean(bad), None, "{bad}");
        }
    }

    #[test]
    fn unsigned_numbers() {
        assert_eq!(unsigned("0"), Some(0));
        assert_eq!(unsigned("4294967296"), Some(4_294_967_296));
        for bad in ["", "-1", "+1", "1.0", " 1", "x"] {
            assert_eq!(unsigned(bad), None, "{bad}");
        }
    }
}
