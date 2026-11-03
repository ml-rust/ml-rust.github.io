//! A minimal Cargo version requirement reader that answers one question:
//! does every version a requirement allows fall below a given version?
//!
//! A requirement is a comma-separated list of comparators. Every version it
//! allows meets every comparator, so the requirement stays below `below` when
//! any one comparator has an upper bound at or below `below`. Lower bounds
//! never matter. The upper bound of each form, Cargo's rules:
//!
//! | Comparator             | Upper bound          |
//! | ---------------------- | -------------------- |
//! | `^1.2.3`, `1.2.3`      | `< 2.0.0`            |
//! | `^0.2.3`, `^0.2`       | `< 0.3.0`            |
//! | `^0.0.3`               | `< 0.0.4`            |
//! | `^0.0`, `^0`, `^1`     | `< 0.1.0`, `< 1.0.0`, `< 2.0.0` |
//! | `~1.2.3`, `~1.2`, `~1` | `< 1.3.0`, `< 1.3.0`, `< 2.0.0` |
//! | `=1.2.3`, `<=1.2.3`    | `<= 1.2.3`           |
//! | `=1.2`, `<=1.2`        | `< 1.3.0`            |
//! | `=1`, `<=1`            | `< 2.0.0`            |
//! | `<1.2.3`, `<1.2`, `<1` | `< 1.2.3`, `< 1.2.0`, `< 1.0.0` |
//! | `>=`, `>`, `*`         | none                 |
//!
//! A `*`, `x` or `X` component ends the version, so `1.*` reads as `1`. A
//! pre-release suffix sorts below its release, so `=0.3.0-alpha.1` stays
//! below `0.3.0`. Build metadata is ignored.

/// A version point. `release` is false for a pre-release, which sorts below its release.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct Point {
    major: u64,
    minor: u64,
    patch: u64,
    release: bool,
}

impl Point {
    fn release(major: u64, minor: u64, patch: u64) -> Self {
        Self {
            major,
            minor,
            patch,
            release: true,
        }
    }
}

/// A version with optional minor and patch, as a comparator writes it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Partial {
    major: u64,
    minor: Option<u64>,
    patch: Option<u64>,
    pre: bool,
}

/// The highest version a comparator allows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Upper {
    /// Every version below this point.
    Below(Point),
    /// Every version up to and including this point.
    AtMost(Point),
    /// No upper bound.
    None,
}

/// Parses `major.minor.patch` with no suffix, the form of a `below` value.
pub(crate) fn exact_version(text: &str) -> Option<Point> {
    let mut parts = text.split('.');
    let mut next = || parts.next().and_then(number);
    let point = Point::release(next()?, next()?, next()?);
    if parts.next().is_some() {
        return None;
    }
    Some(point)
}

/// Whether every version `requirement` allows is below `below`.
///
/// Returns `None` when `requirement` holds a form this reader does not parse.
pub(crate) fn stays_below(requirement: &str, below: Point) -> Option<bool> {
    let mut found = false;
    for comparator in requirement.split(',') {
        let below_it = match upper(comparator.trim())? {
            Upper::Below(point) => point <= below,
            Upper::AtMost(point) => point < below,
            Upper::None => false,
        };
        found |= below_it;
    }
    Some(found)
}

/// The upper bound of one comparator.
fn upper(comparator: &str) -> Option<Upper> {
    let (operator, rest) = split_operator(comparator);
    let rest = rest.trim_start();
    if is_wildcard(rest) {
        return (operator == "^" && !comparator.starts_with('^')).then_some(Upper::None);
    }
    let version = partial(rest)?;
    let bump_major = || {
        version
            .major
            .checked_add(1)
            .map(|m| Point::release(m, 0, 0))
    };
    let bump_minor = |minor: u64| {
        minor
            .checked_add(1)
            .map(|n| Point::release(version.major, n, 0))
    };
    let exact = |minor: u64, patch: u64| Point {
        major: version.major,
        minor,
        patch,
        release: !version.pre,
    };
    let below = |point: Option<Point>| point.map_or(Upper::None, Upper::Below);
    let bound = match (operator, version.minor, version.patch) {
        (">=" | ">", _, _) => Upper::None,
        ("^", None, _) => below(bump_major()),
        ("^", Some(minor), patch) if version.major == 0 => match (minor, patch) {
            (0, Some(patch)) => below(patch.checked_add(1).map(|p| Point::release(0, 0, p))),
            _ => below(bump_minor(minor)),
        },
        ("^", Some(_), _) => below(bump_major()),
        ("~" | "=" | "<=", None, _) => below(bump_major()),
        ("~", Some(minor), _) => below(bump_minor(minor)),
        ("=" | "<=", Some(minor), None) => below(bump_minor(minor)),
        ("=" | "<=", Some(minor), Some(patch)) => Upper::AtMost(exact(minor, patch)),
        ("<", minor, patch) => Upper::Below(exact(minor.unwrap_or(0), patch.unwrap_or(0))),
        _ => return None,
    };
    Some(bound)
}

