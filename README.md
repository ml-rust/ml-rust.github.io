# ml-rust.github.io

A catalogue of Rust ML libraries on crates.io and Rust ML apps outside it. Each entry gets a tier from published rules over measured signals. The site is at https://ml-rust.github.io.

## What makes it different

- A tier comes from fixed, published rules applied to raw signals.
- No LLM decides a tier, writes a published claim, or merges anything.
- GitHub stars alone never qualify a project. GitHub hides who starred, so the repository route also needs forks and contributors.
- Each entry page lists every signal with its raw value, source link, and fetch date.
- Apps outside crates.io are covered.
- Search runs in the browser.

## How tiers work

The tier rules run in a fixed order on every check. Each entry gets one tier per check.

| Tier | What earns it |
| --- | --- |
| Archived | The author or RustSec ends the project. Any one of these archives it: an archived repository, an `unmaintained` advisory, a deprecation notice, every version yanked, or a deleted crate. A dead entry also archives after enough consecutive dead checks. |
| New | The first release is recent. New takes precedence over Featured. |
| Featured | A library passes its adoption, health, and docs gates. Adoption means enough active dependents, enough new dependents, or the repository route. An app passes enough of its five use signals: binary downloads, Homebrew installs, Repology packaging, Docker pull growth, and the repository route. The entry must be active or finished. A library, or an app that ships a crate, needs at least one version that is neither a prerelease nor `0.0.x`. |
| Listed | No other tier applies. |

The repository route needs stars, forks, and contributors to each reach their limit on the same check. It is a gate of separate limits, never a score.

Health covers unpatched vulnerabilities, a repository link that the release commit or a manifest in the repository proves, and the share of peak active dependents that stay active.

- A Featured entry keeps Featured until an approved demotion judgment allows the drop.
- An org-affiliated entry needs approval from a maintainer outside the organization to become Featured.
- A missing signal never counts as zero or false. It keeps the entry out of Featured.

### Rot and unmaintained detection

Each check measures the following:

1. **Expected interval (E).** The days a project normally goes between releases. It comes from the release count, the release pace, and adoption, within published floor and cap values. Apps use the floor. So does a library with no known latest version.
2. **Dormancy.** An entry is dormant when the days since its later of last release and last human commit reach E. Unknown dates leave dormancy unknown.
3. **Rot signals.** The check counts six signals:

| Signal | Meaning |
| --- | --- |
| R1 | An outside issue stays unanswered, with no maintainer reply in the window. |
| R2 | A vulnerability advisory stays unpatched past the grace window. |
| R3 | A library's share of its peak active dependents falls below the limit. |
| R4 | The latest release requires a deprecated dependency. |
| R5 | The repository no longer exists. |
| R6 | The latest release of a library fails `cargo check` on current stable Rust. |

The sandbox check runs in CI and records its date. A sandbox result older than its age limit reads as unknown.

4. **Dead.** A dormant entry with enough rot signals is dead on that check. A `maintained` statement from the project owner blocks the dead verdict. Unknown dormancy is not dead.
5. **Finished.** A dormant entry with no rot signal and still adopted is finished. So is an entry with a `finished` statement from its owner and no rot signal. A finished entry counts as active for Featured.

### Fetch errors

A fetch error reads the newest earlier snapshot of that signal. The snapshot must fall within the fallback age limit. Past that limit, the signal is unknown. The Pages workflow records a snapshot each day.

### Thresholds

