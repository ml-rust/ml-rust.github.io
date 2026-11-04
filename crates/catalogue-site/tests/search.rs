//! The search index: one record per entry, deterministic files, and a native query.

#[path = "common/build.rs"]
mod build;
#[path = "common/fixture.rs"]
mod fixture;

use std::collections::BTreeMap;

use catalogue_core::{Catalogue, Movement, Slug, TierReport};
use catalogue_site::{
    NO_TIER_BOOST, OutputFile, SEARCH_DIR, SiteInputs, build_site, search_records,
};
use oxidoc_search::builder::write::{META_FILE, chunk_file_name};
use oxidoc_search::records::RECORDS_FILE;
use oxidoc_search::{Field, LexicalSearcher, Record, SearchQuery};

/// The fixture catalogue and its tier report.
fn data() -> (Catalogue, TierReport) {
    fixture::data(&fixture::files())
}

/// Every file of a build, keyed by path.
fn build(catalogue: &Catalogue, report: Option<&TierReport>) -> BTreeMap<String, Vec<u8>> {
    let files = build_site(&SiteInputs {
        catalogue,
        report,
        changes: &[],
        first_check: None,
        movement: &Movement::NeedsHistory,
    })
    .unwrap_or_else(|error| panic!("build fails: {error}"));
    files
        .into_iter()
        .map(|OutputFile { path, bytes }| (path, bytes))
        .collect()
}

/// The record of entry `slug`.
fn record<'a>(records: &'a [Record], slug: &str) -> &'a Record {
    records
        .iter()
        .find(|record| record.id == slug)
        .unwrap_or_else(|| panic!("no record for {slug}"))
}

/// The fields of `record` as `(name, text, weight)`.
fn fields(record: &Record) -> Vec<(&str, &str, f32)> {
    record
        .fields
        .iter()
        .map(|Field { name, text, weight }| (name.as_str(), text.as_str(), *weight))
        .collect()
}

/// The tags of `record` as `(name, value)`.
fn tags(record: &Record) -> Vec<(&str, &str)> {
    record
        .tags
        .iter()
        .map(|(name, value)| (name.as_str(), value.as_str()))
        .collect()
}

/// The path of index file `name` in the build output.
fn search_path(name: &str) -> String {
    format!("{SEARCH_DIR}/{name}")
}

/// A searcher over the index files of `files`, with every chunk the query needs.
fn searcher(files: &BTreeMap<String, Vec<u8>>, query: &str) -> LexicalSearcher {
    let read = |name: &str| {
        files
            .get(&search_path(name))
            .unwrap_or_else(|| panic!("the build has no {name}"))
    };
    let mut searcher = LexicalSearcher::from_metadata(read(META_FILE)).expect("metadata");
    for id in searcher.needed_chunk_ids(query) {
        searcher
            .load_chunk(read(&chunk_file_name(id)))
            .expect("chunk");
    }
    searcher.load_records(read(RECORDS_FILE)).expect("records");
    searcher
}

/// The paths of the hits of `text` under `filters`, best first.
fn hits(files: &BTreeMap<String, Vec<u8>>, text: &str, filters: &[(&str, &str)]) -> Vec<String> {
    let query = SearchQuery {
        text: text.to_owned(),
        max_results: 10,
        filters: filters
            .iter()
            .map(|&(name, value)| (name.to_owned(), vec![value.to_owned()]))
            .collect(),
    };
    searcher(files, text)
        .search(&query)
        .into_iter()
        .map(|hit| hit.path)
        .collect()
}

#[test]
fn each_entry_has_one_record_in_slug_order() {
    let (catalogue, report) = data();
    let records = search_records(&catalogue, Some(&report));
    let ids: Vec<&str> = records.iter().map(|record| record.id.as_str()).collect();
    assert_eq!(ids, ["alpha", "beta", "delta", "gamma"]);
}

#[test]
fn a_record_links_names_and_weights_the_entry() {
    let (catalogue, report) = data();
    let records = search_records(&catalogue, Some(&report));
    let alpha = record(&records, "alpha");
    assert_eq!(alpha.url, "/projects/alpha/");
    assert_eq!(alpha.title, "Alpha");
    assert_eq!(alpha.snippet.as_deref(), Some("A tensor library."));
    assert_eq!(
        fields(alpha),
        [
            ("name", "Alpha alpha-crate alpha-extra", 3.0),
            ("topics", "Tensors", 2.0),
            ("category", "Tensor libraries", 2.0),
            ("description", "A tensor library.", 1.5),
        ]
    );
    assert_eq!(
        tags(alpha),
        [
            ("kind", "library"),
            ("org", "community"),
            ("topic", "tensors"),
            ("category", "tensor-libraries"),
            ("license", "apache-2-0-or-mit"),
            ("runs_on", "cuda"),
            ("runs_on", "metal"),
            ("row_downloads", "1.8M"),
            ("row_downloads_full", "1,840,052 downloads over 30 days"),
            ("row_dependents", "12"),
            ("row_stars", "1,200"),
            ("row_released", "1 Sep 2026"),
            ("row_gpu", "CUDA, Metal"),
            ("row_native", "Required"),
            ("row_native_full", "Binds native code: openssl-sys"),
            ("row_python", "Python bindings"),
            (
                "row_python_full",
                "Has Python bindings: alpha-extra needs Python"
            ),
            ("row_license", "Apache-2.0 OR MIT"),
            ("row_version", "0.4.2"),
            ("row_released_iso", "2026-09-01"),
            ("tier", "featured"),
            ("row_description", "A tensor library."),
        ]
    );
    assert_eq!(alpha.boost, 1.5);
}

