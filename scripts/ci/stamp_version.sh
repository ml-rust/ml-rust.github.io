#!/usr/bin/env bash
# Stamp the workspace version from a release tag.
#
#   scripts/ci/stamp_version.sh <version>       # e.g. 0.1.0, 0.1.0-beta.2
#
# Rewrites `version` in `[workspace.package]` of Cargo.toml. Both crates
# inherit it with `version.workspace = true`, so one edit sets both. Then
# refreshes the workspace entries in Cargo.lock, so the `--locked` build that
# follows accepts the new version.
#
# Only a prerelease tag changes anything. release-validate.yml has already
# proved the tag's base version matches the manifest, so for `v0.1.0` this
# is a no-op and for `v0.1.0-beta.2` it appends the suffix. Re-running a
# release stage is idempotent.

set -euo pipefail

VERSION="${1:?usage: stamp_version.sh <version>}"

CURRENT=$(cargo metadata --no-deps --format-version=1 \
    | jq -r '.packages[] | select(.name == "catalogue") | .version')

if [[ "$VERSION" == "$CURRENT" ]]; then
    echo "Version already $VERSION — nothing to stamp."
    exit 0
fi

# Only the `version = "..."` line inside [workspace.package] changes. The
# section flag keeps the edit off dependency versions further down.
STAMP_VERSION="$VERSION" perl -i -pe '
    if (/^\[/) { $in = /^\[workspace\.package\]/ ? 1 : 0 }
    if ($in && !$done && /^version = "/) {
        s/^version = ".*"/version = "$ENV{STAMP_VERSION}"/;
        $done = 1;
    }
' Cargo.toml

# `--workspace` rewrites only the workspace members' own entries. No
# registry package moves.
cargo update --workspace

echo "Stamped workspace version: $CURRENT -> $VERSION"
