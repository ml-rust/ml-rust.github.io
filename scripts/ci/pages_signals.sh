#!/usr/bin/env bash
# The signals job of the pages workflow: one snapshot per run date on the orphan
# `signals` branch, and the tier report the site build reads.
#
#   scripts/ci/pages_signals.sh prepare <history-dir>
#   scripts/ci/pages_signals.sh tiers   <history-dir> <report-file>
#   scripts/ci/pages_signals.sh commit  <history-dir>
#   scripts/ci/pages_signals.sh plan    <output-file>
#   scripts/ci/pages_signals.sh report  <dir> <run-id>
#
# `plan` writes `mode=<mode>` to <output-file>, and with `skip` the Pages run that
# holds the newest tier report.
#
# - The schedule gives `changed`.
# - A push under DATA_PATHS gives `new`. A push under only SCORING_PATHS gives `none`.
#   Any other push gives `skip`. A push it cannot list in full gives `new`.
# - A manual run gives its `mode` input.
# - `skip`, `new` or `none` gives `changed` when LAST_SCAN_FILE on the `signals` branch is
#   absent or older than SCAN_MAX_AGE_DAYS, so a lost or failed scan repairs itself on the
#   next run. Only `changed` and `all` runs write that file: a snapshot of `new` or `none`
#   holds a new date with no real check behind it.
# - `skip` with no earlier report gives `all`.
# - `new`, `none` or `changed` with no history gives `all`. A history with no README
#   store counts as no history, because a run that reuses values reads READMEs from it.
#
# `report` downloads the tier report of Pages run <run-id> into <dir>. It reads
# only runs of the Pages workflow, because sandbox-check.yml runs third-party
# build scripts, and an artifact of that run can claim any name.
#
# `prepare` clones the branch into <history-dir>. A missing branch gives an
# empty repository, and `commit` creates the branch. Any other remote error
# stops the job, so a network error never starts a second history.
#
# `tiers` builds the catalogue binary, fetches the crates.io dump and the
# RustSec archive into $RUNNER_TEMP, downloads the newest `build-checks`
# artifact when one exists, and runs `catalogue tiers --fetch "$FETCH_MODE"`.
# An unset FETCH_MODE reads as `all`. No `build-checks`
# artifact leaves rot signal R6 unknown. A missing dump or RustSec archive ends
# the job. A network signal that fails to fetch falls back to the previous
# snapshot instead, and `catalogue tiers` lists each fallback in its output and in
# the job summary. A rejected token still ends the job. So does a share of fetches
# with no usable earlier value above `fallback.max_failed_share`, in a run of at least
# 20 fetches.
#
# `commit` stages the snapshot and the README store, then pushes them. A run repeated on one day replaces
# that day's file. Unchanged rows give identical bytes, so that run pushes
# nothing. A rejected push fetches the branch, puts the snapshot back on top, and
# tries again.
#
# Every command reads GITHUB_TOKEN from the environment. The token never reaches
# the disk: git asks for it through the helper in `git_remote`, on each request.
# In `tiers`, GITHUB_TOKEN holds the SIGNALS_TOKEN secret, which `catalogue tiers`
# sends. The `gh` calls prefer GH_TOKEN, which reads this repo's runs and artifacts.
#
# Called by pages.yml.

set -euo pipefail

BRANCH="signals"
BOT_NAME="github-actions[bot]"
BOT_EMAIL="41898283+github-actions[bot]@users.noreply.github.com"
BIN="target/release/catalogue"
ARTIFACT="build-checks"
CHECK_WORKFLOW="sandbox-check.yml"
PUSH_ATTEMPTS=5
RETRY_SECONDS=5
PAGES_WORKFLOW="pages.yml"
REPORT_ARTIFACT="tier-report"
# The Pages runs `plan` searches for a tier report. Quick deploys hold none and pass between
# weekly scans, so this covers many pushes.
REPORT_RUNS=200
# The most files the compare API lists. A push this large gives `new`.
COMPARE_LIMIT=300
# The paths that change the set of projects or a threshold: new entries need a fetch.
DATA_PATHS='^data/'
# The paths that change how a tier comes from stored values: no fetch needed.
SCORING_PATHS='^(crates/catalogue-core/|crates/catalogue-signals/|crates/catalogue/src/tiers/)'
SNAPSHOT_FILE='^[0-9]{4}-[0-9]{2}-[0-9]{2}\.jsonl\.gz$'
README_STORE="readmes.jsonl.gz"
# Holds the date of the newest `changed` or `all` run.
LAST_SCAN_FILE="last-scan.txt"
# The weekly scan plus one day, so a scan GitHub starts late never counts as lost.
SCAN_MAX_AGE_DAYS=8
NO_COMMIT="0000000000000000000000000000000000000000"

