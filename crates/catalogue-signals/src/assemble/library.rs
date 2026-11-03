//! The signals of a library entry.

use catalogue_core::{Date, Entry, EntryKind, Signals};

use super::adoption::adoption;
use super::assembled::Assembled;
use super::build::apply_build_checks;
use super::facts::apply_facts;
use super::link::apply_link;
use super::requirements::deprecated_requirement;
use super::sources::LibrarySources;
use super::versions::{family_span, primary_releases};
use crate::dump::DumpSnapshot;
use crate::github::RepoFacts;
use crate::link::LinkCheck;

/// Days of downloads that `downloads_per_month` sums.
const MONTH_DAYS: usize = 30;

/// The signals of library `entry` on `today`.
///
/// `entry.crates[0]` is the primary crate. The rest are family crates. Names
/// resolve the crates.io way. The rules:
///
/// - `first_release`, `last_release`: the earliest and latest unyanked
///   publish day across the family.
/// - `latest_version`, `release_count`, `all_yanked`, `all_prerelease`: the primary's versions.
/// - `crate_deleted`: true when the primary is absent from `crates` and named
///   in `deleted_crates`, false when present, unknown otherwise.
/// - `active_users`, `new_adopters`, `peak_users`, `former_glory`: the family
///   crate with the highest current users. A tie goes to the crate listed first.
/// - `downloads_per_month`: the raw sum of the primary's last 30 days of
///   downloads. The value only decides whether the expected interval doubles.
/// - `rustsec_unmaintained`, `unpatched_vulnerabilities`: the primary's
///   advisories. A vulnerability counts once its age reaches
///   `rot.advisory_grace_days`.
/// - `deprecation_notice`: the primary's description or README notice.
/// - `deprecated_requirement`: the first normal or build requirement of the
///   primary's default version that the deprecated list matches, as `<crate> <req>`,
///   or that names a crate with an unmaintained advisory, as `<crate> (<RUSTSEC-ID>)`.
/// - `docs_present`: the primary publishes a README.
/// - `build_fails`, `build_checked`, `build_toolchain`: the sandbox results for
///   each family crate's latest release. A `fails` for any crate sets true. A
///   `passes` for every crate sets false. Anything else leaves them unknown.
/// - `repo_link_verified`: `link.verified()`. `None` leaves it unknown. A
///   missing commit, a release without commit data, an unsupported host and a
///   dirty release each add a note.
/// - Repository signals come from `facts`. `None` leaves them unknown.
///
/// Each family crate missing from the dump, and each requirement the
/// deprecated list cannot parse, adds a note.
pub fn library_signals(
    entry: &Entry,
    sources: &LibrarySources<'_>,
    facts: Option<&RepoFacts>,
    link: Option<&LinkCheck>,
    today: Date,
) -> Assembled {
    let mut signals = Signals::unknown(EntryKind::Library);
    let mut notes = Vec::new();
    apply_facts(facts, &mut signals);
    apply_link(link, &mut signals, &mut notes);
    let Some(primary) = entry.crates.first() else {
        notes.push("the entry lists no crate".to_owned());
        return Assembled { signals, notes };
    };
    let index = sources.index;
    let mut family = Vec::with_capacity(entry.crates.len());
    for name in &entry.crates {
        match index.id(name.as_str()) {
            Some(id) => family.push(id),
            None if name == primary && index.is_deleted(name.as_str()) => {
                notes.push(format!("crate {name} is deleted from crates.io"));
            }
            None => notes.push(format!("crate {name} is not in the dump")),
        }
    }
    let advisories = sources.advisories;
    signals.rustsec_unmaintained = advisories
        .unmaintained(primary.as_str())
        .map(|advisory| advisory.id.clone());
    let grace = i64::from(sources.thresholds.rot.advisory_grace_days);
    let unpatched = advisories.unpatched_vulnerabilities(primary.as_str(), today.civil(), grace);
    signals.unpatched_vulnerabilities = Some(unpatched.iter().map(|a| a.id.clone()).collect());
    family_span(sources.dump, index, &family, &mut signals);
    apply_build_checks(
        entry,
        sources.dump,
        index,
        sources.build_checks,
        &mut signals,
    );
    adoption(&family, sources.usage, &mut signals);
    match index.id(primary.as_str()) {
        Some(id) => primary_signals(sources, id, &mut signals, &mut notes),
        None if index.is_deleted(primary.as_str()) => signals.crate_deleted = Some(true),
        None => {}
    }
    Assembled { signals, notes }
}

/// Sets the signals that come from the primary crate `id` alone.
fn primary_signals(
    sources: &LibrarySources<'_>,
    id: u32,
    signals: &mut Signals,
    notes: &mut Vec<String>,
) {
    let dump = sources.dump;
    signals.crate_deleted = Some(false);
    primary_releases(dump, sources.index, id, signals);
    signals.downloads_per_month = Some(month_downloads(dump, id));
    if let Some(row) = dump.crates.get(&id) {
        signals.deprecation_notice = row.deprecation.as_ref().map(|n| n.phrase.clone());
        signals.docs_present = Some(row.has_readme);
    }
    let found = deprecated_requirement(dump, id, sources.deprecated, sources.advisories);
    signals.deprecated_requirement = found.first;
    if found.unparsed > 0 {
        notes.push(format!("unparsed_requirements: {}", found.unparsed));
    }
}

/// The raw downloads of crate `id` over the last 30 days of the window. 0 when the dump has none.
fn month_downloads(dump: &DumpSnapshot, id: u32) -> u64 {
    let Some(series) = dump.downloads.get(&id) else {
        return 0;
    };
    series
        .days
        .iter()
        .rev()
        .take(MONTH_DAYS)
        .map(|&count| u64::from(count))
        .sum()
}

