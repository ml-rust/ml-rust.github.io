use std::collections::{BTreeMap, BTreeSet};

/// Whether `target` is reachable from `from` by following `edges`.
///
/// A node reaches itself.
pub(crate) fn reaches<K: Ord>(edges: &BTreeMap<K, Vec<K>>, from: &K, target: &K) -> bool {
    let mut seen: BTreeSet<&K> = BTreeSet::new();
    let mut stack: Vec<&K> = vec![from];
    while let Some(node) = stack.pop() {
        if node == target {
            return true;
        }
        if !seen.insert(node) {
            continue;
        }
        if let Some(next) = edges.get(node) {
            stack.extend(next.iter());
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    fn edges(list: &[(u8, u8)]) -> BTreeMap<u8, Vec<u8>> {
        let mut map: BTreeMap<u8, Vec<u8>> = BTreeMap::new();
        for (from, to) in list {
            map.entry(*from).or_default().push(*to);
        }
        map
    }

    #[test]
    fn follows_chains() {
        let g = edges(&[(1, 2), (2, 3)]);
        assert!(reaches(&g, &1, &3));
        assert!(!reaches(&g, &3, &1));
    }

    #[test]
    fn a_node_reaches_itself() {
        assert!(reaches(&edges(&[]), &1, &1));
    }

    #[test]
    fn terminates_on_cycles() {
        let g = edges(&[(1, 2), (2, 1)]);
        assert!(reaches(&g, &1, &2));
        assert!(!reaches(&g, &1, &9));
    }
}
