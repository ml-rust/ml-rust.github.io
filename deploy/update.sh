#!/usr/bin/env bash
# Install the latest catalogue release.
#
#   /usr/local/libexec/catalogue-update
#
# Runs as the catalogue-update user from catalogue-update.service. It fetches
# the latest full release, checks the checksum and the build attestation, and
# only then extracts the tarball. A release installs to
# $CATALOGUE_PREFIX/versions/<version>. The `current` symlink moves to it in
# one atomic rename. Any error leaves `current` as it was.
#
# Environment:
#   CATALOGUE_PREFIX   install root, default /opt/catalogue
#   GH_TOKEN           read-only token. `gh attestation verify` needs auth.

set -euo pipefail

# The bot runs as another user, so every installed file must be world-readable.
umask 022

REPO="ml-rust/ml-rust.github.io"
PREFIX="${CATALOGUE_PREFIX:-/opt/catalogue}"
KEEP=3

die() {
    echo "catalogue-update: $*" >&2
    exit 1
}

TMP=""
cleanup() {
    if [[ -n "$TMP" ]]; then
        rm -rf -- "$TMP"
    fi
}
trap cleanup EXIT

TAG=$(gh release view --repo "$REPO" --json tagName --jq .tagName) \
    || die "cannot read the latest release of $REPO. Check the network and GH_TOKEN, then rerun"

if [[ ! "$TAG" =~ ^v[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
    die "the latest release tag '$TAG' is not vMAJOR.MINOR.PATCH. Fix the release on $REPO and rerun"
fi
VERSION="${TAG#v}"

CURRENT=""
if [[ -L "$PREFIX/current" ]]; then
    CURRENT=$(basename "$(readlink "$PREFIX/current")")
fi

if [[ "$CURRENT" == "$VERSION" ]]; then
    echo "catalogue $VERSION is already installed."
    exit 0
fi
if [[ -n "$CURRENT" ]]; then
    OLDEST=$(printf '%s\n%s\n' "$VERSION" "$CURRENT" | sort -V | head -n 1)
    if [[ "$OLDEST" == "$VERSION" ]]; then
        die "the latest release $VERSION is older than the installed $CURRENT. Nothing changed. Remove $PREFIX/current to force a downgrade"
    fi
fi

case "$(uname -m)" in
    x86_64) TARGET="x86_64-unknown-linux-musl" ;;
    aarch64) TARGET="aarch64-unknown-linux-musl" ;;
    *) die "no release build for machine type '$(uname -m)'. Supported: x86_64, aarch64" ;;
esac

NAME="catalogue-${VERSION}-${TARGET}"
TARBALL="${NAME}.tar.gz"

mkdir -p -- "$PREFIX/versions"
TMP=$(mktemp -d "$PREFIX/.update.XXXXXX")

gh release download "$TAG" --repo "$REPO" --dir "$TMP" \
    --pattern "$TARBALL" --pattern "${TARBALL}.sha256" \
    || die "cannot download $TARBALL for $TAG. Check the release assets and rerun"
[[ -f "$TMP/$TARBALL" && -f "$TMP/${TARBALL}.sha256" ]] \
    || die "the release $TAG lacks $TARBALL or ${TARBALL}.sha256. Rerun the release workflow"

(cd "$TMP" && sha256sum -c "${TARBALL}.sha256") \
    || die "the checksum of $TARBALL does not match ${TARBALL}.sha256. Nothing installed"

gh attestation verify "$TMP/$TARBALL" --repo "$REPO" \
    --signer-workflow "$REPO/.github/workflows/release.yml" \
    --deny-self-hosted-runners \
    || die "the attestation of $TARBALL is not valid. Nothing installed"

EXTRACT="$TMP/extract"
mkdir -p -- "$EXTRACT"
tar --no-same-owner -xzf "$TMP/$TARBALL" -C "$EXTRACT" \
    || die "cannot extract $TARBALL. Nothing installed"
SRC="$EXTRACT/$NAME"

[[ -x "$SRC/catalogue" ]] \
    || die "$TARBALL holds no executable $NAME/catalogue. Nothing installed"
REPORTED=$("$SRC/catalogue" --version) \
    || die "'$SRC/catalogue --version' exited with an error. Nothing installed"
if [[ "${REPORTED##* }" != "$VERSION" ]]; then
    die "the binary in $TARBALL reports '$REPORTED', expected version $VERSION. Nothing installed"
fi
[[ -f "$SRC/labels.toml" ]] \
    || die "$TARBALL holds no $NAME/labels.toml. Nothing installed"

chmod -R u=rwX,go=rX -- "$SRC"

DEST="$PREFIX/versions/$VERSION"
rm -rf -- "$DEST"
mv -- "$SRC" "$DEST"

ln -s "versions/$VERSION" "$TMP/current"
mv -T -- "$TMP/current" "$PREFIX/current"

# Newest first. Skip the first KEEP, and never remove the current version.
mapfile -t OLD < <(
    find "$PREFIX/versions" -mindepth 1 -maxdepth 1 -type d -printf '%f\n' \
        | sort -rV | tail -n +$((KEEP + 1))
)
for old in "${OLD[@]}"; do
    if [[ "$old" != "$VERSION" ]]; then
        rm -rf -- "${PREFIX:?}/versions/$old"
    fi
done

echo "Installed catalogue $VERSION."
