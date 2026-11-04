//! The `version_downloads` table, summed per crate and day.

use std::io::Read;

use jiff::{SignedDuration, civil};

use crate::dump::error::DumpError;
use crate::dump::names::TableName;
use crate::dump::row::{Col, Row};
use crate::dump::snapshot::{DailyDownloads, IdMap, VersionRow};
use crate::dump::table::for_each_row;

const COLUMNS: &[&str; 3] = &["version_id", "downloads", "date"];

const HOURS_PER_DAY: i64 = 24;

/// The days the reader keeps download counts for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Window {
    start: civil::Date,
    len: usize,
}

impl Window {
    /// The `days` days that end on `end`, `end` included. Zero days keep nothing.
    pub(crate) fn ending(end: civil::Date, days: u16) -> Self {
        let back = i64::from(days.saturating_sub(1));
        let start = end
            .checked_sub(SignedDuration::from_hours(HOURS_PER_DAY * back))
            .unwrap_or(civil::Date::MIN);
        Self {
            start,
            len: usize::from(days),
        }
    }

    /// The day offset of `date` from the window start, if `date` is inside the window.
    fn index(self, date: civil::Date) -> Option<usize> {
        let days = date.duration_since(self.start).as_hours() / HOURS_PER_DAY;
        usize::try_from(days).ok().filter(|&day| day < self.len)
    }

    /// The date of day offset `day` from the window start.
    fn date_at(self, day: usize) -> civil::Date {
        let days = i64::try_from(day).unwrap_or(i64::MAX);
        self.start
            .checked_add(SignedDuration::from_hours(
                HOURS_PER_DAY.saturating_mul(days),
            ))
            .unwrap_or(self.start)
    }
}

/// The window index of the last date field read.
///
/// A row whose date bytes equal the last row's reuses its index without parsing.
#[derive(Default)]
struct DayCache {
    last: Option<(Vec<u8>, Option<usize>)>,
}

impl DayCache {
    /// The window index of the row's date, or `None` outside the window.
    fn index(
        &mut self,
        row: &Row<'_>,
        col: Col,
        window: Window,
    ) -> Result<Option<usize>, DumpError> {
        let raw = row.bytes(col)?;
        if let Some(&(_, day)) = self
            .last
            .as_ref()
            .filter(|(bytes, _)| bytes.as_slice() == raw)
        {
            return Ok(day);
        }
        let day = window.index(row.date(col)?);
        let (bytes, cached) = self.last.get_or_insert_with(|| (Vec::new(), None));
        bytes.clear();
        bytes.extend_from_slice(raw);
        *cached = day;
        Ok(day)
    }
}

