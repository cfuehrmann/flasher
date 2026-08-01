# AGENTS.md — How to work in this repo

Flasher is a personal spaced-repetition flashcard app, rewritten in Rust
(replacing a .NET + React app; see `docs/spec.md` for the motivation and
`docs/plan.html` for the living plan). It is deployed as a single binary
behind Caddy at https://flasher.carstenfuehrmann.org.

This file is the contract for anyone (human or AI) making changes. Follow
it literally; the conventions below are battle-tested and non-optional.

## Stack & layout

- `crates/` — cargo workspace (`resolver = "2"`, edition 2024):
  - `flasher-types` — **contract authority**: every API payload is defined
    here once; server and frontend both depend on it. API drift = compile
    error.
  - `flasher-core` — pure domain logic (SRS scheduling, date formatting).
    No I/O. Prime mutation-testing target.
  - `flasher-store` — SQLite via sqlx (runtime-checked queries, embedded
    migrations, WAL, FK on, rotating pre-migration backups).
  - `flasher-migrate` — JSON filestore → SQLite importer (conflict-safe).
  - `flasher-auth` — passkeys (webauthn-rs), challenges, sessions.
    Extraction-ready for a future central passkey service.
  - `flasher-server` — axum app (lib.rs + thin main.rs). Env config only
    (`FLASHER_*`).
  - `flasher-e2e` — chromiumoxide browser harness + all e2e tests.
- `frontends/leptos/` — **deliberately detached** cargo workspace (own
  Cargo.toml/lock). Leptos 0.8 CSR wasm, Trunk build. Features:
  `default = ["csr"]`, `ssr` for host-target tests.
- `deploy/` — systemd unit, Caddyfile, `update.sh`, `DEPLOY.md`.
- `docs/` — `spec.md` (original spec), `plan.html` (living plan — keep it
  current with every phase/feature; low verbosity for done items).
- The old .NET/React app was deleted in the Phase-7 cleanup. It remains in
  git history as the behavioral reference — when porting behavior, look it
  up there (e.g. `git show master~N:backend/...` or the pre-cleanup commit)
  and cite provenance in comments.

## Commands (justfile is the entry point)

- `just server` — dev run (trunk build + axum, http://localhost:3000,
  auth-free dev bypass via `FLASHER_USER`).
- `just gate` — THE quality gate: web-gate (trunk build, ssr tests, wasm
  size budget) + rust-gate (clippy `-D warnings`, nextest, machete, deny)
  + browser e2e. Must be green before every commit.
- `just e2e` — browser tests only. `just screenshots` — capture all
  screens at desktop+mobile. `just lighthouse` — per-page scores.
- `just mutants <crate> [FILE]` — targeted mutation run. Requires a clean
  git tree or `MUTANTS_ALLOW_DIRTY=1`.
- Deploy: push to GitHub, then `sudo /opt/flasher/update.sh` on the
  server (pull → build backend+frontend → restart → health check).
- Merging PRs: merge commits are disabled on the repo — always
  `gh pr merge <n> --squash --delete-branch`.

## Testing doctrine (the owner cares about this deeply)

1. **The web app in a real browser — clicked like a user — is the ONLY
   valid public test surface.** The HTTP API is internal: no API behavior
   tests (smoke-level only, e.g. health). Sanctioned exception: auth
   security negative paths (`crates/flasher-server/tests/auth_negative.rs`).
2. E2e tests live in `crates/flasher-e2e/tests/`, are `#[ignore =
   "browser"]`, drive a real headless Chromium via CDP, seed the db
   through `flasher_store` (never the API), and may use the db for
   white-box assertions. Each test gets a fresh db + browser profile;
   they must be parallel-safe and non-flaky (state-based waits, never
   sleeps/retries).
3. Passkey ceremonies are e2e-tested with a CDP virtual authenticator.
4. Snapshot testing: `insta` (importer golden tests, migration dumps).
   SSR host-target tests in the leptos crate (`cargo test
   --no-default-features --features ssr`) for markup/route logic.
