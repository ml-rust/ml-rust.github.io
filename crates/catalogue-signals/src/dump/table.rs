//! Header lookup and typed field getters for one dump table.

use std::io::Read;

use csv::StringRecord;
use jiff::{Timestamp, civil};

use super::error::DumpError;
use super::names::TableName;
use super::parse;
use super::snapshot::IdMap;

/// The positions of the needed columns in one table's header row.
struct Columns {
    table: &'static str,
    names: &'static [&'static str],
    positions: Vec<usize>,
}

impl Columns {
    /// Finds each of `names` in `headers`. Extra header columns are ignored.
    fn locate(
        table: &'static str,
        names: &'static [&'static str],
        headers: &StringRecord,
    ) -> Result<Self, DumpError> {
        let positions = names
            .iter()
            .map(|&column| {
                headers
                    .iter()
                    .position(|header| header == column)
                    .ok_or(DumpError::MissingColumn { table, column })
            })
            .collect::<Result<_, _>>()?;
        Ok(Self {
            table,
            names,
            positions,
        })
    }
}

/// One CSV record of a table, read by column name.
pub(crate) struct Row<'a> {
    columns: &'a Columns,
    record: &'a StringRecord,
    number: u64,
}

impl<'a> Row<'a> {
    /// The raw field text. An unquoted empty field (NULL) and `""` both read as empty.
    pub(crate) fn text(&self, column: &'static str) -> Result<&'a str, DumpError> {
        let table = self.columns.table;
        let index = self
            .columns
            .names
            .iter()
            .position(|&name| name == column)
            .and_then(|slot| self.columns.positions.get(slot))
            .ok_or(DumpError::MissingColumn { table, column })?;
        self.record
            .get(*index)
            .ok_or_else(|| self.bad(column, "", "a value in every column"))
    }

    /// The field text, or `None` when the field is NULL or empty.
    pub(crate) fn optional_text(&self, column: &'static str) -> Result<Option<&'a str>, DumpError> {
        self.text(column)
            .map(|text| (!text.is_empty()).then_some(text))
    }

    /// The field as an owned optional string.
    pub(crate) fn optional_string(
        &self,
        column: &'static str,
    ) -> Result<Option<String>, DumpError> {
        self.optional_text(column)
            .map(|text| text.map(str::to_owned))
    }

    /// A crates.io integer id.
    pub(crate) fn id(&self, column: &'static str) -> Result<u32, DumpError> {
        let value = self.count(column)?;
        u32::try_from(value).map_err(|_| DumpError::IdOverflow {
            table: self.columns.table,
            column,
            record: self.number,
            value,
        })
    }

    /// A non-negative integer.
    pub(crate) fn count(&self, column: &'static str) -> Result<u64, DumpError> {
        let text = self.text(column)?;
        parse::unsigned(text).ok_or_else(|| self.bad(column, text, "a non-negative integer"))
    }

    /// A psql boolean, `t` or `f`.
    pub(crate) fn flag(&self, column: &'static str) -> Result<bool, DumpError> {
        let text = self.text(column)?;
        parse::boolean(text).ok_or_else(|| self.bad(column, text, "`t` or `f`"))
    }

    /// A psql `timestamptz`.
    pub(crate) fn timestamp(&self, column: &'static str) -> Result<Timestamp, DumpError> {
        let text = self.text(column)?;
        parse::timestamp(text).ok_or_else(|| {
            self.bad(
                column,
                text,
                "a timestamp such as `2026-05-27 05:23:25.60157+00`",
            )
        })
    }

    /// A bare `YYYY-MM-DD` date.
    pub(crate) fn date(&self, column: &'static str) -> Result<civil::Date, DumpError> {
        let text = self.text(column)?;
        parse::date(text).ok_or_else(|| self.bad(column, text, "a date such as `2026-11-03`"))
    }

    /// A [`DumpError::BadValue`] for this row.
    pub(crate) fn bad(
        &self,
        column: &'static str,
        value: &str,
        expected: &'static str,
    ) -> DumpError {
        DumpError::BadValue {
            table: self.columns.table,
            column,
            record: self.number,
            value: value.to_owned(),
            expected,
        }
    }
}

/// Reads every record of `table` from `input` and passes each to `visit`.
///
/// The header row must name every column in `names`.
pub(crate) fn for_each_row(
    input: impl Read,
    table: TableName,
    names: &'static [&'static str],
    mut visit: impl FnMut(&Row<'_>) -> Result<(), DumpError>,
) -> Result<(), DumpError> {
    let table = table.name();
    let csv_error = |source: csv::Error| DumpError::Csv { table, source };
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(true)
        .from_reader(input);
    let headers = reader.headers().map_err(csv_error)?;
    let columns = Columns::locate(table, names, headers)?;
    let mut record = StringRecord::new();
    let mut number = 0;
    while reader.read_record(&mut record).map_err(csv_error)? {
        number += 1;
        visit(&Row {
            columns: &columns,
            record: &record,
            number,
        })?;
    }
    Ok(())
}

/// Reads a lookup table into a map from `id` to the text column.
///
/// `names` holds the id column, then the text column.
pub(crate) fn id_names(
    input: impl Read,
    table: TableName,
    names: &'static [&'static str; 2],
) -> Result<IdMap<String>, DumpError> {
    let [id, text] = *names;
    let mut out = IdMap::default();
    for_each_row(input, table, names, |row| {
        out.insert(row.id(id)?, row.text(text)?.to_owned());
        Ok(())
    })?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rows(csv: &str, names: &'static [&'static str]) -> Result<Vec<Vec<String>>, DumpError> {
        let mut out = Vec::new();
        for_each_row(csv.as_bytes(), TableName::Crates, names, |row| {
            let mut fields = Vec::new();
            for &name in names {
                fields.push(row.text(name)?.to_owned());
            }
            out.push(fields);
            Ok(())
        })?;
        Ok(out)
    }

