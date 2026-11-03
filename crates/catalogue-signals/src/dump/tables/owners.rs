//! The `crate_owners`, `users` and `teams` tables.

use std::io::Read;

use crate::dump::error::DumpError;
use crate::dump::names::TableName;
use crate::dump::snapshot::{IdMap, Owner, OwnerKind};
use crate::dump::table::{for_each_row, id_names};

const OWNERS: &[&str] = &["crate_id", "owner_id", "owner_kind"];

/// Reads `crate_owners` into a map from crate id to its owners.
pub(crate) fn read_owners(input: impl Read) -> Result<IdMap<Vec<Owner>>, DumpError> {
    let mut out: IdMap<Vec<Owner>> = IdMap::default();
    for_each_row(input, TableName::CrateOwners, OWNERS, |row| {
        let text = row.text("owner_kind")?;
        let kind = match text {
            "0" => OwnerKind::User,
            "1" => OwnerKind::Team,
            _ => return Err(row.bad("owner_kind", text, "`0` or `1`")),
        };
        let owner = Owner {
            id: row.id("owner_id")?,
            kind,
        };
        out.entry(row.id("crate_id")?).or_default().push(owner);
        Ok(())
    })?;
    Ok(out)
}

/// Reads `users` into a map from user id to login.
pub(crate) fn read_users(input: impl Read) -> Result<IdMap<String>, DumpError> {
    id_names(input, TableName::Users, &["id", "username"])
}

/// Reads `teams` into a map from team id to login.
pub(crate) fn read_teams(input: impl Read) -> Result<IdMap<String>, DumpError> {
    id_names(input, TableName::Teams, &["id", "login"])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_user_and_team_owners() {
        let csv = "created_at,crate_id,created_by,owner_id,owner_kind\n\
            2026-01-01 00:00:00+00,1,,7,0\n\
            2026-01-01 00:00:00+00,1,,3,1\n";
        let owners = read_owners(csv.as_bytes()).expect("reads");
        assert_eq!(
            owners.get(&1).map(Vec::as_slice),
            Some(
                &[
                    Owner {
                        id: 7,
                        kind: OwnerKind::User
                    },
                    Owner {
                        id: 3,
                        kind: OwnerKind::Team
                    },
                ][..]
            )
        );
    }

    #[test]
    fn unknown_owner_kind_is_rejected() {
        let csv = "crate_id,owner_id,owner_kind\n1,7,2\n";
        let error = read_owners(csv.as_bytes()).expect_err("rejects");
        assert!(matches!(
            error,
            DumpError::BadValue {
                table: "crate_owners",
                column: "owner_kind",
                ..
            }
        ));
    }

    #[test]
    fn reads_logins() {
        let users = read_users("gh_id,id,username\n99,7,alice\n".as_bytes()).expect("users");
        assert_eq!(users.get(&7).map(String::as_str), Some("alice"));
        let teams = read_teams("avatar,id,login\n,3,github:org:team\n".as_bytes()).expect("teams");
        assert_eq!(teams.get(&3).map(String::as_str), Some("github:org:team"));
    }

    #[test]
    fn users_need_the_username_column() {
        let error = read_users("id,gh_login\n7,alice\n".as_bytes()).expect_err("rejects");
        assert!(matches!(
            error,
            DumpError::MissingColumn {
                table: "users",
                column: "username"
            }
        ));
    }
}
