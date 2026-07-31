//! Screenshot capture: the computer-vision loop (Phase 6). Renders every
//! key screen with realistic seeded content (long prompts, GFM tables,
//! `KaTeX` math, a full groom list, passkeys) at desktop (1280x800) and
//! mobile (390x844) viewport sizes and writes PNGs into
//! `test-output/screenshots/02_screenshots/`.
//!
//! Two tests: [`capture_app_screenshots`] drives the seeded app in
//! dev-bypass mode (quiz prompt/solution/done, groom list + delete modal,
//! editor split view, account tab); [`capture_auth_screenshots`] uses the
//! auth-mode harness for the register and login screens. Reviewers read
//! the PNGs — the assertions here only guarantee the capture landed on
//! the intended, fully rendered state (including `KaTeX` output).

// The seed offsets multiply small indices into millisecond timestamps;
// the pedantic cast lints add no value here (same reasoning as in the
// harness itself).
#![allow(clippy::cast_possible_wrap, clippy::cast_possible_truncation)]

use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use flasher_e2e::{E2E_USER, Error, Result, TestHarness};
use flasher_store::{CardState, NewCard, Store};

/// Timeout for every DOM wait; generous because the wasm bundle has to
/// download and boot first (same reasoning as the harness default).
const TIMEOUT: Duration = Duration::from_secs(15);

/// Desktop viewport: a small laptop.
const DESKTOP: (u32, u32) = (1280, 800);
/// Mobile viewport: an iPhone 12/13/14 in portrait.
const MOBILE: (u32, u32) = (390, 844);

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_millis()).unwrap_or(0))
}

// The error is only formatted, but `map_err` needs an owned receiver.
#[allow(clippy::needless_pass_by_value)]
fn store_err(err: flasher_store::Error) -> Error {
    Error::message(format!("store error: {err}"))
}

/// Opens the second WAL connection for seeding and resolves the e2e
/// user's id.
async fn seed_store(h: &TestHarness) -> Result<(Store, i64)> {
    let store = h.seed_store().await.map_err(store_err)?;
    let user = store
        .get_user_by_name(E2E_USER)
        .await
        .map_err(store_err)?
        .ok_or_else(|| Error::message(format!("user {E2E_USER} not found")))?;
    Ok((store, user.id))
}

/// Inserts one card with exact scheduling fields.
#[allow(clippy::too_many_arguments)]
async fn seed_card(
    store: &Store,
    user_id: i64,
    id: &str,
    prompt: &str,
    solution: &str,
    state: CardState,
    next_time: i64,
    disabled: bool,
) -> Result<()> {
    store
        .insert_card(&NewCard {
            user_id,
            id: id.to_owned(),
            prompt: prompt.to_owned(),
            solution: solution.to_owned(),
            state,
            change_time: now_ms() - 3_600_000,
            next_time,
            disabled,
        })
        .await
        .map_err(store_err)
}

/// The quiz hero card: a long, rich prompt (heading, inline math, bold)
/// and a solution exercising the full rendering pipeline — display
/// `KaTeX`, a GFM table with math cells, lists and code spans.
async fn seed_hero_card(store: &Store, user_id: i64) -> Result<()> {
    let prompt = "## Bayes' theorem — medical testing\n\n\
                  A disease has prevalence $p = 0.1\\%$. The test for it has \
                  **sensitivity** $99\\%$ and **specificity** $95\\%$.\n\n\
                  A patient tests *positive*. What is the probability they \
                  actually have the disease?";
    let solution = "By **Bayes' theorem**:\n\n\
                    $$P(D \\mid +) = \\frac{P(+ \\mid D) \\cdot P(D)}{P(+)}$$\n\n\
                    | Quantity | Value |\n\
                    |----------|-------|\n\
                    | $P(+ \\mid D)$ | $0.99$ |\n\
                    | $P(D)$ | $0.001$ |\n\
                    | $P(+)$ | $0.99 \\cdot 0.001 + 0.05 \\cdot 0.999 \\approx 0.0509$ |\n\n\
                    So $P(D \\mid +) \\approx \\dfrac{0.00099}{0.0509} \\approx 1.9\\%$ — \
                    most positives are false alarms.\n\n\
                    - low prevalence dominates the result\n\
                    - see `man bayes` for nothing at all";
    seed_card(
        store,
        user_id,
        "card-hero",
        prompt,
        solution,
        CardState::Ok,
        now_ms() - 1_000,
        false,
    )
    .await
}