usage() {
    echo "usage: pages_signals.sh prepare <history-dir> | tiers <history-dir> <report-file> | commit <history-dir> | plan <output-file> | report <dir> <run-id>" >&2
    exit 2
}

# The newest successful Pages run on main that still holds a tier report. Empty when none does.
newest_report_run() {
    local run count
    while read -r run; do
        count="$(gh api "repos/${GITHUB_REPOSITORY}/actions/runs/${run}/artifacts" \
            --jq "[.artifacts[] | select(.name == \"${REPORT_ARTIFACT}\" and .expired == false)] | length")"
        if [[ "$count" != "0" ]]; then
            echo "$run"
            return
        fi
    done < <(gh run list --workflow "$PAGES_WORKFLOW" --branch main --status success \
        --limit "$REPORT_RUNS" --json databaseId --jq '.[].databaseId')
}

# The mode of this push. A change it cannot list in full gives `new`, which fetches little and
# applies the rules to every entry.
push_mode() {
    local paths
    if [[ -z "${PUSH_BEFORE:-}" || "$PUSH_BEFORE" == "$NO_COMMIT" ]]; then
        echo "new"
        return
    fi
    if ! paths="$(gh api "repos/${GITHUB_REPOSITORY}/compare/${PUSH_BEFORE}...${GITHUB_SHA}" \
        --jq '.files[].filename')"; then
        echo "::notice::the compare of ${PUSH_BEFORE} failed: this run uses mode new" >&2
        echo "new"
        return
    fi
    if [[ -z "$paths" ]] || (( $(wc -l <<< "$paths") >= COMPARE_LIMIT )); then
        echo "new"
    elif grep --quiet --extended-regexp "$DATA_PATHS" <<< "$paths"; then
        echo "new"
    elif grep --quiet --extended-regexp "$SCORING_PATHS" <<< "$paths"; then
        echo "none"
    else
        echo "skip"
    fi
}

# Whether the `signals` branch holds a snapshot and the README store. A missing branch is no
# history. Any other error stops the job, so a network error never starts a full scan.
has_history() {
    local paths errors status=0
    errors="$(mktemp)"
    paths="$(gh api "repos/${GITHUB_REPOSITORY}/git/trees/${BRANCH}" --jq '.tree[].path' 2> "$errors")" || status=$?
    if (( status != 0 )); then
        if grep --quiet 'HTTP 404' "$errors"; then
            return 1
        fi
        echo "::error::reading the tree of ${BRANCH} failed: $(cat "$errors"). Rerun the job." >&2
        exit 1
    fi
    grep --quiet --extended-regexp "$SNAPSHOT_FILE" <<< "$paths" \
        && grep --quiet --fixed-strings --line-regexp "$README_STORE" <<< "$paths"
}

# Whether the newest real check on the `signals` branch is missing or older than
# SCAN_MAX_AGE_DAYS. Any error other than a missing file stops the job.
scan_is_stale() {
    local text errors status=0 scanned now
    errors="$(mktemp)"
    text="$(gh api "repos/${GITHUB_REPOSITORY}/contents/${LAST_SCAN_FILE}?ref=${BRANCH}" \
        -H "Accept: application/vnd.github.raw" 2> "$errors")" || status=$?
    if (( status != 0 )); then
        if grep --quiet 'HTTP 404' "$errors"; then
            return 0
        fi
        echo "::error::reading ${LAST_SCAN_FILE} on ${BRANCH} failed: $(cat "$errors"). Rerun the job." >&2
        exit 1
    fi
    if ! scanned="$(date -u -d "$text" +%s 2> /dev/null)"; then
        return 0
    fi
    now="$(date -u -d "$(date -u +%F)" +%s)"
    (( (now - scanned) / 86400 > SCAN_MAX_AGE_DAYS ))
}

plan() {
    local output="$1"
    local mode run=""
    case "${GITHUB_EVENT_NAME:-}" in
        push)
            mode="$(push_mode)"
            ;;
        workflow_dispatch)
            mode="${MODE_INPUT:-skip}"
            ;;
        *)
            mode="changed"
            ;;
    esac
    if [[ "$mode" == "skip" || "$mode" == "new" || "$mode" == "none" ]] && scan_is_stale; then
        echo "::notice::the last scan is missing or over ${SCAN_MAX_AGE_DAYS} days old: this run uses mode changed"
        mode="changed"
    fi
    case "$mode" in
        skip)
            run="$(newest_report_run)"
            if [[ -z "$run" ]]; then
                echo "::notice::no earlier tier report remains: this run uses mode all"
                mode="all"
            fi
            ;;
        new | none | changed)
            if ! has_history; then
                echo "::notice::the ${BRANCH} branch holds no usable history: this run uses mode all"
                mode="all"
            fi
            ;;
        all)
            ;;
        *)
            echo "::error::unknown mode ${mode}. Use skip, new, none, changed or all." >&2
            exit 1
            ;;
    esac
    echo "Mode: ${mode}. Report run: ${run:-this run}."
    {
        echo "mode=${mode}"
        echo "report_run=${run}"
    } >> "$output"
}

