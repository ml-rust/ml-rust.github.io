//! The fallback summary of one `catalogue tiers` run.

use catalogue_core::{Date, Slug};

use super::source::Source;

/// The fewest fetches a run needs before `fallback.max_failed_share` applies.
///
/// The limit guards against an outage, and a share over a few fetches says nothing about one. A
/// run that adds one project sends about six, so one lost fetch reads as a large share. Below this
/// count a failed fetch leaves its value unknown or on its fallback, and the next scan fills it.
pub const MIN_FETCHES_FOR_FAILED_SHARE: usize = 20;

/// One fetch that failed and fell back, or left its signal unknown.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct FetchFailure {
    /// The entry.
    pub slug: Slug,
    /// The source that failed.
    pub source: Source,
    /// The date of the value in use. `None` when the signal reads as unknown.
    pub from: Option<Date>,
    /// The error text of the fetch.
    pub error: String,
}

/// Every network fetch of a run and the ones that failed.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FetchSummary {
    /// The network fetches, one per entry and source.
    pub fetches: usize,
    /// The fetches that failed, by slug and source.
    pub failures: Vec<FetchFailure>,
}

impl FetchSummary {
    fn fell_back(&self) -> impl Iterator<Item = (&FetchFailure, Date)> {
        self.failures
            .iter()
            .filter_map(|failure| failure.from.map(|date| (failure, date)))
    }

    fn lost(&self) -> impl Iterator<Item = &FetchFailure> {
        self.failures
            .iter()
            .filter(|failure| failure.from.is_none())
    }

    /// The number of fetches that fell back.
    pub fn fallback_count(&self) -> usize {
        self.fell_back().count()
    }

    /// The lost fetches that a tier reads. No tier reads a README.
    fn lost_signals(&self) -> impl Iterator<Item = &FetchFailure> {
        self.lost()
            .filter(|failure| failure.source != Source::Readme)
    }

    /// The number of fetches a tier reads that failed with no usable earlier value.
    pub fn lost_count(&self) -> usize {
        self.lost_signals().count()
    }

    /// Whether the fetches a tier reads and no earlier value covers exceed `limit`, as a share of
    /// all fetches.
    ///
    /// A fetch that recovered from a snapshot does not count. A run with fewer than
    /// [`MIN_FETCHES_FOR_FAILED_SHARE`] fetches never exceeds the limit.
    pub fn exceeds_lost_limit(&self, limit: f64) -> bool {
        self.fetches >= MIN_FETCHES_FOR_FAILED_SHARE
            && self.lost_count() as f64 / self.fetches as f64 > limit
    }

