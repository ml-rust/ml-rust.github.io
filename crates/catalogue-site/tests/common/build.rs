//! Primitives under both `common` and `fixture`. Each of them uses every item.

use std::collections::BTreeMap;

use catalogue_core::{Catalogue, SourceFile, TierReport};
use catalogue_site::{SEARCH_DIR, SiteInputs, TierChangeRow, build_site};

/// The values of every threshold after `version` and `calibrated`.
const THRESHOLD_TABLES: &str = include_str!("thresholds.toml");

/// The text of a version 3 thresholds file, calibrated on `calibrated` when given.
pub fn thresholds(calibrated: Option<&str>) -> String {
    let date = calibrated.map_or_else(String::new, |date| format!("calibrated = {date}\n"));
    format!("version = 3\n{date}\n{THRESHOLD_TABLES}")
}

/// A data file at `path`.
pub fn source(path: &str, contents: &str) -> SourceFile {
    SourceFile {
        path: path.to_owned(),
        contents: contents.to_owned(),
    }
}

/// Every page and asset of a build over `catalogue`, keyed by path, as UTF-8 text.
///
/// The binary files under [`SEARCH_DIR`] are left out.
pub fn render(
    catalogue: &Catalogue,
    report: Option<&TierReport>,
    changes: &[TierChangeRow],
) -> BTreeMap<String, String> {
    let inputs = SiteInputs {
        catalogue,
        report,
        changes,
    };
    let files = build_site(&inputs).unwrap_or_else(|error| panic!("build fails: {error}"));
    let search = format!("{SEARCH_DIR}/");
    files
        .into_iter()
        .filter(|file| !file.path.starts_with(&search))
        .map(|file| {
            let text = String::from_utf8(file.bytes)
                .unwrap_or_else(|error| panic!("{} is not UTF-8: {error}", file.path));
            (file.path, text)
        })
        .collect()
}
