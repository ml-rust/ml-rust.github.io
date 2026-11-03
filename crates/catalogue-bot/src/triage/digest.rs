//! The digest of the claim, the links and the page sources a link check covers.

use url::Url;

use super::claim::Claim;
use crate::comment::Sha256;

/// The version tag at the start of the hashed text. A format change bumps it.
const FORMAT: &str = "triage-v2";

/// The page sources when only GitHub pages can be read.
const GITHUB_ONLY: &str = "github";

/// The page sources when `web_fetch` reads every other page too.
const WITH_WEB_FETCH: &str = "github,web_fetch";

/// The SHA-256 of a canonical encoding of `claim`, `links` and the page sources.
///
/// `web_fetch` tells whether the check can read pages off GitHub. Each part
/// is written as its byte length, a colon and the bytes, so no two inputs
/// share an encoding. The issue body itself is not part of it. An edit that
/// keeps the claim and the links keeps the digest, so the bot does not check
/// the links again. A `web_fetch` key set or removed changes the digest, so
/// the bot checks the links again.
pub fn triage_digest(claim: &Claim, links: &[Url], web_fetch: bool) -> Sha256 {
    let mut out = Vec::new();
    let sources = if web_fetch {
        WITH_WEB_FETCH
    } else {
        GITHUB_ONLY
    };
    push(&mut out, FORMAT);
    push(&mut out, sources);
    push(&mut out, claim.kind.name());
    push(&mut out, claim.goal);
    push(&mut out, &claim.fields.len().to_string());
    for field in &claim.fields {
        push(&mut out, field.label);
        push(&mut out, &field.value);
    }
    push(&mut out, &links.len().to_string());
    for link in links {
        push(&mut out, link.as_str());
    }
    Sha256::of(&out)
}

/// Appends `part` as `<length>:<bytes>`.
fn push(out: &mut Vec<u8>, part: &str) {
    out.extend_from_slice(part.len().to_string().as_bytes());
    out.push(b':');
    out.extend_from_slice(part.as_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::form::FormKind;
    use crate::triage::claim::ClaimField;

    fn claim(values: &[&str]) -> Claim {
        Claim {
            kind: FormKind::Submit,
            goal: "g",
            fields: values
                .iter()
                .map(|value| ClaimField {
                    label: "Name",
                    value: (*value).to_owned(),
                })
                .collect(),
        }
    }

    fn links(texts: &[&str]) -> Vec<Url> {
        texts
            .iter()
            .map(|text| Url::parse(text).expect(text))
            .collect()
    }

    #[test]
    fn the_same_claim_and_links_give_the_same_digest() {
        let one = triage_digest(&claim(&["a"]), &links(&["https://x.example/"]), true);
        let two = triage_digest(&claim(&["a"]), &links(&["https://x.example"]), true);
        assert_eq!(one, two, "the parsed form of a link is hashed");
    }

    #[test]
    fn any_change_in_the_claim_or_the_links_changes_the_digest() {
        let base = triage_digest(&claim(&["ab"]), &links(&["https://x.example/"]), true);
        for other in [
            triage_digest(&claim(&["a", "b"]), &links(&["https://x.example/"]), true),
            triage_digest(&claim(&["ab"]), &links(&["https://y.example/"]), true),
            triage_digest(&claim(&["ab"]), &links(&[]), true),
            triage_digest(
                &claim(&["ab"]),
                &links(&["https://x.example/", "https://x.example/"]),
                true,
            ),
        ] {
            assert_ne!(base, other);
        }
    }

    #[test]
    fn a_fetch_key_set_or_removed_changes_the_digest() {
        let claim = claim(&["ab"]);
        let links = links(&["https://x.example/"]);
        let github_only = triage_digest(&claim, &links, false);
        let with_fetch = triage_digest(&claim, &links, true);
        assert_ne!(github_only, with_fetch);
        assert_eq!(github_only, triage_digest(&claim, &links, false));
    }
}
