//! The "Why <tier>" block: the verdict's real reasons as plain sentences, with the exact rules.

use catalogue_core::{EntryKind, EntryReport, Rule, Thresholds, Tier};

use super::reason_text::{self, Sentence, featured_lacks, featured_meets, verbatim};
use super::sources::{SourceNote, distinct_sources, fact_named};
use crate::href::SafeHref;
use crate::pages::shared::{TierBadge, tier_name};

/// The signals behind each `instant-archive` reason.
const ARCHIVE_SIGNALS: [&str; 5] = [
    "repo_archived",
    "rustsec_unmaintained",
    "deprecation_notice",
    "all_yanked",
    "crate_deleted",
];

/// The verdict of a New entry. The block's lead line names the age and the limit.
const NEW_VERDICT: &str = "The first release is under the age limit for the Featured gates.";

/// The word that names a rule, and the link to it on the rules page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RuleLink {
    /// The rule name as the rules page heads it.
    pub name: &'static str,
    /// The link to the rule.
    pub href: SafeHref,
}

impl RuleLink {
    fn to(name: &'static str, anchor: &str) -> Self {
        Self {
            name,
            href: SafeHref::new(&format!("/rules/#{anchor}")),
        }
    }

    fn dormancy() -> Self {
        Self::to("Dormancy", "dormancy")
    }

    fn featured(kind: EntryKind) -> Self {
        match kind {
            EntryKind::Library => Self::to("Featured library", "featured-library"),
            EntryKind::App => Self::to("Featured app", "featured-app"),
        }
    }
}

/// One line of the tier block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct WhyLine {
    /// The sentence.
    pub text: String,
    /// The link text and target that back the sentence.
    pub link: Option<(String, SafeHref)>,
    /// The rule behind the line, when the block's own rules do not cover it.
    pub rule: Option<RuleLink>,
}

impl WhyLine {
    fn new(sentence: Sentence, rule: Option<RuleLink>) -> Self {
        Self {
            text: sentence.text,
            link: sentence.link,
            rule,
        }
    }
}

/// The data of the "Why <tier>" block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Why<'a> {
    /// The tier badge.
    pub badge: TierBadge,
    /// Whether the entry is Archived. The page then leads with this block.
    pub archived: bool,
    /// The plain statement that opens the block.
    pub lead: Vec<String>,
    /// What the entry meets. Featured only.
    pub meets: Vec<WhyLine>,
    /// What the entry lacks for Featured.
    pub lacks: Vec<WhyLine>,
    /// What else the check found: activity, finished state and rot signals.
    pub found: Vec<WhyLine>,
    /// The exact rules of the tier.
    pub rules: Vec<RuleLink>,
    /// The sources of the rule inputs.
    pub sources: Vec<SourceNote<'a>>,
    /// The first lack, with no measured value. `None` when the entry lacks nothing the rule names.
    pub first_lack: Option<String>,
}

impl<'a> Why<'a> {
    /// The block for `record` of an entry of `kind`.
    pub(crate) fn new(
        record: &'a EntryReport,
        kind: EntryKind,
        thresholds: Option<&Thresholds>,
    ) -> Self {
        let mut why = Self {
            badge: TierBadge::of(record.tier),
            archived: record.tier == Tier::Archived,
            lead: Vec::new(),
            meets: Vec::new(),
            lacks: Vec::new(),
            found: Vec::new(),
            rules: Vec::new(),
            sources: Vec::new(),
            first_lack: None,
        };
        let mut activity = None;
        for reason in &record.reasons {
            let detail = reason.detail.as_str();
            match reason.rule {
                Rule::Dormancy => {
                    activity = Some(WhyLine::new(
                        reason_text::dormancy(detail),
                        Some(RuleLink::dormancy()),
                    ));
                }
                Rule::Rot => why.found.push(WhyLine::new(
                    reason_text::rot(detail),
                    Some(RuleLink::to("Rot signals", "rot")),
                )),
                Rule::Dead => why.found.push(WhyLine::new(
                    reason_text::dead(detail),
                    Some(RuleLink::to("Dead", "dead")),
                )),
                Rule::Finished => why.found.push(WhyLine::new(
                    reason_text::finished(detail),
                    Some(RuleLink::to("Finished", "finished")),
                )),
                Rule::Featured => why.read_featured(record, thresholds, detail),
                Rule::FeaturedNeedsOutsideApproval => why.lacks.push(WhyLine::new(
                    Sentence {
                        text: "A featured-approval judgment from a maintainer outside the organization. It passes every other Featured gate.".to_owned(),
                        link: None,
                    },
                    Some(RuleLink::to("Outside approval", "outside-approval")),
                )),
                Rule::InstantArchive => why.lead.push(reason_text::archive(detail).text),
                Rule::New => why.lead.push(reason_text::new(detail).text),
                Rule::Listed | Rule::ExpectedInterval | Rule::DemotionPending => {}
            }
        }
        why.finish(record, kind, activity);
        why
    }

