#!/usr/bin/env bash
# flasher updater — install a prebuilt artifact, restart, health-gate.
# Installed at /opt/flasher/update.sh (see deploy/DEPLOY.md).
#
# NORMAL DEPLOYMENT — the build happens on a developer machine
# (deploy/build-artifact.sh --ship); this host needs no Rust/Trunk toolchain:
#
#   sudo /opt/flasher/update.sh --artifact /tmp/flasher-artifact-<rev>.tar.gz
#
# Artifact contents:
#   MANIFEST               app=flasher, rev=<git rev>, built=<date>, builder=<host>
#   bin/flasher            the server binary
#   bin/flasher-migrate    the migration binary
#   dist/                  the built Leptos frontend
#
# SOURCE DEPLOYMENTS — supported for a rebuild in place, but they need a
# toolchain on this host and are not how production is deployed:
#   update.sh              git pull --ff-only from origin, then build
set -euo pipefail

SRC=/opt/flasher/src
APP=/opt/flasher

ARTIFACT=
if [[ ${1:-} == --artifact ]]; then
    ARTIFACT=${2:-}
    if [[ -z $ARTIFACT || ! -f $ARTIFACT ]]; then
        echo "usage: $0 --artifact <flasher-artifact.tar.gz>" >&2
        exit 2
    fi
    shift 2
fi

revision=unknown
src_bin= src_migrate= src_dist=
work=
cleanup() { [[ -n $work ]] && rm -rf "$work"; }
trap cleanup EXIT

if [[ -n $ARTIFACT ]]; then
    work=$(mktemp -d)
    if tar -tzf "$ARTIFACT" | grep -Eq '(^/|(^|/)\.\.(/|$))'; then
        echo "ERROR: unsafe path in $ARTIFACT" >&2
        exit 1
    fi
    tar -xzf "$ARTIFACT" -C "$work"
    if [[ ! -f $work/MANIFEST || ! -x $work/bin/flasher || ! -x $work/bin/flasher-migrate || ! -d $work/dist ]]; then
        echo "ERROR: $ARTIFACT is not a flasher artifact (need MANIFEST, bin/flasher, bin/flasher-migrate, dist/)" >&2
        exit 1
    fi
    revision=$(sed -n 's/^rev=//p' "$work/MANIFEST" | head -1)
    revision=${revision:-unknown}
    src_bin=$work/bin/flasher
    src_migrate=$work/bin/flasher-migrate
    src_dist=$work/dist
    echo "artifact $ARTIFACT (rev $revision)"
else
    if ! command -v cargo >/dev/null 2>&1; then
        echo "ERROR: no Rust toolchain on this host." >&2
        echo "       Production deploys install a prebuilt artifact instead:" >&2
        echo "         deploy/build-artifact.sh --ship    (on a developer machine)" >&2
        exit 1
    fi
    cd "$SRC"
    # The checkout may be owned by a different user than the one running this
    # script (e.g. cloned by root at install time). Accept this exact path.
    git config --global --add safe.directory "$SRC" 2>/dev/null || true
    old=$(git rev-parse --short HEAD)
    self_before=$(sha256sum "$0" | cut -d' ' -f1)
    git pull --ff-only
    revision=$(git rev-parse --short HEAD)
    echo "updating $old -> $revision"
    # If the pull replaced this script, restart into the new version — bash
    # would otherwise keep executing the old content at a stale byte offset.
    self_after=$(sha256sum "$0" | cut -d' ' -f1)
    if [[ $self_before != "$self_after" ]]; then
        echo "update.sh changed; restarting with the new version"
        exec "$0" "$@"
    fi
    cargo build --release -p flasher-server -p flasher-migrate
    (cd frontends/leptos && env -u NO_COLOR trunk build --release)
    src_bin=$SRC/target/release/flasher
    src_migrate=$SRC/target/release/flasher-migrate
    src_dist=$SRC/frontends/leptos/dist
fi

# Stage everything, keep the current release for rollback, then swap.
install -m755 "$src_bin" "$APP/.flasher.new"
install -m755 "$src_migrate" "$APP/.flasher-migrate.new"
rm -rf "$APP/.dist.new"
rsync -a --chmod=D755,F644 "$src_dist/" "$APP/.dist.new/"

rm -rf "$APP/.prev"
install -d "$APP/.prev"
for f in flasher flasher-migrate; do
    [[ -f $APP/$f ]] && cp -a "$APP/$f" "$APP/.prev/$f"
done
[[ -d $APP/dist ]] && cp -a "$APP/dist" "$APP/.prev/dist"

mv -f "$APP/.flasher.new" "$APP/flasher"
mv -f "$APP/.flasher-migrate.new" "$APP/flasher-migrate"
rm -rf "$APP/dist.old"
if [[ -d $APP/dist ]]; then mv "$APP/dist" "$APP/dist.old"; fi
mv "$APP/.dist.new" "$APP/dist"

systemctl restart flasher

# Health gate: on failure restore the previous release and restart it.
for _ in $(seq 1 30); do
    if curl -fsS http://127.0.0.1:3000/api/health > /dev/null 2>&1; then
        rm -rf "$APP/dist.old"
        echo "flasher $revision is up"
        exit 0
    fi
    sleep 1
done

echo "ERROR: flasher did not become healthy after restart; rolling back" >&2
journalctl -u flasher -n 30 --no-pager >&2 || true
for f in flasher flasher-migrate; do
    [[ -f $APP/.prev/$f ]] && cp -a "$APP/.prev/$f" "$APP/$f"
done
if [[ -d $APP/.prev/dist ]]; then
    rm -rf "$APP/dist"
    cp -a "$APP/.prev/dist" "$APP/dist"
fi
systemctl restart flasher || true
exit 1
