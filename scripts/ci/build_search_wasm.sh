#!/usr/bin/env bash
# Build the browser search module of the site.
#
#   scripts/ci/build_search_wasm.sh <out-dir>
#
# Builds `catalogue-search-wasm` for wasm32-unknown-unknown, then runs
# wasm-bindgen on it. Writes `oxidoc_search.js` and `oxidoc_search_bg.wasm`
# to <out-dir>. The search page imports `/search/oxidoc_search.js`, so the
# pages workflow passes `dist/search`.
#
# The installed wasm-bindgen CLI must match the `wasm-bindgen` version in
# Cargo.lock. A different version writes glue the module cannot load, so the
# script exits before the build when they differ.
#
# Honours CARGO_TARGET_DIR. Without it, the build goes to `target/` at the
# repository root.

set -euo pipefail

OUT_DIR="${1:?usage: build_search_wasm.sh <out-dir>}"
# A relative <out-dir> starts at the caller's directory, not the repository root.
if [[ "$OUT_DIR" != /* ]]; then
    OUT_DIR="$PWD/$OUT_DIR"
fi
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
TARGET="wasm32-unknown-unknown"
TARGET_DIR="${CARGO_TARGET_DIR:-$ROOT/target}"

cd "$ROOT"

LOCKED="$(bash "$ROOT/scripts/ci/wasm_bindgen_version.sh")"
if [[ -z "$LOCKED" ]]; then
    echo "::error::Cargo.lock has no wasm-bindgen package. Run cargo build -p catalogue-search-wasm --target $TARGET once to add it."
    exit 1
fi

if ! command -v wasm-bindgen > /dev/null; then
    echo "::error::wasm-bindgen is not installed. Install it: cargo install wasm-bindgen-cli --version $LOCKED --locked"
    exit 1
fi

INSTALLED="$(wasm-bindgen --version | awk '{ print $2 }')"
if [[ "$INSTALLED" != "$LOCKED" ]]; then
    echo "::error::wasm-bindgen CLI is $INSTALLED, but Cargo.lock pins wasm-bindgen $LOCKED. Install the locked version: cargo install wasm-bindgen-cli --version $LOCKED --locked"
    exit 1
fi

cargo build -p catalogue-search-wasm --target "$TARGET" --release --locked

mkdir -p "$OUT_DIR"
wasm-bindgen \
    --target web \
    --no-typescript \
    --out-name oxidoc_search \
    --out-dir "$OUT_DIR" \
    "$TARGET_DIR/$TARGET/release/catalogue_search_wasm.wasm"

echo "Wrote $OUT_DIR/oxidoc_search.js and $OUT_DIR/oxidoc_search_bg.wasm"
