//! `changes/feed.xml`: an Atom feed of tier changes and new entries.
//!
//! Every date comes from the history, so the same inputs give the same bytes. The rows of the
//! first check become one summary item, because every entry appears on that date.

use askama::Template;
use catalogue_core::Date;

use crate::build::{OutputFile, render};
use crate::error::SiteError;
use crate::href::SafeHref;
use crate::inputs::{SiteInputs, TierChangeRow};
use crate::pages::{SITE_NAME, SITE_URL, first_check_line, is_first_check, tier_name};

/// The path of the feed below the site root.
const FEED_FILE: &str = "changes/feed.xml";

/// The most items the feed holds.
const MAX_ITEMS: usize = 100;

/// The `tag:` authority of every item id. It never changes, so ids stay stable.
const TAG_AUTHORITY: &str = "ml-rust.github.io";

/// The `updated` date of a feed with no dated item. Atom requires the element.
const NO_DATE: &str = "1970-01-01T00:00:00Z";

/// The data `feed.xml` reads.
#[derive(Debug, Template)]
#[template(path = "feed.xml")]
struct Feed {
    title: String,
    id: String,
    self_href: SafeHref,
    page_href: SafeHref,
    updated: String,
    author: &'static str,
    items: Vec<FeedItem>,
}

/// One item of the feed.
#[derive(Debug, Clone, PartialEq, Eq)]
struct FeedItem {
    title: String,
    id: String,
    href: SafeHref,
    updated: String,
    summary: String,
}

/// `date` in the form Atom requires.
fn timestamp(date: Date) -> String {
    format!("{date}T00:00:00Z")
}

/// `text` without the characters XML 1.0 forbids. The template escapes the rest.
fn xml_text(text: &str) -> String {
    text.chars()
        .filter(|&c| {
            matches!(c, '\t' | '\n' | '\r') || (c >= ' ' && c != '\u{FFFE}' && c != '\u{FFFF}')
        })
        .collect()
}

/// The absolute link to a site path.
fn site_href(path: &str) -> SafeHref {
    SafeHref::new(&format!("{SITE_URL}{path}"))
}

/// The item of one tier change after the first check.
fn change_item(inputs: &SiteInputs<'_>, change: &TierChangeRow) -> FeedItem {
    let entry = inputs.catalogue.entry(&change.slug);
    let name = xml_text(entry.map_or(change.slug.as_str(), |entry| entry.name.as_str()));
    let to = tier_name(change.to);
    let date = change.date;
    let (title, summary) = match change.from {
        Some(from) => {
            let from = tier_name(from);
            (
                format!("{name}: {from} to {to}"),
                format!("The check on {date} moved {name} from {from} to {to}."),
            )
        }
        None => (
            format!("New entry: {name}"),
            format!("The check on {date} first saw {name}, in tier {to}."),
        ),
    };
    FeedItem {
        title,
        id: format!(
            "tag:{TAG_AUTHORITY},{date}:tier-change/{slug}",
            slug = change.slug
        ),
        href: match entry {
            Some(_) => site_href(&format!("/projects/{}/", change.slug)),
            None => site_href("/changes/"),
        },
        updated: timestamp(date),
        summary,
    }
}

/// The one item that stands for every row of the first check.
fn first_check_item(inputs: &SiteInputs<'_>, date: Date) -> Option<FeedItem> {
    let line = first_check_line(inputs.changes, Some(date))?;
    Some(FeedItem {
        title: format!("First check: {}", line.text),
        id: format!("tag:{TAG_AUTHORITY},{date}:first-check"),
        href: site_href("/changes/"),
        updated: timestamp(date),
        summary: format!(
            "The first check on {date} placed {}. Later items name each change.",
            line.text
        ),
    })
}

/// The items of `inputs`, newest first, with one date in slug order. Empty without a report.
fn items(inputs: &SiteInputs<'_>) -> Vec<FeedItem> {
    if inputs.report.is_none() {
        return Vec::new();
    }
    let mut sorted: Vec<&TierChangeRow> = inputs.changes.iter().collect();
    sorted.sort_by(|a, b| b.date.cmp(&a.date).then_with(|| a.slug.cmp(&b.slug)));
    let mut out = Vec::new();
    let mut summarized = false;
    for change in sorted {
        if !is_first_check(change, inputs.first_check) {
            out.push(change_item(inputs, change));
        } else if !summarized {
            summarized = true;
            out.extend(first_check_item(inputs, change.date));
        }
    }
    out.truncate(MAX_ITEMS);
    out
}

