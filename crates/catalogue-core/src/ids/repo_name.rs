/// Whether `value` is a GitHub repository name.
///
/// A name holds ASCII letters, digits, `-`, `_` and `.`, and is neither
/// empty, `.` nor `..`. The check does not cap the length.
pub fn is_github_repo_name(value: &str) -> bool {
    !value.is_empty()
        && value != "."
        && value != ".."
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_repo_names() {
        for ok in ["a", "Rust.rs_2", "a-b", ".github", "a.."] {
            assert!(is_github_repo_name(ok), "{ok}");
        }
    }

    #[test]
    fn rejects_other_names() {
        for bad in ["", ".", "..", "a b", "a/b", "é", "a?x=1"] {
            assert!(!is_github_repo_name(bad), "{bad}");
        }
    }
}
