//! The tier reasons of a check as plain sentences.
//!
//! Each function reads the exact text a tier rule writes. Text it does not recognise comes back
//! unchanged apart from its first letter and final stop, so no reason ever goes missing.

use catalogue_core::Thresholds;
use catalogue_core::amount_text::whole;

use crate::href::SafeHref;
use crate::pages::shared::{date_text, grouped};

/// What a failed gate writes before the repository route.
const AND_ROUTE: &str = " and repository route ";

/// What a passed reason writes before the repository route of a library.
const ROUTE_FIRST: &str = "repository route ";

/// What a passed reason writes before the repository route of an app.
const ROUTE_LAST: &str = ", repository route ";

/// One sentence of the tier block, with the source link it names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Sentence {
    /// The text.
    pub text: String,
    /// The link text and target that back the sentence.
    pub link: Option<(String, SafeHref)>,
}

impl Sentence {
    fn plain(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            link: None,
        }
    }
}

/// `detail` as a sentence: first letter raised, final stop added.
pub(super) fn verbatim(detail: &str) -> Sentence {
    let mut chars = detail.chars();
    let mut text: String = chars.next().map_or_else(String::new, |first| {
        first.to_uppercase().chain(chars).collect()
    });
    if !text.ends_with('.') {
        text.push('.');
    }
    Sentence::plain(text)
}

/// `value` of a share such as `0.75` as `75%`, or `value` itself when it is no number.
fn percent(value: &str) -> String {
    value.parse::<f64>().map_or_else(
        |_| value.to_owned(),
        |share| {
            let points = share * 100.0;
            if points.fract().abs() < f64::EPSILON {
                format!("{points:.0}%")
            } else {
                format!("{points:.1}%")
            }
        },
    )
}

/// `iso` as a person reads it, or `iso` itself when it is no date.
fn day(iso: &str) -> String {
    date_text(iso).unwrap_or_else(|| iso.to_owned())
}

/// `count` days as words.
fn days(count: &str) -> String {
    if count == "1" {
        "1 day".to_owned()
    } else {
        format!("{count} days")
    }
}

/// The sentence for one `instant-archive` reason.
pub(super) fn archive(detail: &str) -> Sentence {
    if detail == "repository is archived" {
        return Sentence::plain("GitHub reports the repository as archived.");
    }
    if let Some(id) = detail.strip_prefix("RustSec unmaintained advisory ") {
        return Sentence::plain(format!("RustSec lists the unmaintained advisory {id}."));
    }
    if let Some(notice) = detail.strip_prefix("deprecation notice: ") {
        return Sentence::plain(format!("The crate carries a deprecation notice: {notice}"));
    }
    match detail {
        "every version is yanked" => Sentence::plain("Every version is yanked on crates.io."),
        "crate is deleted" => Sentence::plain("The crate is deleted from crates.io."),
        _ => verbatim(detail),
    }
}

/// The sentence for one `dormancy` reason.
pub(super) fn dormancy(detail: &str) -> Sentence {
    if detail == "last release and last human commit unknown: dormancy unknown" {
        return Sentence::plain(
            "The check knows neither the latest release nor the latest human commit, so dormancy is unknown.",
        );
    }
    parse_dormancy(detail).unwrap_or_else(|| verbatim(detail))
}

fn parse_dormancy(detail: &str) -> Option<Sentence> {
    let rest = detail.strip_prefix("last activity ")?;
    let (date, rest) = rest.split_once(", ")?;
    let (idle, rest) = rest.split_once(" days ago, E = ")?;
    let (expected, outcome) = rest.split_once(": ")?;
    let start = format!("The last activity is on {}, {} ago.", day(date), days(idle));
    match outcome {
        "active" => Some(Sentence::plain(format!(
            "{start} The rules expect an update within {}.",
            days(expected)
        ))),
        "dormant" => Some(Sentence::plain(format!(
            "{start} That reaches the {} the rules expect, so the project is dormant.",
            days(expected)
        ))),
        _ => None,
    }
}

