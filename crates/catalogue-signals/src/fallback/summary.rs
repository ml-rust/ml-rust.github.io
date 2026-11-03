//! The fallback summary of one `catalogue tiers` run.

use catalogue_core::{Date, Slug};

use super::source::Source;

/// One fetch that failed and fell back, or left its signal unknown.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct FetchFailure {
    /// The entry.
    pub slug: Slug,
    /// The source that failed.
    pub source: Source,
    /// The date of the value in use. `None` when the signal reads as unknown.
    pub from: Option<Date>,
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

    fn count_line(&self) -> String {
        format!(
            "{} of {} network fetches",
            self.fallback_count(),
            self.fetches
        )
    }

    /// The warning for a run where more than half the fetches failed.
    fn warning(&self) -> Option<String> {
        (self.failures.len().saturating_mul(2) > self.fetches).then(|| {
            format!(
                "WARNING: {} of {} network fetches failed: {} fell back, {} read as unknown. Each value keeps the date of its data",
                self.failures.len(),
                self.fetches,
                self.fallback_count(),
                self.lost().count()
            )
        })
    }

    /// The `fallbacks:` block of the text report.
    ///
    /// One `slug: source (from DATE)` line per fallback, then the count. A block
    /// of fetches that left their signal unknown follows when any did, then the
    /// warning when more than half the fetches failed.
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
        lines.extend(self.warning());
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
        if let Some(warning) = self.warning() {
            lines.extend([String::new(), format!("**{warning}**")]);
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

    #[test]
    fn more_than_half_failed_warns_and_exactly_half_does_not() {
        assert!(!mixed(6).text().contains("WARNING"));
        let text = mixed(5).text();
        assert!(
            text.ends_with(
                "WARNING: 3 of 5 network fetches failed: 2 fell back, 1 read as unknown. Each value keeps the date of its data"
            ),
            "{text}"
        );
        assert!(mixed(5).markdown().contains("**WARNING: 3 of 5"));
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