/// The feed file of `inputs`.
///
/// # Errors
///
/// Returns [`SiteError::Render`] when the template fails.
pub(crate) fn feed_file(inputs: &SiteInputs<'_>) -> Result<OutputFile, SiteError> {
    let items = items(inputs);
    let updated = items
        .first()
        .map(|item| item.updated.clone())
        .or_else(|| inputs.report.map(|report| timestamp(report.generated)));
    let feed = Feed {
        title: format!("{SITE_NAME}: tier changes and new entries"),
        id: format!("{SITE_URL}/{FEED_FILE}"),
        self_href: site_href(&format!("/{FEED_FILE}")),
        page_href: site_href("/changes/"),
        updated: updated.unwrap_or_else(|| NO_DATE.to_owned()),
        author: SITE_NAME,
        items,
    };
    render(FEED_FILE.to_owned(), &feed)
}

#[cfg(test)]
mod tests {
    use catalogue_core::{Catalogue, Movement, Slug, Tier, TierReport};

    use super::*;
    use crate::testing::{catalogue, entry, record, report};

    static NO_MOVEMENT: Movement = Movement::NeedsHistory;

    fn day(month: i8, on: i8) -> Date {
        Date::new(2026, month, on).expect("date")
    }

    fn change(month: i8, on: i8, slug: &str, from: Option<Tier>, to: Tier) -> TierChangeRow {
        TierChangeRow {
            date: day(month, on),
            slug: Slug::new(slug).expect("slug"),
            from,
            to,
        }
    }

    fn feed(
        data: &Catalogue,
        tiers: Option<&TierReport>,
        changes: &[TierChangeRow],
        first_check: Option<Date>,
    ) -> String {
        let file = feed_file(&SiteInputs {
            catalogue: data,
            report: tiers,
            changes,
            first_check,
            movement: &NO_MOVEMENT,
        })
        .expect("renders");
        assert_eq!(file.path, "changes/feed.xml");
        String::from_utf8(file.bytes).expect("UTF-8")
    }

    fn base() -> (Catalogue, TierReport) {
        let data = catalogue(vec![entry("alpha", ""), entry("beta", "")]);
        let tiers = report(vec![
            ("alpha", record(Tier::Listed, None)),
            ("beta", record(Tier::Listed, None)),
        ]);
        (data, tiers)
    }

    #[test]
    fn the_feed_has_every_element_atom_requires() {
        let (data, tiers) = base();
        let changes = [change(10, 5, "alpha", Some(Tier::New), Tier::Listed)];
        let text = feed(&data, Some(&tiers), &changes, None);
        for part in [
            "<?xml version=\"1.0\" encoding=\"utf-8\"?>",
            "<feed xmlns=\"http://www.w3.org/2005/Atom\">",
            "<id>https://ml-rust.github.io/changes/feed.xml</id>",
            "<link rel=\"self\" type=\"application/atom+xml\" href=\"https://ml-rust.github.io/changes/feed.xml\"/>",
            "<updated>2026-10-05T00:00:00Z</updated>",
            "<author><name>Rust ML catalogue</name></author>",
            "<title>alpha: New to Listed</title>",
            "<id>tag:ml-rust.github.io,2026-10-05:tier-change/alpha</id>",
            "href=\"https://ml-rust.github.io/projects/alpha/\"/>",
        ] {
            assert!(text.contains(part), "missing {part} in {text}");
        }
    }

    #[test]
    fn items_come_newest_first_and_a_later_first_row_is_a_new_entry() {
        let (data, tiers) = base();
        let changes = [
            change(10, 5, "alpha", Some(Tier::New), Tier::Listed),
            change(11, 1, "beta", None, Tier::New),
        ];
        let text = feed(&data, Some(&tiers), &changes, Some(day(9, 1)));
        let new = text.find("New entry: beta").expect("new entry");
        let moved = text.find("alpha: New to Listed").expect("change");
        assert!(new < moved, "{text}");
    }

