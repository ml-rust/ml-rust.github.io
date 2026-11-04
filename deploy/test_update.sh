#!/usr/bin/env bash
# Test update.sh with a stub `gh` and a temporary prefix.
#
#   bash deploy/test_update.sh
#
# Each case builds real tarballs that hold a stub `catalogue` script, runs
# update.sh, and asserts the exit code and the target of `current`. Every case
# also asserts that no temporary directory is left in the prefix. The script
# exits nonzero when any case fails and prints the name of each failed case.

set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
UPDATE="$HERE/update.sh"

WORK="$(mktemp -d)"
trap 'rm -rf -- "$WORK"' EXIT

case "$(uname -m)" in
    x86_64) TARGET="x86_64-unknown-linux-musl" ;;
    aarch64) TARGET="aarch64-unknown-linux-musl" ;;
    *)
        echo "test_update.sh: unsupported machine type '$(uname -m)'. Run on x86_64 or aarch64" >&2
        exit 1
        ;;
esac

BIN="$WORK/bin"
mkdir -p "$BIN"

# The stub gh reads its behaviour from the STUB_* variables.
cat > "$BIN/gh" <<'STUB'
#!/usr/bin/env bash
set -euo pipefail
case "$1 $2" in
    "release view")
        echo "$STUB_TAG"
        ;;
    "release download")
        if [[ -n "${STUB_FAIL_DOWNLOAD:-}" ]]; then
            echo "stub gh: download refused" >&2
            exit 1
        fi
        dir=""
        patterns=()
        shift 3
        while [[ $# -gt 0 ]]; do
            case "$1" in
                --dir) dir="$2"; shift 2 ;;
                --pattern) patterns+=("$2"); shift 2 ;;
                *) shift ;;
            esac
        done
        for p in "${patterns[@]}"; do
            cp "$STUB_ASSETS/$p" "$dir/$p"
        done
        ;;
    "attestation verify")
        echo "verify $*" >> "$STUB_LOG"
        if [[ -n "${STUB_FAIL_ATTEST:-}" ]]; then
            echo "stub gh: attestation refused" >&2
            exit 1
        fi
        ;;
    *)
        echo "stub gh: unexpected call: $*" >&2
        exit 2
        ;;
esac
STUB
chmod +x "$BIN/gh"

# The stub tar records each call, then runs the real tar.
REAL_TAR="$(command -v tar)"
cat > "$BIN/tar" <<STUB
#!/usr/bin/env bash
echo "tar \$*" >> "\$STUB_LOG"
exec "$REAL_TAR" "\$@"
STUB
chmod +x "$BIN/tar"

export PATH="$BIN:$PATH"
export STUB_LOG="$WORK/log"

FAILED=0
CASE=""
PREFIX=""
ASSETS=""
CODE=0

fail() {
    echo "FAIL: $CASE: $*"
    FAILED=1
}

# begin <name>: a fresh prefix and asset directory.
begin() {
    CASE="$1"
    PREFIX="$WORK/prefix"
    ASSETS="$WORK/assets"
    rm -rf -- "$PREFIX" "$ASSETS"
    mkdir -p "$PREFIX" "$ASSETS"
    : > "$STUB_LOG"
    unset STUB_FAIL_DOWNLOAD STUB_FAIL_ATTEST
    export STUB_ASSETS="$ASSETS"
}

# make_release <version> [reported-version] [no-labels]
make_release() {
    local version="$1" reported="${2:-$1}" labels="${3:-}"
    local name="catalogue-${version}-${TARGET}"
    local stage="$WORK/stage"
    rm -rf -- "$stage"
    mkdir -p "$stage/$name"
    printf '#!/usr/bin/env bash\necho "catalogue %s"\n' "$reported" > "$stage/$name/catalogue"
    chmod +x "$stage/$name/catalogue"
    if [[ -z "$labels" ]]; then
        echo '# labels' > "$stage/$name/labels.toml"
    fi
    "$REAL_TAR" -czf "$ASSETS/$name.tar.gz" -C "$stage" "$name"
    (cd "$ASSETS" && sha256sum "$name.tar.gz" > "$name.tar.gz.sha256")
}

# install_version <version>: put a version in the prefix as if installed.
install_version() {
    mkdir -p "$PREFIX/versions/$1"
    ln -sfn "versions/$1" "$PREFIX/current"
}

# run_update <tag>: runs update.sh and sets CODE.
run_update() {
    export STUB_TAG="$1"
    CODE=0
    CATALOGUE_PREFIX="$PREFIX" bash "$UPDATE" > "$WORK/out" 2>&1 || CODE=$?
}

expect_code() {
    if [[ "$CODE" -ne "$1" ]]; then
        fail "exit code $CODE, expected $1. Output: $(cat "$WORK/out")"
    fi
}

# expect_current <version|none>
expect_current() {
    local actual="none"
    if [[ -L "$PREFIX/current" ]]; then
        actual="$(basename "$(readlink "$PREFIX/current")")"
    fi
    if [[ "$actual" != "$1" ]]; then
        fail "current is '$actual', expected '$1'"
    fi
}

