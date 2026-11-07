//! Small plain text files for AI agents: one report, one list per job and one card per project.
//!
//! An agent reads only what it needs, so `data/catalogue.json` never has to fill its context.
//! These files leave out stars, downloads and tiers, and they list no archived project. Each
//! list sorts by slug, so its order says nothing about a project.

use catalogue_core::{
    Catalogue, EffectiveEntry, EntryReport, LICENSE_SIGNAL, NATIVE_CODE_SIGNAL, NO_GPU_FEATURE,
    NOT_MEASURED, RUNS_ON_SIGNAL, Slug, StateReport, Tier, TierReport, wraps_native_engine,
};

use super::texts::{plain_name, text_file};
use crate::build::OutputFile;
use crate::pages::{SITE_URL, docs_href, install_line, native_code_fact};
use crate::search::lead_phrase;

/// The id of the latest version fact.
const LATEST_VERSION_SIGNAL: &str = "latest_version";
/// The id of the latest release date fact.
const LAST_RELEASE_SIGNAL: &str = "last_release";
/// The id of the latest human commit date fact.
const LAST_COMMIT_SIGNAL: &str = "last_human_commit";

/// The path of the report for agents.
pub(crate) const STATE_AGENT_FILE: &str = "state/llms.txt";

/// The address of the list of job `id`.
pub(crate) fn job_file_url(id: &Slug) -> String {
    format!("{SITE_URL}/topics/{id}/llms.txt")
}

/// The address of the card of `slug`.
fn card_url(slug: &Slug) -> String {
    format!("{SITE_URL}/projects/{slug}/llms.txt")
}

/// `value` of `name` in `record`, unless the check did not measure it.
fn fact<'a>(record: Option<&'a EntryReport>, name: &str) -> Option<&'a str> {
    record
        .and_then(|record| record.value_of(name))
        .filter(|value| *value != NOT_MEASURED)
}

/// Where the work of a project happens, from the native code fact.
fn engine_words(record: Option<&EntryReport>) -> Option<&'static str> {
    if fact(record, NATIVE_CODE_SIGNAL).is_some_and(wraps_native_engine) {
        return Some("wraps a native engine");
    }
    match native_code_fact(record).short.as_str() {
        "None found" => Some("Rust, no native code found"),
        "Optional" => Some("Rust, native code optional"),
        "Required" => Some("Rust, with native support libraries"),
        "Build tool" => Some("Rust, needs a native build tool"),
        _ => None,
    }
}

/// Whether `record` holds an archived project.
fn archived(record: Option<&EntryReport>) -> bool {
    record.is_some_and(|record| record.tier == Tier::Archived)
}

/// The one line of `effective` in a job list.
fn list_line(effective: &EffectiveEntry<'_>, record: Option<&EntryReport>) -> String {
    let entry = effective.entry;
    let mut about = vec![entry.kind.as_str().to_owned()];
    about.extend(engine_words(record).map(str::to_owned));
    if let Some(version) = fact(record, LATEST_VERSION_SIGNAL) {
        about.push(version.to_owned());
    }
    format!(
        "- {} ({}): {}. {}",
        plain_name(&entry.name),
        about.join(", "),
        plain_name(&lead_phrase(&entry.description.text)).trim_end_matches('.'),
        card_url(effective.slug),
    )
}

/// The list of each job: what a reader compares to choose a project.
fn job_files(catalogue: &Catalogue, report: Option<&TierReport>) -> Vec<OutputFile> {
    catalogue
        .topics()
        .iter()
        .map(|topic| {
            let mut lines = vec![
                format!("# {}", plain_name(&topic.name)),
                String::new(),
                plain_name(&topic.blurb),
                "Sorted by name. The order says nothing about a project.".to_owned(),
                String::new(),
            ];
            for slug in catalogue.entries().keys() {
                let Some(effective) = catalogue.effective(slug) else {
                    continue;
                };
                let record = report.and_then(|report| report.entries.get(slug));
                if effective.topics.contains(&topic.id) && !archived(record) {
                    lines.push(list_line(&effective, record));
                }
            }
            lines.push(String::new());
            text_file(&format!("topics/{}/llms.txt", topic.id), lines.join("\n"))
        })
        .collect()
}

