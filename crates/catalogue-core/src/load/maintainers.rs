use std::collections::BTreeMap;
use std::collections::btree_map::Entry;

use super::diagnostics::Diagnostics;
use super::parse::Parsed;
use crate::registry::Maintainers;

/// Checks the maintainer list.
///
/// Returns each maintainer's lowercase login mapped to `org_member`.
pub(crate) fn check_maintainers(
    file: &Parsed<Maintainers>,
    diags: &mut Diagnostics,
) -> BTreeMap<String, bool> {
    let mut members: BTreeMap<String, bool> = BTreeMap::new();
    for maintainer in &file.value.maintainer {
        let key = maintainer.login.normalized();
        match members.entry(key) {
            Entry::Vacant(slot) => {
                slot.insert(maintainer.org_member);
            }
            Entry::Occupied(_) => diags.push(
                &file.path,
                format!("maintainer `{}` is listed twice", maintainer.login),
            ),
        }
    }
    members
}

#[cfg(test)]
mod tests {
    use crate::testing::{errors, expect_error, put, valid};

    const BOB: &str = "[[maintainer]]\nlogin = \"bob\"\norg_member = false\nsince = 2026-02-01\n";

    #[test]
    fn unique_logins_pass() {
        assert!(errors(valid()).is_empty());
    }

    #[test]
    fn logins_must_be_unique() {
        let mut src = valid();
        put(&mut src, "maintainers.toml", &format!("{BOB}{BOB}"));
        expect_error(src, "maintainer `bob` is listed twice");
    }

    #[test]
    fn logins_compare_without_case() {
        let mut src = valid();
        let upper = BOB.replace("bob", "BOB");
        put(&mut src, "maintainers.toml", &format!("{BOB}{upper}"));
        expect_error(src, "is listed twice");
    }
}