    #[test]
    fn reads_columns_by_header_name() {
        let found = rows("b,extra,a\n2,x,1\n4,y,3\n", &["a", "b"]).expect("reads");
        assert_eq!(found, [["1", "2"], ["3", "4"]]);
    }

    #[test]
    fn missing_column_names_table_and_column() {
        let error = rows("a\n1\n", &["a", "b"]).expect_err("rejects");
        assert!(matches!(
            error,
            DumpError::MissingColumn {
                table: "crates",
                column: "b"
            }
        ));
    }

    #[test]
    fn quoted_fields_keep_commas_quotes_and_newlines() {
        let found = rows("a,b\n\"x, \"\"y\"\"\nz\",\n", &["a", "b"]).expect("reads");
        assert_eq!(found, [["x, \"y\"\nz", ""]]);
    }

    #[test]
    fn bad_values_name_the_record() {
        let error = for_each_row(
            "id,yanked\n1,t\n2,maybe\n".as_bytes(),
            TableName::Versions,
            &["id", "yanked"],
            |row| row.flag("yanked").map(drop),
        )
        .expect_err("rejects");
        match error {
            DumpError::BadValue {
                table,
                column,
                record,
                value,
                ..
            } => {
                assert_eq!(
                    (table, column, record, value.as_str()),
                    ("versions", "yanked", 2, "maybe")
                );
            }
            other => panic!("unexpected error: {other}"),
        }
    }

    #[test]
    fn ids_above_u32_overflow() {
        let error = for_each_row(
            "id\n4294967296\n".as_bytes(),
            TableName::Versions,
            &["id"],
            |row| row.id("id").map(drop),
        )
        .expect_err("rejects");
        assert!(matches!(
            error,
            DumpError::IdOverflow {
                table: "versions",
                column: "id",
                record: 1,
                value: 4_294_967_296
            }
        ));
    }

    #[test]
    fn short_records_are_csv_errors() {
        let error = rows("a,b\n1\n", &["a", "b"]).expect_err("rejects");
        assert!(matches!(
            error,
            DumpError::Csv {
                table: "crates",
                ..
            }
        ));
    }

    #[test]
    fn id_names_reads_lookup_tables() {
        let map = id_names(
            "slug,id,path\nscience,7,x\n".as_bytes(),
            TableName::Categories,
            &["id", "slug"],
        )
        .expect("reads");
        assert_eq!(map.get(&7).map(String::as_str), Some("science"));
    }
}
