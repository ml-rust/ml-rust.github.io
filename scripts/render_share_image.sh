#!/usr/bin/env bash
# Render the site share image from its SVG source.
#
#   scripts/render_share_image.sh
#
# Writes crates/catalogue-site/assets/share-image.png. Needs rsvg-convert and
# the Source Sans 3 font. rsvg-convert swaps a missing font without an error,
# so this script stops first when the font is absent.

set -euo pipefail

ASSETS="$(dirname "${BASH_SOURCE[0]}")/../crates/catalogue-site/assets"
FONT="Source Sans 3"

if ! command -v rsvg-convert > /dev/null; then
    echo "error: rsvg-convert is not installed. Install librsvg, then run this script again." >&2
    exit 1
fi

if [[ -z "$(fc-list "$FONT")" ]]; then
    echo "error: font '$FONT' is not installed. Install it, then run this script again." >&2
    exit 1
fi

rsvg-convert --width 1200 --height 630 --output "$ASSETS/share-image.png" "$ASSETS/share-image.svg"