/// The card of `effective`: the facts that decide whether a project fits.
fn card_file(effective: &EffectiveEntry<'_>, record: Option<&EntryReport>) -> OutputFile {
    let entry = effective.entry;
    let install = install_line(entry);
    let mut lines = vec![
        format!("# {}", plain_name(&entry.name)),
        String::new(),
        plain_name(&entry.description.text),
        String::new(),
        format!("- Kind: {}", entry.kind.as_str()),
        format!(
            "- Jobs: {}",
            effective
                .topics
                .iter()
                .map(Slug::as_str)
                .collect::<Vec<_>>()
                .join(", ")
        ),
    ];
    let mut push = |label: &str, value: Option<&str>| {
        if let Some(value) = value {
            lines.push(format!("- {label}: {}", plain_name(value)));
        }
    };
    push("Work happens in", engine_words(record));
    push("Latest version", fact(record, LATEST_VERSION_SIGNAL));
    push("Latest release", fact(record, LAST_RELEASE_SIGNAL));
    push("Latest human commit", fact(record, LAST_COMMIT_SIGNAL));
    push(
        "GPU",
        fact(record, RUNS_ON_SIGNAL).filter(|value| *value != NO_GPU_FEATURE),
    );
    push("License", fact(record, LICENSE_SIGNAL));
    push("Install", install.command.as_deref());
    push("Repository", Some(entry.repo.as_str()));
    let docs = docs_href(entry);
    push("Docs", docs.as_ref().map(|href| href.as_str()));
    lines.push(format!(
        "- Full page, with the README: {SITE_URL}/projects/{}/",
        effective.slug
    ));
    lines.push(String::new());
    text_file(
        &format!("projects/{}/llms.txt", effective.slug),
        lines.join("\n"),
    )
}

/// `text` with each `[[slug]]` as its plain slug.
fn report_text(text: &str) -> String {
    plain_name(&text.replace("[[", "").replace("]]", ""))
}

/// The state report as plain text.
fn state_file(state: &StateReport) -> OutputFile {
    let summary = &state.summary;
    let slugs = |list: &[Slug]| list.iter().map(Slug::as_str).collect::<Vec<_>>().join(", ");
    let mut lines = vec![
        format!("# The state of ML in Rust, as of {}", state.written),
        String::new(),
        "Written by AI models from catalogue data, not checked by a person.".to_owned(),
        String::new(),
        report_text(&summary.headline),
        String::new(),
        report_text(&summary.summary),
        String::new(),
        "## Jobs".to_owned(),
    ];
    for job in &summary.jobs {
        lines.push(format!(
            "- {}: {}. Tools: {}. {}",
            job.topic,
            report_text(&job.verdict).trim_end_matches('.'),
            slugs(&job.tools),
            job_file_url(&job.topic),
        ));
    }
    lines.push(String::new());
    lines.push("## Use now".to_owned());
    lines.extend(summary.use_now.iter().map(|pick| {
        format!(
            "- {}: {}. {}",
            pick.project,
            report_text(&pick.use_for),
            report_text(&pick.why)
        )
    }));
    lines.push(String::new());
    lines.push("## Watch".to_owned());
    lines.extend(summary.watch.iter().map(|pick| {
        format!(
            "- {}: {}. {}",
            pick.project,
            report_text(&pick.use_for),
            report_text(&pick.why)
        )
    }));
    lines.push(String::new());
    lines.push("## What you can build today".to_owned());
    lines.extend(summary.build_today.iter().map(|path| {
        format!(
            "- {}. Use: {}. {} Watch out: {}",
            report_text(&path.goal),
            slugs(&path.stack),
            report_text(&path.why),
            report_text(&path.watch_out)
        )
    }));
    lines.push(String::new());
    lines.push("## Gaps Rust has filled".to_owned());
    lines.extend(summary.holes_filled.iter().map(|hole| {
        format!(
            "- {}. Use: {}. {}",
            report_text(&hole.hole),
            slugs(&hole.filled_by),
            report_text(&hole.note)
        )
    }));
    lines.push(String::new());
    lines.push("## Gaps still open".to_owned());
    lines.extend(summary.holes_open.iter().map(|hole| {
        format!(
            "- {}. {} Today: {}",
            report_text(&hole.hole),
            report_text(&hole.impact),
            report_text(&hole.workaround)
        )
    }));
    lines.push(String::new());
    text_file(STATE_AGENT_FILE, lines.join("\n"))
}

/// Every file for agents except `llms.txt`. The report file needs `state` and `report`.
pub(crate) fn agent_files(
    catalogue: &Catalogue,
    report: Option<&TierReport>,
    state: Option<&StateReport>,
) -> Vec<OutputFile> {
    let mut files = job_files(catalogue, report);
    for slug in catalogue.entries().keys() {
        let Some(effective) = catalogue.effective(slug) else {
            continue;
        };
        let record = report.and_then(|report| report.entries.get(slug));
        if !archived(record) {
            files.push(card_file(&effective, record));
        }
    }
    if let Some(state) = state.filter(|_| report.is_some()) {
        files.push(state_file(state));
    }
    files
}