/// Reads `version_downloads` into per-crate daily totals over `window`.
///
/// Every row is checked. A row outside the window is skipped. An in-window row
/// whose version id is not in `versions` adds one to `unknown`.
pub(crate) fn read(
    input: impl Read,
    versions: &IdMap<VersionRow>,
    window: Window,
    unknown: &mut u64,
) -> Result<IdMap<DailyDownloads>, DumpError> {
    let mut out: IdMap<DailyDownloads> = IdMap::default();
    let mut cache = DayCache::default();
    for_each_row(
        input,
        TableName::VersionDownloads,
        COLUMNS,
        |row, &[version_id, downloads, date]| {
            let version_id = row.id(version_id)?;
            let downloads = row.count32(downloads)?;
            let Some(day) = cache.index(row, date, window)? else {
                return Ok(());
            };
            let Some(version) = versions.get(&version_id) else {
                *unknown = unknown.saturating_add(1);
                return Ok(());
            };
            let totals = out
                .entry(version.crate_id)
                .or_insert_with(|| DailyDownloads {
                    start: window.start,
                    days: vec![0; window.len],
                });
            if let Some(slot) = totals.days.get_mut(day) {
                *slot = slot
                    .checked_add(downloads)
                    .ok_or_else(|| DumpError::DownloadOverflow {
                        crate_id: version.crate_id,
                        date: window.date_at(day),
                    })?;
            }
            Ok(())
        },
    )?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use jiff::civil::date;

    use super::*;
    use crate::dump::parse;

    fn version(crate_id: u32) -> VersionRow {
        VersionRow {
            crate_id,
            num: "1.0.0".to_owned(),
            created_at: parse::timestamp("2026-01-01 00:00:00+00").expect("timestamp"),
            yanked: false,
            rust_version: None,
            edition: None,
            published_by: None,
            has_lib: Some(true),
            has_bin: false,
        }
    }

    #[test]
    fn window_covers_the_days_up_to_its_end() {
        let window = Window::ending(date(2026, 3, 2), 3);
        assert_eq!(window.start, date(2026, 2, 28));
        assert_eq!(window.index(date(2026, 2, 27)), None);
        assert_eq!(window.index(date(2026, 2, 28)), Some(0));
        assert_eq!(window.index(date(2026, 3, 2)), Some(2));
        assert_eq!(window.index(date(2026, 3, 3)), None);
        assert_eq!(
            Window::ending(date(2026, 3, 2), 0).index(date(2026, 3, 2)),
            None
        );
    }

    #[test]
    fn sums_versions_per_crate_and_day() {
        let versions: IdMap<VersionRow> = [(10, version(1)), (11, version(1)), (20, version(2))]
            .into_iter()
            .collect();
        let csv = "date,downloads,version_id\n\
            2026-03-02,5,10\n\
            2026-03-02,7,11\n\
            2026-03-01,1,20\n\
            2026-02-01,1000,10\n\
            2026-03-01,3,99\n\
            2026-01-01,3,98\n";
        let mut unknown = 0;
        let window = Window::ending(date(2026, 3, 2), 2);
        let totals = read(csv.as_bytes(), &versions, window, &mut unknown).expect("reads");
        let first = totals.get(&1).expect("crate 1");
        assert_eq!(
            (first.start, first.days.as_slice()),
            (date(2026, 3, 1), &[0, 12][..])
        );
        let second = totals.get(&2).expect("crate 2");
        assert_eq!(second.days, [1, 0]);
        assert_eq!(unknown, 1);
    }

    #[test]
    fn bad_dates_are_rejected() {
        let csv = "version_id,downloads,date\n10,1,03/02/2026\n";
        let window = Window::ending(date(2026, 3, 2), 2);
        let error = read(csv.as_bytes(), &IdMap::default(), window, &mut 0).expect_err("rejects");
        assert!(matches!(
            error,
            DumpError::BadValue {
                table: "version_downloads",
                column: "date",
                record: 1,
                ..
            }
        ));
    }

    #[test]
    fn oversized_row_value_is_a_bad_value() {
        let csv = "version_id,downloads,date\n10,4294967296,2026-03-02\n";
        let window = Window::ending(date(2026, 3, 2), 2);
        let error = read(csv.as_bytes(), &IdMap::default(), window, &mut 0).expect_err("rejects");
        assert!(matches!(
            error,
            DumpError::BadValue {
                table: "version_downloads",
                column: "downloads",
                record: 1,
                ..
            }
        ));
    }

    #[test]
    fn day_total_overflow_names_crate_and_date() {
        let versions: IdMap<VersionRow> =
            [(10, version(1)), (11, version(1))].into_iter().collect();
        let csv = "version_id,downloads,date\n\
            10,3000000000,2026-03-01\n\
            11,2000000000,2026-03-02\n\
            11,2000000000,2026-03-01\n";
        let window = Window::ending(date(2026, 3, 2), 2);
        let error = read(csv.as_bytes(), &versions, window, &mut 0).expect_err("rejects");
        match error {
            DumpError::DownloadOverflow {
                crate_id,
                date: day,
            } => {
                assert_eq!((crate_id, day), (1, date(2026, 3, 1)));
            }
            other => panic!("unexpected error: {other}"),
        }
    }

    #[test]
    fn date_cache_matches_uncached_totals_for_interleaved_dates() {
        let versions: IdMap<VersionRow> =
            [(10, version(1)), (11, version(2))].into_iter().collect();
        let dates = [
            "2026-03-02",
            "2026-03-02",
            "2026-03-01",
            "2026-03-02",
            "2026-02-20",
            "2026-02-20",
            "2026-03-01",
            "2026-03-01",
            "2026-03-02",
            "2026-03-01",
        ];
        let mut csv = String::from("version_id,downloads,date\n");
        let mut expected: IdMap<Vec<u32>> = IdMap::default();
        let window = Window::ending(date(2026, 3, 2), 2);
        for (n, text) in dates.iter().enumerate() {
            let version_id = 10 + u32::try_from(n % 2).expect("small");
            let count = u32::try_from(n + 1).expect("small");
            csv.push_str(&format!("{version_id},{count},{text}\n"));
            let parsed = parse::date(text).expect("date");
            if let (Some(day), Some(version)) = (window.index(parsed), versions.get(&version_id)) {
                let totals = expected
                    .entry(version.crate_id)
                    .or_insert_with(|| vec![0; 2]);
                if let Some(slot) = totals.get_mut(day) {
                    *slot += count;
                }
            }
        }
        let totals = read(csv.as_bytes(), &versions, window, &mut 0).expect("reads");
        assert_eq!(totals.len(), expected.len());
        for (crate_id, days) in &expected {
            assert_eq!(totals.get(crate_id).map(|t| &t.days), Some(days));
        }
    }
}