    /// One `slug: source: error` line per fetch that [`lost_count`](Self::lost_count) counts.
    pub fn lost_lines(&self) -> String {
        self.lost_signals()
            .map(|f| format!("  {}: {}: {}", f.slug, f.source, f.error))
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn count_line(&self) -> String {
        format!(
            "{} of {} network fetches",
            self.fallback_count(),
            self.fetches
        )
    }

    /// The `fallbacks:` block of the text report.
    ///
    /// One `slug: source (from DATE)` line per fallback, then the count. A block
    /// of fetches that left their signal unknown follows when any did.
    pub fn text(&self) -> String {
        let mut lines = Vec::new();
        if self.fallback_count() == 0 {
            lines.push("fallbacks: -".to_owned());
        } else {
            lines.push("fallbacks:".to_owned());
            lines.extend(
                self.fell_back()
                    .map(|(f, date)| format!("  {}: {} (from {date})", f.slug, f.source)),
            );
        }
        lines.push(format!("fallback count: {}", self.count_line()));
        if self.lost().next().is_some() {
            lines.push("failed with no usable earlier value:".to_owned());
            lines.extend(self.lost().map(|f| format!("  {}: {}", f.slug, f.source)));
        }
        lines.join("\n")
    }

    /// The same list as [`text`](Self::text), as markdown for a job summary.
    pub fn markdown(&self) -> String {
        let mut lines = vec![
            format!("## Signal fallbacks: {}", self.count_line()),
            String::new(),
        ];
        lines.extend(
            self.fell_back()
                .map(|(f, date)| format!("- `{}`: {} (from {date})", f.slug, f.source)),
        );
        if self.lost().next().is_some() {
            lines.extend([
                String::new(),
                "Failed with no usable earlier value:".to_owned(),
                String::new(),
            ]);
            lines.extend(self.lost().map(|f| format!("- `{}`: {}", f.slug, f.source)));
        }
        lines.push(String::new());
        lines.join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn failure(slug: &str, source: Source, from: Option<i8>) -> FetchFailure {
        FetchFailure {
            slug: Slug::new(slug).expect("slug"),
            source,
            from: from.map(|on| Date::new(2026, 11, on).expect("date")),
            error: "down".to_owned(),
        }
    }

    fn mixed(fetches: usize) -> FetchSummary {
        FetchSummary {
            fetches,
            failures: vec![
                failure("alpha", Source::RepoFacts, Some(1)),
                failure("gamma", Source::Docker, Some(2)),
                failure("gamma", Source::Homebrew, None),
            ],
        }
    }

    #[test]
    fn no_failure_prints_a_dash_and_the_count() {
        let summary = FetchSummary {
            fetches: 3,
            failures: Vec::new(),
        };
        assert_eq!(
            summary.text(),
            "fallbacks: -\nfallback count: 0 of 3 network fetches"
        );
        assert_eq!(
            summary.markdown(),
            "## Signal fallbacks: 0 of 3 network fetches\n\n"
        );
    }

    #[test]
    fn the_text_names_each_fallback_and_each_lost_value() {
        assert_eq!(
            mixed(9).text(),
            "fallbacks:\n  alpha: GitHub repo facts (from 2026-11-01)\n  gamma: Docker Hub (from 2026-11-02)\n\
             fallback count: 2 of 9 network fetches\n\
             failed with no usable earlier value:\n  gamma: Homebrew"
        );
    }

    #[test]
    fn the_markdown_holds_the_same_list() {
        assert_eq!(
            mixed(9).markdown(),
            "## Signal fallbacks: 2 of 9 network fetches\n\n\
             - `alpha`: GitHub repo facts (from 2026-11-01)\n\
             - `gamma`: Docker Hub (from 2026-11-02)\n\n\
             Failed with no usable earlier value:\n\n\
             - `gamma`: Homebrew\n"
        );
    }

    /// `lost` fetches of `fetches` lost, and `recovered` more that fell back.
    fn share(fetches: usize, lost: usize, recovered: usize) -> FetchSummary {
        let lost = (0..lost).map(|_| failure("a", Source::RepoFacts, None));
        let recovered = (0..recovered).map(|_| failure("b", Source::RepoFacts, Some(1)));
        FetchSummary {
            fetches,
            failures: lost.chain(recovered).collect(),
        }
    }

    #[test]
    fn exactly_at_the_limit_passes_and_over_it_fails() {
        assert!(!share(20, 1, 0).exceeds_lost_limit(0.05));
        assert!(share(20, 2, 0).exceeds_lost_limit(0.05));
        assert!(!share(100, 7, 0).exceeds_lost_limit(0.07));
        assert!(!share(3, 3, 0).exceeds_lost_limit(1.0));
    }

    #[test]
    fn a_run_under_the_minimum_never_exceeds_the_limit() {
        let below = MIN_FETCHES_FOR_FAILED_SHARE - 1;
        assert!(!share(below, below, 0).exceeds_lost_limit(0.05));
        let at = MIN_FETCHES_FOR_FAILED_SHARE;
        assert!(share(at, at, 0).exceeds_lost_limit(0.05));
    }

    #[test]
    fn a_recovered_fetch_does_not_count() {
        assert!(!share(20, 0, 20).exceeds_lost_limit(0.05));
        assert!(!share(20, 1, 19).exceeds_lost_limit(0.05));
    }

    #[test]
    fn a_lost_readme_does_not_count() {
        let summary = FetchSummary {
            fetches: 2,
            failures: vec![failure("a", Source::Readme, None)],
        };
        assert_eq!(summary.lost_count(), 0);
        assert!(!summary.exceeds_lost_limit(0.05));
        assert_eq!(summary.lost_lines(), "");
        assert!(summary.text().contains("a: "), "{}", summary.text());
    }

    #[test]
    fn a_run_with_no_fetch_passes() {
        assert!(!FetchSummary::default().exceeds_lost_limit(0.05));
    }

    #[test]
    fn the_lost_lines_name_the_entry_the_source_and_the_error() {
        assert_eq!(mixed(9).lost_lines(), "  gamma: Homebrew: down");
    }

    #[test]
    fn many_failed_fetches_add_no_warning() {
        assert!(!mixed(3).text().contains("WARNING"));
        assert!(!mixed(3).markdown().contains("WARNING"));
    }

    #[test]
    fn failures_sort_by_slug_then_source() {
        let mut failures = [
            failure("b", Source::RepoFacts, None),
            failure("a", Source::Docker, None),
            failure("a", Source::RepoFacts, None),
        ];
        failures.sort();
        let order: Vec<(&str, Source)> = failures
            .iter()
            .map(|f| (f.slug.as_str(), f.source))
            .collect();
        assert_eq!(
            order,
            [
                ("a", Source::RepoFacts),
                ("a", Source::Docker),
                ("b", Source::RepoFacts)
            ]
        );
    }
}
