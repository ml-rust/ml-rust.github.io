//! The owners of one crate.

use serde::Deserialize;

use super::error::CratesIoError;
use crate::agent::USER_AGENT;
use crate::http::{Paced, Pacer, Transport, percent_encode};

/// The crate endpoint of the crates.io API.
pub const OWNERS_ENDPOINT: &str = "https://crates.io/api/v1/crates";

/// One owner of a crate. Unknown fields are ignored.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct CrateOwner {
    /// The owner kind: `user` or `team`.
    pub kind: String,
    /// The login. A team login reads `github:<org>:<team>`.
    pub login: String,
}

/// The owners response.
#[derive(Deserialize)]
struct Owners {
    users: Vec<CrateOwner>,
}

/// Reads the crates.io API through one pacer, so requests stay spaced across calls.
pub struct CratesIo<'a> {
    transport: &'a dyn Transport,
    paced: Paced<'a>,
}

impl<'a> CratesIo<'a> {
    /// A client over `transport` that the `pacer` spaces. The first request does not wait.
    pub fn new(transport: &'a dyn Transport, pacer: &'a mut dyn Pacer) -> Self {
        Self {
            transport,
            paced: Paced::new(pacer),
        }
    }

    /// The owners of crate `name`, users and teams, in API order.
    ///
    /// A crate that crates.io does not know has no owners. A 403 or 429 answer
    /// with a wait hint is retried after the pacer waits.
    ///
    /// # Errors
    ///
    /// Returns a [`CratesIoError`] when no response arrives, the status is not
    /// 200 or 404, the body breaks off or is not the expected JSON, or the rate
    /// limit stays after the retries.
    pub fn owners(&mut self, name: &str) -> Result<Vec<CrateOwner>, CratesIoError> {
        let url = format!("{OWNERS_ENDPOINT}/{}/owners", percent_encode(name));
        let headers = [
            ("Accept", "application/json".to_owned()),
            ("User-Agent", USER_AGENT.to_owned()),
        ];
        let transport = self.transport;
        let Some(bytes) = self
            .paced
            .fetch_optional(&url, || transport.get(&url, &headers))?
        else {
            return Ok(Vec::new());
        };
        let owners: Owners =
            serde_json::from_slice(&bytes).map_err(|source| CratesIoError::Json { url, source })?;
        Ok(owners.users)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn users_and_teams_parse() {
        let owners: Owners = serde_json::from_str(
            r#"{"users":[{"id":1,"login":"dave","kind":"user","url":"u","name":null},
                {"id":2,"login":"github:acme:core","kind":"team","url":"u","name":"Core"}]}"#,
        )
        .expect("parses");
        assert_eq!(owners.users.len(), 2);
        assert_eq!(owners.users[1].kind, "team");
        assert_eq!(owners.users[1].login, "github:acme:core");
    }
}
