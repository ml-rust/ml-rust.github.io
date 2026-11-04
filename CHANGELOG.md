# Changelog

Format: [Keep a Changelog](https://keepachangelog.com/en/1.1.0/). Versioning: [SemVer](https://semver.org/spec/v2.0.0.html).

A release tag needs a non-empty section here. The section becomes the GitHub Release body.

## [Unreleased]

### Added

- Sort and filter on list pages, copy buttons on install lines.
- `/compare/`: up to 4 projects side by side.
- `catalogue validate`: checks every data file and the rules across files.
- `catalogue build`: builds the static site from the data files.
- Tier engine and `data/thresholds.toml`: published rules that place each entry in a tier.
- `catalogue dump fetch` and `dump stats`: download and read the crates.io database dump.
- `catalogue dump usage`: adoption signals of named crates.
- `catalogue discover crates` and `discover github`: candidate crates and app repos for review.
- `catalogue github facts`: repo state, human commits and maintainer response.
- `catalogue github link`: checks that a crate was published from a commit on a branch or tag of its repo. A commit that only a fork holds does not verify.
- `catalogue rustsec fetch` and `rustsec show`: RustSec advisories per crate.
- `catalogue ecosystems crates`: ecosyste.ms dependents per crate.
- `catalogue distribution`: release, Homebrew, Docker Hub and Repology counts for apps.
- `catalogue tiers`: assigns each entry its tier with the reasons.
- `catalogue calibrate`: percentiles of the tier signals, for setting thresholds.
- `catalogue seed`: drafts entries from discovery for human review.
- `catalogue seed`: lists a repository's main crate first, leaves out crates of other owners, and matches topic `terms` in names and descriptions.
- `catalogue seed --leads`: drafts library and app entries from a list of GitHub repos.
- `catalogue tiers --history`: keeps daily snapshots for the two-check dead rule and Docker growth.
- `catalogue tiers`: a failed network fetch falls back to the previous snapshot and the run summary names it.
- `catalogue tiers --json`: writes each entry's tier, reasons and signals with source links and dates.
- `catalogue sandbox`: weekly CI check that dormant crates still build.
- `catalogue bot check`: checks an issue form and prints the would-be data file and a report.
- `deploy/`: systemd units and scripts that install the bot on a VM and update it from the latest release.
- `catalogue bot labels`: syncs the repo labels to `.github/labels.toml` as the GitHub App. It never deletes a label.
- `catalogue bot run`: checks every open issue with a form label and keeps one bot comment and its label current.
- `catalogue bot run`: opens a data PR for each approved issue.
- `catalogue bot run`: adds an advisory LLM check of evidence links.
- `data/synonyms.toml`: search synonyms. A human reviews every group.
- `data/categories.toml`: browse by what a project is, on the site, in search, in seed drafts and in the submit form.
- `data/backends.toml` and entry pages: license, backends, native code, dependents, contributors and install line, none of which counts toward a tier.
- Entry lists: one shared order, strongest tier first, with no score.
- Entry `part_of`: lists show one row per project family, and `/featured/` holds the full Featured list.
- `catalogue seed`: sets `part_of` from a crate name match and a shared crate owner.
- `[movement]` in `data/thresholds.toml`: New, Rising and Falling from daily snapshots, on the changes page and list rows. It never affects a tier.
- `/changes/feed.xml`: Atom feed of tier changes and new entries.
- `/data/catalogue.json`, `/llms.txt`, `sitemap.xml` and `robots.txt` for scripts and AI agents.
- CI and tag-gated release workflows. Releases ship `catalogue` for x86_64 and aarch64 Linux (musl), with build attestation.

### Changed

- Lists lead with downloads and dependents, show versions, and order by downloads.
- "Active users" now reads "active dependents".
- Data file schema 2: adds install lines, docs links and plain tier reasons.
- New site theme on every page. Project pages explain each tier and compare neighbours.
- The New tier now means a recent first release only.
- Row facts state only what the check found, and native code reads deeper dependencies.
- Categories cover more projects and match description terms first.
