#!/usr/bin/env bash
# Installs the wasm-bindgen CLI at the version Cargo.lock pins.
#
#   scripts/ci/install_wasm_bindgen.sh
#
# `build_search_wasm.sh` refuses any other version.
#
# Called by pages.yml.

set -euo pipefail

SCRIPTS="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
VERSION="$(bash "$SCRIPTS/wasm_bindgen_version.sh")"
if [[ -z "$VERSION" ]]; then
    echo "::error::Cargo.lock has no wasm-bindgen package. Run cargo build -p catalogue-search-wasm --target wasm32-unknown-unknown once to add it."
    exit 1
fi

cargo install wasm-bindgen-cli --version "$VERSION" --locked
echo "${CARGO_HOME:-$HOME/.cargo}/bin" >> "${GITHUB_PATH:?GITHUB_PATH is unset: run this script in GitHub Actions}"