/// The sentence for one `rot` reason.
pub(super) fn rot(detail: &str) -> Sentence {
    if detail == "repository is gone" {
        return Sentence::plain("The repository no longer exists.");
    }
    parse_issue_rot(detail)
        .or_else(|| parse_advisory_rot(detail))
        .or_else(|| parse_glory(detail, "below the lost-glory limit of"))
        .or_else(|| parse_deprecated(detail))
        .or_else(|| parse_build(detail))
        .unwrap_or_else(|| verbatim(detail))
}

fn parse_issue_rot(detail: &str) -> Option<Sentence> {
    let rest = detail.strip_prefix("oldest unanswered issue ")?;
    let (waited, rest) = rest.split_once(" days (limit ")?;
    let (limit, rest) = rest.split_once("), no maintainer reply in ")?;
    let window = rest.strip_suffix(" days")?;
    Some(Sentence::plain(format!(
        "The oldest outside issue waits {} for a reply, past the limit of {limit}. No maintainer replied in {}.",
        days(waited),
        days(window)
    )))
}

fn parse_advisory_rot(detail: &str) -> Option<Sentence> {
    let rest = detail.strip_prefix("unpatched past ")?;
    let (grace, ids) = rest.split_once(" days: ")?;
    Some(Sentence::plain(format!(
        "A vulnerability advisory stays unpatched past {}: {ids}.",
        days(grace)
    )))
}

fn parse_glory(detail: &str, limit_words: &str) -> Option<Sentence> {
    let rest = detail.strip_prefix("former_glory ")?;
    let (share, limit) = rest.split_once(" < ")?;
    Some(Sentence::plain(format!(
        "Active dependents are at {} of their peak, {limit_words} {}.",
        percent(share),
        percent(limit)
    )))
}

/// `deprecated requirement `name (ID, https://…)``: the link becomes a real one.
fn parse_deprecated(detail: &str) -> Option<Sentence> {
    let inner = detail
        .strip_prefix("deprecated requirement `")?
        .strip_suffix('`')?;
    let Some((name, rest)) = inner.split_once(" (") else {
        return Some(Sentence::plain(format!(
            "The latest release requires the deprecated dependency {inner}."
        )));
    };
    let rest = rest.strip_suffix(')').unwrap_or(rest);
    let (label, url) = rest.rsplit_once(", ").unwrap_or(("source", rest));
    let href = SafeHref::new(url);
    if !url.starts_with("https://") || href.is_refused() {
        return Some(Sentence::plain(format!(
            "The latest release requires the deprecated dependency {inner}."
        )));
    }
    Some(Sentence {
        text: format!("The latest release requires the deprecated dependency {name}."),
        link: Some((label.to_owned(), href)),
    })
}

fn parse_build(detail: &str) -> Option<Sentence> {
    let rest = detail.strip_prefix("latest release fails `cargo check` on ")?;
    let (toolchain, checked) = rest.split_once(", checked ")?;
    Some(Sentence::plain(format!(
        "The latest release fails cargo check on {toolchain}. The check ran on {}.",
        day(checked)
    )))
}

/// The sentence for one `dead` reason.
pub(super) fn dead(detail: &str) -> Sentence {
    parse_dead(detail)
        .or_else(|| parse_dead_maintained(detail))
        .unwrap_or_else(|| verbatim(detail))
}

fn parse_dead(detail: &str) -> Option<Sentence> {
    let rest = detail.strip_prefix("dormant with ")?;
    let (count, rest) = rest.split_once(" rot signals (limit ")?;
    let (limit, rest) = rest.split_once("): dead on check ")?;
    let (check, rest) = rest.split_once(" of ")?;
    let (needed, outcome) = rest.split_once(", ")?;
    let end = match outcome {
        "archived" => "so the entry is archived",
        "not yet archived" => "so the entry is not yet archived",
        _ => return None,
    };
    Some(Sentence::plain(format!(
        "The project is dormant with {count} rot signals, which reaches the limit of {limit}. This is dead check {check} of {needed}, {end}."
    )))
}

