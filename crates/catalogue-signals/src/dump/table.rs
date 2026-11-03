//! Header lookup and the row loop for one dump table.

use std::io::Read;

use csv::{ByteRecord, StringRecord};

use super::error::DumpError;
use super::names::TableName;
use super::row::{Col, Row};
use super::snapshot::IdMap;

/// Finds each of `names` in `headers`. Extra header columns are ignored.
fn locate<const N: usize>(
    table: &'static str,
    names: &'static [&'static str; N],
    headers: &StringRecord,
) -> Result<[Col; N], DumpError> {
    let mut cols = [Col { name: "", index: 0 }; N];
    for (slot, &name) in cols.iter_mut().zip(names) {
        let index =
            headers
                .iter()
                .position(|header| header == name)
                .ok_or(DumpError::MissingColumn {
                    table,
                    column: name,
                })?;
        *slot = Col { name, index };
    }
    Ok(cols)
}

/// Reads every record of `table` from `input` and passes each to `visit`.
///
/// The header row must name every column in `names`. `visit` gets the columns
/// in the order of `names`, resolved once before the first row.
pub(crate) fn for_each_row<const N: usize>(
    input: impl Read,
    table: TableName,
    names: &'static [&'static str; N],
    mut visit: impl FnMut(&Row<'_>, &[Col; N]) -> Result<(), DumpError>,
) -> Result<(), DumpError> {
    let table = table.name();
    let csv_error = |source: csv::Error| DumpError::Csv { table, source };
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(true)
        .from_reader(input);
    let headers = reader.headers().map_err(csv_error)?;
    let cols = locate(table, names, headers)?;
    let mut record = ByteRecord::new();
    let mut number = 0;
    while reader.read_byte_record(&mut record).map_err(csv_error)? {
        number += 1;
        visit(&Row::new(table, &record, number), &cols)?;
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
    let mut out = IdMap::default();
    for_each_row(input, table, names, |row, &[id, text]| {
        out.insert(row.id(id)?, row.text(text)?.to_owned());
        Ok(())
    })?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rows<const N: usize>(
        csv: &[u8],
        names: &'static [&'static str; N],
    ) -> Result<Vec<Vec<String>>, DumpError> {
        let mut out = Vec::new();
        for_each_row(csv, TableName::Crates, names, |row, cols| {
            let mut fields = Vec::new();
            for &col in cols {
                fields.push(row.text(col)?.to_owned());
            }
            out.push(fields);
            Ok(())
        })?;
        Ok(out)
    }

    #[test]
    fn reads_columns_by_header_name() {
        let found = rows(b"b,extra,a\n2,x,1\n4,y,3\n", &["a", "b"]).expect("reads");
        assert_eq!(found, [["1", "2"], ["3", "4"]]);
    }

    #[test]
    fn missing_column_names_table_and_column() {
        let error = rows(b"a\n1\n", &["a", "b"]).expect_err("rejects");
        assert!(matches!(
            error,
            DumpError::MissingColumn {
                table: "crates",
                column: "b"
            }
        ));
    }

    #[test]
    fn missing_column_is_raised_before_any_row() {
        let mut visited = 0;
        let error = for_each_row(
            b"a\n1\n2\n".as_slice(),
            TableName::Crates,
            &["a", "b"],
            |_, _| {
                visited += 1;
                Ok(())
            },
        )
        .expect_err("rejects");
        assert!(matches!(error, DumpError::MissingColumn { .. }));
        assert_eq!(visited, 0);
    }

    #[test]
    fn quoted_fields_keep_commas_quotes_and_newlines() {
        let found = rows(b"a,b\n\"x, \"\"y\"\"\nz\",\n", &["a", "b"]).expect("reads");
        assert_eq!(found, [["x, \"y\"\nz", ""]]);
    }

    #[test]
    fn invalid_utf8_in_a_read_field_is_a_bad_value() {
        let error = rows(b"a,b\n1,ok\n\xff,x\n", &["a", "b"]).expect_err("rejects");
        assert!(matches!(
            error,
            DumpError::BadValue {
                table: "crates",
                column: "a",
                record: 2,
                expected: "UTF-8 text",
                ..
            }
        ));
    }

    #[test]
    fn invalid_utf8_in_an_unread_field_is_ignored() {
        let found = rows(b"a,junk,b\n1,\xff\xfe,2\n", &["a", "b"]).expect("reads");
        assert_eq!(found, [["1", "2"]]);
    }

    #[test]
    fn bad_values_name_the_record() {
        let error = for_each_row(
            b"id,yanked\n1,t\n2,maybe\n".as_slice(),
            TableName::Versions,
            &["id", "yanked"],
            |row, &[_, yanked]| row.flag(yanked).map(drop),
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
            b"id\n4294967296\n".as_slice(),
            TableName::Versions,
            &["id"],
            |row, &[id]| row.id(id).map(drop),
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
        let error = rows(b"a,b\n1\n", &["a", "b"]).expect_err("rejects");
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
