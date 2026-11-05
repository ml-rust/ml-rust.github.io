//! Helpers shared by the integration tests. Every test file that includes this module uses each item.

use std::collections::BTreeMap;

use catalogue_core::{Catalogue, DataSources, Movement};
use catalogue_site::SiteInputs;

pub use crate::build::thresholds;
use crate::build::{render, source};

/// Two topics, one nested under the other.
pub const TOPICS: &str = r#"[[topic]]
id = "ml"
name = "Machine learning"
blurb = "Teaching programs from data. Find models and tools."

[[topic]]
id = "tensors"
name = "Tensors"
blurb = "Arrays and the math on them. Find tensor libraries."
parent = "ml"
"#;

/// One category that no entry names.
const CATEGORIES: &str = r#"[[category]]
id = "vector-databases"
name = "Vector databases"
blurb = "Databases that store vectors and find the ones nearest to a query."
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

/// A checked catalogue with the `alpha` entry, `topics`, and `thresholds` when given.
pub fn catalogue(topics: &str, thresholds: Option<&str>) -> Catalogue {
    let sources = DataSources {
        entries: vec![source("entries/alpha.toml", ALPHA)],
        topics: Some(source("topics.toml", topics)),
        categories: Some(source("categories.toml", CATEGORIES)),
        thresholds: thresholds.map(|text| source("thresholds.toml", text)),
        ..DataSources::default()
    };
    Catalogue::load(sources)
        .unwrap_or_else(|diagnostics| panic!("fixture is invalid:\n{diagnostics}"))
}

/// Every file of a build without a tier report, keyed by path, as UTF-8 text.
pub fn pages(catalogue: &Catalogue) -> BTreeMap<String, String> {
    render(&SiteInputs {
        catalogue,
        report: None,
        changes: &[],
        first_check: None,
        movement: &Movement::NeedsHistory,
    })
}
