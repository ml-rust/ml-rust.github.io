//! View data that several pages share.

mod badge;
mod category_card;
mod change_row;
mod choice;
mod date_text;
mod entry_row;
mod examples;
mod install;
mod labels;
mod license_family;
mod list_head;
mod listing;
mod movement;
mod order;
mod paths;
mod showcase_card;
mod tier_help;
mod topic_card;

pub(crate) use badge::TierBadge;
pub(crate) use category_card::{CategoryCard, category_cards};
pub(crate) use change_row::{
    ChangeDay, ChangeRow, FirstCheckLine, change_days, change_rows, first_check_line,
    is_first_check,
};
pub(crate) use choice::{
    ChoiceText, dependents_text, downloads_signal, fact_text, native_code_fact, whole_text,
};
pub(crate) use date_text::{ago_text, date_text, grouped};
pub(crate) use entry_row::{
    DORMANT, DORMANT_MEANING, EntryRow, RelatedLink, row_statuses, tier_of,
};
pub(crate) use examples::{ExampleSearch, example_searches};
pub(crate) use install::{InstallLine, docs_href, install_line};
pub(crate) use labels::{kind_name, project_kind_name, status_name, tier_name};
pub(crate) use list_head::{ListHead, checked_dates};
pub(crate) use listing::{EntryGroup, featured_rows, featured_slugs, group_entries, project_count};
pub(crate) use movement::{movement_note, movement_word};
pub(crate) use order::{
    FamilyHead, family_heads, featured_order_text, order_text, strongest_first,
};
pub(crate) use paths::{
    EvidenceLink, NamedLink, category_file, category_href, category_link, compare_href, entry_link,
    project_file, project_href, topic_file, topic_href, topic_link,
};
pub(crate) use showcase_card::{ShowcaseSection, showcase_section};
pub(crate) use tier_help::{FEATURED_MEANING, TierLine, tier_lines};
pub(crate) use topic_card::{TopicCard, topic_cards};