#[test]
fn synonyms_of_the_entry_text_become_aliases() {
    let (catalogue, report) = data();
    let records = search_records(&catalogue, Some(&report));
    let beta = record(&records, "beta");
    assert_eq!(
        fields(beta),
        [
            ("name", "Beta", 3.0),
            ("topics", "Machine learning", 2.0),
            ("category", "Inference runtimes", 2.0),
            ("description", "A command line tool that runs models.", 1.5),
            ("aliases", "ml, cli, command-line interface", 1.0),
        ]
    );
    assert_eq!(
        tags(beta),
        [
            ("kind", "app"),
            ("org", "ml-rust"),
            ("topic", "ml"),
            ("category", "inference-runtimes"),
            ("license", "none-stated"),
            ("runs_on", "not-measured"),
            ("row_downloads", "1.5k per release"),
            ("row_downloads_full", "1,500 binary downloads per release"),
            ("row_dependents", "Not applicable"),
            ("row_stars", "40"),
            ("row_released", "Not measured"),
            ("row_gpu", "Not measured"),
            ("row_native", "Not measured"),
            ("row_license", "None stated"),
            ("row_status", "Stable"),
            (
                "row_status_meaning",
                "The project is finished: dormant by design, not decayed."
            ),
            ("tier", "listed"),
            ("row_description", "A command line tool that runs models."),
        ]
    );
    assert_eq!(beta.boost, 1.1);
    let delta = record(&records, "delta");
    assert_eq!(
        delta.fields.last().map(|field| field.text.as_str()),
        Some("autodiff")
    );
}

/// The fixture report with `terms` on entry `name`.
fn with_terms(report: &mut TierReport, name: &str, terms: &[&str]) {
    let slug = Slug::new(name).expect("slug");
    let found = report.entries.get_mut(&slug).expect("record");
    found.search_terms = terms.iter().map(|term| (*term).to_owned()).collect();
}

#[test]
fn the_keywords_field_holds_each_term_and_its_words_below_the_description() {
    let (catalogue, mut report) = data();
    with_terms(&mut report, "beta", &["linear-algebra", "science::math"]);
    let records = search_records(&catalogue, Some(&report));
    let beta = fields(record(&records, "beta"));
    let names: Vec<&str> = beta.iter().map(|(name, _, _)| *name).collect();
    assert_eq!(
        names,
        [
            "name",
            "topics",
            "category",
            "description",
            "keywords",
            "aliases"
        ]
    );
    assert_eq!(
        beta[4],
        (
            "keywords",
            "linear-algebra, linear, algebra, science::math, science, math",
            1.2
        )
    );
    assert!(beta[3].2 > beta[4].2 && beta[4].2 >= beta[5].2, "{beta:?}");
}

#[test]
fn a_query_for_one_word_finds_an_entry_tagged_with_the_longer_term() {
    let (catalogue, mut report) = data();
    with_terms(&mut report, "gamma", &["linear-algebra"]);
    let files = build(&catalogue, Some(&report));
    assert_eq!(hits(&files, "algebra", &[]), ["/projects/gamma/"]);
}

#[test]
fn synonym_lookup_reads_the_terms_and_never_spans_two_of_them() {
    let (catalogue, mut report) = data();
    with_terms(&mut report, "gamma", &["autodiff", "machine", "learning"]);
    let records = search_records(&catalogue, Some(&report));
    let gamma = fields(record(&records, "gamma"));
    let aliases = gamma
        .iter()
        .find(|(name, _, _)| *name == "aliases")
        .expect("aliases");
    assert_eq!(aliases.1, "automatic differentiation");
}

#[test]
fn an_entry_with_no_terms_and_a_build_without_a_report_have_no_keywords_field() {
    let (catalogue, report) = data();
    for report in [Some(&report), None] {
        for found in search_records(&catalogue, report) {
            assert!(
                fields(&found)
                    .iter()
                    .all(|(name, _, _)| *name != "keywords"),
                "{}",
                found.id
            );
        }
    }
}

