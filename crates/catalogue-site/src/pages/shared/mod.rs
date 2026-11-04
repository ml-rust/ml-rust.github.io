//! View data that several pages share.

mod badge;
mod change_row;
mod choice;
mod entry_row;
mod install;
mod labels;
mod listing;
mod movement;
mod order;
mod paths;
mod topic_card;

pub(crate) use badge::TierBadge;
pub(crate) use change_row::{
    ChangeRow, FirstCheckLine, change_rows, first_check_line, is_first_check,
};
pub(crate) use choice::{fact_text, whole_text};
pub(crate) use entry_row::{EntryRow, RelatedLink, tier_of};
pub(crate) use install::{InstallLine, install_line};
pub(crate) use labels::{kind_name, project_kind_name, status_name, tier_name};
pub(crate) use listing::{EntryGroup, featured_rows, group_entries, group_install_rows};
pub(crate) use movement::{movement_note, movement_word};
pub(crate) use order::{family_heads, order_text, strongest_first};
pub(crate) use paths::{
    EvidenceLink, NamedLink, category_file, category_href, category_link, category_links,
    entry_link, project_file, project_href, topic_file, topic_href, topic_link,
};
pub(crate) use topic_card::{TopicCard, topic_cards};