fn parse_dead_maintained(detail: &str) -> Option<Sentence> {
    let rest = detail.strip_prefix("dormant with ")?;
    let count =
        rest.strip_suffix(" rot signals, but the maintainer states `maintained`: not dead")?;
    Some(Sentence::plain(format!(
        "The project is dormant with {count} rot signals. A maintainer states it is maintained, so it is not dead."
    )))
}

/// The sentence for one `finished` reason.
pub(super) fn finished(detail: &str) -> Sentence {
    if detail == "the maintainer states `finished` and no rot signal exists: stable" {
        return Sentence::plain(
            "A maintainer states the project is finished, and no rot signal exists.",
        );
    }
    let evidence = detail
        .strip_prefix("dormant, no rot, still adopted (")
        .and_then(|rest| rest.strip_suffix("): stable"));
    let Some(evidence) = evidence else {
        return verbatim(detail);
    };
    let words = if let Some(count) = evidence.strip_prefix("binary downloads per release ") {
        format!("{count} binary downloads per release")
    } else {
        let parts = evidence
            .strip_prefix("new_adopters ")
            .and_then(|rest| rest.split_once(", former_glory "));
        let Some((adopters, glory)) = parts else {
            return verbatim(detail);
        };
        format!(
            "{adopters} new dependents in 365 days, and active dependents at {} of their peak",
            percent(glory)
        )
    };
    Sentence::plain(format!(
        "The project is dormant with no rot signal and is still adopted: {words}."
    ))
}

/// The sentence for one `new` reason.
pub(super) fn new(detail: &str) -> Sentence {
    let parsed = detail
        .strip_prefix("new: first release ")
        .and_then(|rest| rest.split_once(" is under "))
        .and_then(|(date, rest)| Some((date, rest.strip_suffix(" days old")?)));
    match parsed {
        Some((date, limit)) => Sentence::plain(format!(
            "Its first release on {} is under {limit} days old.",
            day(date)
        )),
        None => verbatim(detail),
    }
}

/// One reason a `featured` rule gives for failing, with whether a rule other than Featured owns it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Lack {
    /// The sentence.
    pub sentence: Sentence,
    /// The sentence with no measured value, so two entries with the same lack read the same.
    pub verdict: String,
    /// Whether the reason is the dormancy condition.
    pub dormant: bool,
}

/// What a failed `featured` reason lacks. `None` when `detail` is not a failure.
///
/// `label` gives the reader's name of a signal.
pub(super) fn featured_lacks(detail: &str, label: impl Fn(&str) -> String) -> Option<Vec<Lack>> {
    let failures = detail.strip_prefix("not featured: ")?;
    if let Some(names) = failures.strip_prefix("unknown ") {
        let labels: Vec<String> = names.split(", ").map(label).collect();
        return Some(vec![Lack {
            sentence: Sentence::plain(format!(
                "The check holds no value for {}. A gate that lacks its signal does not pass.",
                labels.join(", ")
            )),
            verdict: "The check holds no value for a signal that a Featured gate reads.".to_owned(),
            dormant: false,
        }]);
    }
    Some(failures.split("; ").map(one_failure).collect())
}

fn one_failure(failure: &str) -> Lack {
    let dormant = failure == "dormant and not stable";
    let measured = if parse_adoption(failure).is_some() {
        Some(if failure.contains(AND_ROUTE) {
            "It has too few dependents for Featured, and it does not pass the repository route."
        } else {
            "It has too few active dependents and new dependents for Featured."
        })
    } else if parse_glory(failure, "below the Featured floor of").is_some() {
        Some("Its active dependents are too far below their peak for Featured.")
    } else if parse_use_signals(failure).is_some() {
        Some("Too few use signals pass for Featured.")
    } else {
        None
    };
    let sentence = match failure {
        "dormant and not stable" => Sentence::plain("The project is dormant and not finished."),
        "unpatched vulnerabilities exist" => {
            Sentence::plain("A vulnerability advisory stays unpatched past the grace window.")
        }
        "repository link not verified" => Sentence::plain(
            "The check finds no release commit on a branch or tag of the repository.",
        ),
        "docs missing" => {
            Sentence::plain("Docs are missing: the primary crate publishes no README.")
        }
        "every version is a prerelease or 0.0.x" => {
            Sentence::plain("Every version is a prerelease or 0.0.x.")
        }
        other => parse_adoption(other)
            .or_else(|| parse_glory(other, "below the Featured floor of"))
            .or_else(|| parse_use_signals(other))
            .unwrap_or_else(|| verbatim(other)),
    };
    Lack {
        verdict: measured.map_or_else(|| sentence.text.clone(), str::to_owned),
        sentence,
        dormant,
    }
}

