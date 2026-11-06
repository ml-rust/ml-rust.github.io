//! `/state/`: the state of ML in Rust as of the day the report was written.
//!
//! The words and the chart choice come from the report, which models write. Every number and bar
//! comes from the tier report, in this file.

use askama::Template;
use catalogue_core::{
    BuildPath, CONTRIBUTORS_SIGNAL, Catalogue, ChartId, ChartNote, DEPENDENTS_SIGNAL, Date,
    EntryKind, EntryReport, FilledHole, JobState, NATIVE_CODE_SIGNAL, OpenHole, PythonNeed,
    RUNS_ON_SEPARATOR, RUNS_ON_SIGNAL, Readiness, Slug, StateReport, Tier, TierReport, ToolPick,
    wraps_native_engine,
};

use super::layout::Layout;
use super::shared::{date_text, native_code_fact, project_href, topic_href};
use crate::href::SafeHref;

/// A report older than this many days carries a notice.
const STALE_DAYS: i64 = 21;

/// The releases the chart counts as recent, in days.
const RECENT_DAYS: i64 = 90;

/// The releases the chart counts as this year, in days.
const YEAR_DAYS: i64 = 365;

/// The id of the latest release fact.
const LATEST_RELEASE_SIGNAL: &str = "last_release";

/// One run of text, or one link to a project.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Piece {
    /// The text.
    pub text: String,
    /// The project page, for a mention.
    pub href: Option<SafeHref>,
}

/// A project link.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ProjectLink {
    /// The project name.
    pub name: String,
    /// The project page.
    pub href: SafeHref,
    /// Whether the check found that the project requires native code.
    pub wrapper: bool,
}

/// One row of the jobs table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct JobView {
    name: String,
    href: SafeHref,
    readiness: &'static str,
    readiness_class: &'static str,
    python: &'static str,
    verdict: Vec<Piece>,
    /// The apps the report picks, with wrappers last.
    run_now: Vec<ProjectLink>,
    /// The libraries the report picks, with wrappers last.
    build_with: Vec<ProjectLink>,
    /// The projects of the organization in this job. Code lists them, whatever the models wrote.
    from_org: Vec<ProjectLink>,
}

/// One crate to use or watch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PickView {
    link: Option<ProjectLink>,
    use_for: Vec<Piece>,
    why: Vec<Piece>,
}

/// One thing to build.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PathView {
    goal: Vec<Piece>,
    stack: Vec<ProjectLink>,
    why: Vec<Piece>,
    watch_out: Vec<Piece>,
}

/// One need Rust now covers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FilledView {
    hole: Vec<Piece>,
    filled_by: Vec<ProjectLink>,
    note: Vec<Piece>,
}

/// One hole still open.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct OpenView {
    hole: Vec<Piece>,
    impact: Vec<Piece>,
    workaround: Vec<Piece>,
}

/// One part of a chart bar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BarPart {
    label: &'static str,
    class: &'static str,
    count: usize,
    /// The start in the 100-wide bar, as SVG text.
    x: String,
    /// The width in the 100-wide bar, as SVG text.
    width: String,
}

/// One row of a chart.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ChartRow {
    name: String,
    href: Option<SafeHref>,
    parts: Vec<BarPart>,
    /// The counts in words, for every reader.
    text: String,
}

/// One entry of a chart legend.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LegendItem {
    label: &'static str,
    class: &'static str,
}

/// One chart.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Chart {
    title: &'static str,
    takeaway: Vec<Piece>,
    note: String,
    legend: Vec<LegendItem>,
    rows: Vec<ChartRow>,
}

/// The state page.
#[derive(Debug, Template)]
#[template(path = "state.html")]
pub(crate) struct StatePage {
    /// The shared layout.
    pub layout: Layout,
    written: String,
    written_text: String,
    stale: bool,
    headline: Vec<Piece>,
    summary: Vec<Piece>,
    jobs: Vec<JobView>,
    use_now: Vec<PickView>,
    watch: Vec<PickView>,
    build_today: Vec<PathView>,
    holes_filled: Vec<FilledView>,
    holes_open: Vec<OpenView>,
    charts: Vec<Chart>,
}

