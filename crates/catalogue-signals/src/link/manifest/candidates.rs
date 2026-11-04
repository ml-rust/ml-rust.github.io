//! Which manifests of a repo tree the proof reads, and in what order.

use catalogue_core::canonical_crate_name;

/// The most manifests read per check.
pub(super) const MAX_MANIFESTS: usize = 40;

/// The file name of a manifest.
const MANIFEST: &str = "Cargo.toml";

/// Directories that hold copies of other projects. A manifest there proves nothing about this repo.
const COPIED: &[&str] = &[
    "vendor",
    "vendored",
    "third_party",
    "third-party",
    "thirdparty",
    "external",
];

/// The manifests among `paths`, in read order, up to [`MAX_MANIFESTS`].
///
/// The root manifest comes first. Next come the manifests whose parent directory is named like
/// `crate_name`, with `-` and `_` read as equal. The rest follow. Each group sorts by depth, then
/// by path. A path that a URL or a published note cannot carry stays out, and so does a path under
/// a directory of [`COPIED`].
pub(super) fn manifest_candidates(paths: Vec<String>, crate_name: &str) -> Vec<String> {
    let wanted = canonical_crate_name(crate_name);
    let mut ranked: Vec<(u8, usize, String)> = paths
        .into_iter()
        .filter(|path| is_manifest(path))
        .map(|path| (group(&path, &wanted), depth(&path), path))
        .collect();
    ranked.sort();
    ranked.dedup();
    ranked
        .into_iter()
        .take(MAX_MANIFESTS)
        .map(|(_, _, path)| path)
        .collect()
}

/// The read group of a manifest: 0 for the root, 1 for a parent named like the crate, 2 otherwise.
fn group(path: &str, wanted: &str) -> u8 {
    let parent = path.rsplit('/').nth(1);
    match parent {
        None => 0,
        Some(parent) if canonical_crate_name(parent) == wanted => 1,
        Some(_) => 2,
    }
}

fn depth(path: &str) -> usize {
    path.bytes().filter(|&b| b == b'/').count()
}

/// Whether `path` names a manifest with no empty, `.` or `..` segment and no control character.
fn is_manifest(path: &str) -> bool {
    path.rsplit('/').next() == Some(MANIFEST)
        && !path.chars().any(char::is_control)
        && path
            .split('/')
            .all(|segment| !matches!(segment, "" | "." | "..") && !COPIED.contains(&segment))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn owned(paths: &[&str]) -> Vec<String> {
        paths.iter().map(|&path| path.to_owned()).collect()
    }

    #[test]
    fn a_manifest_under_a_directory_of_copies_stays_out() {
        let paths = owned(&[
            "vendor/my-crate/Cargo.toml",
            "third_party/my-crate/Cargo.toml",
            "crates/external/my-crate/Cargo.toml",
            "crates/my-crate/Cargo.toml",
        ]);
        assert_eq!(
            manifest_candidates(paths, "my-crate"),
            ["crates/my-crate/Cargo.toml"]
        );
    }

    #[test]
    fn the_root_comes_first_then_a_parent_named_like_the_crate_then_depth_and_path() {
        let paths = owned(&[
            "z/Cargo.toml",
            "a/b/Cargo.toml",
            "crates/my_crate/Cargo.toml",
            "my-crate/Cargo.toml",
            "a/Cargo.toml",
            "Cargo.toml",
            "src/lib.rs",
        ]);
        assert_eq!(
            manifest_candidates(paths, "my-crate"),
            [
                "Cargo.toml",
                "my-crate/Cargo.toml",
                "crates/my_crate/Cargo.toml",
                "a/Cargo.toml",
                "z/Cargo.toml",
                "a/b/Cargo.toml",
            ]
        );
    }

    #[test]
    fn the_order_does_not_depend_on_the_tree_order() {
        let forward = owned(&["b/Cargo.toml", "a/Cargo.toml", "Cargo.toml"]);
        let mut backward = forward.clone();
        backward.reverse();
        assert_eq!(
            manifest_candidates(forward, "x"),
            manifest_candidates(backward, "x")
        );
    }

    #[test]
    fn the_list_stops_at_the_cap_and_keeps_the_best_ranked() {
        let mut paths: Vec<String> = (0..MAX_MANIFESTS + 5)
            .map(|index| format!("dir{index:03}/Cargo.toml"))
            .collect();
        paths.push("x/y/z/hit/Cargo.toml".to_owned());
        let found = manifest_candidates(paths, "hit");
        assert_eq!(found.len(), MAX_MANIFESTS);
        assert_eq!(found[0], "x/y/z/hit/Cargo.toml");
        assert_eq!(found[1], "dir000/Cargo.toml");
    }

    #[test]
    fn only_a_usable_manifest_path_counts() {
        let paths = owned(&[
            "cargo.toml",
            "NotCargo.toml",
            "Cargo.toml.orig",
            "a/../Cargo.toml",
            "./Cargo.toml",
            "a//Cargo.toml",
            "a\nb/Cargo.toml",
        ]);
        assert!(manifest_candidates(paths, "x").is_empty());
    }

    #[test]
    fn a_repeated_path_counts_once() {
        let paths = owned(&["a/Cargo.toml", "a/Cargo.toml"]);
        assert_eq!(manifest_candidates(paths, "x"), ["a/Cargo.toml"]);
    }
}