/// One groom-list entry: `(id, prompt, state, hours until due, disabled)`.
/// Negative hours mean overdue (only used together with `disabled`, so
/// the quiz still sees exactly one due card — the hero).
struct GroomSeed {
    id: &'static str,
    prompt: &'static str,
    state: CardState,
    hours_until_due: i64,
    disabled: bool,
}

/// Sixteen realistic cards: mixed states, ascending due dates, two
/// disabled new cards (overdue, so they sort near the front like any
/// card — `disabled` is not a sort key). With
/// the hero card this fills two pages (server page size 10).
// The function is one literal seed table; the line count is data, not
// logic (same reasoning as the long view functions in the app).
#[allow(clippy::too_many_lines)]
async fn seed_groom_cards(store: &Store, user_id: i64) -> Result<()> {
    let cards = [
        GroomSeed {
            id: "card-tcp",
            prompt: "Which TCP flags are set on the first packet of the three-way handshake?",
            state: CardState::Ok,
            hours_until_due: 3,
            disabled: false,
        },
        GroomSeed {
            id: "card-http",
            prompt: "301 vs 302 redirects — which one are browsers allowed to cache?",
            state: CardState::Ok,
            hours_until_due: 8,
            disabled: false,
        },
        GroomSeed {
            id: "card-big-o",
            prompt: "Average and worst-case time complexity of quicksort?",
            state: CardState::Failed,
            hours_until_due: 26,
            disabled: false,
        },
        GroomSeed {
            id: "card-capital",
            prompt: "What is the capital of Australia?",
            state: CardState::New,
            hours_until_due: 50,
            disabled: false,
        },
        GroomSeed {
            id: "card-dns",
            prompt: "Which DNS record type maps a hostname to an IPv6 address?",
            state: CardState::Ok,
            hours_until_due: 74,
            disabled: false,
        },
        GroomSeed {
            id: "card-rust",
            prompt: "What does the `?` operator desugar to in a function returning `Result`?",
            state: CardState::Ok,
            hours_until_due: 98,
            disabled: false,
        },
        GroomSeed {
            id: "card-git",
            prompt: "How do you undo the last commit without rewriting history?",
            state: CardState::New,
            hours_until_due: 122,
            disabled: false,
        },
        GroomSeed {
            id: "card-sql",
            prompt: "Which SQL JOIN keeps every row of the left table?",
            state: CardState::Ok,
            hours_until_due: 146,
            disabled: false,
        },
        GroomSeed {
            id: "card-entropy",
            prompt: "Shannon entropy of a discrete distribution — the formula?",
            state: CardState::Failed,
            hours_until_due: 170,
            disabled: false,
        },
        GroomSeed {
            id: "card-cache",
            prompt: "Write-through vs write-back cache — what is the difference?",
            state: CardState::Ok,
            hours_until_due: 194,
            disabled: false,
        },
        GroomSeed {
            id: "card-osi",
            prompt: "Name the seven OSI layers from the wire up.",
            state: CardState::New,
            hours_until_due: 218,
            disabled: false,
        },
        GroomSeed {
            id: "card-ipv6",
            prompt: "How many bits does an IPv6 address have?",
            state: CardState::Ok,
            hours_until_due: 242,
            disabled: false,
        },
        GroomSeed {
            id: "card-long",
            prompt: "A colleague suggests storing user passwords as unsalted MD5 hashes \
                      \"because the database is internal anyway\". Name at least three \
                      concrete attacks this enables and the standard mitigation for each.",
            state: CardState::Ok,
            hours_until_due: 266,
            disabled: false,
        },
        GroomSeed {
            id: "card-regex",
            prompt: "How do you make the `*` quantifier lazy instead of greedy?",
            state: CardState::New,
            hours_until_due: 290,
            disabled: false,
        },
        GroomSeed {
            id: "card-teapot",
            prompt: "What does HTTP status 418 mean?",
            state: CardState::New,
            hours_until_due: -2,
            disabled: true,
        },
        GroomSeed {
            id: "card-quic",
            prompt: "Which port does QUIC / HTTP3 use by default?",
            state: CardState::New,
            hours_until_due: -5,
            disabled: true,
        },
    ];
    let now = now_ms();
    for card in &cards {
        seed_card(
            store,
            user_id,
            card.id,
            card.prompt,
            &format!("Solution to {}.", card.id),
            card.state,
            now + card.hours_until_due * 3_600_000,
            card.disabled,
        )
        .await?;
    }
    Ok(())
}