/// One part of the repository route: its value and its limit.
struct RoutePart<'d> {
    value: &'d str,
    needs: &'d str,
}

impl<'d> RoutePart<'d> {
    /// `<value> (needs <limit>)`.
    fn parse(text: &'d str) -> Option<Self> {
        let (value, needs) = text.split_once(" (needs ")?;
        Some(Self {
            value,
            needs: needs.strip_suffix(')')?,
        })
    }

    /// The value with its noun, as a person reads it.
    fn value_words(&self, one: &str, many: &str) -> String {
        match self.value {
            "unknown" => format!("{many} not measured"),
            "1" => format!("1 {one}"),
            count => format!("{} {many}", grouped(count)),
        }
    }
}

/// The three parts of the repository route.
struct Route<'d> {
    stars: RoutePart<'d>,
    forks: RoutePart<'d>,
    contributors: RoutePart<'d>,
}

impl<'d> Route<'d> {
    /// The route as `stars <part>, forks <part>, contributors <part>`, as the Featured rule
    /// writes it after the words `repository route`.
    fn parse(text: &'d str) -> Option<Self> {
        let rest = text.strip_prefix("stars ")?;
        let (stars, rest) = rest.split_once(", forks ")?;
        let (forks, contributors) = rest.split_once(", contributors ")?;
        Some(Self {
            stars: RoutePart::parse(stars)?,
            forks: RoutePart::parse(forks)?,
            contributors: RoutePart::parse(contributors)?,
        })
    }

    /// The three values, as a person reads them.
    fn values(&self) -> String {
        format!(
            "{}, {} and {}",
            self.stars.value_words("star", "stars"),
            self.forks.value_words("fork", "forks"),
            self.contributors.value_words("contributor", "contributors")
        )
    }

    /// The sentence that states the three limits.
    fn limits(&self) -> String {
        format!(
            "It needs {} stars, {} forks and {} contributors together.",
            grouped(self.stars.needs),
            grouped(self.forks.needs),
            grouped(self.contributors.needs)
        )
    }
}

/// `failure` split into its own text and the route text after it, when it names the route.
fn split_route(failure: &str) -> (&str, Option<&str>) {
    match failure.split_once(AND_ROUTE) {
        Some((own, route)) => (own, Some(route)),
        None => (failure, None),
    }
}

/// `text` with the sentences of the failed route after it. `None` when the route does not parse.
fn with_route_lack(text: String, route: Option<&str>) -> Option<Sentence> {
    let Some(route) = route else {
        return Some(Sentence::plain(text));
    };
    let route = Route::parse(route)?;
    Some(Sentence::plain(format!(
        "{text} The repository route reads {}. {}",
        route.values(),
        route.limits()
    )))
}

fn parse_adoption(failure: &str) -> Option<Sentence> {
    let (own, route) = split_route(failure);
    let rest = own.strip_prefix("active_users ")?;
    let (users, rest) = rest.split_once(" < ")?;
    let (users_limit, rest) = rest.split_once(" and new_adopters ")?;
    let (adopters, adopters_limit) = rest.split_once(" < ")?;
    with_route_lack(
        format!(
            "It has {users} active dependents and {adopters} new dependents in 365 days. Featured needs {users_limit} active dependents or {adopters_limit} new dependents in 365 days."
        ),
        route,
    )
}

fn parse_use_signals(failure: &str) -> Option<Sentence> {
    let (own, route) = split_route(failure);
    let (passing, rest) = own.split_once(" of ")?;
    let (total, needed) = rest.split_once(" use signals pass, ")?;
    let needed = needed.strip_suffix(" needed")?;
    with_route_lack(
        format!("{passing} of {total} use signals pass. Featured needs {needed}."),
        route,
    )
}

