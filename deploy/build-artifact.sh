#!/usr/bin/env bash
# Build a deployable flasher artifact and (with --ship) install it on the server.
#
# The server has no Rust/Trunk toolchain by design: builds happen here, on a
# developer machine, and only the binaries + frontend are shipped.
#
# Usage (from anywhere inside the repository):
#   deploy/build-artifact.sh                 # build, leave the artifact in /tmp
#   deploy/build-artifact.sh --ship          # build, scp, run the server updater
#
# Overrides: FLASHER_HOST (default hetzner-wg), FLASHER_ARTIFACT_DIR (default /tmp).
#
# A glibc guard runs before shipping: the artifact is refused if the binaries
# need a glibc newer than the target's (development machines run a newer glibc
# than the Ubuntu server).
set -euo pipefail

root=$(CDPATH= cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
host=${FLASHER_HOST:-hetzner-wg}
out=${FLASHER_ARTIFACT_DIR:-/tmp}
ship=0

while [[ $# -gt 0 ]]; do
    case $1 in
        --ship) ship=1; shift ;;
        --host) host=${2:?--host needs a value}; shift 2 ;;
        --out) out=${2:?--out needs a value}; shift 2 ;;
        -h|--help) sed -n '2,18p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
        *) echo "unknown argument: $1" >&2; exit 2 ;;
    esac
done

# One TCP connection per host per run: ufw on the server rate-limits new SSH
# connections (`22/tcp LIMIT`), and a deploy would otherwise trip it.
SSH_OPTS=(-o BatchMode=yes -o ControlMaster=auto
          -o "ControlPath=$HOME/.ssh/cm-%r@%h-%p" -o ControlPersist=60)

cd "$root"
rev=$(git rev-parse --short HEAD)
if [[ -n $(git status --porcelain) ]]; then
    echo "WARNING: working tree is dirty; the artifact will not match $rev exactly" >&2
fi

echo "building flasher (rev $rev)"
cargo build --release -p flasher-server -p flasher-migrate
(cd frontends/leptos && env -u NO_COLOR trunk build --release)

stage=$(mktemp -d)
trap 'rm -rf "$stage"' EXIT
install -d "$stage/bin" "$stage/dist"
install -m755 target/release/flasher "$stage/bin/flasher"
install -m755 target/release/flasher-migrate "$stage/bin/flasher-migrate"
rsync -a --chmod=D755,F644 frontends/leptos/dist/ "$stage/dist/"
need=$(for b in "$stage/bin/flasher" "$stage/bin/flasher-migrate"; do
    objdump -T "$b" 2>/dev/null | sed -n 's/.*GLIBC_\([0-9.]*\).*/\1/p'
done | sort -uV | tail -1)
{
    echo "app=flasher"
    echo "rev=$rev"
    echo "built=$(date -Is)"
    echo "builder=$(hostname)"
    echo "glibc_max=$need"
} > "$stage/MANIFEST"

artifact="$out/flasher-artifact-$rev.tar.gz"
tar -czf "$artifact" -C "$stage" MANIFEST bin dist
sha=$(sha256sum "$artifact" | cut -d' ' -f1)
echo "artifact: $artifact"
echo "sha256:   $sha"
echo "glibc needed: ${need:-unknown}"

if [[ $ship = 0 ]]; then
    echo
    echo "to deploy: scp $artifact $host:/tmp/ && ssh $host 'sudo /opt/flasher/update.sh --artifact /tmp/$(basename "$artifact")'"
    exit 0
fi

target_glibc=$(ssh "${SSH_OPTS[@]}" "$host" "ldd --version | head -1 | awk '{print \$NF}'")
if [[ -n $need && -n $target_glibc ]] && [[ $need != "$(printf '%s\n%s\n' "$need" "$target_glibc" | sort -V | head -1)" ]]; then
    echo "ERROR: artifact needs glibc $need but $host has $target_glibc." >&2
    echo "       Build in an environment with glibc <= $target_glibc." >&2
    exit 1
fi

remote=/tmp/$(basename "$artifact")
scp "${SSH_OPTS[@]}" -q "$artifact" "$host:$remote"
echo "uploaded to $host:$remote"
# The updater runs as root, so this step is interactive: after the sudoers
# narrowing on the server, sudo prompts once for that host's password.
ssh -t -o BatchMode=no "$host" "sha256sum '$remote' | grep -q '$sha' && sudo /opt/flasher/update.sh --artifact '$remote' && rm -f '$remote'"
echo "deployed flasher $rev"
