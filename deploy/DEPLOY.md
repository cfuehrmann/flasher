# Deploying Flasher (Rust rewrite) to the Hetzner server

Target host: `116.203.151.104` (currently hosts the old .NET flasher).
This runbook is written to be executable step-by-step — e.g. by kimi-code on
the server. Steps marked **[laptop]** run on the development machine, steps
marked **[server]** on the cloud server.

## 0. Decisions to make FIRST (one minute, human)

1. **Final domain.** Passkeys bind to the relying-party ID (= the domain) and
   do NOT transfer. Decide the final domain now (e.g. the same one the old
   app uses, or a new one). Write it as `DOMAIN` below. Testing on a
   throwaway subdomain first is fine, but passkeys registered there must be
   re-registered on the final domain.
2. **Bootstrap token.** Generate one: `openssl rand -hex 24` (or
   `uuidgen | tr -d -`). Needed exactly once, for the first registration.

## 1. Get the source onto the server — [laptop]

Preferred (once the repo is committed+pushed): `git clone` on the server.

Without git, from the laptop:

```sh
cd /home/carsten/flasher
tar czf /tmp/flasher-src.tgz Cargo.toml Cargo.lock justfile .cargo crates frontends/leptos deploy
scp /tmp/flasher-src.tgz root@116.203.151.104:/root/
```

Also copy the PREBUILT frontend bundle (arch-independent; saves installing
trunk/wasm tooling on the server):

```sh
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
```

## 3. Migrate the card data — [server]

Find the old store (check the old app's config/systemd unit for
`FileStore:Directory`, e.g. `/home/*/flasher-store` or `../flasher-store`
relative to the old working dir). The store is only READ:

```sh
/opt/flasher/flasher-migrate --from /PATH/TO/flasher-store --db /var/lib/flasher/flasher.db
# Expect: per-user report + "verify: OK". Re-running later refuses if the db
# has diverged (that is the safety feature; --overwrite would restore).
```

(The importer runs fine before the `flasher` user exists; just `chown` after:
`useradd -r -s /usr/sbin/nologin flasher; mkdir -p /var/lib/flasher; chown -R flasher:flasher /var/lib/flasher`.)

## 4. systemd — [server]

```sh
# Edit DOMAIN_HERE and FLASHER_BOOTSTRAP_TOKEN first!
install -m644 /opt/flasher/src/deploy/flasher.service /etc/systemd/system/flasher.service
systemctl daemon-reload && systemctl enable --now flasher
systemctl status flasher   # expect "listening" + the open-bootstrap warning
```

## 5. nginx + TLS — [server]

Reuse the existing nginx setup. Add the vhost from
`/opt/flasher/src/deploy/nginx-flasher.conf` (set `server_name` + cert paths;
if it's a new (sub)domain: `certbot --nginx -d DOMAIN` first), then
`nginx -t && systemctl reload nginx`.

Recommendation: run the new app on a **parallel (sub)domain or port first**,
keep the old flasher untouched, and switch the main vhost only after step 6
works end-to-end. Rollback = point the vhost back.

## 6. Claim the account (single-user seal) — [browser]

1. Browse `https://DOMAIN` → register screen (system has zero passkeys).
2. Enter your username (use the existing one from the old store,
   case-insensitive, to attach to your migrated cards), enter the bootstrap
   token, create the passkey.
3. Sign in. Verify: your cards are there, quiz works.
4. **Seal it:** remove the `FLASHER_BOOTSTRAP_TOKEN` line from
   `/etc/systemd/system/flasher.service`, then
   `systemctl daemon-reload && systemctl restart flasher`.

Single-user is structural from here: registration without a session is only
possible while the system has zero passkeys — after your registration that
door is closed forever, and adding passkeys requires your session. Never set
`FLASHER_USER` on the server (that's the auth-free dev bypass).

## 7. Verify — [server/browser]

- Quiz: rate a card; check the schedule updates (reload, next card).
- Groom: search (umlauts!), enable a card, it becomes quizzable.
- Editor: edit, wait for "draft saved", F5 — the editor and text survive.
- Account: add a second passkey, rename one.
- `sqlite3 /var/lib/flasher/flasher.db 'select count(*) from cards'` matches
  the importer report; `ls /var/lib/flasher/backups/` shows pre-migration
  backups.

## 8. Retire the old app — [server, after a soak period]

Stop/disable the old .NET service and remove its vhost. KEEP the old JSON
`flasher-store` directory untouched as a backup. Deleting the old code in the
repo is Phase 7's final step, done separately.

## Notes

- Config is env-only (see `deploy/flasher.service`): PORT, DIST, DB, RP_ID,
  ORIGIN, BOOTSTRAP_TOKEN, plus SRS tunables (`FLASHER_OK_MULTIPLIER`,
  `FLASHER_FAILED_MULTIPLIER`, `FLASHER_NEW_CARD_WAITING_MS`,
  `FLASHER_PAGE_SIZE`).
- The db self-backs-up (rotating, keep-10) into
  `/var/lib/flasher/backups/` before every schema migration at startup.
- Logs: `journalctl -u flasher -f`.