    /// Sorts the `featured` reason into what the entry meets or lacks.
    fn read_featured(
        &mut self,
        record: &EntryReport,
        thresholds: Option<&Thresholds>,
        detail: &str,
    ) {
        if let Some(meets) = featured_meets(detail, thresholds) {
            self.meets
                .extend(meets.into_iter().map(|line| WhyLine::new(line, None)));
            return;
        }
        let label = |name: &str| {
            fact_named(record, name).map_or_else(|| name.to_owned(), |fact| fact.label.clone())
        };
        let Some(lacks) = featured_lacks(detail, label) else {
            self.lacks.push(WhyLine::new(verbatim(detail), None));
            return;
        };
        self.first_lack = lacks.first().map(|lack| lack.verdict.clone());
        self.lacks.extend(lacks.into_iter().map(|lack| {
            let rule = lack.dormant.then(RuleLink::dormancy);
            WhyLine::new(lack.sentence, rule)
        }));
    }

    /// Adds the lead, the activity line, the rules and the sources once every reason is read.
    fn finish(&mut self, record: &'a EntryReport, kind: EntryKind, activity: Option<WhyLine>) {
        let reasons = |rule: Rule| record.reasons.iter().any(|reason| reason.rule == rule);
        match record.tier {
            Tier::Featured => {
                if let Some(lower) = record.demotion_pending {
                    self.lead.push(format!(
                        "A demotion to {} waits for approval. The entry stays Featured until a demotion judgment is approved.",
                        tier_name(lower)
                    ));
                } else {
                    self.lead.push("It passes every Featured gate.".to_owned());
                }
                self.rules.push(RuleLink::featured(kind));
                if record.demotion_pending.is_some() {
                    self.rules
                        .push(RuleLink::to("Demotion guard", "demotion-guard"));
                }
            }
            Tier::Listed => {
                self.lead
                    .push(if reasons(Rule::FeaturedNeedsOutsideApproval) {
                        "It passes the Featured gates and waits for outside approval.".to_owned()
                    } else {
                        "No archive, New or Featured rule places it, so it is Listed.".to_owned()
                    });
                self.rules.push(RuleLink::to("Listed", "listed"));
                self.rules.push(RuleLink::featured(kind));
            }
            Tier::New => {
                self.lead.push(
                    "The check tests the Featured gates once the first release passes this age limit."
                        .to_owned(),
                );
                self.rules.push(RuleLink::to("New", "new"));
            }
            Tier::Archived => {
                if reasons(Rule::InstantArchive) {
                    self.rules
                        .push(RuleLink::to("Instant archive", "instant-archive"));
                }
                if reasons(Rule::Dead) {
                    self.rules.push(RuleLink::to("Dead", "dead"));
                }
                if self.lead.is_empty() {
                    self.lead.push("The rules archive this entry.".to_owned());
                }
            }
        }
        if let Some(line) = activity {
            if record.tier == Tier::Featured && record.demotion_pending.is_none() {
                self.meets.push(line);
            } else {
                self.found.insert(0, line);
            }
        }
        let fired = if record.tier == Tier::Archived && reasons(Rule::InstantArchive) {
            distinct_sources(
                ARCHIVE_SIGNALS
                    .iter()
                    .filter_map(|name| fact_named(record, name))
                    .filter(|fact| archive_signal_fired(fact.name.as_str(), fact.value.as_deref())),
            )
        } else {
            Vec::new()
        };
        self.sources = if fired.is_empty() {
            distinct_sources(record.signals.iter().filter(|fact| fact.counts_toward_tier))
        } else {
            fired
        };
    }
}

/// The sentences of the "Why <tier>" block, for a reader away from the page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct WhyText {
    /// The one line that says why the entry has its tier. It holds no measured value when the
    /// rule text allows it, so two entries in the same place read the same.
    pub verdict: String,
    /// The plain statement that opens the block.
    pub lead: Vec<String>,
    /// What the entry meets. Featured only.
    pub meets: Vec<String>,
    /// What the entry lacks for Featured.
    pub lacks: Vec<String>,
    /// What else the check found.
    pub found: Vec<String>,
}

impl WhyText {
    /// The sentences for `record` of an entry of `kind`.
    pub(crate) fn of(
        record: &EntryReport,
        kind: EntryKind,
        thresholds: Option<&Thresholds>,
    ) -> Self {
        let why = Why::new(record, kind, thresholds);
        let waits_for_approval = record
            .reasons
            .iter()
            .any(|reason| reason.rule == Rule::FeaturedNeedsOutsideApproval);
        let lead = why.lead.first().cloned().unwrap_or_default();
        let verdict = match (record.tier, why.first_lack) {
            (Tier::New, _) => NEW_VERDICT.to_owned(),
            (Tier::Listed, Some(lack)) if !waits_for_approval => lack,
            _ => lead,
        };
        let texts = |lines: Vec<WhyLine>| lines.into_iter().map(|line| line.text).collect();
        Self {
            verdict,
            lead: why.lead,
            meets: texts(why.meets),
            lacks: texts(why.lacks),
            found: texts(why.found),
        }
    }
}

