set shell := ["bash", "-uc"]

# Build the leptos bundle and serve it (default http://localhost:3000, override via FLASHER_PORT).
server:
    #!/usr/bin/env bash
    set -euo pipefail
    (cd frontends/leptos && env -u NO_COLOR trunk build)
    echo "Serving at http://localhost:${FLASHER_PORT:-3000}"
    cargo run -p flasher-server

# Debug build: leptos bundle + full Rust workspace.
build:
    cd frontends/leptos && env -u NO_COLOR trunk build
    cargo build --workspace

# Release build: leptos bundle + full Rust workspace.
build-release:
    cd frontends/leptos && env -u NO_COLOR trunk build --release
    cargo build --release --workspace

# Static checks + tests for the root workspace. Shared by rust-gate and gate.
_rust-checks:
    #!/usr/bin/env bash
    set -euo pipefail
    cargo clippy --workspace --all-targets -- -D warnings
    # Test runner: nextest if available, else cargo test. Override via FLASHER_TEST_RUNNER=nextest|cargo.
    runner="${FLASHER_TEST_RUNNER:-auto}"
    if [[ "$runner" == "auto" ]]; then
        if command -v cargo-nextest >/dev/null 2>&1; then runner=nextest; else runner=cargo; fi
    fi
    if [[ "$runner" == "nextest" ]]; then
        cargo nextest run --workspace
    else
        cargo test --workspace
    fi
    # Unused-dependency check. The leptos frontend is a detached cargo workspace,
    # so it needs its own machete run in a subshell.
    cargo machete
    if [[ -d frontends/leptos ]]; then
        (cd frontends/leptos && cargo machete)
    else
        echo "frontends/leptos not present yet, skipping machete there"
    fi
    cargo deny check

# Root workspace checks plus the leptos host-target ssr test.
rust-gate: _rust-checks
    cargo test --manifest-path frontends/leptos/Cargo.toml --no-default-features --features ssr

# Leptos bundle build plus its host-target ssr test and the wasm size budget.
web-gate:
    cd frontends/leptos && env -u NO_COLOR trunk build
    just wasm-size-check
    cargo test --manifest-path frontends/leptos/Cargo.toml --no-default-features --features ssr

