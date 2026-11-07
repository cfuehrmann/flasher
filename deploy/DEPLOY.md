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
clone, not a tarball. The repo is public, so use the credential-free HTTPS
remote (pulls need no key as any user; pushes happen from the laptop):

```sh
# laptop: push the repo (once)
git push origin master

# server:
git clone https://github.com/cfuehrmann/flasher.git /opt/flasher/src
```

### A2. Toolchain (development machine only) — [dev]

The server does **not** build. It has no Rust/Trunk toolchain by design: builds
run on a development machine and only the artifact travels.

On the development machine (versions must match the frontend pins):

```sh
rustup target add wasm32-unknown-unknown
cargo install --locked --version 0.21.14 trunk
cargo install --locked --version 0.2.129 wasm-bindgen-cli

# build, ship, install, restart, health-check — one command
deploy/build-artifact.sh --ship
```

On the server, once: install `deploy/update.sh` as `/opt/flasher/update.sh`, then
provision the service user and state directory:

```sh
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

## Nightly backup

`backup-flasher.timer` (01:00 UTC) runs `~/bin/backup-flasher.sh`, which rsyncs a
consistent snapshot of the SQLite database to the home server
(`carsten@172.30.0.2:web-server-flasher-db/`, key `backup_ed25519`, restricted to
`rrsync -wo` on the receiving side) and only then pings the Better Stack
heartbeat.

The snapshot comes from `/usr/local/sbin/sqlite-snapshot flasher`: sqlite's
online `.backup` as the `flasher` service user (carsten cannot create the WAL
`-shm` file), integrity-checked and handed to carsten as a single clean
`flasher.db`. That replaced an earlier rsync of the live
`flasher.db`/`-wal`/`-shm` files, which was not transactional.

These pieces are host-level and live in the private
[`hetzner-infra`](https://github.com/cfuehrmann/hetzner-infra) repository
(`host/sqlite-snapshot`, `host/backup-flasher.*`, `host/sudoers.d/carsten`), not
here — they are shared with prepper, and the Better Stack heartbeat URLs must
never be versioned in this public repository:

```sh
git clone git@github.com:cfuehrmann/hetzner-infra
sudo bash hetzner-infra/install.sh
```

`/etc/sudoers.d/carsten` grants exactly one passwordless command,
`/usr/local/sbin/sqlite-snapshot`; everything else — including this app's
updater — still requires a password, so deployments use the artifact path from a
development machine (Part B).

---

## Part B — updating (every deploy after the first)

The build happens on a **development machine**, not on the server: the server
has no Rust/Trunk toolchain (by design — it has 4 cores and a small disk). The
artifact carries the `flasher` and `flasher-migrate` binaries plus the frontend
dist, and the server-side updater stages them, keeps the previous release for
rollback, restarts, and health-checks:

```sh
deploy/build-artifact.sh --ship        # build here, scp, install, restart, verify
```

`deploy/build-artifact.sh` refuses to ship when the binaries need a glibc newer
than the server's (development machines run 2.44, the server 2.39; flasher needs
2.34). Without `--ship` it only produces `/tmp/flasher-artifact-<rev>.tar.gz` and
prints the manual scp/ssh pair. The server-side entry point is
`sudo /opt/flasher/update.sh --artifact <file>`.

The updater still supports a source build in place (`sudo /opt/flasher/update.sh`
with no argument: `git pull --ff-only`, `cargo build`, `trunk build`) for a
reproducible rebuild on a host that does have a toolchain; it is not how
production is deployed.

**Planned: build in CI.** The end state is that GitHub Actions builds these
artifacts and the server pulls them, so neither a developer machine nor a
server compiler is involved. Tracked in
[issue #143](https://github.com/cfuehrmann/flasher/issues/143); until then the
machine that builds is a developer machine.

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
  `FLASHER_NEW_CARD_WAITING_MS`). Never set
  `FLASHER_USER` in production (auth-free dev bypass).
- The db self-backs-up (rotating, keep-10) before every schema migration
  into `/var/lib/flasher/backups/`.
- Migration history was squashed at version `0005` after production and the
  checked-in development database reached the labels schema. The first
  release containing that baseline accepts exactly the old `0001`–`0004`
  history, verifies the current schema, and removes those history rows; an
  older or unknown database refuses to start rather than guessing.
- Logs: `journalctl -u flasher -f`.
