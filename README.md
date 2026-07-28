# Flasher

A personal spaced-repetition flashcard app. Rust backend (axum + SQLite),
Leptos/wasm frontend, passkey authentication. Live at
https://flasher.carstenfuehrmann.org (single-user).

Formerly a .NET + React app; fully rewritten in Rust (see `docs/spec.md`
for the why and `docs/plan.html` for the living plan). The old code was
removed in the Phase-7 cleanup; it lives on in git history.

## Quick start

```sh
just server     # dev run: builds the frontend + serves everything on :3000
```

Open http://localhost:3000. Development runs in an auth-free single-user
bypass (`FLASHER_USER`, default `dev`); passkey auth activates when that
variable is unset.

## Working in this repo

Read **`AGENTS.md`** — it defines the testing doctrine (browser e2e as the
only public test surface), the quality gates, the adversarial-review
protocol, and the feature workflow. The short version:

```sh
just gate       # THE quality gate: clippy, tests, machete, deny, browser e2e
just e2e        # browser tests (headless Chromium, click-driven)
just lighthouse # per-page Lighthouse scores
just mutants <crate>   # targeted mutation testing
```

## Layout

- `crates/` — backend workspace (types/core/store/auth/server/e2e/migrate)
- `frontends/leptos/` — detached Leptos workspace (Trunk build)
- `deploy/` — systemd unit, Caddy site, `update.sh`, `DEPLOY.md`
- `docs/` — `spec.md`, `plan.html`

## Deployment

See `deploy/DEPLOY.md`. Updates: push to GitHub, then
`sudo /opt/flasher/update.sh` on the server (pull → build → restart →
health check).

## License

MIT (see `LICENSE`).
