#!/usr/bin/env bash
# Prints the `wasm-bindgen` version that Cargo.lock pins. Prints nothing when the
# lock file has no such package.
#
#   scripts/ci/wasm_bindgen_version.sh
#
# Called by build_search_wasm.sh and install_wasm_bindgen.sh.

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"

awk '
    $0 == "name = \"wasm-bindgen\"" { found = 1; next }
    found && /^version = / { gsub(/^version = "|"$/, ""); print; exit }
' "$ROOT/Cargo.lock"