expect_no_temp() {
    local left
    left="$(find "$PREFIX" -maxdepth 1 -name '.update.*' | head -n 1)"
    if [[ -n "$left" ]]; then
        fail "temporary directory left behind: $left"
    fi
}

expect_log_empty() {
    if [[ -s "$STUB_LOG" ]] && grep -q '^tar ' "$STUB_LOG"; then
        fail "tar ran, but must not: $(cat "$STUB_LOG")"
    fi
}

end() {
    expect_no_temp
}

begin "fresh install"
make_release 1.2.3
run_update v1.2.3
expect_code 0
expect_current 1.2.3
[[ -x "$PREFIX/versions/1.2.3/catalogue" ]] || fail "binary not installed"
[[ -f "$PREFIX/versions/1.2.3/labels.toml" ]] || fail "labels.toml not installed"
grep -q -- '--signer-workflow ml-rust/ml-rust.github.io/.github/workflows/release.yml' "$STUB_LOG" \
    || fail "attestation call lacks --signer-workflow"
grep -q -- '--deny-self-hosted-runners' "$STUB_LOG" || fail "attestation call lacks --deny-self-hosted-runners"
end

begin "same version is a no-op"
make_release 1.2.3
install_version 1.2.3
run_update v1.2.3
expect_code 0
expect_current 1.2.3
[[ ! -s "$STUB_LOG" ]] || fail "update ran work for the installed version"
end

begin "newer version swaps"
make_release 1.3.0
install_version 1.2.3
run_update v1.3.0
expect_code 0
expect_current 1.3.0
[[ -d "$PREFIX/versions/1.2.3" ]] || fail "previous version removed"
end

begin "lower version refused"
make_release 1.2.3
install_version 1.10.0
run_update v1.2.3
expect_code 1
expect_current 1.10.0
end

begin "malformed tag refused"
make_release 1.2.3
run_update v1.2.3-beta.1
expect_code 1
expect_current none
grep -q 'v1.2.3-beta.1' "$WORK/out" || fail "error does not name the tag"
end

begin "checksum mismatch installs nothing"
make_release 1.2.3
echo "0000000000000000000000000000000000000000000000000000000000000000  catalogue-1.2.3-${TARGET}.tar.gz" \
    > "$ASSETS/catalogue-1.2.3-${TARGET}.tar.gz.sha256"
run_update v1.2.3
expect_code 1
expect_current none
expect_log_empty
end

begin "attestation failure installs nothing and never extracts"
make_release 1.2.3
export STUB_FAIL_ATTEST=1
run_update v1.2.3
expect_code 1
expect_current none
expect_log_empty
[[ ! -d "$PREFIX/versions/1.2.3" ]] || fail "version directory created"
end

begin "wrong reported version installs nothing"
make_release 1.2.3 1.2.4
run_update v1.2.3
expect_code 1
expect_current none
[[ ! -d "$PREFIX/versions/1.2.3" ]] || fail "version directory created"
end

begin "missing labels.toml installs nothing"
make_release 1.2.3 1.2.3 no-labels
run_update v1.2.3
expect_code 1
expect_current none
[[ ! -d "$PREFIX/versions/1.2.3" ]] || fail "version directory created"
end

begin "pruning keeps three"
make_release 1.6.0
for v in 1.1.0 1.2.0 1.3.0 1.4.0 1.5.0; do
    mkdir -p "$PREFIX/versions/$v"
done
ln -sfn versions/1.5.0 "$PREFIX/current"
run_update v1.6.0
expect_code 0
expect_current 1.6.0
kept="$(find "$PREFIX/versions" -mindepth 1 -maxdepth 1 -printf '%f\n' | sort -V | tr '\n' ' ')"
[[ "$kept" == "1.4.0 1.5.0 1.6.0 " ]] || fail "kept versions: $kept"
end

begin "pruning never removes current"
make_release 1.0.5
for v in 1.0.0 1.0.1 1.0.2 1.0.3; do
    mkdir -p "$PREFIX/versions/$v"
done
ln -sfn versions/1.0.0 "$PREFIX/current"
run_update v1.0.5
expect_code 0
expect_current 1.0.5
end

begin "failed asset download installs nothing"
make_release 1.2.3
export STUB_FAIL_DOWNLOAD=1
run_update v1.2.3
expect_code 1
expect_current none
end

begin "a strict umask still leaves the release readable by the bot user"
make_release 1.2.3
saved_umask="$(umask)"
umask 077
run_update v1.2.3
umask "$saved_umask"
expect_code 0
expect_current 1.2.3
for path in versions versions/1.2.3 versions/1.2.3/catalogue; do
    mode="$(stat -c '%a' "$PREFIX/$path")"
    [[ "$mode" == "755" ]] || fail "$path has mode $mode, expected 755"
done
mode="$(stat -c '%a' "$PREFIX/versions/1.2.3/labels.toml")"
[[ "$mode" == "644" ]] || fail "labels.toml has mode $mode, expected 644"
end

if [[ "$FAILED" -ne 0 ]]; then
    echo "test_update.sh: one or more cases failed" >&2
    exit 1
fi
echo "test_update.sh: all cases passed"
