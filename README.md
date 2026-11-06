<div align="center">

# ML Rust catalogue

<h3>Machine learning libraries and apps in Rust.</h3>

<p>
Find a Rust library or app for your machine learning job, with real usage data for each project.
</p>

<p>
  <a href="https://ml-rust.github.io"><strong>Browse the catalogue</strong></a>
  ·
  <a href="https://ml-rust.github.io/state/"><strong>State of ML in Rust</strong></a>
  ·
  <a href="https://ml-rust.github.io/rules/"><strong>Tier rules</strong></a>
  ·
  <a href="https://ml-rust.github.io/submit/"><strong>Add a project</strong></a>
</p>

<p>
  <a href="https://discord.gg/jBhFk9kHPg">
    <img src="https://img.shields.io/discord/1453357769720594543?label=Discord&logo=discord&logoColor=white&color=5865F2" alt="Join the Discord">
  </a>
</p>

<p>
  <a href="https://github.com/ml-rust/ml-rust.github.io/actions/workflows/ci.yml">
    <img src="https://img.shields.io/github/actions/workflow/status/ml-rust/ml-rust.github.io/ci.yml?branch=main&label=ci" alt="CI status">
  </a>
  <a href="LICENSE">
    <img src="https://img.shields.io/github/license/ml-rust/ml-rust.github.io" alt="License">
  </a>
  <a href="https://github.com/ml-rust/ml-rust.github.io/stargazers">
    <img src="https://img.shields.io/github/stars/ml-rust/ml-rust.github.io?style=social" alt="GitHub stars">
  </a>
</p>

</div>

