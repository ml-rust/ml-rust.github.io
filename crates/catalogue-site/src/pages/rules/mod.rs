//! `/rules/`: every tier rule in order, each beside the thresholds it reads.
//!
//! The rule text follows the rule docs in `catalogue_core`'s tier modules.

#[cfg(test)]
pub(crate) mod fixtures;
mod page;
mod rows;
mod sections;

pub(crate) use page::RulesPage;
