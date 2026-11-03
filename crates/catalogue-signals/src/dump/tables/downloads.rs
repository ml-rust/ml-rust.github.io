//! The `version_downloads` table, summed per crate and day.

use std::io::Read;

use jiff::{SignedDuration, civil};

use crate::dump::error::DumpError;
use crate::dump::names::TableName;
use crate::dump::snapshot::{DailyDownloads, IdMap, VersionRow};
use crate::dump::table::for_each_row;

const COLUMNS: &[&str] = &["version_id", "downloads", "date"];

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
    for_each_row(input, TableName::VersionDownloads, COLUMNS, |row| {
        let version_id = row.id("version_id")?;
        let downloads = row.count("downloads")?;
        let Some(day) = window.index(row.date("date")?) else {
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
            *slot = slot.saturating_add(downloads);
        }
        Ok(())
    })?;
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
}