impl StatePage {
    /// The page of `state` over `catalogue` and `report`, in `layout`.
    pub(crate) fn new(
        layout: Layout,
        catalogue: &Catalogue,
        report: &TierReport,
        state: &StateReport,
    ) -> Self {
        let words = Words { catalogue, report };
        let written = state.written.to_string();
        let summary = &state.summary;
        Self {
            layout,
            written_text: date_text(&written).unwrap_or_else(|| written.clone()),
            written,
            stale: report.generated.days_since(state.written) > STALE_DAYS,
            headline: words.pieces(&summary.headline),
            summary: words.pieces(&summary.summary),
            jobs: summary.jobs.iter().map(|job| words.job(job)).collect(),
            use_now: summary
                .use_now
                .iter()
                .map(|pick| words.pick(pick))
                .collect(),
            watch: summary.watch.iter().map(|pick| words.pick(pick)).collect(),
            build_today: summary
                .build_today
                .iter()
                .map(|path| words.path(path))
                .collect(),
            holes_filled: summary
                .holes_filled
                .iter()
                .map(|hole| words.filled(hole))
                .collect(),
            holes_open: summary
                .holes_open
                .iter()
                .map(|hole| words.open(hole))
                .collect(),
            charts: state
                .charts
                .iter()
                .map(|note| chart(catalogue, report, note, &words))
                .collect(),
        }
    }
}

/// Turns report text and slugs into what the template prints.
struct Words<'a> {
    catalogue: &'a Catalogue,
    report: &'a TierReport,
}

impl Words<'_> {
    /// The link of `slug`, or `None` when the catalogue no longer holds it.
    fn link(&self, slug: &Slug) -> Option<ProjectLink> {
        let entry = self.catalogue.entries().get(slug)?;
        Some(ProjectLink {
            name: entry.name.clone(),
            href: project_href(slug),
            wrapper: self
                .report
                .entries
                .get(slug)
                .and_then(|record| record.value_of(NATIVE_CODE_SIGNAL))
                .is_some_and(wraps_native_engine),
        })
    }

    /// The links of `slugs` that are apps, or that are not, with wrappers last.
    fn tools(&self, slugs: &[Slug], apps: bool) -> Vec<ProjectLink> {
        let chosen: Vec<Slug> = slugs
            .iter()
            .filter(|slug| {
                self.catalogue
                    .entries()
                    .get(slug)
                    .is_some_and(|entry| (entry.kind == EntryKind::App) == apps)
            })
            .cloned()
            .collect();
        let mut links = self.links(&chosen);
        links.sort_by_key(|link| link.wrapper);
        links
    }

    fn links(&self, slugs: &[Slug]) -> Vec<ProjectLink> {
        slugs.iter().filter_map(|slug| self.link(slug)).collect()
    }

    /// `text` with each `[[slug]]` as a link. A slug the catalogue lost prints as plain text.
    fn pieces(&self, text: &str) -> Vec<Piece> {
        let mut pieces = Vec::new();
        let mut rest = text;
        while let Some(start) = rest.find("[[") {
            let Some(length) = rest[start..].find("]]") else {
                break;
            };
            if start > 0 {
                pieces.push(plain(&rest[..start]));
            }
            let name = &rest[start + 2..start + length];
            let link = Slug::new(name.to_owned())
                .ok()
                .and_then(|slug| self.link(&slug));
            pieces.push(match link {
                Some(found) => Piece {
                    text: found.name,
                    href: Some(found.href),
                },
                None => plain(name),
            });
            rest = &rest[start + length + 2..];
        }
        if !rest.is_empty() {
            pieces.push(plain(rest));
        }
        pieces
    }

    fn job(&self, job: &JobState) -> JobView {
        let (readiness, readiness_class) = match job.readiness {
            Readiness::Ready => ("Ready", "status-ready"),
            Readiness::Usable => ("Usable, with limits", "status-partial"),
            Readiness::Early => ("Too early", "status-gap"),
        };
        let mut tools = job.tools.clone();
        for slug in mentions(&job.verdict) {
            if !tools.contains(&slug) {
                tools.push(slug);
            }
        }
        JobView {
            name: topic_name(self.catalogue, &job.topic),
            href: topic_href(&job.topic),
            readiness,
            readiness_class,
            python: match job.python {
                PythonNeed::None => "Not needed",
                PythonNeed::Partly => "For some steps",
                PythonNeed::Needed => "Still needed",
            },
            verdict: self.pieces(&job.verdict),
            run_now: self.tools(&tools, true),
            build_with: self.tools(&tools, false),
            from_org: self
                .catalogue
                .entries()
                .iter()
                .filter(|(slug, entry)| {
                    entry.org_affiliated
                        && entry.topics.contains(&job.topic)
                        && !is_archived(self.report, slug)
                })
                .filter_map(|(slug, _)| self.link(slug))
                .collect(),
        }
    }

    fn pick(&self, pick: &ToolPick) -> PickView {
        PickView {
            link: self.link(&pick.project),
            use_for: self.pieces(&pick.use_for),
            why: self.pieces(&pick.why),
        }
    }

    fn path(&self, path: &BuildPath) -> PathView {
        PathView {
            goal: self.pieces(&path.goal),
            stack: self.links(&path.stack),
            why: self.pieces(&path.why),
            watch_out: self.pieces(&path.watch_out),
        }
    }

    fn filled(&self, hole: &FilledHole) -> FilledView {
        FilledView {
            hole: self.pieces(&hole.hole),
            filled_by: self.links(&hole.filled_by),
            note: self.pieces(&hole.note),
        }
    }

    fn open(&self, hole: &OpenHole) -> OpenView {
        OpenView {
            hole: self.pieces(&hole.hole),
            impact: self.pieces(&hole.impact),
            workaround: self.pieces(&hole.workaround),
        }
    }
}

