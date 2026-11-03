//! Signals and contexts shared by the tier rule tests.

use jiff::Span;

use super::context::Context;
use super::signals::Signals;
use crate::date::Date;
use crate::entry::EntryKind;

pub(crate) use crate::testing::thresholds;

/// A calendar date.
pub(crate) fn day(year: i16, month: i8, day: i8) -> Date {
    Date::new(year, month, day).expect("valid date")
}

/// The check date of every test.
pub(crate) fn today() -> Date {
    day(2026, 11, 3)
}

/// The date `days` before [`today`].
pub(crate) fn days_ago(days: i64) -> Date {
    let civil = today()
        .civil()
        .checked_sub(Span::new().days(days))
        .expect("date in range");
    Date::new(civil.year(), civil.month(), civil.day()).expect("valid date")
}

/// A first check on [`today`].
pub(crate) fn context() -> Context {
    Context::new(today())
}

/// A library that passes every Featured gate. E is 2800 days.
pub(crate) fn featured_library() -> Signals {
    Signals {
        first_release: Some(day(2023, 1, 1)),
        latest_version: Some("1.2.5".to_owned()),
        release_count: Some(30),
        last_release: Some(day(2026, 9, 1)),
        all_yanked: Some(false),
        crate_deleted: Some(false),
        all_prerelease: Some(false),
        active_users: Some(50.0),
        new_adopters: Some(12.0),
        peak_users: Some(55.0),
        former_glory: Some(0.9),
        downloads_per_month: Some(100_000),
        unpatched_vulnerabilities: Some(Vec::new()),
        repo_archived: Some(false),
        repo_gone: Some(false),
        last_human_commit: Some(day(2026, 10, 20)),
        oldest_unanswered_days: Some(10),
        maintainer_replies_365: Some(25),
        docs_present: Some(true),
        repo_link_verified: Some(true),
        build_fails: Some(false),
        build_checked: Some(day(2026, 10, 27)),
        build_toolchain: Some("rustc 1.95.0".to_owned()),
        ..Signals::unknown(EntryKind::Library)
    }
}

/// A library with E = 365 days, last active 400 days ago, no rot, and little adoption.
pub(crate) fn dormant_library() -> Signals {
    Signals {
        latest_version: Some("0.3.0".to_owned()),
        release_count: Some(10),
        last_release: Some(days_ago(400)),
        last_human_commit: Some(days_ago(400)),
        active_users: Some(5.0),
        new_adopters: Some(0.0),
        former_glory: Some(0.6),
        downloads_per_month: Some(100),
        ..featured_library()
    }
}

/// [`dormant_library`] with two rot signals: lost glory and a gone repository.
pub(crate) fn dead_library() -> Signals {
    Signals {
        former_glory: Some(0.2),
        repo_gone: Some(true),
        ..dormant_library()
    }
}

/// An app that passes every Featured gate.
pub(crate) fn featured_app() -> Signals {
    Signals {
        first_release: Some(day(2022, 1, 1)),
        latest_version: Some("2.0.0".to_owned()),
        release_count: Some(20),
        last_release: Some(day(2026, 8, 1)),
        all_prerelease: Some(false),
        unpatched_vulnerabilities: Some(Vec::new()),
        repo_archived: Some(false),
        repo_gone: Some(false),
        last_human_commit: Some(day(2026, 10, 1)),
        oldest_unanswered_days: Some(5),
        maintainer_replies_365: Some(10),
        binary_downloads_last3: Some(5_000),
        binary_downloads_per_release: Some(1_500),
        homebrew_on_request_365d: Some(800),
        repology_families: Some(3),
        docker_pull_growth_90d: Some(20_000),
        ..Signals::unknown(EntryKind::App)
    }
}
