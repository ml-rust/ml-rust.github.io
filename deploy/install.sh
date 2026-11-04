#!/usr/bin/env bash
# Install the catalogue bot deploy on a VM.
#
#   sudo bash deploy/install.sh        # run as root from a checkout
#
# Creates the two system users and the directories, installs the update script
# and the four units, and enables each timer once its inputs exist. The script
# is idempotent. It never overwrites a config or a secret.

set -euo pipefail

SRC="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BOT_USER="catalogue-bot"
UPDATE_USER="catalogue-update"
OPT="/opt/catalogue"
ETC="/etc/catalogue-bot"
UNIT_DIR="/etc/systemd/system"
UNITS=(catalogue-bot.service catalogue-bot.timer catalogue-update.service catalogue-update.timer)

if [[ "$(id -u)" -ne 0 ]]; then
    echo "install.sh: must run as root, but the user id is $(id -u). Rerun with: sudo bash $0" >&2
    exit 1
fi

for file in update.sh "${UNITS[@]}"; do
    if [[ ! -f "$SRC/$file" ]]; then
        echo "install.sh: $SRC/$file is missing. Run this script from a full checkout" >&2
        exit 1
    fi
done

for user in "$BOT_USER" "$UPDATE_USER"; do
    if ! id -u "$user" > /dev/null 2>&1; then
        useradd --system --no-create-home --shell /usr/sbin/nologin "$user"
        echo "Created user $user."
    fi
done

install -d -m 0755 -o "$UPDATE_USER" -g "$UPDATE_USER" "$OPT"
install -d -m 0755 -o root -g root "$ETC"
install -d -m 0755 -o root -g root /usr/local/libexec
install -m 0755 -o root -g root "$SRC/update.sh" /usr/local/libexec/catalogue-update
for unit in "${UNITS[@]}"; do
    install -m 0644 -o root -g root "$SRC/$unit" "$UNIT_DIR/$unit"
done
systemctl daemon-reload

MISSING=()

if ! command -v gh > /dev/null 2>&1; then
    MISSING+=("gh: the update script needs the GitHub CLI on the system PATH. Install it from https://cli.github.com")
fi

if [[ -f "$ETC/update.env" ]]; then
    systemctl enable --now catalogue-update.timer
else
    MISSING+=("$ETC/update.env: read-only GitHub token for the update check. Run: install -m 0600 -o root -g root /dev/null $ETC/update.env, then add the line GH_TOKEN=<token>")
fi

BOT_READY=1
if [[ ! -f "$ETC/bot.toml" ]]; then
    BOT_READY=0
    MISSING+=("$ETC/bot.toml: bot config. Run: install -m 0644 -o root -g root bot.example.toml $ETC/bot.toml, then edit it. Set private_key_path = \"$ETC/app.pem\"")
fi
if [[ ! -f "$ETC/app.pem" ]]; then
    BOT_READY=0
    MISSING+=("$ETC/app.pem: GitHub App private key. Run: install -m 0400 -o $BOT_USER -g $BOT_USER <key.pem> $ETC/app.pem")
elif [[ "$(stat -c '%U %a' "$ETC/app.pem")" != "$BOT_USER 400" ]]; then
    BOT_READY=0
    MISSING+=("$ETC/app.pem has owner and mode '$(stat -c '%U %a' "$ETC/app.pem")', expected '$BOT_USER 400'. Run: chown $BOT_USER:$BOT_USER $ETC/app.pem && chmod 0400 $ETC/app.pem")
fi
if [[ "$BOT_READY" -eq 1 ]]; then
    systemctl enable --now catalogue-bot.timer
fi

if [[ ! -L "$OPT/current" ]]; then
    MISSING+=("$OPT/current: no release is installed. The bot pass fails until one is. Run: systemctl start catalogue-update.service")
fi

if [[ ! -f "$ETC/env" ]]; then
    echo "Optional: $ETC/env holds model keys for triage, for example OLLAMA_API_KEY=<key>. Create it with mode 0600, owner root."
fi

if [[ "${#MISSING[@]}" -eq 0 ]]; then
    echo "Install complete. Both timers are enabled."
else
    echo "Install incomplete. Still missing:"
    for item in "${MISSING[@]}"; do
        echo "  - $item"
    done
    echo "Rerun this script after each fix."
fi