/// What a passed `featured` reason of a library or an app meets. `None` when it did not pass.
pub(super) fn featured_meets(
    detail: &str,
    thresholds: Option<&Thresholds>,
) -> Option<Vec<Sentence>> {
    let values = detail.strip_prefix("featured: ")?;
    library_meets(values, thresholds).or_else(|| app_meets(values, thresholds))
}

/// The sentences of a library that passes on the repository route.
fn route_meets(route: &Route<'_>, users: &str, adopters: &str) -> Vec<Sentence> {
    let passes = Sentence::plain(format!(
        "It passes the repository route: {}. {} Stars alone never qualify a project.",
        route.values(),
        route.limits()
    ));
    let dependents = if users == "unknown" || adopters == "unknown" {
        "The check holds no count of its dependents.".to_owned()
    } else {
        let shown = |value: &str| whole(value).unwrap_or_else(|| value.to_owned());
        format!(
            "It has {} active dependents and {} new dependents in 365 days.",
            shown(users),
            shown(adopters)
        )
    };
    vec![passes, Sentence::plain(dependents)]
}

fn library_meets(values: &str, thresholds: Option<&Thresholds>) -> Option<Vec<Sentence>> {
    let (route, rest) = match values.strip_prefix(ROUTE_FIRST) {
        Some(after) => {
            let (route, rest) = after.split_once(", active_users ")?;
            (Some(Route::parse(route)?), rest)
        }
        None => (None, values.strip_prefix("active_users ")?),
    };
    let (users, rest) = rest.split_once(", new_adopters ")?;
    let (adopters, glory) = rest.split_once(", former_glory ")?;
    let shown = |value: &str| whole(value).unwrap_or_else(|| value.to_owned());
    let limits = thresholds.map(|t| &t.library_featured);
    let adoption_limit = limits.map_or_else(String::new, |t| {
        format!(
            " Featured needs {} active dependents or {} new dependents in 365 days.",
            t.min_active_users, t.min_new_adopters
        )
    });
    let glory_limit = limits.map_or_else(String::new, |t| {
        format!(
            " Featured needs at least {}.",
            percent(&t.min_former_glory.to_string())
        )
    });
    let adoption = match &route {
        Some(route) => route_meets(route, users, adopters),
        None => vec![Sentence::plain(format!(
            "It has {} active dependents and {} new dependents in 365 days.{adoption_limit}",
            shown(users),
            shown(adopters)
        ))],
    };
    let mut lines = adoption;
    lines.extend([
        Sentence::plain(format!(
            "Active dependents are at {} of their peak.{glory_limit}",
            percent(glory)
        )),
        Sentence::plain(
            "Docs are present. The release commit is on a branch or tag of the repository.",
        ),
        Sentence::plain(
            "No vulnerability advisory stays unpatched past the grace window. At least one version is neither a prerelease nor 0.0.x.",
        ),
    ]);
    Some(lines)
}

fn app_meets(values: &str, thresholds: Option<&Thresholds>) -> Option<Vec<Sentence>> {
    values.strip_prefix("binary downloads ")?;
    let (values, route) = match values.split_once(ROUTE_LAST) {
        Some((own, route)) => (own, Some(Route::parse(route)?)),
        None => (values, None),
    };
    let needed = thresholds.map_or_else(String::new, |t| {
        format!(
            " Featured needs {} of these to reach their limits.",
            t.app_featured.min_signals
        )
    });
    let mut lines = vec![Sentence::plain(format!(
        "Its use signals read: {values}.{needed}"
    ))];
    if let Some(route) = route {
        lines.push(Sentence::plain(format!(
            "It also passes the repository route, which counts as one use signal: {}. {}",
            route.values(),
            route.limits()
        )));
    }
    Some(lines)
}

#[cfg(test)]
mod tests {
    use catalogue_core::{Context, Date, EntryKind, Rule, Signals, assess};

    use super::*;
    use crate::pages::rules::fixtures::thresholds;

    fn text(sentence: Sentence) -> String {
        sentence.text
    }