Every threshold lives in `data/thresholds.toml`. The [rules page](https://ml-rust.github.io/rules/) shows each rule beside its thresholds. Each change to the file raises its `version`.

## Submit or correct a project

Open the [issue chooser](https://github.com/ml-rust/ml-rust.github.io/issues/new/choose) and pick a form. Blank issues are off. Read the [rules](https://ml-rust.github.io/rules/) first.

| Form | Use it to | Label |
| --- | --- | --- |
| Submit | Propose a library or app. | `submit` |
| Correctness report | Report a wrong fact on an entry. | `judgment:correctness` |
| Topic change | Replace the topics of an entry. | `judgment:topic` |
| Product or demo | Set an entry as a product or a demo. | `judgment:kind` |
| Featured demotion | Approve a demotion that the rules propose. | `judgment:demotion` |
| Featured approval | Approve Featured for an org-affiliated entry. | `judgment:featured-approval` |
| Maintainer statement | State that a project is maintained or finished. | `judgment:maintainer` |
| Supersedes or fork | Report that one entry replaces another. | `judgment:supersedes` |

### What happens next

A person makes every approval.

1. The form applies its label when the issue opens.
2. The bot checks the form on its next pass. It posts a check comment and adds `bot-checked` or `needs-fix`.
3. The advisory link check adds text to that comment only.
4. A maintainer listed in `data/maintainers.toml` adds `approved`. The bot refuses an approval from a bot account, from the submitter, or from any account the rules exclude. A refused approval loses the label.
5. If the issue body changes after the check, the bot removes `approved`. A maintainer approves again.
6. The bot opens a data PR on the branch `bot/issue-<number>` and adds `pr-open`.
7. A person reviews and merges the data PR. The bot does not merge.
8. Merging updates the data on `main`. The Pages workflow then rebuilds the site.

Each judgment appears on its entry page with a link to its issue.

## Repository layout

| Path | Holds |
| --- | --- |
| `crates/catalogue-core` | Data model, validation, and tier rules. No IO. |
| `crates/catalogue-signals` | Reads the crates.io database dump, fetches the APIs, and keeps signal history. |
| `crates/catalogue-bot` | Parses issue forms, writes check comments, and plans label and data changes. No IO. |
| `crates/catalogue-github` | The bot's GitHub App client. |
| `crates/catalogue-triage` | The bot's advisory link check. |
| `crates/catalogue-site` | Site templates, page data, and refusal checks. No IO. |
| `crates/catalogue-search-wasm` | The browser search runtime. |
| `crates/catalogue` | The `catalogue` CLI. |
| `data/` | Entries, judgments, maintainers, topics, categories, synonyms, discovery rules, tier thresholds, deprecated requirements, and backends. |
| `deploy/` | systemd units and scripts for the bot VM. |

## Develop

Install Rust 1.95 or later. The workspace sets this as `rust-version` in `Cargo.toml`.

Copy the local patch first:

```bash
cp .cargo/config.toml.example .cargo/config.toml
```

Set the `path` in `.cargo/config.toml` to your oxidoc checkout. `oxidoc-search` is not on crates.io yet. Until it is, `--locked` fails. The patch changes `Cargo.lock`, so do not commit that change. `.cargo/config.toml` is gitignored.

Run each command on its own line. CI adds `--locked` to the cargo builds.

```bash
cargo clippy --workspace --all-targets -- -D warnings
cargo nextest run --workspace --no-fail-fast
cargo fmt --all --check
cargo deny --exclude-dev check
cargo deny --exclude-dev --manifest-path crates/catalogue-search-wasm/Cargo.toml --target wasm32-unknown-unknown check
scripts/ci/build_search_wasm.sh OUT_DIR
shellcheck scripts/ci/*.sh deploy/*.sh
bash deploy/test_update.sh
cargo run -p catalogue -- validate --data data
cargo run -p catalogue -- build --no-tiers
```

- `validate` checks every data file and every rule across files. Run it after any change under `data/`.
- `build_search_wasm.sh` builds the search wasm files. It needs the wasm-bindgen CLI at the version `Cargo.lock` pins.
- `build --no-tiers` writes the site to `dist/` without tier data. It replaces `dist/` whole. The build needs either `--report` or `--no-tiers`, and it does not write the search wasm files. It also writes `changes/feed.xml`, `data/catalogue.json`, `data/README.txt`, `llms.txt`, `sitemap.xml` and `robots.txt`.

## Deploy

- The site deploys from `main` through `.github/workflows/pages.yml`.
- The bot runs on a VM. Run `sudo bash deploy/install.sh` on it. The script prints each input still missing, with the command that adds it.

## License

- **Code:** Apache License 2.0 (`LICENSE`). The workspace `Cargo.toml` sets this license, and every crate inherits it. No crate sets a different license. Every crate sets `publish = false`.
- **Data:** Creative Commons Attribution 4.0 International (CC-BY-4.0) (`data/LICENSE`) covers `data/`.
