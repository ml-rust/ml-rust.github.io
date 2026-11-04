//! The `dependencies` table.

use std::io::Read;

use crate::dump::error::DumpError;
use crate::dump::names::TableName;
use crate::dump::row::{Col, Row};
use crate::dump::snapshot::{DependencyEdge, DependencyKind, IdMap, IdSet, Rename, Requirement};
use crate::dump::table::for_each_row;

const COLUMNS: &[&str; 5] = &["version_id", "crate_id", "req", "kind", "optional"];

/// [`COLUMNS`], then the name a depender uses for a renamed dependency.
const COLUMNS_WITH_NAME: &[&str; 6] = &[
    "version_id",
    "crate_id",
    "req",
    "kind",
    "optional",
    "explicit_name",
];

/// The edges of every row, and the requirements of each default version.
#[derive(Debug)]
pub(crate) struct Dependencies {
    /// Every dependency row.
    pub(crate) edges: Vec<DependencyEdge>,
    /// Requirements of each crate's default version, by depender crate id.
    pub(crate) default_requirements: IdMap<Vec<Requirement>>,
    /// The renamed dependencies of each kept crate's default version, by depender crate id.
    pub(crate) renames: IdMap<Vec<Rename>>,
}

/// The rows read so far.
struct Collector {
    depender: IdMap<u32>,
    edges: Vec<DependencyEdge>,
    default_requirements: IdMap<Vec<Requirement>>,
}

impl Collector {
    fn new(default_version: &IdMap<u32>) -> Self {
        Self {
            depender: default_version
                .iter()
                .map(|(&crate_id, &version_id)| (version_id, crate_id))
                .collect(),
            edges: Vec::new(),
            default_requirements: IdMap::default(),
        }
    }

    /// Adds one row. For a row of a default version, returns the depender crate id and
    /// the crate id it depends on.
    fn add(&mut self, row: &Row<'_>, cols: &[Col; 5]) -> Result<Option<(u32, u32)>, DumpError> {
        let &[version_id, crate_id, req, kind_col, optional] = cols;
        let edge = DependencyEdge {
            version_id: row.id(version_id)?,
            crate_id: row.id(crate_id)?,
            kind: kind(row, kind_col)?,
            optional: row.flag(optional)?,
        };
        let owner = self.depender.get(&edge.version_id).copied();
        if let Some(owner) = owner {
            self.default_requirements
                .entry(owner)
                .or_default()
                .push(Requirement {
                    req: row.text(req)?.to_owned(),
                    crate_id: edge.crate_id,
                    kind: edge.kind,
                    optional: edge.optional,
                });
        }
        self.edges.push(edge);
        Ok(owner.map(|owner| (owner, edge.crate_id)))
    }
}

/// Reads `dependencies`. `default_version` maps crate id to default version id.
pub(crate) fn read(
    input: impl Read,
    default_version: &IdMap<u32>,
) -> Result<Dependencies, DumpError> {
    let mut found = Collector::new(default_version);
    for_each_row(input, TableName::Dependencies, COLUMNS, |row, cols| {
        found.add(row, cols).map(|_| ())
    })?;
    Ok(Dependencies {
        edges: found.edges,
        default_requirements: found.default_requirements,
        renames: IdMap::default(),
    })
}

/// [`read`], and the renamed dependencies of the default version of each crate in `kept`.
pub(crate) fn read_with_renames(
    input: impl Read,
    default_version: &IdMap<u32>,
    kept: &IdSet,
) -> Result<Dependencies, DumpError> {
    let mut found = Collector::new(default_version);
    let mut renames: IdMap<Vec<Rename>> = IdMap::default();
    for_each_row(
        input,
        TableName::Dependencies,
        COLUMNS_WITH_NAME,
        |row, &[version_id, crate_id, req, kind_col, optional, explicit_name]| {
            let cols = [version_id, crate_id, req, kind_col, optional];
            if let Some((owner, on)) = found.add(row, &cols)?
                && kept.contains(&owner)
                && let Some(name) = row.optional_string(explicit_name)?
            {
                let rename = Rename { crate_id: on, name };
                renames.entry(owner).or_default().push(rename);
            }
            Ok(())
        },
    )?;
    Ok(Dependencies {
        edges: found.edges,
        default_requirements: found.default_requirements,
        renames,
    })
}

fn kind(row: &Row<'_>, col: Col) -> Result<DependencyKind, DumpError> {
    let text = row.text(col)?;
    match text {
        "0" => Ok(DependencyKind::Normal),
        "1" => Ok(DependencyKind::Build),
        "2" => Ok(DependencyKind::Dev),
        _ => Err(row.bad(col, text, "`0`, `1` or `2`")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CSV: &str = "crate_id,default_features,features,id,kind,optional,req,target,version_id\n\
        50,t,{},1,0,f,^1.0,,10\n\
        51,t,{},2,2,f,0.3,,10\n\
        52,t,{},3,1,t,~2,,11\n\
        53,t,{},4,0,f,^0.1,,20\n";

    #[test]
    fn keeps_every_edge_and_default_requirements() {
        let defaults: IdMap<u32> = [(1, 11), (2, 20)].into_iter().collect();
        let found = read(CSV.as_bytes(), &defaults).expect("reads");
        assert_eq!(found.edges.len(), 4);
        assert_eq!(
            found.edges.first(),
            Some(&DependencyEdge {
                version_id: 10,
                crate_id: 50,
                kind: DependencyKind::Normal,
                optional: false,
            })
        );
        assert_eq!(
            found.default_requirements.get(&1).map(Vec::as_slice),
            Some(
                &[Requirement {
                    req: "~2".to_owned(),
                    crate_id: 52,
                    kind: DependencyKind::Build,
                    optional: true,
                }][..]
            )
        );
        let second = found.default_requirements.get(&2).expect("crate 2");
        assert_eq!(second.first().map(|r| r.req.as_str()), Some("^0.1"));
        assert_eq!(found.default_requirements.len(), 2);
    }

    #[test]
    fn renames_are_kept_only_for_a_kept_crate_and_only_on_its_default_version() {
        let csv = "crate_id,explicit_name,kind,optional,req,version_id\n\
            50,cuda,0,t,^1,11\n\
            51,,0,f,^1,11\n\
            52,old,0,f,^1,10\n\
            53,other,0,f,^1,20\n";
        let defaults: IdMap<u32> = [(1, 11), (2, 20)].into_iter().collect();
        let kept: IdSet = [1].into_iter().collect();
        let found = read_with_renames(csv.as_bytes(), &defaults, &kept).expect("reads");
        assert_eq!(found.edges.len(), 4);
        assert_eq!(
            found.renames.get(&1).map(Vec::as_slice),
            Some(
                &[Rename {
                    crate_id: 50,
                    name: "cuda".to_owned()
                }][..]
            )
        );
        assert_eq!(found.renames.len(), 1);
    }

    #[test]
    fn unknown_kind_is_rejected() {
        let csv = "version_id,crate_id,req,kind,optional\n10,50,^1,3,f\n";
        let error = read(csv.as_bytes(), &IdMap::default()).expect_err("rejects");
        assert!(matches!(
            error,
            DumpError::BadValue {
                table: "dependencies",
                column: "kind",
                record: 1,
                ..
            }
        ));
    }
}