    #[test]
    fn unknown_text_stays_as_it_is() {
        assert_eq!(text(verbatim("fails: x below y")), "Fails: x below y.");
        assert_eq!(
            text(archive("the repository is gone")),
            "The repository is gone."
        );
        assert_eq!(text(rot("something new")), "Something new.");
        assert_eq!(text(dormancy("something new")), "Something new.");
        assert_eq!(text(dead("something new")), "Something new.");
        assert_eq!(text(finished("something new")), "Something new.");
        assert_eq!(text(new("something new")), "Something new.");
    }

    #[test]
    fn each_archive_reason_reads_plainly() {
        assert_eq!(
            text(archive("repository is archived")),
            "GitHub reports the repository as archived."
        );
        assert_eq!(
            text(archive("RustSec unmaintained advisory RUSTSEC-2026-0196")),
            "RustSec lists the unmaintained advisory RUSTSEC-2026-0196."
        );
        assert_eq!(
            text(archive("every version is yanked")),
            "Every version is yanked on crates.io."
        );
        assert_eq!(
            text(archive("crate is deleted")),
            "The crate is deleted from crates.io."
        );
    }

    #[test]
    fn dormancy_states_the_date_and_the_interval() {
        assert_eq!(
            text(dormancy(
                "last activity 2026-11-03, 1 days ago, E = 1200: active"
            )),
            "The last activity is on 3 Nov 2026, 1 day ago. The rules expect an update within 1200 days."
        );
        assert_eq!(
            text(dormancy(
                "last activity 2024-01-01, 900 days ago, E = 365: dormant"
            )),
            "The last activity is on 1 Jan 2024, 900 days ago. That reaches the 365 days the rules expect, so the project is dormant."
        );
        assert!(
            text(dormancy(
                "last release and last human commit unknown: dormancy unknown"
            ))
            .ends_with("so dormancy is unknown.")
        );
    }

    #[test]
    fn rot_reasons_read_plainly_and_a_deprecated_requirement_links_its_advisory() {
        assert_eq!(
            text(rot(
                "oldest unanswered issue 709 days (limit 270), no maintainer reply in 365 days"
            )),
            "The oldest outside issue waits 709 days for a reply, past the limit of 270. No maintainer replied in 365 days."
        );
        assert_eq!(
            text(rot("repository is gone")),
            "The repository no longer exists."
        );
        assert_eq!(
            text(rot("former_glory 0.25 < 0.5")),
            "Active dependents are at 25% of their peak, below the lost-glory limit of 50%."
        );
        assert_eq!(
            text(rot("unpatched past 60 days: RUSTSEC-1, RUSTSEC-2")),
            "A vulnerability advisory stays unpatched past 60 days: RUSTSEC-1, RUSTSEC-2."
        );
        assert_eq!(
            text(rot(
                "latest release fails `cargo check` on 1.90.0, checked 2026-10-01"
            )),
            "The latest release fails cargo check on 1.90.0. The check ran on 1 Oct 2026."
        );
        let linked = rot(
            "deprecated requirement `paste (RUSTSEC-2024-0436, https://rustsec.org/advisories/RUSTSEC-2024-0436.html)`",
        );
        assert_eq!(
            linked.text,
            "The latest release requires the deprecated dependency paste."
        );
        let (label, href) = linked.link.expect("link");
        assert_eq!(label, "RUSTSEC-2024-0436");
        assert_eq!(
            href.as_str(),
            "https://rustsec.org/advisories/RUSTSEC-2024-0436.html"
        );
    }

    #[test]
    fn a_deprecated_requirement_never_links_anything_but_https() {
        let refused = rot("deprecated requirement `x (ID, javascript:alert(1))`");
        assert_eq!(refused.link, None);
        assert!(refused.text.contains("javascript:alert(1)"));
    }

    #[test]
    fn the_dead_rule_states_how_far_the_entry_is_from_archive() {
        assert_eq!(
            text(dead(
                "dormant with 2 rot signals (limit 2): dead on check 1 of 2, not yet archived"
            )),
            "The project is dormant with 2 rot signals, which reaches the limit of 2. This is dead check 1 of 2, so the entry is not yet archived."
        );
        assert_eq!(
            text(dead(
                "dormant with 3 rot signals, but the maintainer states `maintained`: not dead"
            )),
            "The project is dormant with 3 rot signals. A maintainer states it is maintained, so it is not dead."
        );
    }