/// Splits the operator from the version. A bare version reads as `^`.
fn split_operator(comparator: &str) -> (&'static str, &str) {
    for operator in [">=", "<=", ">", "<", "=", "^", "~"] {
        if let Some(rest) = comparator.strip_prefix(operator) {
            return (operator, rest);
        }
    }
    ("^", comparator)
}

fn is_wildcard(text: &str) -> bool {
    matches!(text, "*" | "x" | "X")
}

/// Parses `1`, `1.2`, `1.2.3`, `1.2.3-pre+build` and the wildcard forms `1.*`, `1.2.*`.
fn partial(text: &str) -> Option<Partial> {
    let core = text.split('+').next().unwrap_or(text);
    let (numbers, pre) = match core.split_once('-') {
        Some((numbers, suffix)) if !suffix.is_empty() => (numbers, true),
        Some(_) => return None,
        None => (core, false),
    };
    let mut parts = numbers.split('.');
    let major = number(parts.next()?)?;
    let mut rest = [None, None];
    let mut ended = false;
    for slot in &mut rest {
        let Some(part) = parts.next() else { break };
        if is_wildcard(part) {
            ended = true;
        } else if ended {
            return None;
        } else {
            *slot = Some(number(part)?);
        }
    }
    let [minor, patch] = rest;
    let full = minor.is_some() && patch.is_some();
    if parts.next().is_some() || (pre && !full) {
        return None;
    }
    Some(Partial {
        major,
        minor,
        patch,
        pre,
    })
}

fn number(text: &str) -> Option<u64> {
    if text.is_empty() || !text.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    text.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn below(requirement: &str, version: &str) -> Option<bool> {
        let point = exact_version(version).expect("below version");
        stays_below(requirement, point)
    }

    #[test]
    fn exact_version_needs_three_numbers() {
        assert_eq!(exact_version("0.3.0"), Some(Point::release(0, 3, 0)));
        for bad in ["0.3", "1", "1.2.3.4", "1.2.3-rc.1", "1.2.x", "", "a.b.c"] {
            assert_eq!(exact_version(bad), None, "{bad}");
        }
    }

    #[test]
    fn bare_and_caret_versions() {
        assert_eq!(below("0.1", "1.0.0"), Some(true));
        assert_eq!(below("^0.1.5", "1.0.0"), Some(true));
        assert_eq!(below("1", "1.0.0"), Some(false));
        assert_eq!(below("^1.2", "1.0.0"), Some(false));
        assert_eq!(below("^0.2.3", "0.3.0"), Some(true));
        assert_eq!(below("^0.3", "0.3.0"), Some(false));
        assert_eq!(below("^0.0.3", "0.0.4"), Some(true));
        assert_eq!(below("^0.0", "0.1.0"), Some(true));
        assert_eq!(below("^0", "1.0.0"), Some(true));
        assert_eq!(below("^0", "0.5.0"), Some(false));
    }

    #[test]
    fn tilde_versions() {
        assert_eq!(below("~0.2", "1.0.0"), Some(true));
        assert_eq!(below("~0.2.4", "0.3.0"), Some(true));
        assert_eq!(below("~1", "2.0.0"), Some(true));
        assert_eq!(below("~1", "1.5.0"), Some(false));
    }

    #[test]
    fn exact_and_at_most_versions() {
        assert_eq!(below("=0.2.9", "0.3.0"), Some(true));
        assert_eq!(below("=0.3.0", "0.3.0"), Some(false));
        assert_eq!(below("<=0.3.0", "0.3.0"), Some(false));
        assert_eq!(below("<=0.2", "0.3.0"), Some(true));
        assert_eq!(below("=0", "1.0.0"), Some(true));
        assert_eq!(below("=0.3.0-alpha.1", "0.3.0"), Some(true));
    }

    #[test]
    fn less_than_and_ranges() {
        assert_eq!(below(">=0.1, <0.3", "1.0.0"), Some(true));
        assert_eq!(below(">= 0.1 , < 0.3", "0.3.0"), Some(true));
        assert_eq!(below("<1.0.0", "1.0.0"), Some(true));
        assert_eq!(below("<1.0.1", "1.0.0"), Some(false));
        assert_eq!(below(">=0.1", "1.0.0"), Some(false));
        assert_eq!(below(">0.1.0, <=2.0.0", "1.0.0"), Some(false));
    }

    #[test]
    fn wildcards() {
        assert_eq!(below("*", "1.0.0"), Some(false));
        assert_eq!(below("0.2.*", "0.3.0"), Some(true));
        assert_eq!(below("0.*", "1.0.0"), Some(true));
        assert_eq!(below("1.*", "1.0.0"), Some(false));
    }

    #[test]
    fn unparsed_forms_are_none() {
        for bad in [
            "", "^*", ">=*", "abc", "1.2.3.4", "1.*.3", "1.2-pre", "1.2.3-", ",", "~>1",
        ] {
            assert_eq!(below(bad, "1.0.0"), None, "{bad}");
        }
    }
}