It covers crates on crates.io and apps outside it, including [burn](https://ml-rust.github.io/projects/burn/), [candle](https://ml-rust.github.io/projects/candle/), [ort](https://ml-rust.github.io/projects/ort/), [polars](https://ml-rust.github.io/projects/polars/), [qdrant](https://ml-rust.github.io/projects/qdrant/), [rig](https://ml-rust.github.io/projects/rig/) and [tantivy](https://ml-rust.github.io/projects/tantivy/).

## Find a project by job

- [Run and serve models](https://ml-rust.github.io/topics/run-models/)
- [Build LLM apps and agents](https://ml-rust.github.io/topics/llm-apps/)
- [Train models](https://ml-rust.github.io/topics/train-models/)
- [Speech and audio](https://ml-rust.github.io/topics/speech-audio/)
- [Text, embeddings and search](https://ml-rust.github.io/topics/text-search/)
- [Images and vision](https://ml-rust.github.io/topics/images-vision/)
- [Video](https://ml-rust.github.io/topics/video/)
- [Documents and OCR](https://ml-rust.github.io/topics/documents/)
- [Tabular data and classic ML](https://ml-rust.github.io/topics/tabular-ml/)
- [Math and science](https://ml-rust.github.io/topics/math-science/)
- [Tensors and GPU compute](https://ml-rust.github.io/topics/tensors-gpu/)
- [Datasets and model formats](https://ml-rust.github.io/topics/data-formats/)

## Find a project by kind

| | | |
| --- | --- | --- |
| [ML frameworks](https://ml-rust.github.io/categories/ml-frameworks/) | [Inference runtimes](https://ml-rust.github.io/categories/inference-runtimes/) | [LLM clients](https://ml-rust.github.io/categories/llm-clients/) |
| [Agents and LLM apps](https://ml-rust.github.io/categories/agent-frameworks/) | [Vector databases](https://ml-rust.github.io/categories/vector-databases/) | [Embeddings](https://ml-rust.github.io/categories/embeddings/) |
| [Search engines](https://ml-rust.github.io/categories/search-engines/) | [Text and tokenizers](https://ml-rust.github.io/categories/text-tools/) | [Speech and audio](https://ml-rust.github.io/categories/speech-audio/) |
| [Computer vision](https://ml-rust.github.io/categories/computer-vision/) | [Classical ML](https://ml-rust.github.io/categories/classical-ml/) | [Tensors and math](https://ml-rust.github.io/categories/math-libraries/) |
| [Model formats](https://ml-rust.github.io/categories/model-formats/) | [Data formats and dataframes](https://ml-rust.github.io/categories/data-tools/) | [Developer tools](https://ml-rust.github.io/categories/developer-tools/) |

Or [search everything](https://ml-rust.github.io/search/). Search runs in your browser.

## What you get for each project

- **What it does:** a short description and the project's README.
- **Whether people use it:** downloads and dependents, shown first.
- **Whether it is alive:** release and commit activity, unpatched advisories and deprecation notices.
- **Whether it fits:** compatibility facts and similar projects. You can compare up to 4 projects.
- **Where each number comes from:** the raw value, a source link and the fetch date.

## How projects are ranked

Each project gets one tier from [published rules](https://ml-rust.github.io/rules/) over measured data.

| Tier | Meaning |
| --- | --- |
| [Featured](https://ml-rust.github.io/featured/) | Shown first. It passes every adoption, health and docs gate. |
| Listed | Shown in the normal listing. |
| New | Its first release is recent. |
| Archived | No longer maintained, or ended by its author. |

- No LLM decides a tier, writes a claim on a project or topic page, or merges anything.
- The [state of ML in Rust](https://ml-rust.github.io/state/) is the one page AI models write. It says so, and no tier reads it.
- GitHub stars alone never earn a tier. GitHub hides who starred, so stars cannot be screened for fakes.
- Every tier change appears on the [changes page](https://ml-rust.github.io/changes/) and in its [feed](https://ml-rust.github.io/changes/feed.xml).

## Who runs this

The ML Rust organization runs this catalogue and maintains some of the listed projects.

- [Its own projects](https://ml-rust.github.io/ml-rust/) pass the same rules as every other project.
- Featured for its own projects also needs approval from a maintainer outside the organization.
- The home page labels a hand-picked list of its projects. No tier reads that list.

## Add or correct a project

Open the [issue chooser](https://github.com/ml-rust/ml-rust.github.io/issues/new/choose) and pick a form.

| Form | Use it to |
| --- | --- |
| Submit | Propose a library or app. |
| Correctness report | Report a wrong fact on an entry. |
| Topic change | Replace the topics of an entry. |
| Product or demo | Set an entry as a product or a demo. |
| Supersedes or fork | Report that one entry replaces another. |
| Maintainer statement | State that a project is maintained or finished. |
| Featured demotion | Approve a demotion that the rules propose. |
| Featured approval | Approve Featured for an org-affiliated entry. |

A person makes every approval and merges every change:

1. The bot checks the form and posts a comment. An advisory link check adds text to that comment only.
2. A maintainer listed in `data/maintainers.toml` adds `approved`. The bot refuses an approval from a bot account or the submitter.
3. If the issue body changes after the check, the bot removes `approved`.
4. The bot opens a data PR. A person reviews and merges it, and the site rebuilds.

Each judgment appears on its project page with a link to its issue.

## Use the data

| URL | Holds |
| --- | --- |
| [`/data/catalogue.json`](https://ml-rust.github.io/data/catalogue.json) | Every project, as JSON. |
| [`/changes/feed.xml`](https://ml-rust.github.io/changes/feed.xml) | Tier changes and new projects, as an Atom feed. |
| [`/llms.txt`](https://ml-rust.github.io/llms.txt) | A site summary for LLM tools. |

## Develop

Install Rust 1.95 or later. Run each command on its own line. CI adds `--locked` to the cargo builds.

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
- `build --no-tiers` replaces `dist/` with a site without tier data. It does not write the search wasm files. The build needs either `--report` or `--no-tiers`.

| Path | Holds |
| --- | --- |
| `crates/catalogue-core` | Data model, validation and tier rules. No IO. |
| `crates/catalogue-signals` | Reads the crates.io database dump, fetches the APIs and keeps signal history. |
| `crates/catalogue-bot` | Parses issue forms, writes check comments, and plans label and data changes. No IO. |
| `crates/catalogue-github` | The bot's GitHub App client. |
| `crates/catalogue-triage` | The bot's advisory link check. |
| `crates/catalogue-state` | Writes and checks the state report: what models read, the prompts and the checks on each answer. |
| `crates/catalogue-site` | Site templates, page data and refusal checks. No IO. |
| `crates/catalogue-search-wasm` | The browser search runtime. |
| `crates/catalogue` | The `catalogue` CLI. |
| `data/` | Entries, judgments, maintainers, topics, categories, synonyms, discovery rules, tier thresholds, deprecated requirements, backends and the showcase. |
| `deploy/` | systemd units and scripts for the bot VM. |

## Deploy

- The site deploys from `main` through `.github/workflows/pages.yml`.
- A push deploys in minutes with the newest scores.
- A push that changes `data/` fetches new entries only, then applies the rules to every entry.
- A push that changes only the scoring crates fetches nothing, then applies the rules to every entry.
- The weekly scan runs on Sunday at 22:23 UTC. It fetches what changed.
- When the last scan is over 8 days old, the next push or manual run scans instead, unless the manual run asks for `all` or `changed`.
- To deploy by hand, run `gh workflow run pages.yml`. Add `-f mode=new`, `none`, `changed` or `all` to score first. `all` takes over an hour.
- The Pages workflow needs the `SIGNALS_TOKEN` secret: a fine-grained token with public repositories read-only.
- The bot runs on a VM. Run `sudo bash deploy/install.sh` on it. The script prints each input still missing, with the command that adds it.

## License

- **Code:** Apache License 2.0 (`LICENSE`). No crate is published to crates.io.
- **Data:** CC-BY-4.0 (`data/LICENSE`) covers `data/`.