    #[test]
    fn the_finished_rule_states_its_evidence() {
        assert_eq!(
            text(finished(
                "dormant, no rot, still adopted (new_adopters 3, former_glory 0.9): stable"
            )),
            "The project is dormant with no rot signal and is still adopted: 3 new dependents in 365 days, and active dependents at 90% of their peak."
        );
        assert_eq!(
            text(finished(
                "dormant, no rot, still adopted (binary downloads per release 900): stable"
            )),
            "The project is dormant with no rot signal and is still adopted: 900 binary downloads per release."
        );
        assert_eq!(
            text(finished(
                "the maintainer states `finished` and no rot signal exists: stable"
            )),
            "A maintainer states the project is finished, and no rot signal exists."
        );
    }

    #[test]
    fn the_new_rule_states_the_date_and_the_limit() {
        assert_eq!(
            text(new("new: first release 2026-05-30 is under 180 days old")),
            "Its first release on 30 May 2026 is under 180 days old."
        );
    }

    fn lacks(detail: &str) -> Vec<String> {
        featured_lacks(detail, |name| format!("<{name}>"))
            .expect("a failure")
            .into_iter()
            .map(|lack| lack.sentence.text)
            .collect()
    }

    #[test]
    fn a_failed_featured_reason_lists_each_gate_it_lacks() {
        assert_eq!(
            lacks(
                "not featured: active_users 2 < 11 and new_adopters 2 < 9; repository link not verified; docs missing; every version is a prerelease or 0.0.x; dormant and not stable"
            ),
            [
                "It has 2 active dependents and 2 new dependents in 365 days. Featured needs 11 active dependents or 9 new dependents in 365 days.",
                "The check finds no release commit on a branch or tag of the repository.",
                "Docs are missing: the primary crate publishes no README.",
                "Every version is a prerelease or 0.0.x.",
                "The project is dormant and not finished.",
            ]
        );
        assert_eq!(
            lacks("not featured: former_glory 0.3 < 0.5; unpatched vulnerabilities exist"),
            [
                "Active dependents are at 30% of their peak, below the Featured floor of 50%.",
                "A vulnerability advisory stays unpatched past the grace window.",
            ]
        );
        assert_eq!(
            lacks("not featured: 1 of 4 use signals pass, 2 needed"),
            ["1 of 4 use signals pass. Featured needs 2."]
        );
    }

    #[test]
    fn a_failed_route_states_every_number_and_an_unknown_part_reads_not_measured() {
        assert_eq!(
            lacks(
                "not featured: 1 of 5 use signals pass, 2 needed and repository route stars 1 (needs 2000), forks unknown (needs 200), contributors 25 (needs 25)"
            ),
            [
                "1 of 5 use signals pass. Featured needs 2. The repository route reads 1 star, forks not measured and 25 contributors. It needs 2,000 stars, 200 forks and 25 contributors together."
            ]
        );
    }

    #[test]
    fn a_route_text_that_does_not_parse_stays_as_it_is() {
        assert_eq!(
            lacks(
                "not featured: active_users 2 < 11 and new_adopters 2 < 9 and repository route x"
            ),
            ["Active_users 2 < 11 and new_adopters 2 < 9 and repository route x."]
        );
    }

    /// A library that passes every gate but the adoption gate, with these repository numbers.
    fn low_dependents(stars: u64, forks: u64, contributors: u64) -> Signals {
        let day = |year, month, on| Date::new(year, month, on).ok();
        Signals {
            first_release: day(2023, 1, 1),
            latest_version: Some("1.2.5".to_owned()),
            release_count: Some(30),
            last_release: day(2026, 9, 1),
            all_yanked: Some(false),
            crate_deleted: Some(false),
            all_prerelease: Some(false),
            active_users: Some(1.0),
            new_adopters: Some(0.0),
            peak_users: Some(1.0),
            former_glory: Some(1.0),
            downloads_per_month: Some(100),
            unpatched_vulnerabilities: Some(Vec::new()),
            repo_archived: Some(false),
            repo_gone: Some(false),
            last_human_commit: day(2026, 10, 20),
            oldest_unanswered_days: Some(0),
            maintainer_replies_365: Some(1),
            stars: Some(stars),
            forks: Some(forks),
            contributors: Some(contributors),
            docs_present: Some(true),
            repo_link_verified: Some(true),
            ..Signals::unknown(EntryKind::Library)
        }
    }

