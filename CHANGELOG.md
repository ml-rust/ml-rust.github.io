# Changelog

Format: [Keep a Changelog](https://keepachangelog.com/en/1.1.0/). Versioning: [SemVer](https://semver.org/spec/v2.0.0.html).

A release tag needs a non-empty section here. The section becomes the GitHub Release body.

## [Unreleased]

### Added

- `catalogue validate`: checks every data file and the rules across files.
- Tier engine and `data/thresholds.toml`: published rules that place each entry in a tier.
- `catalogue dump fetch` and `dump stats`: download and read the crates.io database dump.
- `catalogue dump usage`: adoption signals of named crates.
- `catalogue discover crates` and `discover github`: candidate crates and app repos for review.
- `catalogue github facts`: repo state, human commits and maintainer response.
- `catalogue github link`: checks that a crate was published from a commit on the default branch of its repo. A commit that only a fork holds does not verify.
- `catalogue rustsec fetch` and `rustsec show`: RustSec advisories per crate.
- `catalogue ecosystems crates`: ecosyste.ms dependents per crate.
- `catalogue distribution`: release, Homebrew, Docker Hub and Repology counts for apps.
- `catalogue tiers`: assigns each entry its tier with the reasons.
- `catalogue calibrate`: percentiles of the tier signals, for setting thresholds.
- `catalogue seed`: drafts entries from discovery for human review.
- `catalogue tiers --history`: keeps daily snapshots for the two-check dead rule and Docker growth.
- `catalogue bot check`: checks an issue form and prints the would-be data file and a report.
- `catalogue bot labels`: syncs the repo labels to `.github/labels.toml` as the GitHub App. It never deletes a label.
- `catalogue bot run`: checks every open issue with a form label and keeps one bot comment and its label current.
- CI and tag-gated release workflows. Releases ship `catalogue` for x86_64 and aarch64 Linux (musl), with build attestation.
