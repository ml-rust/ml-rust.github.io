//! The `catalogue dump usage` action: adoption signals of named crates.

use std::collections::HashMap;
use std::path::PathBuf;

use catalogue_core::canonical_crate_name;
use catalogue_signals::dump::{DumpSnapshot, IdSet, utc_date};
use catalogue_signals::fetch;
use catalogue_signals::usage::{self, Usage};

use crate::cache_dir::resolve_cache_dir;
use crate::dump::{DumpCommandError, read_cached};

/// Reads the cached dump and renders the signals of each crate in `names`.
pub(crate) fn usage_dump(
    names: &[String],
    cache_dir: Option<PathBuf>,
    window_days: u16,
) -> Result<String, DumpCommandError> {
    let cache_dir = resolve_cache_dir(cache_dir)?;
    let snapshot = read_cached(&fetch::dump_path(&cache_dir), window_days)?;
    let ids = resolve_names(&snapshot, names)?;
    let targets: IdSet = ids.iter().copied().collect();
    let today = utc_date(snapshot.meta.timestamp);
    let found = usage::compute(&snapshot, &targets, today);
    let rows: Vec<(&str, &Usage)> = ids
        .iter()
        .filter_map(|id| {
            let name = snapshot.crates.get(id)?.name.as_str();
            Some((name, found.get(id)?))
        })
        .collect();
    Ok(render_usage(&rows))
}

/// The crate id of each name, in the order given, without repeats.
///
/// Names match the way crates.io matches them: ASCII case and `-` against
/// `_` do not matter. Every name with no crate is listed in the error.
pub fn resolve_names(
    snapshot: &DumpSnapshot,
    names: &[String],
) -> Result<Vec<u32>, DumpCommandError> {
    let by_name: HashMap<String, u32> = snapshot
        .crates
        .iter()
        .map(|(&id, row)| (canonical_crate_name(&row.name), id))
        .collect();
    let mut ids = Vec::new();
    let mut unknown = Vec::new();
    for name in names {
        match by_name.get(&canonical_crate_name(name)) {
            Some(id) if !ids.contains(id) => ids.push(*id),
            Some(_) => {}
            None => unknown.push(name.clone()),
        }
    }
    if unknown.is_empty() {
        Ok(ids)
    } else {
        Err(DumpCommandError::UnknownCrates { names: unknown })
    }
}

/// The `key: value` lines of `dump usage`, one block per crate, blank line between.
///
/// Signals print with 2 decimals, run counts as integers, `lost_users` as `true` or `false`.
pub fn render_usage(rows: &[(&str, &Usage)]) -> String {
    rows.iter()
        .map(|(name, found)| {
            let lines = [
                ("name", (*name).to_owned()),
                ("current", format!("{:.2}", found.current)),
                ("peak", format!("{:.2}", found.peak)),
                ("former_glory", format!("{:.2}", found.former_glory)),
                ("growth", format!("{:.2}", found.growth)),
                ("new_adopters", format!("{:.2}", found.new_adopters)),
                ("removed", found.removed.to_string()),
                ("expired", found.expired.to_string()),
                ("lost_users", found.lost_users().to_string()),
            ];
            lines
                .iter()
                .map(|(key, value)| format!("{key}: {value}"))
                .collect::<Vec<_>>()
                .join("\n")
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

#[cfg(test)]
mod tests {
    use catalogue_signals::dump::{CrateRow, DumpMeta, IdMap};

    use super::*;

    fn snapshot(names: &[(u32, &str)]) -> DumpSnapshot {
        let crates: IdMap<CrateRow> = names
            .iter()
            .map(|&(id, name)| {
                let row = CrateRow {
                    name: name.to_owned(),
                    created_at: Default::default(),
                    updated_at: Default::default(),
                    description: None,
                    homepage: None,
                    repository: None,
                    deprecation: None,
                    has_readme: false,
                };
                (id, row)
            })
            .collect();
        DumpSnapshot {
            meta: DumpMeta {
                timestamp: Default::default(),
                crates_io_commit: String::new(),
                directory: String::new(),
            },
            crates,
            versions: Default::default(),
            default_version: Default::default(),
            edges: Vec::new(),
            default_requirements: Default::default(),
            details: Default::default(),
            owners: Default::default(),
            users: Default::default(),
            teams: Default::default(),
            categories: Default::default(),
            keywords: Default::default(),
            deleted: Vec::new(),
            downloads: Default::default(),
            warnings: Default::default(),
        }
    }

    fn names(list: &[&str]) -> Vec<String> {
        list.iter().map(|name| (*name).to_owned()).collect()
    }

    #[test]
    fn names_resolve_like_crates_io_in_order() {
        let snapshot = snapshot(&[(1, "numr"), (2, "foo_bar")]);
        let ids = resolve_names(&snapshot, &names(&["Foo-Bar", "numr", "NUMR"])).expect("known");
        assert_eq!(ids, [2, 1]);
    }

    #[test]
    fn unknown_names_are_all_listed() {
        let snapshot = snapshot(&[(1, "numr")]);
        let error =
            resolve_names(&snapshot, &names(&["nope", "numr", "gone"])).expect_err("unknown");
        assert!(matches!(
            error,
            DumpCommandError::UnknownCrates { ref names } if names == &["nope", "gone"]
        ));
    }

    #[test]
    fn usage_prints_every_key_with_two_decimals() {
        let found = Usage {
            current: 2.0,
            peak: 3.333,
            former_glory: 0.876,
            growth: 1.0,
            new_adopters: 0.5,
            removed: 4,
            expired: 1,
            history: Vec::new(),
        };
        let text = render_usage(&[("numr", &found), ("blazr", &found)]);
        let block = "current: 2.00\npeak: 3.33\nformer_glory: 0.88\ngrowth: 1.00\n\
            new_adopters: 0.50\nremoved: 4\nexpired: 1\nlost_users: true";
        assert_eq!(text, format!("name: numr\n{block}\n\nname: blazr\n{block}"));
    }
}
