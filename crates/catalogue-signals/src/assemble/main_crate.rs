//! The main crate of a repository that an entry leaves out.

use catalogue_core::{Entry, normalize_repo_url};

use super::index::CrateIndex;
use crate::dump::DumpSnapshot;
use crate::seed::RepoName;

/// The crate with the repository's own name that claims the repository of `entry`, when the entry
/// lists no crate with that name. The first name in sort order when several qualify.
///
/// The repository route measures a whole repository. An entry that lists only side crates of a
/// bigger repository does not inherit its numbers. Any host works, because the link is the only
/// input.
pub(super) fn main_crate_left_out(
    entry: &Entry,
    dump: &DumpSnapshot,
    index: &CrateIndex,
) -> Option<String> {
    let key = normalize_repo_url(entry.repo.as_str())?;
    let repo = RepoName::new(key.rsplit('/').next().unwrap_or_default());
    if entry
        .crates
        .iter()
        .any(|name| repo.is_own_name(name.as_str()))
    {
        return None;
    }
    index
        .claimants(&key)
        .iter()
        .filter_map(|id| dump.crates.get(id))
        .map(|row| row.name.as_str())
        .filter(|name| repo.is_own_name(name))
        .min()
        .map(str::to_owned)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assemble::fixtures::{Dump, library};

    /// A dump where each `(id, name, link)` crate names `link`.
    fn dump(crates: &[(u32, &str, &str)]) -> Dump {
        let mut dump = Dump::new();
        for (id, name, link) in crates {
            dump.add_crate(*id, name).repository = Some((*link).to_owned());
        }
        dump
    }

    /// A library entry that lists `crates` and holds the repository `link`.
    fn entry(crates: &[&str], link: &str) -> Entry {
        let mut entry = library(crates);
        entry.repo = link.parse().expect("link");
        entry
    }

    fn left_out(crates: &[&str], link: &str, dump: &Dump) -> Option<String> {
        let index = CrateIndex::new(&dump.snapshot);
        main_crate_left_out(&entry(crates, link), &dump.snapshot, &index)
    }

    const WASMTIME: &str = "https://github.com/bytecodealliance/wasmtime";

    #[test]
    fn an_entry_of_side_crates_leaves_the_main_crate_out() {
        let found = dump(&[
            (1, "wasmtime", WASMTIME),
            (2, "wasmtime-wasi-nn", WASMTIME),
            (3, "wasmtime-wasi-threads", WASMTIME),
        ]);
        assert_eq!(
            left_out(
                &["wasmtime-wasi-nn", "wasmtime-wasi-threads"],
                WASMTIME,
                &found
            )
            .as_deref(),
            Some("wasmtime")
        );
    }

    #[test]
    fn a_listed_main_crate_leaves_nothing_out_in_any_spelling() {
        let link = "https://github.com/o/foo-bar";
        let found = dump(&[(1, "foo_bar", link), (2, "foo-bar-core", link)]);
        assert_eq!(left_out(&["foo-bar", "foo-bar-core"], link, &found), None);
    }

    #[test]
    fn a_family_without_a_crate_of_the_repo_name_stays_eligible() {
        let link = "https://github.com/huggingface/candle";
        let found = dump(&[
            (1, "candle-core", link),
            (2, "candle-nn", link),
            (3, "candle", "https://github.com/someone/else"),
        ]);
        assert_eq!(left_out(&["candle-core", "candle-nn"], link, &found), None);
    }

    #[test]
    fn a_listed_crate_of_the_repo_name_keeps_a_candle_shape_eligible() {
        let link = "https://github.com/huggingface/candle";
        let found = dump(&[(1, "candle", link), (2, "candle-core", link)]);
        assert_eq!(left_out(&["candle-core", "candle"], link, &found), None);
    }

    #[test]
    fn a_language_affix_on_the_repo_name_is_ignored() {
        for (link, name) in [
            ("https://github.com/o/tch-rs", "tch"),
            ("https://github.com/o/rust-tch", "tch"),
            ("https://github.com/o/tch-rs", "tch_rs"),
            ("https://github.com/o/mistral.rs", "mistralrs"),
        ] {
            let found = dump(&[(1, name, link), (2, "side", link)]);
            assert_eq!(left_out(&["side"], link, &found).as_deref(), Some(name));
        }
    }

    #[test]
    fn an_affix_on_the_crate_name_is_no_main_crate() {
        let found = dump(&[(1, "wasmtime-rust", WASMTIME), (2, "wasmtime", WASMTIME)]);
        assert_eq!(left_out(&["wasmtime"], WASMTIME, &found), None);
        let side = dump(&[(1, "wasmtime-rust", WASMTIME), (2, "side", WASMTIME)]);
        assert_eq!(left_out(&["side"], WASMTIME, &side), None);
    }

    #[test]
    fn a_repo_on_another_host_works_the_same() {
        let link = "https://codeberg.org/o/tool";
        let found = dump(&[(1, "tool", link), (2, "tool-extra", link)]);
        assert_eq!(
            left_out(&["tool-extra"], link, &found).as_deref(),
            Some("tool")
        );
    }

    #[test]
    fn the_same_name_on_another_owner_is_no_claim() {
        let found = dump(&[(1, "wasmtime", "https://github.com/fork/wasmtime")]);
        assert_eq!(left_out(&["side"], WASMTIME, &found), None);
    }
}
