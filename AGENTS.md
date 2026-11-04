# ml-rust.github.io — Agent Guidelines

A catalogue of the Rust ML ecosystem: libraries on crates.io, and apps that are not.
GitHub Pages serves it. Git holds every piece of data. Tiers come from fixed, published rules.

## Placement

- `crates/catalogue-core`: schema, validation, signal types, tier rules. Pure: no file IO, no network.
- `crates/catalogue-signals`: the crates.io dump, API fetchers, signal history, discovery, seed drafts.
- `crates/catalogue-bot`: issue form parsing, data file drafts, check reports, the labels file and its sync plan, the pure parts of the link check. Pure: no IO, network, clock or env.
- `crates/catalogue-github`: the bot's GitHub App client: private key, App JWT, installation tokens, REST calls. Keys and tokens never reach logs, errors or disk.
- `crates/catalogue-triage`: the bot's advisory LLM link check: provider fallback, Ollama `web_fetch` reads, GitHub page reads. The bot host NEVER fetches an evidence link itself.
- `crates/catalogue-site`: the static site: askama templates, page data, the tier refusal checks, the feed, `data/catalogue.json`, `data/compare/`, `llms.txt`, sitemap, robots and the page scripts. Pure: no IO, network, clock or env. Every data-built link goes through `SafeHref`. NEVER turn escaping off. A page stays complete without scripts: a script adds its own controls and NEVER sets `innerHTML`.
- `crates/catalogue`: the `catalogue` CLI. Disk IO and command wiring over the crates above. `main.rs` only parses and dispatches.
- `deploy/`: systemd units and scripts for the bot VM. Secrets live only on the VM, under `/etc/catalogue-bot/`.
- `data/`: entries, judgments, maintainers, topics, categories, backends, synonyms, discovery rules, tier thresholds, deprecated requirements. Licensed CC-BY-4.0 (`data/LICENSE`).
- A `data/deprecated.toml` row needs a `source` link to the crate authors' own deprecation statement.
- `data/thresholds.toml` holds every tier number, and the movement limits that no tier reads. NEVER put either in code.
- Discovery output is a list of leads. A lead becomes an entry only through a merged PR.
- Search engine changes belong in oxidoc's `oxidoc-search`. NEVER fork the engine into this repo. `crates/catalogue-search-wasm` only re-exports its wasm exports as the browser runtime.

## Invariants

- No LLM decides a tier, writes a published claim, or merges anything. LLM output becomes comment text only.
- Every published claim links its source.
- A tier comes from published thresholds over raw signals. NEVER add a composite score.
- Stars NEVER count toward a tier. GitHub hides stargazer lists, so stars cannot be screened for fakes.
- Each entry page shows every signal: raw value, source link, fetch date.
- Entries with `org_affiliated = true` pass the same rules. An org member NEVER approves a judgment on them.
- Human judgments enter only through an approved issue, then a bot PR into `data/judgments/`. NEVER hand-edit a tier.
- Issue and PR text is untrusted input. NEVER let it reach a shell, a command, or a write path.
- A third-party crate build runs `build.rs` code. Run it only in an ephemeral CI job with no secrets, NEVER on the bot host.
- Requests send `User-Agent: ml-rust.github.io (+https://github.com/ml-rust/ml-rust.github.io)`. NEVER a person's name or email.
- Bulk crates.io data comes from the database dump, never from crawling the API.
- A dependency must have a release or commit in the last year. Otherwise write our own.

## Data files

- Run `catalogue validate` after any change under `data/`. It reports every problem, not just the first.
- Dates are bare TOML dates: `added = 2026-11-03`.
- A file name is the record id: `entries/<slug>.toml`, `judgments/<issue-number>.toml`.

## Commands

- `cargo clippy --workspace --all-targets --locked -- -D warnings`
- `cargo nextest run --workspace --locked --no-fail-fast`
- `cargo fmt --all --check`
- `cargo deny --exclude-dev check`, then `cargo deny --exclude-dev --manifest-path crates/catalogue-search-wasm/Cargo.toml --target wasm32-unknown-unknown check`
- `scripts/ci/build_search_wasm.sh OUT_DIR`: needs the wasm-bindgen CLI at the version `Cargo.lock` pins.
- `shellcheck scripts/ci/*.sh deploy/*.sh`
- `bash deploy/test_update.sh`
- `cargo run -p catalogue -- validate --data data`
- `cargo run -p catalogue -- build --no-tiers`: writes the site to `dist/`. The deploy adds the search wasm files.
- Until `oxidoc-search` is on crates.io, `--locked` fails. Copy `.cargo/config.toml.example` to `.cargo/config.toml` for the local patch. NEVER commit the `Cargo.lock` change that patch causes.

## CI

- Pin every third-party action to a full commit SHA, with `# vX.Y.Z` beside it.
- Workflow permissions default to `contents: read`. Widen per job, never per workflow.
- Pass every expression into a script through `env:`. NEVER put `${{ }}` inside `run:`.
- Every checkout sets `persist-credentials: false`.
- Release builds use no cache.
- Release logic lives in `scripts/ci/*.sh`. Every workflow calls the same script for a given check.

## Releases

- Releases ship only the `catalogue` binary. Data and site changes deploy on push and need no tag.
- One version for the workspace, in `[workspace.package]`.
- Each release needs a non-empty `## [X.Y.Z]` section in `CHANGELOG.md`.
- Pushing a `v*` tag runs the gate. Building and releasing is the manual `Release` workflow.