# Runs git with a credential helper that reads GITHUB_TOKEN when git asks.
git_remote() {
    # shellcheck disable=SC2016 # The shell git starts expands these, not this one.
    git -c credential.helper= \
        -c credential.helper='!f() { test "$1" = get || return 0; printf "username=x-access-token\npassword=%s\n" "$GITHUB_TOKEN"; }; f' \
        "$@"
}

prepare() {
    local dir="$1"
    local url="${GITHUB_SERVER_URL}/${GITHUB_REPOSITORY}.git"
    local status=0
    git_remote ls-remote --exit-code --heads "$url" "refs/heads/${BRANCH}" > /dev/null || status=$?
    case "$status" in
        0)
            git_remote clone --quiet --depth 1 --branch "$BRANCH" --single-branch "$url" "$dir"
            ;;
        2)
            echo "Branch ${BRANCH} is absent: this run creates it"
            git init --quiet --initial-branch="$BRANCH" "$dir"
            git -C "$dir" remote add origin "$url"
            ;;
        *)
            echo "::error::git ls-remote on ${url} exited ${status}. Rerun the job once the remote answers."
            exit 1
            ;;
    esac
}

# Writes the newest build-checks artifact into <dir>. Absent writes nothing.
download_build_checks() {
    local dir="$1"
    local run_id count
    run_id="$(gh run list --workflow "$CHECK_WORKFLOW" --branch main --status success --limit 1 \
        --json databaseId --jq '.[0].databaseId // empty')"
    if [[ -z "$run_id" ]]; then
        echo "::notice::no successful ${CHECK_WORKFLOW} run: rot signal R6 reads as unknown"
        return
    fi
    count="$(gh api "repos/${GITHUB_REPOSITORY}/actions/runs/${run_id}/artifacts" \
        --jq "[.artifacts[] | select(.name == \"${ARTIFACT}\" and .expired == false)] | length")"
    if [[ "$count" == "0" ]]; then
        echo "::notice::run ${run_id} of ${CHECK_WORKFLOW} holds no ${ARTIFACT} artifact: rot signal R6 reads as unknown"
        return
    fi
    gh run download "$run_id" --name "$ARTIFACT" --dir "$dir"
}

tiers() {
    local history="$1"
    local report="$2"
    local cache="${RUNNER_TEMP}/catalogue-cache"
    local checks_dir="${RUNNER_TEMP}/${ARTIFACT}"
    local checks="${checks_dir}/${ARTIFACT}.jsonl"

    cargo build --release --locked -p catalogue
    "$BIN" dump fetch --cache-dir "$cache"
    "$BIN" rustsec fetch --cache-dir "$cache"

    export GH_REPO="$GITHUB_REPOSITORY"
    mkdir -p "$checks_dir"
    download_build_checks "$checks_dir"

    local args=(--data data --cache-dir "$cache" --history "$history" --json "$report" --fetch "${FETCH_MODE:-all}")
    if [[ -f "$checks" ]]; then
        args+=(--build-checks "$checks")
    fi
    local started
    started="$(date -u +%F)"
    "$BIN" tiers "${args[@]}"
    if [[ "${FETCH_MODE:-all}" == "changed" || "${FETCH_MODE:-all}" == "all" ]]; then
        echo "$started" > "${history}/${LAST_SCAN_FILE}"
    fi
}

stage() {
    git -C "$1" add -- '*.jsonl.gz'
    if [[ -f "$1/${LAST_SCAN_FILE}" ]]; then
        git -C "$1" add -- "$LAST_SCAN_FILE"
    fi
}

commit_staged() {
    git -C "$1" -c user.name="$BOT_NAME" -c user.email="$BOT_EMAIL" \
        commit --quiet --message "Add signal snapshot"
}

commit_snapshot() {
    local dir="$1"
    local saved attempt=1
    local files=()

    stage "$dir"
    mapfile -t files < <(git -C "$dir" diff --cached --name-only)
    if (( ${#files[@]} == 0 )); then
        echo "Snapshot unchanged: nothing to push"
        return
    fi
    echo "Snapshot changed: ${files[*]}"

    saved="$(mktemp -d -p "$RUNNER_TEMP")"
    local file
    for file in "${files[@]}"; do
        cp -- "${dir}/${file}" "${saved}/"
    done
    commit_staged "$dir"

    until git_remote -C "$dir" push --quiet origin "HEAD:refs/heads/${BRANCH}"; do
        if (( attempt >= PUSH_ATTEMPTS )); then
            echo "::error::push to ${BRANCH} failed ${PUSH_ATTEMPTS} times. Rerun the job."
            exit 1
        fi
        sleep $(( attempt * RETRY_SECONDS ))
        attempt=$(( attempt + 1 ))
        if ! git_remote -C "$dir" fetch --quiet --depth 1 origin "$BRANCH"; then
            echo "::warning::fetch of ${BRANCH} failed: pushing again without it"
            continue
        fi
        git -C "$dir" reset --quiet --hard FETCH_HEAD
        cp -- "$saved"/* "${dir}/"
        stage "$dir"
        if git -C "$dir" diff --cached --quiet; then
            echo "The remote branch holds the same snapshot: nothing to push"
            return
        fi
        commit_staged "$dir"
    done
}

ACTION="${1:-}"
DIR="${2:-}"
if [[ -z "$DIR" ]]; then
    usage
fi
if [[ "$ACTION" == "tiers" && -z "${GITHUB_TOKEN:-}" ]]; then
    echo "::error::the SIGNALS_TOKEN secret is empty. Set it to a fine-grained personal access token with public repositories read-only, then rerun the job."
    exit 1
fi
: "${GITHUB_TOKEN:?GITHUB_TOKEN is unset: pass it to this step through env}"

case "$ACTION" in
    prepare)
        : "${GITHUB_SERVER_URL:?GITHUB_SERVER_URL is unset: run this script in GitHub Actions}"
        : "${GITHUB_REPOSITORY:?GITHUB_REPOSITORY is unset: run this script in GitHub Actions}"
        prepare "$DIR"
        ;;
    tiers)
        if [[ -z "${3:-}" ]]; then
            usage
        fi
        : "${RUNNER_TEMP:?RUNNER_TEMP is unset: run this script in GitHub Actions}"
        : "${GITHUB_REPOSITORY:?GITHUB_REPOSITORY is unset: run this script in GitHub Actions}"
        tiers "$DIR" "$3"
        ;;
    commit)
        : "${RUNNER_TEMP:?RUNNER_TEMP is unset: run this script in GitHub Actions}"
        commit_snapshot "$DIR"
        ;;
    plan)
        : "${GITHUB_REPOSITORY:?GITHUB_REPOSITORY is unset: run this script in GitHub Actions}"
        : "${GITHUB_SHA:?GITHUB_SHA is unset: run this script in GitHub Actions}"
        export GH_REPO="$GITHUB_REPOSITORY"
        plan "$DIR"
        ;;
    report)
        if [[ -z "${3:-}" ]]; then
            usage
        fi
        : "${GITHUB_REPOSITORY:?GITHUB_REPOSITORY is unset: run this script in GitHub Actions}"
        export GH_REPO="$GITHUB_REPOSITORY"
        gh run download "$3" --name "$REPORT_ARTIFACT" --dir "$DIR"
        ;;
    *)
        usage
        ;;
esac
