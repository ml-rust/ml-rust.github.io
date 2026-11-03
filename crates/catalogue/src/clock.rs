//! The system clock.

use jiff::Timestamp;
use jiff::civil::Date;
use jiff::tz::TimeZone;

/// Today's date in UTC, read from the system clock.
pub(crate) fn today_utc() -> Date {
    Timestamp::now().to_zoned(TimeZone::UTC).date()
}