/// Three passkeys with realistic device names and ages; the first two
/// have been used to log in, the newest never. The `data` blob is opaque
/// to everything except `WebAuthn` ceremonies (never parsed by the UI).
async fn seed_passkeys(store: &Store, user_id: i64) -> Result<()> {
    let now = now_ms();
    let day = 86_400_000;
    let passkeys = [
        ("cred-macbook", "MacBook Pro — Touch ID", 210, 2),
        ("cred-yubikey", "YubiKey 5C NFC", 120, 12),
        ("cred-iphone", "iPhone 15 Pro", 20, -1),
    ];
    for (credential_id, name, created_days_ago, used_days_ago) in passkeys {
        let id = store
            .insert_passkey(
                user_id,
                credential_id,
                name,
                "{}",
                now - created_days_ago * day,
            )
            .await
            .map_err(store_err)?;
        if used_days_ago >= 0 {
            store
                .update_passkey_after_auth(user_id, id, "{}", now - used_days_ago * day)
                .await
                .map_err(store_err)?;
        }
    }
    Ok(())
}

/// Polls a JS boolean expression until it holds (`KaTeX` typesets
/// asynchronously after the DOM update) or the deadline elapses.
async fn wait_for_js(h: &TestHarness, expr: &str, timeout: Duration) -> Result<()> {
    let deadline = Instant::now() + timeout;
    loop {
        if let Ok(true) = h.eval::<bool>(expr).await {
            return Ok(());
        }
        if Instant::now() >= deadline {
            return Err(Error::message(format!(
                "timed out waiting for JS condition: {expr}"
            )));
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

/// Polls until no element matches `sel`.
async fn wait_until_gone(h: &TestHarness, sel: &str, timeout: Duration) -> Result<()> {
    let deadline = Instant::now() + timeout;
    loop {
        let exists: bool = h
            .eval(&format!("!!document.querySelector({sel:?})"))
            .await?;
        if !exists {
            return Ok(());
        }
        if Instant::now() >= deadline {
            return Err(Error::message(format!(
                "{sel} still present after {timeout:?}"
            )));
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

/// Captures the current screen at both viewport sizes as
/// `02_screenshots/<name>-desktop.png` and `-mobile.png`.
async fn shoot_both(h: &TestHarness, name: &str) -> Result<()> {
    h.set_viewport(DESKTOP.0, DESKTOP.1).await?;
    h.screenshot(&format!("02_screenshots/{name}-desktop"))
        .await?;
    h.set_viewport(MOBILE.0, MOBILE.1).await?;
    h.screenshot(&format!("02_screenshots/{name}-mobile"))
        .await?;
    Ok(())
}

/// All authenticated screens: quiz prompt, quiz solution (table +
/// `KaTeX`), quiz done, groom list, groom delete modal, editor split
/// view, account tab — each at desktop and mobile width.
#[tokio::test]
#[ignore = "browser"]
async fn capture_app_screenshots() -> Result<()> {
    let h = TestHarness::start().await?;
    let (store, user_id) = seed_store(&h).await?;
    seed_hero_card(&store, user_id).await?;
    seed_groom_cards(&store, user_id).await?;
    seed_passkeys(&store, user_id).await?;

    // --- Quiz: prompt ---
    h.goto("/").await?;
    h.wait_for_text("#quiz-prompt", "Bayes", TIMEOUT).await?;
    // The prompt carries inline math; wait until KaTeX typeset it.
    wait_for_js(
        &h,
        "document.querySelectorAll('#quiz-prompt .katex').length > 0",
        TIMEOUT,
    )
    .await?;
    shoot_both(&h, "quiz-prompt").await?;

    // --- Quiz: solution revealed (display math + GFM table) ---
    h.click("#show-solution").await?;
    h.wait_for_text("#quiz-solution", "Bayes", TIMEOUT).await?;
    wait_for_js(
        &h,
        "document.querySelectorAll('#quiz-solution .katex-display').length > 0",
        TIMEOUT,
    )
    .await?;
    shoot_both(&h, "quiz-solution").await?;

    // --- Quiz: done state (the hero was the only due card) ---
    h.click("#rate-ok").await?;
    h.wait_for_text("#quiz-done", "All done", TIMEOUT).await?;
    shoot_both(&h, "quiz-done").await?;

    // --- Groom: full list (two pages of realistic cards) ---
    h.click("#tab-groom").await?;
    h.wait_for_text("#groom-page-info", "of 17", TIMEOUT)
        .await?;
    shoot_both(&h, "groom").await?;

    // --- Groom: delete confirm modal on a learned card (progress warning) ---
    h.click("#menu-card-tcp").await?;
    h.wait_for_selector("#delete-card-tcp", TIMEOUT).await?;
    h.click("#delete-card-tcp").await?;
    h.wait_for_selector("#modal-progress-warning", TIMEOUT)
        .await?;
    shoot_both(&h, "groom-modal").await?;
    h.click("#modal-cancel").await?;
    wait_until_gone(&h, "#groom-modal", TIMEOUT).await?;

    // --- Editor: split view, prefilled with the hero card's Markdown ---
    h.click("#edit-card-hero").await?;
    h.wait_for_selector("#editor-prompt", TIMEOUT).await?;
    wait_for_js(
        &h,
        "document.querySelectorAll('#editor-preview-solution .katex-display').length > 0",
        TIMEOUT,
    )
    .await?;
    shoot_both(&h, "editor").await?;
    h.click("#editor-cancel").await?;
    h.wait_for_selector("#groom-search", TIMEOUT).await?;

    // --- Account: identity + passkey management ---
    h.click("#tab-account").await?;
    h.wait_for_text("#passkeys-list", "YubiKey", TIMEOUT)
        .await?;
    shoot_both(&h, "account").await?;

    // --- Recovery banner: a leftover autosave draft prompts on start ---
    store
        .put_autosave(
            user_id,
            None,
            "Draft of a card about the CAP theorem …",
            "… with its half-finished solution.",
            now_ms() - 300_000,
        )
        .await
        .map_err(store_err)?;
    h.goto("/").await?;
    h.wait_for_selector("#recovery-banner", TIMEOUT).await?;
    shoot_both(&h, "recovery-banner").await?;
    h.click("#discard-draft").await?;
    wait_until_gone(&h, "#recovery-banner", TIMEOUT).await?;

    Ok(())
}

/// The auth-mode screens: first-run register, then the login variant
/// after a successful registration — each at desktop and mobile width.
#[tokio::test]
#[ignore = "browser"]
async fn capture_auth_screenshots() -> Result<()> {
    let h = TestHarness::start_with_auth().await?;
    h.add_virtual_authenticator().await?;

    // Zero passkeys in the system: the register variant shows.
    h.wait_for_selector("#register-username", TIMEOUT).await?;
    shoot_both(&h, "auth-register").await?;

    h.type_into("#register-username", "carsten").await?;
    h.click("#create-passkey").await?;
    // Register/finish does not log in: the screen flips to the login
    // variant with the success note.
    h.wait_for_text("body", "Passkey created", TIMEOUT).await?;
    shoot_both(&h, "auth-login").await?;

    Ok(())
}
