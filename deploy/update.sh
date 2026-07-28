#!/usr/bin/env bash
# flasher updater — pull, rebuild, restart. No AI, no manual steps.
# Installed at /opt/flasher/update.sh (see deploy/DEPLOY.md, step A4).
set -euo pipefail

SRC=/opt/flasher/src
APP=/opt/flasher

cd "$SRC"
# The checkout may be owned by a different user than the one running this
# script (e.g. cloned by root at install time). Accept this exact path.
git config --global --add safe.directory "$SRC" 2>/dev/null || true
old=$(git rev-parse --short HEAD)
git pull --ff-only
new=$(git rev-parse --short HEAD)
echo "updating $old -> $new"

# Build everything first; only swap artifacts on success.
cargo build --release -p flasher-server -p flasher-migrate
(cd frontends/leptos && env -u NO_COLOR trunk build --release)

install -m755 target/release/flasher "$APP/flasher"
install -m755 target/release/flasher-migrate "$APP/flasher-migrate"
rsync -a --delete frontends/leptos/dist/ "$APP/dist/"

systemctl restart flasher

# Health gate: fail loudly (and leave logs) if the app does not come up.
for _ in $(seq 1 30); do
    if curl -fsS http://127.0.0.1:3000/api/health > /dev/null 2>&1; then
        echo "flasher $new is up"
        exit 0
    fi
    sleep 1
done
echo "ERROR: flasher did not become healthy after restart" >&2
journalctl -u flasher -n 30 --no-pager >&2 || true
exit 1