/// Whether the value of an instant archive signal is the one that archives.
fn archive_signal_fired(name: &str, value: Option<&str>) -> bool {
    match name {
        "repo_archived" | "all_yanked" | "crate_deleted" => value == Some("true"),
        _ => value.is_some_and(|value| value != "none"),
    }
}

#[cfg(test)]
mod tests {
    use catalogue_core::{Date, ReasonRecord, SignalFact, SourceRef};
    use url::Url;

    use super::*;

    fn reason(rule: Rule, detail: &str) -> ReasonRecord {
        ReasonRecord {
            rule,
            detail: detail.to_owned(),
        }
    }

    fn fact(name: &str, value: Option<&str>, source: &str, counts: bool) -> SignalFact {
        SignalFact {
            name: name.to_owned(),
            label: name.to_uppercase(),
            value: value.map(str::to_owned),
            source: Some(SourceRef::new(
                source,
                Url::parse("https://example.org/").expect("url"),
            )),
            fetched: Date::new(2026, 11, 3).ok(),
            counts_toward_tier: counts,
        }
    }

    fn record(tier: Tier, reasons: Vec<ReasonRecord>, signals: Vec<SignalFact>) -> EntryReport {
        EntryReport {
            tier,
            stable: false,
            dormant: Some(false),
            demotion_pending: None,
            reasons,
            unknown: Vec::new(),
            notes: Vec::new(),
            signals,
        }
    }

    fn texts(lines: &[WhyLine]) -> Vec<&str> {
        lines.iter().map(|line| line.text.as_str()).collect()
    }

    #[test]
    fn the_verdict_of_two_listed_entries_with_the_same_lack_reads_the_same() {
        let verdict = |users: &str| {
            let detail = format!(
                "not featured: active_users {users} < 11 and new_adopters {users} < 9; docs missing"
            );
            let found = record(
                Tier::Listed,
                vec![reason(Rule::Featured, &detail)],
                Vec::new(),
            );
            WhyText::of(&found, EntryKind::Library, None).verdict
        };
        assert_eq!(verdict("2"), verdict("5"));
        assert_eq!(
            verdict("2"),
            "It has too few active users and new adopters for Featured."
        );
    }

    #[test]
    fn the_verdict_follows_the_tier() {
        let of = |tier, reasons| {
            WhyText::of(&record(tier, reasons, Vec::new()), EntryKind::Library, None).verdict
        };
        assert_eq!(
            of(Tier::Featured, Vec::new()),
            "It passes every Featured gate."
        );
        assert_eq!(of(Tier::New, Vec::new()), NEW_VERDICT);
        assert_eq!(
            of(Tier::Listed, Vec::new()),
            "No archive, New or Featured rule places it, so it is Listed."
        );
        assert_eq!(
            of(
                Tier::Listed,
                vec![reason(Rule::FeaturedNeedsOutsideApproval, "needs approval")],
            ),
            "It passes the Featured gates and waits for outside approval."
        );
        assert_eq!(
            of(
                Tier::Listed,
                vec![reason(
                    Rule::Featured,
                    "not featured: dormant and not stable"
                )],
            ),
            "The project is dormant and not finished."
        );
    }

    #[test]
    fn a_listed_entry_states_exactly_what_it_lacks_for_featured() {
        let record = record(
            Tier::Listed,
            vec![
                reason(
                    Rule::Dormancy,
                    "last activity 2026-11-01, 3 days ago, E = 365: active",
                ),
                reason(
                    Rule::Featured,
                    "not featured: active_users 2 < 11 and new_adopters 2 < 9; docs missing",
                ),
                reason(
                    Rule::Listed,
                    "listed: no archive, new or featured rule applies",
                ),
            ],
            Vec::new(),
        );
        let why = Why::new(&record, EntryKind::Library, None);
        assert_eq!(
            why.lead,
            ["No archive, New or Featured rule places it, so it is Listed."]
        );
        assert_eq!(
            texts(&why.lacks),
            [
                "It has 2 active users and 2 new adopters. Featured needs 11 active users or 9 new adopters.",
                "Docs are missing: the primary crate publishes no README.",
            ]
        );
        assert!(why.meets.is_empty());
        assert_eq!(
            texts(&why.found),
            [
                "The last activity is on 1 Nov 2026, 3 days ago. The rules expect an update within 365 days."
            ]
        );
        let anchors: Vec<&str> = why.rules.iter().map(|rule| rule.href.as_str()).collect();
        assert_eq!(anchors, ["/rules/#listed", "/rules/#featured-library"]);
    }

