#!/usr/bin/env bash
# Create or update the GitHub Release of a tag, then upload its files.
#
#   scripts/ci/publish_release.sh <tag> <title> <prerelease> <notes-file> <dist-dir>
#
# <prerelease> is `true` or `false`. An existing release is updated and files
# with the same name are replaced, so a re-run is safe. A missing or empty
# <dist-dir> uploads nothing and keeps the existing files.
#
# Needs GH_TOKEN with `contents: write`.

set -euo pipefail

TAG="${1:?usage: publish_release.sh <tag> <title> <prerelease> <notes-file> <dist-dir>}"
TITLE="${2:?missing <title>}"
PRERELEASE="${3:?missing <prerelease>}"
NOTES="${4:?missing <notes-file>}"
DIST="${5:?missing <dist-dir>}"

if [[ "$PRERELEASE" != "true" && "$PRERELEASE" != "false" ]]; then
    echo "::error::<prerelease> must be true or false, got '$PRERELEASE'."
    exit 1
fi

if gh release view "$TAG" > /dev/null 2>&1; then
    gh release edit "$TAG" --title "$TITLE" --notes-file "$NOTES" \
        --draft=false --prerelease="$PRERELEASE"
elif [[ "$PRERELEASE" == "true" ]]; then
    gh release create "$TAG" --verify-tag --title "$TITLE" --notes-file "$NOTES" --prerelease
else
    gh release create "$TAG" --verify-tag --title "$TITLE" --notes-file "$NOTES"
fi

shopt -s nullglob
FILES=("$DIST"/*)
if ((${#FILES[@]} > 0)); then
    gh release upload "$TAG" "${FILES[@]}" --clobber
fi
