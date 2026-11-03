//! Typed field getters for one CSV record of a dump table.

use csv::ByteRecord;
use jiff::{Timestamp, civil};

use super::error::DumpError;
use super::parse;

/// Most characters of an invalid field that an error carries.
const VALUE_CHARS: usize = 120;

/// A needed column, resolved to its position in the table header.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Col {
    pub(crate) name: &'static str,
    pub(crate) index: usize,
}

/// One CSV record of a table, read through resolved columns.
pub(crate) struct Row<'a> {
    table: &'static str,
    record: &'a ByteRecord,
    number: u64,
}

impl<'a> Row<'a> {
    /// Wraps `record`, the `number`th record of `table`.
    pub(crate) fn new(table: &'static str, record: &'a ByteRecord, number: u64) -> Self {
        Self {
            table,
            record,
            number,
        }
    }

    /// The raw field bytes. An unquoted empty field (NULL) and `""` both read as empty.
    pub(crate) fn bytes(&self, col: Col) -> Result<&'a [u8], DumpError> {
        self.record
            .get(col.index)
            .ok_or_else(|| self.bad(col, "", "a value in every column"))
    }

    /// The field text. Only this field is checked for UTF-8.
    pub(crate) fn text(&self, col: Col) -> Result<&'a str, DumpError> {
        let bytes = self.bytes(col)?;
        std::str::from_utf8(bytes).map_err(|_| {
            let lossy = String::from_utf8_lossy(bytes);
            let value: String = lossy.chars().take(VALUE_CHARS).collect();
            self.bad(col, &value, "UTF-8 text")
        })
    }

    /// The field text, or `None` when the field is NULL or empty.
    pub(crate) fn optional_text(&self, col: Col) -> Result<Option<&'a str>, DumpError> {
        self.text(col)
            .map(|text| (!text.is_empty()).then_some(text))
    }

    /// The field as an owned optional string.
    pub(crate) fn optional_string(&self, col: Col) -> Result<Option<String>, DumpError> {
        self.optional_text(col).map(|text| text.map(str::to_owned))
    }

    /// A crates.io integer id.
    pub(crate) fn id(&self, col: Col) -> Result<u32, DumpError> {
        let value = self.count(col)?;
        u32::try_from(value).map_err(|_| DumpError::IdOverflow {
            table: self.table,
            column: col.name,
            record: self.number,
            value,
        })
    }

    /// A crates.io integer id, or `None` when the field is NULL or empty.
    pub(crate) fn optional_id(&self, col: Col) -> Result<Option<u32>, DumpError> {
        if self.bytes(col)?.is_empty() {
            return Ok(None);
        }
        self.id(col).map(Some)
    }

    /// A non-negative integer.
    pub(crate) fn count(&self, col: Col) -> Result<u64, DumpError> {
        let text = self.text(col)?;
        parse::unsigned(text).ok_or_else(|| self.bad(col, text, "a non-negative integer"))
    }

    /// A non-negative integer that fits in `u32`.
    pub(crate) fn count32(&self, col: Col) -> Result<u32, DumpError> {
        let text = self.text(col)?;
        parse::unsigned(text)
            .and_then(|value| u32::try_from(value).ok())
            .ok_or_else(|| self.bad(col, text, "a non-negative integer that fits in 32 bits"))
    }

    /// A psql boolean, `t` or `f`.
    pub(crate) fn flag(&self, col: Col) -> Result<bool, DumpError> {
        let text = self.text(col)?;
        parse::boolean(text).ok_or_else(|| self.bad(col, text, "`t` or `f`"))
    }

    /// A psql `timestamptz`.
    pub(crate) fn timestamp(&self, col: Col) -> Result<Timestamp, DumpError> {
        let text = self.text(col)?;
        parse::timestamp(text).ok_or_else(|| {
            self.bad(
                col,
                text,
                "a timestamp such as `2026-05-27 05:23:25.60157+00`",
            )
        })
    }

    /// A bare `YYYY-MM-DD` date.
    pub(crate) fn date(&self, col: Col) -> Result<civil::Date, DumpError> {
        let text = self.text(col)?;
        parse::date(text).ok_or_else(|| self.bad(col, text, "a date such as `2026-11-03`"))
    }

    /// A [`DumpError::BadValue`] for this row.
    pub(crate) fn bad(&self, col: Col, value: &str, expected: &'static str) -> DumpError {
        DumpError::BadValue {
            table: self.table,
            column: col.name,
            record: self.number,
            value: value.to_owned(),
            expected,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const COL: Col = Col {
        name: "c",
        index: 0,
    };

    fn record(field: &[u8]) -> ByteRecord {
        ByteRecord::from(vec![field])
    }

    #[test]
    fn invalid_utf8_in_a_read_field_is_a_bad_value() {
        let record = record(b"ab\xffcd");
        let error = Row::new("crates", &record, 3)
            .text(COL)
            .expect_err("rejects");
        match error {
            DumpError::BadValue {
                table,
                column,
                record,
                value,
                expected,
            } => assert_eq!(
                (table, column, record, value.as_str(), expected),
                ("crates", "c", 3, "ab\u{fffd}cd", "UTF-8 text")
            ),
            other => panic!("unexpected error: {other}"),
        }
    }

    #[test]
    fn invalid_utf8_value_is_truncated() {
        let mut field = vec![b'x'; 200];
        field.push(0xff);
        let record = record(&field);
        let error = Row::new("crates", &record, 1)
            .text(COL)
            .expect_err("rejects");
        assert!(matches!(
            error,
            DumpError::BadValue { value, .. } if value.chars().count() == VALUE_CHARS
        ));
    }

    #[test]
    fn missing_field_is_a_bad_value() {
        let record = ByteRecord::new();
        let error = Row::new("crates", &record, 1)
            .bytes(COL)
            .expect_err("rejects");
        assert!(matches!(error, DumpError::BadValue { .. }));
    }

    #[test]
    fn optional_id_reads_null_as_none() {
        let null = record(b"");
        assert_eq!(
            Row::new("t", &null, 1).optional_id(COL).expect("null"),
            None
        );
        let id = record(b"42");
        assert_eq!(
            Row::new("t", &id, 1).optional_id(COL).expect("id"),
            Some(42)
        );
        let bad = record(b"x");
        let error = Row::new("t", &bad, 1)
            .optional_id(COL)
            .expect_err("rejects");
        assert!(matches!(error, DumpError::BadValue { .. }));
    }

    #[test]
    fn count32_rejects_values_above_u32() {
        let ok = record(b"4294967295");
        assert_eq!(Row::new("t", &ok, 1).count32(COL).expect("fits"), u32::MAX);
        let big = record(b"4294967296");
        let error = Row::new("t", &big, 1).count32(COL).expect_err("rejects");
        assert!(matches!(error, DumpError::BadValue { .. }));
    }
}