    /// The detail the Featured rule writes for `signals`.
    fn rule_text(signals: &Signals) -> String {
        let today = Date::new(2026, 11, 3).expect("date");
        let verdict = assess(signals, &Context::new(today), &thresholds(None));
        verdict
            .reasons
            .into_iter()
            .find(|reason| reason.rule == Rule::Featured)
            .map(|reason| reason.detail)
            .expect("a Featured reason")
    }

    #[test]
    fn the_sentences_read_the_exact_text_the_rule_writes() {
        let route = thresholds(None).repo_route;
        let passed = rule_text(&low_dependents(
            route.min_stars,
            route.min_forks,
            route.min_contributors,
        ));
        let meets = featured_meets(&passed, None).expect("a pass");
        assert_eq!(
            meets[0].text,
            "It passes the repository route: 2,000 stars, 200 forks and 25 contributors. It needs 2,000 stars, 200 forks and 25 contributors together. Stars alone never qualify a project."
        );
        assert_eq!(
            meets[1].text,
            "It has 1 active dependents and 0 new dependents in 365 days."
        );
        let failed = rule_text(&low_dependents(route.min_stars, 0, 0));
        let lines = lacks(&failed);
        assert_eq!(
            lines,
            [
                "It has 1 active dependents and 0 new dependents in 365 days. Featured needs 11 active dependents or 9 new dependents in 365 days. The repository route reads 2,000 stars, 0 forks and 0 contributors. It needs 2,000 stars, 200 forks and 25 contributors together."
            ]
        );
    }

    #[test]
    fn an_unknown_gate_names_the_signals_it_lacks() {
        assert_eq!(
            lacks("not featured: unknown homebrew_on_request_365d, repology_families"),
            [
                "The check holds no value for <homebrew_on_request_365d>, <repology_families>. A gate that lacks its signal does not pass."
            ]
        );
    }

    #[test]
    fn a_passed_reason_is_no_failure_and_a_failure_is_no_pass() {
        assert_eq!(
            featured_lacks(
                "featured: active_users 3, new_adopters 2, former_glory 1",
                str::to_owned
            ),
            None
        );
        assert_eq!(featured_meets("not featured: docs missing", None), None);
    }

    #[test]
    fn a_passed_featured_reason_states_the_values() {
        let meets = featured_meets(
            "featured: active_users 39.5, new_adopters 23, former_glory 0.99",
            None,
        )
        .expect("a pass");
        assert_eq!(
            meets[0].text,
            "It has 40 active dependents and 23 new dependents in 365 days."
        );
        assert_eq!(meets[1].text, "Active dependents are at 99% of their peak.");
        let app = featured_meets(
            "featured: binary downloads 900, Homebrew 600, Repology families unknown, Docker growth unknown",
            None,
        )
        .expect("a pass");
        assert_eq!(
            app[0].text,
            "Its use signals read: binary downloads 900, Homebrew 600, Repology families unknown, Docker growth unknown."
        );
        let on_route = featured_meets(
            "featured: binary downloads 900, Homebrew 0, Repology families 0, Docker growth 0, repository route stars 2000 (needs 2000), forks 200 (needs 200), contributors 25 (needs 25)",
            None,
        )
        .expect("a pass");
        assert_eq!(
            on_route[0].text,
            "Its use signals read: binary downloads 900, Homebrew 0, Repology families 0, Docker growth 0."
        );
        assert_eq!(
            on_route[1].text,
            "It also passes the repository route, which counts as one use signal: 2,000 stars, 200 forks and 25 contributors. It needs 2,000 stars, 200 forks and 25 contributors together."
        );
    }
}
