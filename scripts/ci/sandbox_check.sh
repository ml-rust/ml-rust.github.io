#!/usr/bin/env bash
# Sandbox build check: `cargo check` on the latest release of dormant crates.
#
#   scripts/ci/sandbox_check.sh list  <history-dir> <list-file>
#   scripts/ci/sandbox_check.sh check <list-file>   <result-file>
#
# `list` builds the catalogue binary, fetches the crates.io dump into
# $RUNNER_TEMP, and writes one JSON line per crate to check. A missing history
# directory lists only crates that a `finished` statement names.
#
# `check` builds the catalogue binary, records `rustc --version` as the
# toolchain label, and runs `catalogue sandbox check`. That command runs
# third-party build scripts. It refuses to run unless GITHUB_ACTIONS is `true`
# and RUNNER_TEMP is set. Call it only from a job with no secrets.
#
# Called by sandbox-check.yml.

set -euo pipefail

usage() {
    echo "usage: sandbox_check.sh list <history-dir> <list-file> | check <list-file> <result-file>" >&2
    exit 2
}

ACTION="${1:-}"
INPUT="${2:-}"
OUTPUT="${3:-}"
if [[ -z "$INPUT" || -z "$OUTPUT" ]]; then
    usage
fi

RUNNER_DIR="${RUNNER_TEMP:?RUNNER_TEMP is unset: run this script in GitHub Actions}"
BIN="target/release/catalogue"

build() {
    cargo build --release --locked -p catalogue
}

case "$ACTION" in
    list)
        build
        CACHE="${RUNNER_DIR}/catalogue-cache"
        "$BIN" dump fetch --cache-dir "$CACHE"
        if [[ ! -d "$INPUT" ]]; then
            echo "::warning::no signal history at ${INPUT}: no entry reads as dormant"
        fi
        "$BIN" sandbox list --data data --cache-dir "$CACHE" --history "$INPUT" --out "$OUTPUT"
        ;;
    check)
        build
        LABEL=$(rustc --version)
        echo "Toolchain: ${LABEL}"
        "$BIN" sandbox check --list "$INPUT" --out "$OUTPUT" --toolchain-label "$LABEL"
        ;;
    *)
        usage
        ;;
esac