#[test]
fn an_entry_without_a_category_has_no_category_field_or_tag() {
    let (catalogue, report) = data();
    let records = search_records(&catalogue, Some(&report));
    let delta = record(&records, "delta");
    assert!(fields(delta).iter().all(|(name, _, _)| *name != "category"));
    assert!(tags(delta).iter().all(|(name, _)| *name != "category"));
}

#[test]
fn the_category_filter_and_its_facets_work_like_the_topic_filter() {
    let (catalogue, report) = data();
    let files = build(&catalogue, Some(&report));
    let filtered = hits(&files, "tensor", &[("category", "tensor-libraries")]);
    assert_eq!(filtered, ["/projects/alpha/", "/projects/gamma/"]);
    assert!(hits(&files, "tensor", &[("category", "vector-databases")]).is_empty());
    assert_eq!(
        hits(&files, "runtimes", &[]),
        ["/projects/beta/"],
        "a category name is searchable"
    );
    let facets = searcher(&files, "").facets("", &[]);
    let (_, counts) = facets
        .iter()
        .find(|(key, _)| key == "category")
        .expect("a category facet");
    assert_eq!(
        counts.as_slice(),
        [
            ("tensor-libraries".to_owned(), 2),
            ("inference-runtimes".to_owned(), 1),
        ]
    );
}

#[test]
fn the_license_and_runs_on_filters_and_their_facets_work_like_the_category_filter() {
    let (catalogue, report) = data();
    let files = build(&catalogue, Some(&report));
    assert_eq!(
        hits(&files, "tensor", &[("runs_on", "cuda")]),
        ["/projects/alpha/"]
    );
    assert!(hits(&files, "tensor", &[("license", "none-stated")]).is_empty());
    assert_eq!(
        hits(&files, "command", &[("license", "none-stated")]),
        ["/projects/beta/"]
    );
    let facets = searcher(&files, "").facets("", &[]);
    let counts = |name: &str| {
        let (_, found) = facets
            .iter()
            .find(|(key, _)| key == name)
            .unwrap_or_else(|| panic!("no {name} facet"));
        found.clone()
    };
    let license = counts("license");
    for expected in [
        ("apache-2-0-or-mit".to_owned(), 1),
        ("mit".to_owned(), 1),
        ("none-stated".to_owned(), 1),
        ("not-measured".to_owned(), 1),
    ] {
        assert!(license.contains(&expected), "{expected:?}: {license:?}");
    }
    let runs_on = counts("runs_on");
    for expected in [
        ("cuda".to_owned(), 1),
        ("metal".to_owned(), 1),
        ("no-gpu-feature".to_owned(), 1),
        ("not-measured".to_owned(), 2),
    ] {
        assert!(runs_on.contains(&expected), "{expected:?}: {runs_on:?}");
    }
}

#[test]
fn judgments_set_the_topics_and_tiers_set_the_boost() {
    let (catalogue, report) = data();
    let records = search_records(&catalogue, Some(&report));
    let delta = record(&records, "delta");
    assert_eq!(delta.fields[1].text, "Tensors");
    assert!(tags(delta).contains(&("topic", "tensors")));
    assert!(tags(delta).contains(&("tier", "new")));
    assert_eq!(delta.boost, 1.0);
    assert_eq!(record(&records, "gamma").boost, 0.7);
}

#[test]
fn a_build_without_a_report_has_no_tier() {
    let (catalogue, _) = data();
    for record in search_records(&catalogue, None) {
        assert!(
            record
                .tags
                .iter()
                .all(|(name, _)| !["tier", "license", "runs_on"].contains(&name.as_str())),
            "{}",
            record.id
        );
        assert_eq!(record.boost, NO_TIER_BOOST, "{}", record.id);
    }
}

#[test]
fn the_build_writes_the_index_files() {
    let (catalogue, report) = data();
    let files = build(&catalogue, Some(&report));
    for name in [META_FILE, RECORDS_FILE, &chunk_file_name(0)] {
        assert!(files.contains_key(&search_path(name)), "{name}");
    }
    let rendered = fixture::site(&fixture::files(), true);
    assert!(
        rendered
            .keys()
            .all(|path| !(path.starts_with(SEARCH_DIR) && path.ends_with(".bin")))
    );
    assert!(rendered.contains_key(&search_path("index.html")));
}

#[test]
fn two_builds_give_the_same_bytes() {
    let (catalogue, report) = data();
    for report in [Some(&report), None] {
        assert_eq!(build(&catalogue, report), build(&catalogue, report));
    }
}

#[test]
fn a_query_finds_the_best_entry_first() {
    let (catalogue, report) = data();
    let files = build(&catalogue, Some(&report));
    let found = hits(&files, "tensor", &[]);
    assert_eq!(found.first().map(String::as_str), Some("/projects/alpha/"));
    assert_eq!(hits(&files, "autodiff", &[]), ["/projects/delta/"]);
    assert_eq!(
        hits(&files, "models", &[("org", "ml-rust")]),
        ["/projects/beta/"]
    );
}