#[cfg(test)]
mod tests {
    use catalogue_core::deprecation::{DeprecationNotice, Field};

    use super::*;
    use crate::assemble::fixtures::{World, day, found_facts};
    use crate::dump::DependencyKind;
    use crate::rustsec::AdvisoryKind;

    #[test]
    fn dates_and_adoption_span_the_family_and_versions_come_from_the_primary() {
        let mut world = World::new();
        world.dump.add_crate(1, "main");
        world.dump.add_crate(2, "helper-crate");
        world
            .dump
            .add_version(10, 1, "1.4.0", day(2024, 3, 1), false);
        world
            .dump
            .add_version(20, 2, "0.1.0", day(2022, 5, 1), false);
        world
            .dump
            .add_version(21, 2, "0.2.0", day(2026, 9, 1), false);
        world.users(1, 3.0);
        world.users(2, 5.0);
        let found = world.run(&["main", "Helper_Crate"], None).signals;
        assert_eq!(found.active_users, Some(5.0));
        assert_eq!(found.peak_users, Some(10.0));
        assert_eq!(found.first_release, Some(Date::from(day(2022, 5, 1))));
        assert_eq!(found.last_release, Some(Date::from(day(2026, 9, 1))));
        assert_eq!(found.latest_version.as_deref(), Some("1.4.0"));
        assert_eq!(found.release_count, Some(1));
        assert_eq!(found.crate_deleted, Some(false));
        assert_eq!(found.repo_link_verified, None);
    }

    #[test]
    fn downloads_sum_the_last_30_days_raw() {
        let mut world = World::new();
        world.dump.add_crate(1, "main");
        let mut days = vec![1_000; 10];
        days.extend(vec![2; 30]);
        world.dump.set_downloads(1, day(2026, 9, 25), days);
        assert_eq!(
            world.run(&["main"], None).signals.downloads_per_month,
            Some(60)
        );
        world.dump.snapshot.downloads.clear();
        assert_eq!(
            world.run(&["main"], None).signals.downloads_per_month,
            Some(0)
        );
    }

    #[test]
    fn notices_docs_and_deprecated_requirements_come_from_the_primary() {
        let mut world = World::new();
        let row = world.dump.add_crate(1, "main");
        row.has_readme = false;
        row.deprecation = Some(DeprecationNotice {
            field: Field::Description,
            phrase: "deprecated".to_owned(),
        });
        world.dump.add_crate(2, "tokio");
        world
            .dump
            .add_requirement(1, 2, "~>0.1", DependencyKind::Normal);
        world
            .dump
            .add_requirement(1, 2, "0.2", DependencyKind::Build);
        let assembled = world.run(&["main"], None);
        let found = assembled.signals;
        assert_eq!(found.deprecation_notice.as_deref(), Some("deprecated"));
        assert_eq!(found.docs_present, Some(false));
        assert_eq!(
            found.deprecated_requirement.as_deref(),
            Some("tokio 0.2 (https://example.com/r)")
        );
        assert_eq!(assembled.notes, ["unparsed_requirements: 1"]);
    }

    #[test]
    fn advisories_name_the_unmaintained_and_the_unpatched_past_grace() {
        let mut world = World::new();
        world.dump.add_crate(1, "main");
        world.advisory(
            "RUSTSEC-2026-0001",
            AdvisoryKind::Unmaintained,
            day(2026, 1, 1),
        );
        world.advisory(
            "RUSTSEC-2026-0002",
            AdvisoryKind::Vulnerability,
            day(2026, 7, 1),
        );
        world.advisory(
            "RUSTSEC-2026-0003",
            AdvisoryKind::Vulnerability,
            day(2026, 10, 20),
        );
        let found = world.run(&["main"], None).signals;
        assert_eq!(
            found.rustsec_unmaintained.as_deref(),
            Some("RUSTSEC-2026-0001")
        );
        assert_eq!(
            found.unpatched_vulnerabilities,
            Some(vec!["RUSTSEC-2026-0002".to_owned()])
        );
        let clean = world.run(&["other"], None).signals;
        assert_eq!(clean.rustsec_unmaintained, None);
        assert_eq!(clean.unpatched_vulnerabilities, Some(Vec::new()));
    }

    #[test]
    fn missing_crates_are_noted_and_a_deleted_primary_is_deleted() {
        let mut world = World::new();
        world.dump.add_crate(2, "helper");
        let absent = world.run(&["main", "helper", "gone"], None);
        assert_eq!(absent.signals.crate_deleted, None);
        assert_eq!(absent.signals.docs_present, None);
        assert_eq!(
            absent.notes,
            [
                "crate main is not in the dump",
                "crate gone is not in the dump"
            ]
        );
        world.dump.add_deleted("main");
        let deleted = world.run(&["main"], None);
        assert_eq!(deleted.signals.crate_deleted, Some(true));
        assert_eq!(deleted.notes, ["crate main is deleted from crates.io"]);
        let none = world.run(&[], None);
        assert_eq!(none.notes, ["the entry lists no crate"]);
    }

    #[test]
    fn repository_signals_come_from_the_facts() {
        let mut world = World::new();
        world.dump.add_crate(1, "main");
        let facts = RepoFacts::Found(found_facts());
        let found = world.run(&["main"], Some(&facts)).signals;
        assert_eq!(found.repo_gone, Some(false));
        assert_eq!(found.maintainer_replies_365, Some(7));
        assert_eq!(world.run(&["main"], None).signals.repo_gone, None);
    }
}
