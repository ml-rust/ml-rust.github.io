//! The GraphQL requests of the commit lookup and their parsed answers.
//!
//! Every value travels as a variable, never inside the query text.

use serde::Deserialize;
use serde_json::{Value, json};

use super::types::CrateRelease;
use crate::github::{ApiError, RepoRef};

/// Whether GitHub knows the object. A commit that only a fork holds counts as known.
///
/// `compare` fails the whole query for an unknown commit, so this request runs first.
const COMMIT_QUERY: &str = "\
query($owner: String!, $name: String!, $sha: GitObjectID!) {
  repository(owner: $owner, name: $name) {
    object(oid: $sha) { __typename }
  }
}";

/// The commit against the default branch and the tags named after the release.
const NAMED_REFS_QUERY: &str = "\
query($owner: String!, $name: String!, $sha: String!, $tag0: String!, $tag1: String!, $tag2: String!, $tag3: String!) {
  repository(owner: $owner, name: $name) {
    defaultBranchRef { name compare(headRef: $sha) { status } }
    tag0: ref(qualifiedName: $tag0) { name compare(headRef: $sha) { status } }
    tag1: ref(qualifiedName: $tag1) { name compare(headRef: $sha) { status } }
    tag2: ref(qualifiedName: $tag2) { name compare(headRef: $sha) { status } }
    tag3: ref(qualifiedName: $tag3) { name compare(headRef: $sha) { status } }
  }
}";

/// One page of branches, each compared with the commit.
const BRANCHES_QUERY: &str = "\
query($owner: String!, $name: String!, $sha: String!, $after: String) {
  repository(owner: $owner, name: $name) {
    refs(refPrefix: \"refs/heads/\", first: 100, after: $after) {
      pageInfo { hasNextPage endCursor }
      nodes { name compare(headRef: $sha) { status } }
    }
  }
}";

/// One page of tags with the objects they point at.
///
/// `compare` fails the whole query on a tag that points at a tree, so this
/// query reads object ids and sends no compare.
const TAGS_QUERY: &str = "\
query($owner: String!, $name: String!, $after: String) {
  repository(owner: $owner, name: $name) {
    refs(refPrefix: \"refs/tags/\", first: 100, after: $after) {
      pageInfo { hasNextPage endCursor }
      nodes { name target { oid ... on Tag { target { oid } } } }
    }
  }
}";

/// The tags a release is likely to carry: `v<version>`, `<version>`,
/// `<crate>-v<version>` and `<crate>-<version>`, in that order.
fn tag_names(release: &CrateRelease) -> [String; 4] {
    let CrateRelease { name, version } = release;
    [
        format!("v{version}"),
        version.clone(),
        format!("{name}-v{version}"),
        format!("{name}-{version}"),
    ]
}

fn body(query: &str, variables: Value) -> Vec<u8> {
    json!({ "query": query, "variables": variables })
        .to_string()
        .into_bytes()
}

/// The request for [`CommitLookup`].
pub(super) fn commit_body(repo: &RepoRef, sha1: &str) -> Vec<u8> {
    body(
        COMMIT_QUERY,
        json!({ "owner": repo.owner, "name": repo.name, "sha": sha1 }),
    )
}

/// The request for [`NamedRefs`].
pub(super) fn named_refs_body(repo: &RepoRef, release: &CrateRelease, sha1: &str) -> Vec<u8> {
    let [tag0, tag1, tag2, tag3] = tag_names(release).map(|tag| format!("refs/tags/{tag}"));
    body(
        NAMED_REFS_QUERY,
        json!({
            "owner": repo.owner,
            "name": repo.name,
            "sha": sha1,
            "tag0": tag0,
            "tag1": tag1,
            "tag2": tag2,
            "tag3": tag3,
        }),
    )
}

/// The request for the page of branches after the cursor `after`.
pub(super) fn branches_body(repo: &RepoRef, sha1: &str, after: Option<&str>) -> Vec<u8> {
    body(
        BRANCHES_QUERY,
        json!({ "owner": repo.owner, "name": repo.name, "sha": sha1, "after": after }),
    )
}

/// The request for the page of tags after the cursor `after`.
pub(super) fn tags_body(repo: &RepoRef, after: Option<&str>) -> Vec<u8> {
    body(
        TAGS_QUERY,
        json!({ "owner": repo.owner, "name": repo.name, "after": after }),
    )
}

/// The top level of an answer. `T` is the `repository` object.
#[derive(Debug, Deserialize)]
pub(super) struct Answer<T> {
    pub(super) data: Option<Data<T>>,
    #[serde(default)]
    pub(super) errors: Vec<ApiError>,
}

/// The `data` object. The repository is null when GitHub does not know it.
#[derive(Debug, Deserialize)]
pub(super) struct Data<T> {
    pub(super) repository: Option<T>,
}

