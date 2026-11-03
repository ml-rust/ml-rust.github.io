# Changelog

Format: [Keep a Changelog](https://keepachangelog.com/en/1.1.0/). Versioning: [SemVer](https://semver.org/spec/v2.0.0.html).

A release tag needs a non-empty section here. The section becomes the GitHub Release body.

## [Unreleased]

### Added

- `catalogue validate`: checks every data file and the rules across files.
- `catalogue dump fetch` and `dump stats`: download and read the crates.io database dump.
- `catalogue dump usage`: adoption signals of named crates.
- `catalogue discover crates` and `discover github`: candidate crates and app repos for review.
- `catalogue github facts`: repo state, human commits and maintainer response.
- `catalogue rustsec fetch` and `rustsec show`: RustSec advisories per crate.
- `catalogue ecosystems crates`: ecosyste.ms dependents per crate.
- CI and tag-gated release workflows. Releases ship `catalogue` for x86_64 and aarch64 Linux (musl), with build attestation.
