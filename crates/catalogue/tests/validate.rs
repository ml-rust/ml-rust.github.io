mod common;

use catalogue::{DataDirError, read_data_dir, validate};
use common::{expect_invalid, fixture, valid_summary};

#[test]
fn valid_set_passes_and_counts_its_contents() {
    assert_eq!(
        valid_summary("valid"),
        "ok: 3 entries, 7 judgments, 2 topics"
    );
}

#[test]
fn valid_set_reads_files_sorted_by_name() {
    let data = read_data_dir(&fixture("valid")).expect("fixture reads");
    let names: Vec<&str> = data
        .sources
        .entries
        .iter()
        .map(|file| file.path.rsplit('/').next().unwrap_or_default())
        .collect();
    assert_eq!(names, ["alpha.toml", "beta.toml", "gamma.toml"]);
    assert!(data.diagnostics.is_empty());
    assert!(data.sources.synonyms.is_some());
    assert!(data.sources.discovery.is_some());
}

#[test]
fn duplicate_crate_is_reported() {
    expect_invalid(
        "duplicate-crate",
        "crate `alpha-crate` is already listed",
        1,
    );
}

#[test]
fn unknown_entry_topic_is_reported() {
    expect_invalid("unknown-topic", "topic `missing` does not exist", 1);
}

#[test]
fn topic_parent_cycle_is_reported() {
    expect_invalid("topic-cycle", "parent chain forms a cycle", 2);
}

#[test]
fn judgment_for_unknown_entry_is_reported() {
    expect_invalid("unknown-entry", "entry `ghost` does not exist", 1);
}

#[test]
fn supersedes_cycle_is_reported() {
    expect_invalid("supersedes-cycle", "forms a cycle", 2);
}

#[test]
fn issue_link_that_differs_from_the_file_name_is_reported() {
    expect_invalid("issue-mismatch", "to match the file name", 1);
}

#[test]
fn approver_outside_the_maintainers_is_reported() {
    expect_invalid(
        "approver-not-maintainer",
        "approver `zed` is not a maintainer",
        1,
    );
}

#[test]
fn self_approval_is_reported() {
    expect_invalid("self-approval", "must differ from the reporter", 1);
}

#[test]
fn org_member_approving_an_affiliated_entry_is_reported() {
    expect_invalid("org-conflict", "must not be an org member", 1);
}

#[test]
fn synonym_term_in_two_groups_is_reported() {
    expect_invalid(
        "synonym-duplicate",
        "term `cuda` already appears in group 1",
        1,
    );
}

#[test]
fn discovery_rule_break_is_reported() {
    expect_invalid(
        "discovery-invalid",
        "discovery.toml: keyword `LLM` must be lowercase",
        1,
    );
}

#[test]
fn github_topic_rule_breaks_are_reported() {
    expect_invalid("discovery-github-invalid", "GitHub topic", 2);
}

#[test]
fn duplicate_maintainer_is_reported() {
    expect_invalid(
        "duplicate-maintainer",
        "maintainer `bob` is listed twice",
        1,
    );
}

#[test]
fn file_without_the_toml_extension_is_reported() {
    expect_invalid("non-toml", "notes.txt: not a .toml file", 1);
}

#[test]
fn parse_error_names_the_file_and_line() {
    expect_invalid("parse-error", "broken.toml: line 1:", 1);
}

#[test]
fn missing_data_directory_is_an_error_that_names_the_path() {
    let error = validate(&fixture("absent")).expect_err("directory is missing");
    assert!(matches!(error, DataDirError::Missing { .. }));
    assert!(error.to_string().contains("absent"));
}

#[test]
fn data_path_that_is_a_file_is_an_error() {
    let file = fixture("valid").join("topics.toml");
    let error = validate(&file).expect_err("path is a file");
    assert!(matches!(error, DataDirError::NotADirectory { .. }));
}
