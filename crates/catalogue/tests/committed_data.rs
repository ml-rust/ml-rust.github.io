//! The `data/` directory in the repository loads and passes every check.

use std::path::Path;

use catalogue::{load_catalogue, parse_leads};

#[test]
fn the_committed_data_directory_is_valid() {
    let data = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
    let catalogue = load_catalogue(&data)
        .expect("the data directory reads")
        .expect("the data directory is valid");
    let backends = catalogue.backends();
    assert!(!backends.backend.is_empty());
    assert!(!backends.python.is_empty());
    assert!(!backends.not_native.is_empty());
    assert!(!backends.native_build.is_empty());
}

#[test]
fn the_committed_lead_list_parses_and_holds_what_belongs() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/leads.txt");
    let text = std::fs::read_to_string(&path).expect("the lead list reads");
    let leads: Vec<String> = parse_leads(&path, &text)
        .expect("the lead list parses")
        .iter()
        .map(ToString::to_string)
        .collect();
    for name in [
        "axp",
        "blazr",
        "boostr",
        "compressr",
        "nexar",
        "numr",
        "oxidizr",
        "regexr",
        "solvr",
        "splintr",
        "treeboost",
        "venus",
        "vidr",
        "voxr",
    ] {
        let lead = format!("ml-rust/{name}");
        assert_eq!(
            leads.iter().filter(|listed| **listed == lead).count(),
            1,
            "{lead}"
        );
    }
    for name in ["ml-rust/fluxbench", "ml-rust/cargo-reclaim"] {
        assert!(!leads.iter().any(|lead| lead == name), "{name}");
    }
}