# Fail if the largest wasm bundle exceeds the size budget.
wasm-size-check:
    #!/usr/bin/env bash
    set -euo pipefail
    # Budget: ~1.5x the current largest bundle (3785 KiB, debug trunk
    # build measured 2026-07-27), rounded up.
    budget_kb=5700
    largest=$(ls -S frontends/leptos/dist/*.wasm | head -1)
    size_kb=$(( ($(stat -c%s "$largest") + 1023) / 1024 ))
    echo "largest wasm bundle: $largest ($size_kb KiB, budget $budget_kb KiB)"
    if (( size_kb > budget_kb )); then
        echo "wasm-size-check: budget exceeded ($size_kb > $budget_kb KiB)" >&2
        exit 1
    fi

# Full gate: web first (fast failure on frontend breakage), then rust,
# then the browser e2e suite. `rust-gate` stays Chrome-free.
# Output is teed to test-output/gate-<timestamp>.log; gate-latest.log symlinks to it.
gate:
    #!/usr/bin/env bash
    set -uo pipefail
    mkdir -p test-output
    log="test-output/gate-$(date +%Y%m%d-%H%M%S).log"
    status=0
    { just web-gate && just rust-gate && just e2e; } 2>&1 | tee "$log" || status=$?
    ln -sf "$(basename "$log")" test-output/gate-latest.log
    if [[ $status -eq 0 ]]; then
        echo "GATE: PASS (web-gate + rust-gate + e2e; log: $log)"
    else
        echo "GATE: FAIL (web-gate + rust-gate + e2e; log: $log)"
        exit 1
    fi

# Format both cargo workspaces (leptos is detached).
fmt:
    cargo fmt --all
    cargo fmt --manifest-path frontends/leptos/Cargo.toml

# Lighthouse audit of the release build (on-demand, NOT part of the gate):
# release trunk bundle + release server on a free port with a throwaway DB
# and a dev-bypass user (so the real pages render, not the auth screen).
# Every tab (/quiz, /groom, /add, /account) is audited; the per-page
# reports land in test-output/lighthouse/<page>.json and a compact score
# table is printed. latest.json stays the /quiz report (comparable with
# earlier runs).
lighthouse:
    #!/usr/bin/env bash
    set -euo pipefail
    (cd frontends/leptos && env -u NO_COLOR trunk build --release)
    cargo build --release -p flasher-server
    mkdir -p test-output/lighthouse
    tmp=$(mktemp -d)
    srv_pid=""
    cleanup() {
        [[ -n "$srv_pid" ]] && kill "$srv_pid" 2>/dev/null || true
        rm -rf "$tmp"
    }
    trap cleanup EXIT
    port=$(python3 -c 'import socket; s=socket.socket(); s.bind(("127.0.0.1", 0)); print(s.getsockname()[1]); s.close()')
    FLASHER_USER=lighthouse FLASHER_PORT=$port FLASHER_DB="$tmp/flasher.db" ./target/release/flasher >/dev/null 2>&1 &
    srv_pid=$!
    for _ in $(seq 1 50); do
        curl -sf "http://localhost:$port/api/health" >/dev/null 2>&1 && break
        sleep 0.2
    done
    pages=(quiz groom add account)
    for page in "${pages[@]}"; do
        echo "auditing /$page ..."
        CHROME_PATH=/usr/bin/chromium pnpm dlx lighthouse "http://localhost:$port/$page" \
            --quiet --chrome-flags="--headless --no-sandbox --disable-dev-shm-usage" \
            --output=json --output-path="test-output/lighthouse/$page.json"
    done
    cp test-output/lighthouse/quiz.json test-output/lighthouse/latest.json
    python3 - <<'PY'
    import json
    pages = ["quiz", "groom", "add", "account"]
    reports = {p: json.load(open(f"test-output/lighthouse/{p}.json")) for p in pages}
    cats = list(reports["quiz"]["categories"].keys())
    width = max(len(c) for c in cats) + 2
    print("\n=== Lighthouse scores (per page) ===")
    print("page".ljust(9) + "".join(c.rjust(width) for c in cats))
    for p in pages:
        scores = [round(reports[p]["categories"][c]["score"] * 100) for c in cats]
        print(p.ljust(9) + "".join(str(s).rjust(width) for s in scores))
    print("=== Key metrics (/quiz) ===")
    audits = reports["quiz"]["audits"]
    for key in ["first-contentful-paint", "largest-contentful-paint",
                "total-blocking-time", "cumulative-layout-shift",
                "speed-index", "interactive"]:
        print(f"{key}: {audits[key]['displayValue']}")
    PY

# Browser e2e: real Chromium against the real `flasher` binary.
# Tests are #[ignore = "browser"] so rust-gate never launches a browser;
# the full `gate` runs this recipe at the end.
e2e:
    #!/usr/bin/env bash
    set -euo pipefail
    cargo nextest run --run-ignored ignored-only -p flasher-e2e --test-threads 4

# Capture the app's screens into test-output/screenshots/ (02_screenshots only).
screenshots:
    #!/usr/bin/env bash
    set -euo pipefail
    cargo nextest run --run-ignored ignored-only -p flasher-e2e --test-threads 4 -- screenshots

# Targeted mutation testing for one crate (optionally one file).
# --in-place reuses the warm target cache and -j1 measured fastest in omega.
mutants CRATE FILE="":
    #!/usr/bin/env bash
    set -euo pipefail
    scripts/mutants-guard
    # --in-place reuses the warm target dir (fastest measured in omega);
    # cargo-mutants 27 forbids combining --in-place with -j.
    args=(-p "{{CRATE}}" --cap-lints=true --in-place)
    if [[ -n "{{FILE}}" ]]; then
        args+=(--file "{{FILE}}")
    fi
    cargo mutants "${args[@]}"

# Full mutation sweep in copy mode with a scratch dir.
# systemd-run wrapping: runaway mutants get OOM-killed in their own cgroup and recorded as caught.
mutants-all:
    #!/usr/bin/env bash
    set -euo pipefail
    scripts/mutants-guard
    mkdir -p ~/.cache/cargo-mutants-tmp
    systemd-run --user --scope -p MemoryMax=20G -p MemorySwapMax=0 -p OOMPolicy=continue \
        cargo mutants --workspace --cap-lints=true -j1 \
        --tmpdir ~/.cache/cargo-mutants-tmp

# Install git hooks (worktree-aware).
install-hooks:
    scripts/install-hooks

# Clean build artifacts (root workspace + leptos bundle).
clean:
    cargo clean
    rm -rf frontends/leptos/dist
