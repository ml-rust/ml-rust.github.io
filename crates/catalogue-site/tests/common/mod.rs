//! Helpers shared by the integration tests. Every test file that includes this module uses each item.

use std::collections::BTreeMap;

use catalogue_core::{Catalogue, DataSources};

pub use crate::build::thresholds;
use crate::build::{render, source};

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

/// A checked catalogue with the `alpha` entry, `topics`, and `thresholds` when given.
pub fn catalogue(topics: &str, thresholds: Option<&str>) -> Catalogue {
    let sources = DataSources {
        entries: vec![source("entries/alpha.toml", ALPHA)],
        topics: Some(source("topics.toml", topics)),
        thresholds: thresholds.map(|text| source("thresholds.toml", text)),
        ..DataSources::default()
    };
    Catalogue::load(sources)
        .unwrap_or_else(|diagnostics| panic!("fixture is invalid:\n{diagnostics}"))
}

/// Every file of a build without a tier report, keyed by path, as UTF-8 text.
pub fn pages(catalogue: &Catalogue) -> BTreeMap<String, String> {
    render(catalogue, None, &[])
}
