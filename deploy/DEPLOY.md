# Deploying Flasher (Rust rewrite)

Target: the Hetzner server `116.203.151.104`, public URL
**https://carstenfuehrmann.org/flasher/** (the old .NET flasher lives there
today). This runbook is executable step-by-step, e.g. by kimi-code on the
server. **[laptop]** = your development machine, **[server]** = the cloud
server.

## 0. Prerequisites

1. **Base-path support (code task, do this first).** The app currently
   assumes it runs at the domain root, but it will live under `/flasher/`.
   Serving it there needs the "base path" feature (app + frontend URL
   prefix). Until that lands, this runbook's nginx/systemd steps will
   produce a broken site. Alternative with zero code changes: deploy on a
   subdomain (e.g. `flasher.carstenfuehrmann.org`) — passkeys still work
   with `FLASHER_RP_ID=carstenfuehrmann.org` (parent domain).
2. **Bootstrap token** (one-time): `openssl rand -hex 24`.
3. **First registration is the security-critical moment** — see "How
   single-user works" at the bottom before starting.

## 1. Source + frontend bundle — [laptop]

```sh
# Source (once pushed to GitHub, cloning on the server is nicer):
cd /home/carsten/flasher
git archive --format=tgz -o /tmp/flasher-src.tgz HEAD
scp /tmp/flasher-src.tgz root@116.203.151.104:/root/

# Prebuilt frontend bundle (arch-independent):
cd /home/carsten/flasher/frontends/leptos && env -u NO_COLOR trunk build --release
tar czf /tmp/flasher-dist.tgz -C dist .
scp /tmp/flasher-dist.tgz root@116.203.151.104:/root/
```

## 2. Build the server binaries — [server]

The laptop's glibc is newer than the server's — do NOT copy binaries; build:

```sh
apt-get update && apt-get install -y build-essential pkg-config libssl-dev curl
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
. "$HOME/.cargo/env"
mkdir -p /opt/flasher/src && tar xzf /root/flasher-src.tgz -C /opt/flasher/src
cd /opt/flasher/src
cargo build --release -p flasher-server -p flasher-migrate
install -m755 target/release/flasher /opt/flasher/flasher
install -m755 target/release/flasher-migrate /opt/flasher/flasher-migrate
mkdir -p /opt/flasher/dist && tar xzf /root/flasher-dist.tgz -C /opt/flasher/dist
useradd -r -s /usr/sbin/nologin flasher || true
mkdir -p /var/lib/flasher && chown -R flasher:flasher /var/lib/flasher
```

## 3. Migrate the card data — [server]

Find the old JSON store (old app's config: `FileStore:Directory`). It is
only READ:

```sh
su -s /bin/bash flasher -c \
  '/opt/flasher/flasher-migrate --from /PATH/TO/flasher-store --db /var/lib/flasher/flasher.db'
# Expect: per-user report + "verify: OK".
```

## 4. systemd — [server]

```sh
# Set FLASHER_BOOTSTRAP_TOKEN in the file first!
install -m644 /opt/flasher/src/deploy/flasher.service /etc/systemd/system/flasher.service
systemctl daemon-reload && systemctl enable --now flasher
journalctl -u flasher -n 20   # expect "listening" + open-bootstrap warning
```

## 5. nginx — [server]

Add the location block from `/opt/flasher/src/deploy/nginx-flasher.conf` to
the EXISTING HTTPS vhost for carstenfuehrmann.org, then
`nginx -t && systemctl reload nginx`.

## 6. Claim your account — [browser, time-sensitive]

1. Open `https://carstenfuehrmann.org/flasher/` → register screen.
2. Username: `kakimena` (attaches the passkey to your migrated cards),
   plus the bootstrap token → create passkey → sign in.
3. Verify your cards/quiz work.
4. Seal it: remove the `FLASHER_BOOTSTRAP_TOKEN` line from
   `/etc/systemd/system/flasher.service`, then
   `systemctl daemon-reload && systemctl restart flasher`.

## 7. Verify — [browser/server]

Quiz a card, search umlauts in Groom, enable a card, edit + F5 (state
survives), add a second passkey in Account.
`ls /var/lib/flasher/backups/` shows pre-migration backups.

## 8. Retire the old app — [server, after a soak period]

Stop/disable the old .NET service, remove its nginx bits. KEEP the old JSON
`flasher-store` directory untouched as a backup.

## How single-user works (read once)

The app allows account creation **only while it has zero passkeys
registered**. That is the only moment a stranger could create an account —
or take over yours, since your migrated user `kakimena` initially has no
passkey. Two protections overlap:

1. **The bootstrap token**: with `FLASHER_BOOTSTRAP_TOKEN` set, the first
   registration is rejected unless the token is supplied. Even if someone
   stumbles onto the fresh site before you, they can't register.
2. **The window self-closes**: the instant your first passkey exists, open
   registration is gone forever — the server re-checks "zero passkeys" at
   the *end* of the ceremony, so even a registration that was started
   before yours can no longer complete. From then on, no one can create
   accounts at all, and adding passkeys requires your logged-in session.
   (The token can and should be removed at that point.)

## Notes

- Env config (all optional except where noted): `FLASHER_PORT`,
  `FLASHER_DIST`, `FLASHER_DB`, `FLASHER_RP_ID`, `FLASHER_ORIGIN`,
  `FLASHER_BOOTSTRAP_TOKEN`, SRS tunables (`FLASHER_OK_MULTIPLIER`,
  `FLASHER_FAILED_MULTIPLIER`, `FLASHER_NEW_CARD_WAITING_MS`,
  `FLASHER_PAGE_SIZE`). Never set `FLASHER_USER` in production (auth-free
  dev bypass).
- The db self-backs-up (rotating, keep-10) before every schema migration
  into `/var/lib/flasher/backups/`.
- Logs: `journalctl -u flasher -f`.
