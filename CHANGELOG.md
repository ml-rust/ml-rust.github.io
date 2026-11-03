# Changelog

All notable changes to this project are documented here. The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

Each release must have a non-empty section here before it can be tagged — `.github/workflows/release-validate.yml` refuses a tag whose version has no entry, and the GitHub Release body is this file's section for that version. The workspace releases one binary, `catalogue`, and nothing is published to crates.io.

## [Unreleased]

### Added

- **Data model** — `catalogue-core` defines entries, judgments, tiers, topics, synonyms and maintainers. It loads the TOML data files and checks the rules that span files.
- **`catalogue validate`** — the `catalogue` binary checks every data file in a directory (`--data`, default `data`). It exits 0 for valid data, 1 for data errors and 2 for a read error.
- **CI workflows** — `ci.yml` and `test.yml` run rustfmt, rustdoc, shellcheck, clippy, tests, the MSRV check, cargo-deny, data validation and zizmor.
- **Release workflow** — pushing a `v*` tag runs `Release Prepare`, which validates the tag and the changelog section and runs the full suite. The manually dispatched `Release` workflow then builds `catalogue` for `x86_64-unknown-linux-musl` and `aarch64-unknown-linux-musl`, attests build provenance, and creates the GitHub Release with the tarballs and SHA-256 files.
