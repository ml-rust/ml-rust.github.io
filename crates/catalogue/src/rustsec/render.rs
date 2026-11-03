//! Text output of `catalogue rustsec show`.

use catalogue_signals::rustsec::Advisories;
use jiff::civil;

/// The block of lines `rustsec show` prints for crate `name`.
///
/// The first line is `name`. Each advisory follows as
/// `id<TAB>date<TAB>kind<TAB>withdrawn:<date|-><TAB>patched:<n>`. The block
/// ends with the `unmaintained:` and `unpatched_vulnerabilities:` lines.
pub fn render_crate(
    advisories: &Advisories,
    name: &str,
    today: civil::Date,
    min_age_days: i64,
) -> String {
    let mut lines = vec![name.to_owned()];
    for advisory in advisories.for_crate(name) {
        let withdrawn = advisory
            .withdrawn
            .map_or_else(|| "-".to_owned(), |date| date.to_string());
        lines.push(format!(
            "{}\t{}\t{}\twithdrawn:{withdrawn}\tpatched:{}",
            advisory.id,
            advisory.date,
            advisory.kind.name(),
            advisory.patched.len()
        ));
    }
    let unmaintained = advisories
        .unmaintained(name)
        .map_or("-", |advisory| advisory.id.as_str());
    lines.push(format!("unmaintained: {unmaintained}"));
    let unpatched: Vec<&str> = advisories
        .unpatched_vulnerabilities(name, today, min_age_days)
        .into_iter()
        .map(|advisory| advisory.id.as_str())
        .collect();
    let unpatched = if unpatched.is_empty() {
        "-".to_owned()
    } else {
        unpatched.join(", ")
    };
    lines.push(format!("unpatched_vulnerabilities: {unpatched}"));
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use catalogue_signals::rustsec::{Advisory, AdvisoryKind};
    use jiff::civil::date;

    use super::*;

    fn advisory(id: &str, day: civil::Date, kind: AdvisoryKind, patched: usize) -> Advisory {
        Advisory {
            id: id.to_owned(),
            package: "foo-bar".to_owned(),
            date: day,
            kind,
            withdrawn: None,
            patched: vec![">= 1.0.0".to_owned(); patched],
            unaffected: Vec::new(),
            url: None,
        }
    }

    #[test]
    fn renders_advisory_lines_and_summaries() {
        let mut withdrawn = advisory("R2", date(2022, 2, 2), AdvisoryKind::Unmaintained, 0);
        withdrawn.withdrawn = Some(date(2023, 3, 3));
        let list = vec![
            advisory("R1", date(2021, 1, 1), AdvisoryKind::Vulnerability, 0),
            withdrawn,
            advisory("R3", date(2022, 5, 5), AdvisoryKind::Unmaintained, 2),
        ];
        let advisories = Advisories {
            by_crate: HashMap::from([("foo-bar".to_owned(), list)]),
            warnings: Vec::new(),
        };
        let text = render_crate(&advisories, "Foo_Bar", date(2026, 11, 3), 60);
        assert_eq!(
            text,
            "Foo_Bar\n\
             R1\t2021-01-01\tvulnerability\twithdrawn:-\tpatched:0\n\
             R2\t2022-02-02\tunmaintained\twithdrawn:2023-03-03\tpatched:0\n\
             R3\t2022-05-05\tunmaintained\twithdrawn:-\tpatched:2\n\
             unmaintained: R3\n\
             unpatched_vulnerabilities: R1"
        );
    }

    #[test]
    fn a_crate_without_advisories_renders_dashes() {
        let text = render_crate(&Advisories::default(), "none", date(2026, 11, 3), 60);
        assert_eq!(text, "none\nunmaintained: -\nunpatched_vulnerabilities: -");
    }
}
