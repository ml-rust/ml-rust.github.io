//! Helpers shared by the integration tests. Every test file that includes this module uses each item.

use std::collections::BTreeMap;

use catalogue_core::{Catalogue, DataSources, SourceFile};
use catalogue_site::{SiteInputs, build_site};

/// Two topics, one nested under the other.
pub const TOPICS: &str = r#"[[topic]]
id = "ml"
name = "Machine learning"

[[topic]]
id = "tensors"
name = "Tensors"
parent = "ml"
"#;

/// A library entry on the `tensors` topic.
const ALPHA: &str = r#"name = "Alpha"
kind = "library"
repo = "https://github.com/ml-rust/alpha"
crates = ["alpha-crate"]
topics = ["tensors"]
added = 2026-11-03

[description]
text = "A tensor library."
source = "human"
"#;

/// The values of every threshold after `version` and `calibrated`.
const THRESHOLD_TABLES: &str = r#"
[new]
max_age_days = 180
min_active_users = 2.0

[library_featured]
min_active_users = 11.0
min_new_adopters = 9.0
min_former_glory = 0.5

[app_featured]
min_signals = 2
binary_downloads = 884
homebrew_on_request_365d = 500
repology_families = 2
docker_pull_growth_90d = 10000

[dormancy]
min_days = 365
dead_rot_signals = 2
dead_consecutive_checks = 2

[interval]
patch_gt3_major_ge1 = 700
patch_gt3 = 450
patch_gt0 = 300
major_ge1 = 200
minor_gt3 = 140
other = 80
days_per_release_cap = 50
downloads_per_month_double = 50000
active_users_double = 10.0

[rot]
unanswered_days = 270
maintainer_silence_days = 365
advisory_grace_days = 60
lost_glory_below = 0.5
build_check_max_age_days = 45

[finished]
min_former_glory = 0.8
min_new_adopters = 1.0
min_binary_downloads_per_release = 200
"#;

/// The text of a version 3 thresholds file, calibrated on `calibrated` when given.
pub fn thresholds(calibrated: Option<&str>) -> String {
    let date = calibrated.map_or_else(String::new, |date| format!("calibrated = {date}\n"));
    format!("version = 3\n{date}{THRESHOLD_TABLES}")
}

fn file(path: &str, contents: &str) -> SourceFile {
    SourceFile {
        path: path.to_owned(),
        contents: contents.to_owned(),
    }
}

/// A checked catalogue with the `alpha` entry, `topics`, and `thresholds` when given.
pub fn catalogue(topics: &str, thresholds: Option<&str>) -> Catalogue {
    let sources = DataSources {
        entries: vec![file("entries/alpha.toml", ALPHA)],
        topics: Some(file("topics.toml", topics)),
        thresholds: thresholds.map(|text| file("thresholds.toml", text)),
        ..DataSources::default()
    };
    Catalogue::load(sources)
        .unwrap_or_else(|diagnostics| panic!("fixture is invalid:\n{diagnostics}"))
}

/// Every file of a build without a tier report, keyed by path, as UTF-8 text.
pub fn pages(catalogue: &Catalogue) -> BTreeMap<String, String> {
    let inputs = SiteInputs {
        catalogue,
        report: None,
        changes: &[],
    };
    let files = build_site(&inputs).unwrap_or_else(|error| panic!("build fails: {error}"));
    files
        .into_iter()
        .map(|file| {
            let text = String::from_utf8(file.bytes)
                .unwrap_or_else(|error| panic!("{} is not UTF-8: {error}", file.path));
            (file.path, text)
        })
        .collect()
}
