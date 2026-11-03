//! The validation section: the data rules on the record, against `main`.

use std::num::NonZeroU64;

use catalogue_core::{Catalogue, Slug, SourceFile};

use super::report::{Block, Section, sorted};
use super::submission::{Context, Record};
use crate::convert::{JudgmentDraft, Submission};

/// The path of the new entry while it loads with `main`. No data directory holds such a path.
fn candidate_path(number: NonZeroU64, slug: &Slug) -> String {
    format!("<issue {number}>/entries/{slug}.toml")
}

/// Checks the record of `context`, whose file text is `text`, and sets `blocking` on any error.
///
/// Errors in files on `main` appear apart, as already broken on main, and never block.
pub(super) fn validation_section(
    context: &Context<'_>,
    text: &str,
    blocking: &mut bool,
) -> Section {
    let mut blocks = Vec::new();
    match &context.record {
        Some(Record::Submit(found)) => submit_blocks(context, found, text, blocking, &mut blocks),
        Some(Record::Judgment(draft)) => judgment_blocks(context, draft, blocking, &mut blocks),
        None => blocks.push(Block::Text(
            "Not run. The form must parse first.".to_owned(),
        )),
    }
    if let Err(diagnostics) = &context.main {
        blocks.push(Block::Text(
            "Already broken on main. These errors are not in this issue:".to_owned(),
        ));
        let lines = diagnostics.iter().map(ToString::to_string).collect();
        blocks.push(Block::Quoted(sorted(lines)));
    }
    Section {
        title: "Validation",
        blocks,
    }
}

/// Loads `main` with the new entry file and keeps the errors on that file.
fn submit_blocks(
    context: &Context<'_>,
    found: &Submission,
    text: &str,
    blocking: &mut bool,
    blocks: &mut Vec<Block>,
) {
    let path = candidate_path(context.input.meta.number, &found.slug);
    let mut sources = context.input.main.clone();
    sources.entries.push(SourceFile {
        path: path.clone(),
        contents: text.to_owned(),
    });
    let problems: Vec<String> = match Catalogue::load(sources) {
        Ok(_) => Vec::new(),
        Err(diagnostics) => diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.path == path)
            .map(|diagnostic| diagnostic.message.clone())
            .collect(),
    };
    if problems.is_empty() {
        blocks.push(Block::Text(
            "The new file passes every data rule.".to_owned(),
        ));
        return;
    }
    *blocking = true;
    blocks.push(Block::Text("The new file breaks these rules:".to_owned()));
    blocks.push(Block::Quoted(sorted(problems)));
    if context.main.is_err() {
        blocks.push(Block::Text(
            "Main does not load, so an error above can come from main.".to_owned(),
        ));
    }
}

/// Checks the draft on its own and its references against `main`.
fn judgment_blocks(
    context: &Context<'_>,
    draft: &JudgmentDraft,
    blocking: &mut bool,
    blocks: &mut Vec<Block>,
) {
    let mut problems = draft.problems();
    match &context.main {
        Ok(catalogue) => {
            if catalogue.judgments().contains_key(&draft.number.get()) {
                problems.push(format!("judgment file `{}` already exists", draft.path()));
            }
            problems.extend(catalogue.judgment_reference_problems(&draft.entry, &draft.judgment));
        }
        Err(_) => {
            *blocking = true;
            blocks.push(Block::Text(
                "The reference check did not run, because main does not load.".to_owned(),
            ));
        }
    }
    if problems.is_empty() {
        if context.main.is_ok() {
            blocks.push(Block::Text(
                "The judgment passes every rule that needs no approver.".to_owned(),
            ));
        }
        return;
    }
    *blocking = true;
    blocks.push(Block::Text("The judgment breaks these rules:".to_owned()));
    blocks.push(Block::Quoted(sorted(problems)));
}
