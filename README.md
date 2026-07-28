# Flasher

A spaced-repetition flashcard app. Rust backend (axum + SQLite),
Leptos/wasm frontend, passkey authentication. Runs as a single binary
behind a reverse proxy; live at https://flasher.carstenfuehrmann.org.

## Getting started

Prerequisites: a Rust toolchain with the `wasm32-unknown-unknown` target,
plus `trunk` and `wasm-bindgen-cli` (`cargo install trunk wasm-bindgen-cli`).

```sh
just server     # builds the frontend, serves everything on http://localhost:3000
```

Development runs in an auth-free single-user bypass (`FLASHER_USER`,
default `dev`); passkey auth activates when that variable is unset.
Useful commands: `just gate` (full quality gate), `just e2e` (browser
tests), `just lighthouse`, `just mutants <crate>`. Deployment:
`deploy/DEPLOY.md`; updates on the server are `sudo /opt/flasher/update.sh`.

## History & approach

This started as a .NET + Vite/React app and was ported to Rust by an AI
agent (Kimi Code), with the owner acting as architect and reviewer. The
port went well beyond a translation: passkey authentication (WebAuthn)
replacing passwords, SQLite with embedded migrations replacing JSON files,
markdown + KaTeX card rendering, URL routing with full state restore
across reloads, and a one-command, AI-free deployment.

QA techniques used throughout: browser end-to-end tests driving real
Chromium (including passkey ceremonies via a CDP virtual authenticator),
snapshot testing, repeated unprimed adversarial reviews, mutation testing
(cargo-mutants; the codebase is currently mutation-clean), and per-page
Lighthouse checks. `AGENTS.md` documents the doctrine and workflow.

## For developers / contributors

Read **`AGENTS.md`** first — it defines the testing doctrine (browser e2e
as the only public test surface), the gates, and the review protocol.
`docs/plan.html` is the living plan; `docs/spec.md` the original spec.

Layout: `crates/` (backend workspace: types, core, store, auth, server,
e2e, migrate), `frontends/leptos/` (detached Leptos workspace), `deploy/`
(systemd, Caddy, update script), `docs/`.

## License

MIT (see `LICENSE`).
