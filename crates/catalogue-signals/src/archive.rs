//! Entry helpers shared by the tar readers.

use std::path::{Component, Path};

use tar::EntryType;

/// Whether `kind` is an entry that holds file content.
pub(crate) fn is_file(kind: EntryType) -> bool {
    matches!(kind, EntryType::Regular | EntryType::Continuous)
}

/// The UTF-8 name parts of `path` without `.` parts.
///
/// Returns `None` for an absolute path, a `..` part or a name that is not UTF-8.
pub(crate) fn name_parts(path: &Path) -> Option<Vec<&str>> {
    let mut parts = Vec::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::Normal(part) => parts.push(part.to_str()?),
            _ => return None,
        }
    }
    Some(parts)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn name_parts_skips_dot_and_rejects_absolute_and_parent_paths() {
        assert_eq!(name_parts(Path::new("./a/b")), Some(vec!["a", "b"]));
        assert_eq!(name_parts(Path::new("/a/b")), None);
        assert_eq!(name_parts(Path::new("a/../b")), None);
    }

    #[test]
    fn only_regular_and_continuous_entries_are_files() {
        assert!(is_file(EntryType::Regular));
        assert!(is_file(EntryType::Continuous));
        assert!(!is_file(EntryType::Directory));
    }
}
