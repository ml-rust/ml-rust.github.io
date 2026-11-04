//! View data that several pages share.

mod badge;
mod change_row;
mod choice;
mod entry_row;
mod install;
mod labels;
mod listing;
mod paths;
mod topic_card;

pub(crate) use badge::TierBadge;
pub(crate) use change_row::{ChangeRow, change_rows};
pub(crate) use choice::{fact_text, whole_text};
pub(crate) use entry_row::{EntryRow, sort_by_name, tier_of};
pub(crate) use install::{InstallLine, install_line};
pub(crate) use labels::{kind_name, project_kind_name, status_name, tier_name};
pub(crate) use listing::{EntryGroup, group_entries, group_install_rows};
pub(crate) use paths::{
    EvidenceLink, NamedLink, category_file, category_href, category_link, category_links,
    entry_link, project_file, project_href, topic_file, topic_href, topic_link,
};
pub(crate) use topic_card::{TopicCard, topic_cards};
