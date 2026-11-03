//! The marker line that opens every bot check comment.

use catalogue_core::CommitSha;

use super::digest::Sha256;

/// The marker text up to the body digest.
const PREFIX: &str = "<!-- catalogue-bot:check v1 body=sha256:";
/// The text between the body digest and the report digest.
const REPORT_KEY: &str = " report=sha256:";
/// The text between the report digest and the main SHA.
const MAIN_KEY: &str = " main=";
/// The end of the marker.
const SUFFIX: &str = " -->";
/// The main value when the commit is unknown.
const NO_MAIN: &str = "none";
/// The length of a digest in hex digits.
const HEX_LEN: usize = 64;

/// What one bot check comment covers: the issue body, the report and the commit of `main`.
///
/// The marker renders as one HTML comment line:
/// `<!-- catalogue-bot:check v1 body=sha256:<hex> report=sha256:<hex> main=<sha-or-none> -->`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Marker {
    /// The digest of the issue body bytes.
    pub body: Sha256,
    /// The digest of the report markdown.
    pub report: Sha256,
    /// The commit of `main` the check read. `None` when unknown.
    pub main: Option<CommitSha>,
}

impl Marker {
    /// The marker line, without a line break.
    pub fn render(&self) -> String {
        let main = self.main.as_ref().map_or(NO_MAIN, CommitSha::as_str);
        format!(
            "{PREFIX}{}{REPORT_KEY}{}{MAIN_KEY}{main}{SUFFIX}",
            self.body, self.report
        )
    }

    /// Parses one marker line. The match is exact: prefix, key order,
    /// 64 lowercase hex digits per digest and a full SHA or `none`. Any other
    /// text gives `None`.
    pub fn parse(line: &str) -> Option<Self> {
        let rest = line.strip_prefix(PREFIX)?;
        let (body, rest) = rest.split_at_checked(HEX_LEN)?;
        let body = Sha256::parse_hex(body)?;
        let rest = rest.strip_prefix(REPORT_KEY)?;
        let (report, rest) = rest.split_at_checked(HEX_LEN)?;
        let report = Sha256::parse_hex(report)?;
        let main = rest.strip_prefix(MAIN_KEY)?.strip_suffix(SUFFIX)?;
        let main = if main == NO_MAIN {
            None
        } else {
            Some(CommitSha::new(main).ok()?)
        };
        Some(Self { body, report, main })
    }

    /// The marker on the first line of comment `text`, or `None`.
    ///
    /// A `\r` before the first line break is dropped.
    pub fn of_comment(text: &str) -> Option<Self> {
        let line = text.split('\n').next()?;
        Self::parse(line.strip_suffix('\r').unwrap_or(line))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sha() -> CommitSha {
        CommitSha::new("0123456789abcdef0123456789abcdef01234567").expect("sha")
    }

    fn marker(main: Option<CommitSha>) -> Marker {
        Marker {
            body: Sha256::of(b"body"),
            report: Sha256::of(b"report"),
            main,
        }
    }

    #[test]
    fn markers_round_trip() {
        for found in [marker(Some(sha())), marker(None)] {
            let line = found.render();
            assert!(line.starts_with(PREFIX) && line.ends_with(SUFFIX), "{line}");
            assert_eq!(Marker::parse(&line), Some(found.clone()));
            assert_eq!(
                Marker::of_comment(&format!("{line}\r\n\n## Catalogue check")),
                Some(found)
            );
        }
        assert!(marker(None).render().ends_with(" main=none -->"));
    }

    #[test]
    fn malformed_markers_are_ignored() {
        let good = marker(Some(sha())).render();
        let body = Sha256::of(b"body").to_string();
        let report = Sha256::of(b"report").to_string();
        let sha = sha().to_string();
        let bad = [
            format!(" {good}"),
            format!("{good} "),
            good.replace("v1", "v2"),
            good.replace("<!-- ", "<!--"),
            good.replace(&body, &body.to_ascii_uppercase()),
            good.replace(&body, &body[..63]),
            good.replace(&report, &format!("{report}0")),
            good.replace(&sha, &sha[..7]),
            good.replace(&sha, "NONE"),
            format!(
                "<!-- catalogue-bot:check v1 report=sha256:{report} body=sha256:{body} main=none -->"
            ),
            good.replace(" -->", "-->"),
            String::new(),
        ];
        for line in bad {
            assert_eq!(Marker::parse(&line), None, "{line}");
        }
        assert_eq!(Marker::of_comment(&format!("hello\n{good}")), None);
    }
}