/// The slugs `text` mentions as `[[slug]]`, so a project a verdict names is also in its tool list.
fn mentions(text: &str) -> Vec<Slug> {
    text.split("[[")
        .skip(1)
        .filter_map(|part| part.split_once("]]"))
        .filter_map(|(name, _)| Slug::new(name.to_owned()).ok())
        .collect()
}

fn plain(text: &str) -> Piece {
    Piece {
        text: text.to_owned(),
        href: None,
    }
}

fn is_archived(report: &TierReport, slug: &Slug) -> bool {
    report
        .entries
        .get(slug)
        .is_some_and(|record| record.tier == Tier::Archived)
}

fn topic_name(catalogue: &Catalogue, id: &Slug) -> String {
    catalogue
        .topics()
        .iter()
        .find(|topic| &topic.id == id)
        .map_or_else(|| id.to_string(), |topic| topic.name.clone())
}

/// The parts of a bar from `(label, class, count)`, each as a share of the sum of counts.
fn bar(counts: &[(&'static str, &'static str, usize)]) -> Vec<BarPart> {
    let total: usize = counts.iter().map(|(_, _, count)| count).sum();
    if total == 0 {
        return Vec::new();
    }
    let mut start = 0_usize;
    counts
        .iter()
        .filter(|(_, _, count)| *count > 0)
        .map(|&(label, class, count)| {
            let part = BarPart {
                label,
                class,
                count,
                x: percent(start, total),
                width: percent(count, total),
            };
            start = start.saturating_add(count);
            part
        })
        .collect()
}

/// `part` of `total` on a 100-wide scale, with two decimals.
fn percent(part: usize, total: usize) -> String {
    let hundredths = part.saturating_mul(10_000) / total.max(1);
    format!("{}.{:02}", hundredths / 100, hundredths % 100)
}

