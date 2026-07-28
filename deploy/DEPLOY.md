# Deploying Flasher (Rust rewrite)

Target: the Hetzner server `116.203.151.104`, public URL
**https://flasher.carstenfuehrmann.org** (subdomain of the existing site;
passkeys use the parent domain `carstenfuehrmann.org` as RP ID, so they
work domain-wide). This runbook is executable step-by-step, e.g. by
kimi-code on the server.

**Read this first — the shape of the work:**
- **Part A (one-time, ~15 min):** server setup, data migration, account
  claim. Done once, never repeated.
- **Part B (every update):** one command — `ssh root@116.203.151.104
  /opt/flasher/update.sh`.

---

## Part A — one-time setup

### A0. Decisions/inputs (human, one minute)

- DNS: A record `flasher.carstenfuehrmann.org` → `116.203.151.104`.
- Bootstrap token (one-time): `openssl rand -hex 24`.
- Read "How single-user works" at the bottom before A6.

### A1. Source on the server — [laptop + server]

`update.sh` (Part B) works via `git pull`, so the source must be a real
clone, not a tarball:

```sh
# laptop: push the repo (once)
git push origin master

# server:
git clone git@github.com:cfuehrmann/flasher.git /opt/flasher/src
# (needs a read deploy key or your GitHub auth on the server)
```

### A2. Toolchain + build — [server]

The laptop's glibc is newer than the server's — do NOT copy binaries; build
on the server. Everything in this step is one-time; `update.sh` relies on it
later (including the frontend toolchain):

```sh
apt-get update && apt-get install -y build-essential pkg-config libssl-dev curl git
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
. "$HOME/.cargo/env"
rustup target add wasm32-unknown-unknown
cargo install --locked --version 0.21.14 trunk
cargo install --locked --version 0.2.126 wasm-bindgen-cli   # must match the frontend's wasm-bindgen pin

cd /opt/flasher/src
cargo build --release -p flasher-server -p flasher-migrate
(cd frontends/leptos && env -u NO_COLOR trunk build --release)
install -m755 target/release/flasher /opt/flasher/flasher
install -m755 target/release/flasher-migrate /opt/flasher/flasher-migrate
mkdir -p /opt/flasher/dist && rsync -a --delete frontends/leptos/dist/ /opt/flasher/dist/
useradd -r -s /usr/sbin/nologin flasher || true
mkdir -p /var/lib/flasher && chown -R flasher:flasher /var/lib/flasher
```

### A3. Migrate the card data — [server]

Find the old JSON store (old app's config: `FileStore:Directory`). It is
only READ:

```sh
su -s /bin/bash flasher -c \
  '/opt/flasher/flasher-migrate --from /PATH/TO/flasher-store --db /var/lib/flasher/flasher.db'
# Expect: per-user report + "verify: OK".
```

### A4. systemd + update script — [server]

```sh
# Set FLASHER_BOOTSTRAP_TOKEN in the unit file first!
install -m644 /opt/flasher/src/deploy/flasher.service /etc/systemd/system/flasher.service
install -m755 /opt/flasher/src/deploy/update.sh /opt/flasher/update.sh
systemctl daemon-reload && systemctl enable --now flasher
journalctl -u flasher -n 20   # expect "listening" + open-bootstrap warning
```

### A5. Caddy + TLS — [server]

The server uses Caddy (automatic HTTPS — no certbot needed). Add the site
block from `/opt/flasher/src/deploy/Caddyfile` to the existing Caddyfile
(e.g. `/etc/caddy/Caddyfile`), then `caddy validate` (if available) and
`systemctl reload caddy`. Caddy fetches the certificate for
`flasher.carstenfuehrmann.org` on first request.

Keep the old flasher's site untouched until the soak is over — rollback is
removing the new block.

### A6. Claim your account — [browser, one-time]

1. Open `https://flasher.carstenfuehrmann.org` → register screen.
2. Username `kakimena` (attaches to your migrated cards) + bootstrap token
   → create passkey → sign in → verify cards/quiz.
3. Seal it: remove the `FLASHER_BOOTSTRAP_TOKEN` line from
   `/etc/systemd/system/flasher.service`, then
   `systemctl daemon-reload && systemctl restart flasher`.

### A7. Verify — [browser/server]

Quiz a card, search umlauts in Groom, enable a card, edit + F5 (state
survives), add a second passkey in Account.
`ls /var/lib/flasher/backups/` shows pre-migration backups.

### A8. Retire the old app — [server, after a soak period]

Stop/disable the old .NET service, remove its vhost/site config. KEEP the old JSON
`flasher-store` directory untouched as a backup.

---

## Part B — updating (every deploy after the first)

One command, from anywhere, **no AI and no laptop build involved** —
the server pulls, rebuilds backend AND frontend, swaps artifacts, restarts,
and health-checks itself:

```sh
ssh root@116.203.151.104 /opt/flasher/update.sh
```

Optional later automation (still no AI): a GitHub webhook or a systemd
timer that calls `update.sh`.

---

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
   the *end* of the ceremony, so even a registration started before yours
   can no longer complete. From then on, adding passkeys requires your
   logged-in session. (Remove the token at that point.)

## Notes

- Env config: `FLASHER_PORT`, `FLASHER_DIST`, `FLASHER_DB`,
  `FLASHER_RP_ID`, `FLASHER_ORIGIN`, `FLASHER_BOOTSTRAP_TOKEN`, SRS
  tunables (`FLASHER_OK_MULTIPLIER`, `FLASHER_FAILED_MULTIPLIER`,
  `FLASHER_NEW_CARD_WAITING_MS`, `FLASHER_PAGE_SIZE`). Never set
  `FLASHER_USER` in production (auth-free dev bypass).
- The db self-backs-up (rotating, keep-10) before every schema migration
  into `/var/lib/flasher/backups/`.
- Logs: `journalctl -u flasher -f`.
