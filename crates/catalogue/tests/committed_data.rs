//! The `data/` directory in the repository loads and passes every check.

use std::path::Path;

use catalogue::load_catalogue;

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
