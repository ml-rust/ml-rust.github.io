//! Resolution of crate names in the rules to crate ids.

use std::collections::hash_map::Entry;
use std::collections::{HashMap, HashSet};

use catalogue_core::{canonical_crate_name, push_canonical_crate_name};

use crate::dump::{DumpSnapshot, IdMap};

/// The crates of one rule list found in a snapshot.
pub(super) struct RuleCrates {
    /// The rule's spelling of each found crate, by crate id.
    pub(super) by_id: IdMap<String>,
    /// Rule names with no crate in the snapshot, in rule order.
    pub(super) unknown: Vec<String>,
}

/// Resolves `names` against `snapshot.crates` in one pass over the crates.
///
/// Repeated names, which crates.io reads as one crate, count once.
pub(super) fn resolve(snapshot: &DumpSnapshot, names: &[String]) -> RuleCrates {
    let mut wanted: HashMap<String, &str> = HashMap::new();
    let mut order: Vec<String> = Vec::new();
    for name in names {
        if let Entry::Vacant(slot) = wanted.entry(canonical_crate_name(name)) {
            order.push(slot.key().clone());
            slot.insert(name.as_str());
        }
    }
    let mut by_id = IdMap::default();
    let mut found: HashSet<&str> = HashSet::new();
    let mut buffer = String::new();
    for (&id, row) in &snapshot.crates {
        buffer.clear();
        push_canonical_crate_name(&mut buffer, &row.name);
        if let Some(&spelling) = wanted.get(buffer.as_str()) {
            by_id.insert(id, spelling.to_owned());
            found.insert(spelling);
        }
    }
    let unknown = order
        .iter()
        .filter_map(|key| wanted.get(key.as_str()).copied())
        .filter(|spelling| !found.contains(spelling))
        .map(str::to_owned)
        .collect();
    RuleCrates { by_id, unknown }
}
