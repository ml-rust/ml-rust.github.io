//! The `dependencies` table.

use std::io::Read;

use crate::dump::error::DumpError;
use crate::dump::names::TableName;
use crate::dump::row::{Col, Row};
use crate::dump::snapshot::{DependencyEdge, DependencyKind, IdMap, Requirement};
use crate::dump::table::for_each_row;

const COLUMNS: &[&str; 5] = &["version_id", "crate_id", "req", "kind", "optional"];

/// The edges of every row, and the requirements of each default version.
#[derive(Debug)]
pub(crate) struct Dependencies {
    /// Every dependency row.
    pub(crate) edges: Vec<DependencyEdge>,
    /// Requirements of each crate's default version, by depender crate id.
    pub(crate) default_requirements: IdMap<Vec<Requirement>>,
}

/// Reads `dependencies`. `default_version` maps crate id to default version id.
pub(crate) fn read(
    input: impl Read,
    default_version: &IdMap<u32>,
) -> Result<Dependencies, DumpError> {
    let depender: IdMap<u32> = default_version
        .iter()
        .map(|(&crate_id, &version_id)| (version_id, crate_id))
        .collect();
    let mut edges = Vec::new();
    let mut default_requirements: IdMap<Vec<Requirement>> = IdMap::default();
    for_each_row(
        input,
        TableName::Dependencies,
        COLUMNS,
        |row, &[version_id, crate_id, req, kind_col, optional]| {
            let edge = DependencyEdge {
                version_id: row.id(version_id)?,
                crate_id: row.id(crate_id)?,
                kind: kind(row, kind_col)?,
                optional: row.flag(optional)?,
            };
            if let Some(&crate_id) = depender.get(&edge.version_id) {
                default_requirements
                    .entry(crate_id)
                    .or_default()
                    .push(Requirement {
                        req: row.text(req)?.to_owned(),
                        crate_id: edge.crate_id,
                        kind: edge.kind,
                        optional: edge.optional,
                    });
            }
            edges.push(edge);
            Ok(())
        },
    )?;
    Ok(Dependencies {
        edges,
        default_requirements,
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