5. **Mutation testing** (cargo-mutants): after writing tests, run
   `just mutants <crate>` (or the leptos manifest-path variant for
   frontend logic); `just mutants-all` for a full workspace sweep. The
   point is multi-fold, not just a coverage number:
   - *Test completeness*: line coverage says code ran; a killed mutant
     says a test would notice it breaking.
   - *Dead-code detection*: a survivor may mean there is no real-life
     use case — if no test can observe the change, maybe nothing needs
     the code. Check for callers before reaching for a test.
   - *Harness gaps*: survivors in glue code can reveal a layer with no
     test surface at all (e.g. a CLI `main` nothing drives).
   Triage every survivor on its merits — never discount one as trivial
   unread, never disable a mutant to make it go away. Each survivor gets
   exactly one fate: write the missing test (the default), delete the
   dead code, document it in `.cargo/mutants.toml` (resp.
   `frontends/leptos/.cargo/mutants.toml`) as equivalent/browser-only
   with a real justification, or — rarely — a code comment explaining
   why it is deliberately left uncovered. Every mutant must end CAUGHT,
   UNVIABLE, or documented.
6. Computer-vision loop: after any UI change, regenerate screenshots and
   READ the PNGs before declaring done. If it looks broken, it is broken.

## Adversarial review (standing, unprimed)

After each feature/phase, launch a fresh review agent with a **clean
context**: give it ONLY `docs/spec.md` + `docs/plan.html` and pointers to
the code — no implementation narrative, no list of known deviations, no
hints. The review must include a **behavioral test**: actually drive the
app in a browser (scratch tests via the e2e harness allowed; tree left
unchanged) and judge UX and latency, not just code. Triage findings:
fix real ones, record deferrals in `docs/plan.html`.

## Workflow for a feature prompt

1. Read `docs/plan.html` + this file. Update the plan with the feature
   (badge: in progress).
2. Implement as a **vertical slice**: domain logic (flasher-core) +
   internal API + UI + browser e2e together — never a backend without
   its user-facing test surface. Port behavior faithfully from the old
   app where it exists (cite the source in comments).
3. `just gate` green. Targeted mutation run on new pure logic.
4. CV check of affected screens (desktop + mobile). `just lighthouse` —
   keep scores (perf ≥ 88; others 100).
5. Unprimed adversarial review; triage; re-verify.
6. Update `docs/plan.html` (badge: done, one or two lines). Commit
   (pre-commit hook runs the gate; don't bypass it).

## Code conventions

- Workspace lints: `unsafe_code = "forbid"`; clippy all + pedantic;
  `unwrap_used`/`expect_used`/`panic` = warn — **also in tests** (use
  Result-returning test fns). `.cargo/config.toml` sets `-D warnings`.
- Dependencies: keep the tree lean. `default-features = false` wherever
  sensible; never `features = ["full"]`; exact pins with justification
  comments for wasm-ABI-sensitive crates; no new dependency without a
  proven need; `cargo machete` and `cargo deny check` must stay green.
- Loud schema change rule: no defensive `#[serde(default)]` — schema
  drift fails loudly; migrations handle stored data.
- The ONE sanctioned `inner_html` is the markdown pipeline
  (pulldown-cmark → ammonia). Never add another.
- Timestamps are i64 epoch millis. JSON is snake_case.
- trunk quirk: invoke as `env -u NO_COLOR trunk build` in scripts.
- Single-user production model: registration without a session exists
  only while zero passkeys exist. `FLASHER_USER` (auth-free dev bypass)
  must never be set in production.
- SQLite migration policy: the embedded `0005_current_schema_baseline.sql`
  is the current schema baseline. The one-time squash accepts only a complete
  pre-squash `0001`–`0004` history whose schema is already current, then
  records baseline `0005` and removes the old history rows. Older or unknown
  histories fail loudly; future schema changes append migrations after 0005.
  Before a squash or schema release, verify production and the checked-in
  development database are on the current schema. Do not edit the baseline
  after release.

## Don'ts

- Don't add API-level behavior tests (see doctrine; auth exception noted).
- Don't add a router/state-manager/CSS-framework/npm dependency.
- Don't skip the adversarial review or the mutation run because "small".
- Don't commit `flasher.db`, `backups/`, `mutants.out*` (gitignored).
