# Changelog

Format: [Keep a Changelog](https://keepachangelog.com/en/1.1.0/). Versioning: [SemVer](https://semver.org/spec/v2.0.0.html).

A release tag needs a non-empty section here. The section becomes the GitHub Release body.

## [Unreleased]

### Added

- `catalogue tiers --fetch`: picks what a run fetches: `all`, `changed`, `new` or `none`. Use `none` to apply new rules with no requests.
- `Pages`: the scheduled scan runs weekly on Sunday and fetches what changed. Run the workflow with `mode` to score by hand. A lost scan repairs itself on the next run.

### Fixed

- `search`: a project that says it does the job now ranks above one that only mentions the word.

## [0.1.2]

### Added

- `catalogue state`: writes a one-page report on ML in Rust with AI models, then audits it. Set `state.toml` and the model keys to run it.
- `site`: `/state/` shows that report, labeled as AI-written, with charts from catalogue data.

### Changed

- `topics`: `vision-documents` splits into `images-vision`, `video` and `documents`, and `math-gpu` splits into `math-science` and `tensors-gpu`. Old topic links open a page that links the new ones.
- `site`: a project with no GPU match reads `Unknown`, because it can still run on a GPU. `tch-rs` shows CUDA.
- `site`: similar projects are picked by category, shared jobs and description, not by downloads. A project outside ML Rust also shows the closest ML Rust project, and says so.

## [0.1.1]

### Changed

- `data/topics.toml`: every topic needs a `blurb`. Add one line that defines the topic.
- `site`: page titles fit search results, topic pages define their job, and shared links show an image.

## [0.1.0]

### Added

- `catalogue validate`: checks every data file and the rules across files.
- `catalogue tiers`: places each entry in a tier from `data/thresholds.toml` and writes a report with every source and date.
- `catalogue build`: builds the static site from the data files and a tier report.
- `catalogue seed`: drafts entries from discovery and `data/leads.txt` for human review.
- `catalogue discover`: lists candidate crates, app repos and repos below the active-user floor for review.
- `catalogue dump`, `github`, `rustsec`, `ecosystems`, `distribution`, `calibrate`: read and inspect each signal source.
- `catalogue sandbox`: weekly CI check that dormant crates still build.
- `catalogue bot`: checks issue forms, syncs labels and opens a data PR for each approved issue. A person merges every PR.
- Site: browse by job or category, search in the browser, sort, filter and compare up to 4 projects.
- Home page: a labeled, hand-picked list of ML Rust projects from `data/showcase.toml`. `/ml-rust/` lists them all.
- Project pages: the README, compatibility facts, similar projects, and every signal with its source.
- `/changes/`, `/changes/feed.xml`, `/data/catalogue.json`, `/llms.txt`: tier changes and machine-readable data.
- `deploy/`: systemd units and scripts that run the bot on a VM.
- Releases ship `catalogue` for x86_64 and aarch64 Linux (musl).