/// The counts in words, such as `3 older, 2 last year`.
fn counts_text(parts: &[BarPart]) -> String {
    let words: Vec<String> = parts
        .iter()
        .map(|part| format!("{} {}", part.count, part.label.to_lowercase()))
        .collect();
    words.join(", ")
}

/// The chart a report note picks.
fn chart(catalogue: &Catalogue, report: &TierReport, note: &ChartNote, words: &Words<'_>) -> Chart {
    let (title, method, rows, legend) = match note.chart {
        ChartId::Engine => engine_chart(catalogue, report),
        ChartId::Releases => release_chart(catalogue, report),
        ChartId::Gpu => gpu_chart(catalogue, report),
        ChartId::Team => team_chart(catalogue, report),
        ChartId::UsedBy => used_by_chart(catalogue, report),
    };
    Chart {
        title,
        takeaway: words.pieces(&note.takeaway),
        note: method,
        legend,
        rows,
    }
}

/// What a chart function gives: its title, its method, its rows and its legend.
type ChartParts = (&'static str, String, Vec<ChartRow>, Vec<LegendItem>);

/// One row per topic and a legend, counting `classify` over the live projects of each topic.
fn topic_rows(
    catalogue: &Catalogue,
    report: &TierReport,
    labels: &[(&'static str, &'static str)],
    classify: &dyn Fn(&EntryReport) -> Option<usize>,
) -> (Vec<ChartRow>, Vec<LegendItem>) {
    let rows = catalogue
        .topics()
        .iter()
        .map(|topic| {
            let mut counts = vec![0_usize; labels.len()];
            let records = catalogue
                .entries()
                .iter()
                .filter(|(slug, entry)| {
                    entry.topics.contains(&topic.id) && !is_archived(report, slug)
                })
                .filter_map(|(slug, _)| report.entries.get(slug));
            for record in records {
                if let Some(slot) = classify(record).and_then(|index| counts.get_mut(index)) {
                    *slot = slot.saturating_add(1);
                }
            }
            let triples: Vec<(&'static str, &'static str, usize)> = labels
                .iter()
                .zip(&counts)
                .map(|(&(label, class), &count)| (label, class, count))
                .collect();
            let parts = bar(&triples);
            ChartRow {
                name: topic.name.clone(),
                href: Some(topic_href(&topic.id)),
                text: counts_text(&parts),
                parts,
            }
        })
        .collect();
    let legend = labels
        .iter()
        .map(|&(label, class)| LegendItem { label, class })
        .collect();
    (rows, legend)
}

/// The amount of the fact `name` in `record`.
fn amount(record: &EntryReport, name: &str) -> Option<f64> {
    record.value_of(name)?.parse().ok()
}

fn engine_chart(catalogue: &Catalogue, report: &TierReport) -> ChartParts {
    let labels = [
        ("No native code found", "step-4"),
        ("Native build tool", "step-3"),
        ("Some native code", "step-2"),
        ("Wraps a native engine", "step-1"),
    ];
    let (rows, legend) = topic_rows(catalogue, report, &labels, &|record| {
        let wraps = record
            .value_of(NATIVE_CODE_SIGNAL)
            .is_some_and(wraps_native_engine);
        match native_code_fact(Some(record)).short.as_str() {
            "None found" => Some(0),
            "Build tool" => Some(1),
            "Required" if wraps => Some(3),
            "Optional" | "Required" => Some(2),
            _ => None,
        }
    });
    let method = "Projects in each job, from the crates each one depends on. Native code kept outside crates.io stays unseen.";
    ("Where the work happens", method.to_owned(), rows, legend)
}

fn release_chart(catalogue: &Catalogue, report: &TierReport) -> ChartParts {
    let labels = [
        ("Last 90 days", "step-4"),
        ("Last year", "step-3"),
        ("Older", "step-1"),
    ];
    let today = report.generated;
    let (rows, legend) = topic_rows(catalogue, report, &labels, &|record| {
        let released = record.value_of(LATEST_RELEASE_SIGNAL)?;
        let days = today.days_since(released.parse::<jiff::civil::Date>().ok().map(Date::from)?);
        Some(if days <= RECENT_DAYS {
            0
        } else if days <= YEAR_DAYS {
            1
        } else {
            2
        })
    });
    let method = "Projects in each job, by their latest release. A finished project can ship rarely and stay sound.";
    (
        "How recently projects shipped",
        method.to_owned(),
        rows,
        legend,
    )
}

fn team_chart(catalogue: &Catalogue, report: &TierReport) -> ChartParts {
    let labels = [
        ("Ten or more people", "step-4"),
        ("Two to nine people", "step-3"),
        ("One person", "step-1"),
    ];
    let (rows, legend) = topic_rows(catalogue, report, &labels, &|record| {
        let count = amount(record, CONTRIBUTORS_SIGNAL)?;
        Some(if count >= 10.0 {
            0
        } else if count >= 2.0 {
            1
        } else {
            2
        })
    });
    let method = "Projects in each job, by the contributors GitHub counts.";
    (
        "Who stands behind projects",
        method.to_owned(),
        rows,
        legend,
    )
}

fn used_by_chart(catalogue: &Catalogue, report: &TierReport) -> ChartParts {
    let labels = [
        ("Hundreds of teams", "step-4"),
        ("Dozens of teams", "step-3"),
        ("A few teams", "step-2"),
        ("No other team", "step-1"),
    ];
    let (rows, legend) = topic_rows(catalogue, report, &labels, &|record| {
        let count = amount(record, DEPENDENTS_SIGNAL)?;
        Some(if count >= 100.0 {
            0
        } else if count >= 10.0 {
            1
        } else if count >= 1.0 {
            2
        } else {
            3
        })
    });
    let method = "Libraries in each job, by the crates from other owners that depend on them.";
    ("Who builds on projects", method.to_owned(), rows, legend)
}

fn gpu_chart(catalogue: &Catalogue, report: &TierReport) -> ChartParts {
    let live: Vec<&EntryReport> = catalogue
        .entries()
        .keys()
        .filter(|slug| !is_archived(report, slug))
        .filter_map(|slug| report.entries.get(slug))
        .collect();
    let count_with = |name: &str| {
        live.iter()
            .filter(|record| {
                record
                    .value_of(RUNS_ON_SIGNAL)
                    .is_some_and(|value| value.split(RUNS_ON_SEPARATOR).any(|part| part == name))
            })
            .count()
    };
    let counts: Vec<(String, usize)> = catalogue
        .backends()
        .backend
        .iter()
        .map(|backend| (backend.name.clone(), count_with(&backend.name)))
        .collect();
    let most = counts.iter().map(|(_, count)| *count).max().unwrap_or(0);
    let rows = counts
        .into_iter()
        .map(|(name, count)| ChartRow {
            name,
            href: None,
            text: format!("{count} projects"),
            parts: if count == 0 {
                Vec::new()
            } else {
                vec![BarPart {
                    label: "Projects",
                    class: "step-3",
                    count,
                    x: "0.00".to_owned(),
                    width: percent(count, most),
                }]
            },
        })
        .collect();
    let method = "Projects that name each backend in a dependency or a feature. Many others can still run on a GPU, and the check cannot see it.";
    (
        "GPU backends the check can see",
        method.to_owned(),
        rows,
        Vec::new(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_bar_leaves_out_empty_parts_and_fills_the_width() {
        let parts = bar(&[("A", "a", 1), ("B", "b", 0), ("C", "c", 3)]);
        assert_eq!(parts.len(), 2);
        assert_eq!(parts[0].x, "0.00");
        assert_eq!(parts[0].width, "25.00");
        assert_eq!(parts[1].x, "25.00");
        assert_eq!(parts[1].width, "75.00");
        assert_eq!(counts_text(&parts), "1 a, 3 c");
        assert!(bar(&[("A", "a", 0)]).is_empty());
    }
}