    #[test]
    fn the_first_check_is_one_summary_item() {
        let (data, tiers) = base();
        let changes = [
            change(10, 5, "alpha", Some(Tier::New), Tier::Listed),
            change(9, 1, "alpha", None, Tier::New),
            change(9, 1, "beta", None, Tier::Listed),
        ];
        let text = feed(&data, Some(&tiers), &changes, Some(day(9, 1)));
        assert_eq!(text.matches("<entry>").count(), 2, "{text}");
        assert!(
            text.contains("<title>First check: 2 entries: Listed 1, New 1</title>"),
            "{text}"
        );
        assert!(
            text.contains("<id>tag:ml-rust.github.io,2026-09-01:first-check</id>"),
            "{text}"
        );
        assert!(!text.contains("New entry:"), "{text}");
    }

    #[test]
    fn the_feed_stops_at_the_cap_and_keeps_the_newest() {
        let (data, tiers) = base();
        let mut changes = Vec::new();
        for month in 1..=6 {
            for on in 1..=28 {
                changes.push(change(month, on, "alpha", Some(Tier::New), Tier::Listed));
            }
        }
        let text = feed(&data, Some(&tiers), &changes, None);
        assert_eq!(text.matches("<entry>").count(), MAX_ITEMS);
        assert!(text.contains("2026-06-28:tier-change/alpha"), "{text}");
        assert!(!text.contains("2026-01-01:tier-change/alpha"), "{text}");
    }

    #[test]
    fn text_is_escaped_and_control_characters_go() {
        let hostile = catalogue_with_name("<script>\"'&\u{1}x");
        let tiers = report(vec![("alpha", record(Tier::Listed, None))]);
        let changes = [change(10, 5, "alpha", Some(Tier::New), Tier::Listed)];
        let text = feed(&hostile, Some(&tiers), &changes, None);
        assert!(!text.contains("<script"), "{text}");
        assert!(
            text.contains("&#60;script&#62;&#34;&#39;&#38;x: New to Listed"),
            "{text}"
        );
        assert!(!text.contains('\u{1}'), "{text}");
    }

    /// One entry `alpha` whose display name is `name`.
    fn catalogue_with_name(name: &str) -> Catalogue {
        let mut file = entry("alpha", "");
        let quoted = name
            .replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('\u{1}', "\\u0001");
        file.contents = file
            .contents
            .replace("name = \"alpha\"", &format!("name = \"{quoted}\""));
        catalogue(vec![file])
    }

    #[test]
    fn the_same_inputs_give_the_same_bytes_in_any_input_order() {
        let (data, tiers) = base();
        let forward = [
            change(10, 5, "alpha", Some(Tier::New), Tier::Listed),
            change(10, 5, "beta", Some(Tier::New), Tier::Listed),
        ];
        let reversed = [forward[1].clone(), forward[0].clone()];
        assert_eq!(
            feed(&data, Some(&tiers), &forward, None),
            feed(&data, Some(&tiers), &reversed, None)
        );
    }

    #[test]
    fn a_removed_entry_links_the_changes_page_and_names_its_slug() {
        let (data, tiers) = base();
        let changes = [change(10, 5, "ghost", Some(Tier::New), Tier::Listed)];
        let text = feed(&data, Some(&tiers), &changes, None);
        assert!(
            text.contains("<title>ghost: New to Listed</title>"),
            "{text}"
        );
        assert!(
            text.contains("href=\"https://ml-rust.github.io/changes/\"/>\n<updated>2026-10-05"),
            "{text}"
        );
    }

    #[test]
    fn without_a_report_the_feed_holds_no_item_and_stays_valid() {
        let (data, _) = base();
        let changes = [change(10, 5, "alpha", Some(Tier::New), Tier::Listed)];
        let text = feed(&data, None, &changes, None);
        assert_eq!(text.matches("<entry>").count(), 0);
        assert!(
            text.contains("<updated>1970-01-01T00:00:00Z</updated>"),
            "{text}"
        );
    }

    #[test]
    fn an_empty_history_dates_the_feed_from_the_report() {
        let (data, tiers) = base();
        let text = feed(&data, Some(&tiers), &[], None);
        assert!(
            text.contains("<updated>2026-11-03T00:00:00Z</updated>"),
            "{text}"
        );
    }
}