/// The answer to [`commit_body`].
#[derive(Debug, Deserialize)]
pub(super) struct CommitLookup {
    object: Option<GitObject>,
}

#[derive(Debug, Deserialize)]
struct GitObject {
    #[serde(rename = "__typename")]
    kind: String,
}

impl CommitLookup {
    /// Whether the object exists and is a commit.
    pub(super) fn is_commit(&self) -> bool {
        self.object
            .as_ref()
            .is_some_and(|object| object.kind == "Commit")
    }
}

/// A ref compared with the commit: the ref is the base, the commit is the head.
#[derive(Debug, Deserialize)]
pub(super) struct ComparedRef {
    name: String,
    compare: Option<Comparison>,
}

#[derive(Debug, Deserialize)]
struct Comparison {
    status: String,
}

impl ComparedRef {
    /// Whether the ref contains the commit.
    ///
    /// `BEHIND` means every commit of the head is in the base. `IDENTICAL`
    /// means the ref points at the commit. `AHEAD` and `DIVERGED` mean the
    /// commit is not in the ref. A commit that only a fork holds gives one of
    /// those two. Any other status and a missing comparison do not count.
    fn holds(&self) -> bool {
        self.compare
            .as_ref()
            .is_some_and(|compare| matches!(compare.status.as_str(), "BEHIND" | "IDENTICAL"))
    }

    /// The full ref name, when this branch contains the commit.
    pub(super) fn holding_branch(&self) -> Option<String> {
        self.holds().then(|| format!("refs/heads/{}", self.name))
    }

    fn holding_tag(&self) -> Option<String> {
        self.holds().then(|| format!("refs/tags/{}", self.name))
    }
}

/// The answer to [`named_refs_body`]. A tag that does not exist is null.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct NamedRefs {
    default_branch_ref: Option<ComparedRef>,
    tag0: Option<ComparedRef>,
    tag1: Option<ComparedRef>,
    tag2: Option<ComparedRef>,
    tag3: Option<ComparedRef>,
}

impl NamedRefs {
    /// The full name of the first ref that contains the commit.
    pub(super) fn holder(&self) -> Option<String> {
        let default_branch = self
            .default_branch_ref
            .as_ref()
            .and_then(ComparedRef::holding_branch);
        default_branch.or_else(|| {
            [&self.tag0, &self.tag1, &self.tag2, &self.tag3]
                .into_iter()
                .flatten()
                .find_map(ComparedRef::holding_tag)
        })
    }
}

/// The answer to one page request. `T` is one ref.
#[derive(Debug, Deserialize)]
pub(super) struct RefPage<T> {
    pub(super) refs: Option<Refs<T>>,
}

/// One page of refs. A node can be null.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Refs<T> {
    page_info: PageInfo,
    nodes: Option<Vec<Option<T>>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PageInfo {
    has_next_page: bool,
    end_cursor: Option<String>,
}

impl<T> Refs<T> {
    /// The refs that are not null.
    pub(super) fn items(&self) -> impl Iterator<Item = &T> {
        self.nodes.iter().flatten().flatten()
    }

    /// The cursor of the next page, when one exists.
    pub(super) fn next_cursor(self) -> Option<String> {
        if self.page_info.has_next_page {
            self.page_info.end_cursor
        } else {
            None
        }
    }
}

/// One tag and the object it points at.
#[derive(Debug, Deserialize)]
pub(super) struct TagRef {
    name: String,
    target: Option<TagTarget>,
}

/// The tag target. An annotated tag names the object it wraps.
#[derive(Debug, Deserialize)]
struct TagTarget {
    oid: String,
    target: Option<WrappedObject>,
}

#[derive(Debug, Deserialize)]
struct WrappedObject {
    oid: String,
}

