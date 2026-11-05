#!/usr/bin/env bash
# The signals job of the pages workflow: one snapshot per day on the orphan
# `signals` branch, and the tier report the site build reads.
#
#   scripts/ci/pages_signals.sh prepare <history-dir>
#   scripts/ci/pages_signals.sh tiers   <history-dir> <report-file>
#   scripts/ci/pages_signals.sh commit  <history-dir>
#
# `prepare` clones the branch into <history-dir>. A missing branch gives an
# empty repository, and `commit` creates the branch. Any other remote error
# stops the job, so a network error never starts a second history.
#
# `tiers` builds the catalogue binary, fetches the crates.io dump and the
# RustSec archive into $RUNNER_TEMP, downloads the newest `build-checks`
# artifact when one exists, and runs `catalogue tiers`. No `build-checks`
# artifact leaves rot signal R6 unknown. A missing dump or RustSec archive ends
# the job. A network signal that fails to fetch falls back to the previous
# snapshot instead, and `catalogue tiers` lists each fallback in its output and in
# the job summary. A rejected token still ends the job. So does a share of fetches
# with no usable earlier value above `fallback.max_failed_share`.
#
# `commit` stages the snapshot and pushes it. A run repeated on one day replaces
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

usage() {
    echo "usage: pages_signals.sh prepare <history-dir> | tiers <history-dir> <report-file> | commit <history-dir>" >&2
    exit 2
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

    local args=(--data data --cache-dir "$cache" --history "$history" --json "$report")
    if [[ -f "$checks" ]]; then
        args+=(--build-checks "$checks")
    fi
    "$BIN" tiers "${args[@]}"
}

stage() {
    git -C "$1" add -- '*.jsonl.gz'
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
    *)
        usage
        ;;
esac