    #[test]
    fn a_listed_app_links_the_app_rule_and_names_unknown_signals_by_label() {
        let record = record(
            Tier::Listed,
            vec![reason(
                Rule::Featured,
                "not featured: unknown repology_families",
            )],
            vec![fact("repology_families", None, "x", true)],
        );
        let why = Why::new(&record, EntryKind::App, None);
        assert!(
            why.lacks[0].text.contains("REPOLOGY_FAMILIES"),
            "{:?}",
            why.lacks
        );
        assert_eq!(why.rules[1].href.as_str(), "/rules/#featured-app");
    }

    #[test]
    fn a_featured_entry_states_what_it_meets_and_keeps_rot_as_a_finding() {
        let record = record(
            Tier::Featured,
            vec![
                reason(
                    Rule::Dormancy,
                    "last activity 2026-11-01, 3 days ago, E = 1200: active",
                ),
                reason(Rule::Rot, "repository is gone"),
                reason(
                    Rule::Featured,
                    "featured: active_users 312, new_adopters 289, former_glory 1",
                ),
            ],
            Vec::new(),
        );
        let why = Why::new(&record, EntryKind::Library, None);
        assert_eq!(why.lead, ["It passes every Featured gate."]);
        assert_eq!(
            why.meets[0].text,
            "It has 312 active users and 289 new adopters."
        );
        assert!(
            why.meets
                .last()
                .expect("activity")
                .text
                .contains("1 Nov 2026")
        );
        assert_eq!(texts(&why.found), ["The repository no longer exists."]);
        assert!(why.lacks.is_empty());
    }

    #[test]
    fn a_held_featured_entry_states_the_demotion_and_what_it_lacks() {
        let mut held = record(
            Tier::Featured,
            vec![reason(Rule::Featured, "not featured: docs missing")],
            Vec::new(),
        );
        held.demotion_pending = Some(Tier::Listed);
        let why = Why::new(&held, EntryKind::Library, None);
        assert!(why.lead[0].starts_with("A demotion to Listed waits for approval."));
        assert_eq!(why.lacks.len(), 1);
        assert!(
            why.rules
                .iter()
                .any(|rule| rule.href.as_str() == "/rules/#demotion-guard")
        );
    }

    #[test]
    fn a_new_entry_says_why_and_what_comes_next() {
        let record = record(
            Tier::New,
            vec![reason(
                Rule::New,
                "new: first release 2026-05-30 is under 180 days old",
            )],
            Vec::new(),
        );
        let why = Why::new(&record, EntryKind::Library, None);
        assert_eq!(
            why.lead[0],
            "Its first release on 30 May 2026 is under 180 days old."
        );
        assert_eq!(why.rules[0].href.as_str(), "/rules/#new");
    }

    #[test]
    fn an_archived_entry_cites_only_the_signal_that_archived_it() {
        let record = record(
            Tier::Archived,
            vec![reason(Rule::InstantArchive, "repository is archived")],
            vec![
                fact("repo_archived", Some("true"), "GitHub repository", true),
                fact("all_yanked", Some("false"), "crates.io", true),
                fact("first_release", Some("2020-01-01"), "crates.io", true),
            ],
        );
        let why = Why::new(&record, EntryKind::Library, None);
        assert!(why.archived);
        assert_eq!(why.lead, ["GitHub reports the repository as archived."]);
        let names: Vec<&str> = why.sources.iter().map(|source| source.name).collect();
        assert_eq!(names, ["GitHub repository"]);
    }

    #[test]
    fn a_dead_archive_keeps_its_rule_and_cites_every_input() {
        let record = record(
            Tier::Archived,
            vec![reason(
                Rule::Dead,
                "dormant with 2 rot signals (limit 2): dead on check 2 of 2, archived",
            )],
            vec![fact("repo_gone", Some("false"), "GitHub repository", true)],
        );
        let why = Why::new(&record, EntryKind::Library, None);
        assert_eq!(why.rules[0].href.as_str(), "/rules/#dead");
        assert!(why.found[0].text.contains("so the entry is archived"));
        assert_eq!(why.sources.len(), 1);
    }

    #[test]
    fn signals_outside_the_rules_are_not_rule_sources() {
        let record = record(
            Tier::Listed,
            Vec::new(),
            vec![
                fact("stars", Some("9"), "GitHub", false),
                fact("docs_present", Some("true"), "dump", true),
            ],
        );
        let why = Why::new(&record, EntryKind::Library, None);
        let names: Vec<&str> = why.sources.iter().map(|source| source.name).collect();
        assert_eq!(names, ["dump"]);
    }
}