impl TagRef {
    /// The full ref name, when this tag points straight at `sha1`.
    pub(super) fn holding(&self, sha1: &str) -> Option<String> {
        let target = self.target.as_ref()?;
        let wrapped = target.target.as_ref().map(|object| object.oid.as_str());
        (target.oid == sha1 || wrapped == Some(sha1)).then(|| format!("refs/tags/{}", self.name))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SHA: &str = "a866b336f14aa57a07f0d0be9f8762746e64ecb4";

    fn release() -> CrateRelease {
        CrateRelease {
            name: "burn".to_owned(),
            version: "0.22.1".to_owned(),
        }
    }

    fn repo() -> RepoRef {
        RepoRef::parse("tracel-ai/burn").expect("repo")
    }

    fn parsed(body: &[u8]) -> Value {
        serde_json::from_slice(body).expect("json")
    }

    fn compared(status: &str) -> Option<ComparedRef> {
        Some(ComparedRef {
            name: "x".to_owned(),
            compare: Some(Comparison {
                status: status.to_owned(),
            }),
        })
    }

    fn named(default_branch_ref: Option<ComparedRef>, tag1: Option<ComparedRef>) -> NamedRefs {
        NamedRefs {
            default_branch_ref,
            tag0: None,
            tag1,
            tag2: None,
            tag3: None,
        }
    }

    #[test]
    fn the_tag_names_follow_the_release_in_order() {
        assert_eq!(
            tag_names(&release()),
            ["v0.22.1", "0.22.1", "burn-v0.22.1", "burn-0.22.1"]
        );
    }

    #[test]
    fn values_travel_as_variables() {
        let body = parsed(&named_refs_body(&repo(), &release(), SHA));
        assert_eq!(body["variables"]["owner"], "tracel-ai");
        assert_eq!(body["variables"]["name"], "burn");
        assert_eq!(body["variables"]["sha"], SHA);
        assert_eq!(body["variables"]["tag0"], "refs/tags/v0.22.1");
        assert_eq!(body["variables"]["tag3"], "refs/tags/burn-0.22.1");
        assert!(!body["query"].as_str().expect("query").contains(SHA));

        let body = parsed(&commit_body(&repo(), SHA));
        assert_eq!(body["variables"]["sha"], SHA);

        let body = parsed(&branches_body(&repo(), SHA, Some("c1")));
        assert_eq!(body["variables"]["after"], "c1");
        assert_eq!(body["variables"]["sha"], SHA);

        let body = parsed(&tags_body(&repo(), None));
        assert_eq!(body["variables"]["after"], Value::Null);
        assert_eq!(body["variables"].get("sha"), None);
    }

    #[test]
    fn only_behind_and_identical_hold_the_commit() {
        for (status, holds) in [
            ("BEHIND", true),
            ("IDENTICAL", true),
            ("AHEAD", false),
            ("DIVERGED", false),
            ("SURPRISE", false),
        ] {
            let found = compared(status).expect("ref");
            assert_eq!(found.holds(), holds, "{status}");
        }
        let unresolved = ComparedRef {
            name: "x".to_owned(),
            compare: None,
        };
        assert!(!unresolved.holds());
    }

    #[test]
    fn the_default_branch_comes_before_a_tag() {
        let refs = named(compared("BEHIND"), compared("IDENTICAL"));
        assert_eq!(refs.holder().as_deref(), Some("refs/heads/x"));
    }

    #[test]
    fn a_tag_holds_when_the_default_branch_does_not() {
        let refs = named(compared("DIVERGED"), compared("IDENTICAL"));
        assert_eq!(refs.holder().as_deref(), Some("refs/tags/x"));
    }

    #[test]
    fn no_holder_when_every_ref_lacks_the_commit_or_is_absent() {
        assert_eq!(
            named(compared("AHEAD"), compared("DIVERGED")).holder(),
            None
        );
        assert_eq!(named(None, None).holder(), None);
    }

    #[test]
    fn a_tag_points_at_the_commit_directly_or_through_an_annotated_tag() {
        let parse = |json: &str| serde_json::from_str::<TagRef>(json).expect("tag");
        let light = format!(r#"{{"name":"a","target":{{"oid":"{SHA}"}}}}"#);
        assert_eq!(parse(&light).holding(SHA).as_deref(), Some("refs/tags/a"));
        let annotated =
            format!(r#"{{"name":"b","target":{{"oid":"ffff","target":{{"oid":"{SHA}"}}}}}}"#);
        assert_eq!(
            parse(&annotated).holding(SHA).as_deref(),
            Some("refs/tags/b")
        );
        let other = r#"{"name":"c","target":{"oid":"ffff","target":{"oid":"eeee"}}}"#;
        assert_eq!(parse(other).holding(SHA), None);
        assert_eq!(parse(r#"{"name":"d","target":null}"#).holding(SHA), None);
    }

    #[test]
    fn a_page_gives_its_items_and_its_cursor() {
        let json = r#"{"pageInfo":{"hasNextPage":true,"endCursor":"c1"},"nodes":[null,{"name":"a","compare":null}]}"#;
        let page: Refs<ComparedRef> = serde_json::from_str(json).expect("page");
        assert_eq!(page.items().count(), 1);
        assert_eq!(page.next_cursor().as_deref(), Some("c1"));

        let json = r#"{"pageInfo":{"hasNextPage":false,"endCursor":"c2"},"nodes":[]}"#;
        let last: Refs<ComparedRef> = serde_json::from_str(json).expect("page");
        assert_eq!(last.next_cursor(), None);

        let json = r#"{"pageInfo":{"hasNextPage":true,"endCursor":null},"nodes":null}"#;
        let broken: Refs<ComparedRef> = serde_json::from_str(json).expect("page");
        assert_eq!(broken.items().count(), 0);
        assert_eq!(broken.next_cursor(), None);
    }
}
