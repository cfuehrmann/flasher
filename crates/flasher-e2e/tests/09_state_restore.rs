//! UI session-state restore e2e tests (Phase 6.6): a browser refresh
//! keeps the tab and the groom editor (`/groom/edit/{id}`), but NOT the
//! quiz's solution-revealed state — that is transient and a refresh
//! always starts collapsed at the prompt. A
//! server-side autosave draft matching the restored editor prefills it
//! inline (F5 as a mini crash recovery) instead of prompting the
//! recovery banner; a non-matching draft still banners. All click-driven
//! through the browser, with the database only used for seeding and
//! white-box verification.

use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use flasher_e2e::{E2E_USER, Error, Result, TestHarness};
use flasher_store::{CardState, NewCard, Store};

/// Timeout for every DOM wait; generous because the wasm bundle has to
/// download and boot first (same reasoning as the harness default).
const TIMEOUT: Duration = Duration::from_secs(15);

/// Waiting for the autosave indicator: the interval ticks every 5 s, so
/// the first "draft saved" can take a full tick plus the PUT round-trip.
const AUTOSAVE_TIMEOUT: Duration = Duration::from_secs(25);

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

/// Opens the second WAL connection for seeding/verification and resolves
/// the e2e user's id.
async fn seed_store(h: &TestHarness) -> Result<(Store, i64)> {
    let store = h.seed_store().await.map_err(store_err)?;
    let user = store
        .get_user_by_name(E2E_USER)
        .await
        .map_err(store_err)?
        .ok_or_else(|| Error::message(format!("user {E2E_USER} not found")))?;
    Ok((store, user.id))
}

/// Inserts one enabled card with exact scheduling fields.
async fn seed_card(
    store: &Store,
    user_id: i64,
    id: &str,
    prompt: &str,
    solution: &str,
    next_time: i64,
) -> Result<()> {
    store
        .insert_card(&NewCard {
            user_id,
            id: id.to_owned(),
            prompt: prompt.to_owned(),
            solution: solution.to_owned(),
            state: CardState::New,
            change_time: now_ms(),
            next_time,
            labels: vec!["Enabled".to_owned()],
        })
        .await
        .map_err(store_err)
}

/// Polls `location.pathname` until it equals `path` (a `pushState` and
/// `history.back()` are fire-and-forget).
async fn wait_for_path(h: &TestHarness, path: &str) -> Result<()> {
    let deadline = Instant::now() + TIMEOUT;
    loop {
        let current: String = h.eval("location.pathname").await?;
        if current == path {
            return Ok(());
        }
        if Instant::now() >= deadline {
            return Err(Error::message(format!(
                "pathname is {current:?}, expected {path:?} after {TIMEOUT:?}"
            )));
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

/// The `.value` of a textarea/input.
async fn field_value(h: &TestHarness, sel: &str) -> Result<String> {
    h.eval::<String>(&format!("document.querySelector({sel:?}).value"))
        .await
}

/// Appends text to a field like a user: click to focus, `End` to jump
/// past the prefilled content, then type.
async fn append_text(h: &TestHarness, sel: &str, text: &str) -> Result<()> {
    h.click(sel).await?;
    let el = h.page.find_element(sel).await.map_err(Error::Cdp)?;
    el.press_key("End").await.map_err(Error::Cdp)?;
    el.type_str(text).await.map_err(Error::Cdp)?;
    Ok(())
}

/// Asserts no element matches `sel` after giving the app a beat to
/// (not) render it.
async fn assert_absent(h: &TestHarness, sel: &str) -> Result<()> {
    tokio::time::sleep(Duration::from_millis(500)).await;
    if h.eval::<bool>(&format!("!!document.querySelector({sel:?})"))
        .await?
    {
        return Err(Error::message(format!("{sel} must not be present")));
    }
    Ok(())
}

/// (a) The reveal state is transient: revealing the solution does not
/// touch the URL, and a reload comes back COLLAPSED at the prompt
/// (solution hidden behind "Show solution"). Rating then advances to
/// the next card's prompt as before.
#[tokio::test]
#[ignore = "browser"]
async fn solution_reveal_collapses_on_reload() -> Result<()> {
    let h = TestHarness::start().await?;
    let (store, user_id) = seed_store(&h).await?;
    let now = now_ms();
    seed_card(
        &store,
        user_id,
        "card-a",
        "Prompt A",
        "Solution A",
        now - 60_000,
    )
    .await?;
    seed_card(
        &store,
        user_id,
        "card-b",
        "Prompt B",
        "Solution B",
        now - 30_000,
    )
    .await?;

    h.goto("/quiz").await?;
    h.wait_for_text("#quiz-prompt", "Prompt A", TIMEOUT).await?;
    h.click("#show-solution").await?;
    h.wait_for_text("#quiz-solution", "Solution A", TIMEOUT)
        .await?;
    // Revealing is pure in-memory state: the URL stays /quiz.
    wait_for_path(&h, "/quiz").await?;

    // Reload: the card comes back COLLAPSED — prompt plus "Show
    // solution", no solution, no rating buttons.
    h.goto("/quiz").await?;
    h.wait_for_text("#quiz-prompt", "Prompt A", TIMEOUT).await?;
    h.wait_for_selector("#show-solution", TIMEOUT).await?;
    if h.eval::<bool>("!!document.querySelector('#quiz-solution, #rate-ok, #rate-failed')")
        .await?
    {
        return Err(Error::message(
            "a reload must collapse the quiz back to the prompt",
        ));
    }
    h.screenshot("09_state_restore/quiz-prompt-after-reload")
        .await?;

    // Revealing and rating still work after the reload.
    h.click("#show-solution").await?;
    h.wait_for_text("#quiz-solution", "Solution A", TIMEOUT)
        .await?;
    h.click("#rate-ok").await?;
    h.wait_for_text("#quiz-prompt", "Prompt B", TIMEOUT).await?;
    if h.eval::<bool>("!!document.querySelector('#quiz-solution')")
        .await?
    {
        return Err(Error::message(
            "after rating, the next card must show the prompt only",
        ));
    }
    Ok(())
}

/// (b) Revealing adds no history entry: quiz → reveal → browser Back
/// must leave the quiz (the previous tab), not merely un-reveal the
/// card.
#[tokio::test]
#[ignore = "browser"]
async fn reveal_adds_no_history_entry() -> Result<()> {
    let h = TestHarness::start().await?;
    let (store, user_id) = seed_store(&h).await?;
    seed_card(
        &store,
        user_id,
        "card-a",
        "Prompt A",
        "Solution A",
        now_ms() - 60_000,
    )
    .await?;

    h.goto("/").await?;
    h.wait_for_selector("#show-solution", TIMEOUT).await?;
    h.click("#tab-groom").await?;
    wait_for_path(&h, "/groom").await?;
    h.wait_for_selector("#groom-search", TIMEOUT).await?;
    h.click("#tab-quiz").await?;
    wait_for_path(&h, "/quiz").await?;
    h.wait_for_selector("#show-solution", TIMEOUT).await?;

    h.click("#show-solution").await?;
    h.wait_for_selector("#rate-ok", TIMEOUT).await?;
    wait_for_path(&h, "/quiz").await?;

    // Back: the stack is [/quiz, /groom, /quiz], so this lands on
    // Groom — the reveal added no entry of its own.
    let _: bool = h.eval("(() => { history.back(); return true; })()").await?;
    wait_for_path(&h, "/groom").await?;
    h.wait_for_selector("#groom-search", TIMEOUT).await?;
    if h.eval::<bool>("!!document.querySelector('#show-solution, #rate-ok')")
        .await?
    {
        return Err(Error::message(
            "Back after revealing must leave the quiz, not un-reveal it",
        ));
    }
    Ok(())
}

/// (c) The groom editor survives a reload WITH its draft: open Edit,
/// type, wait for the autosave, reload at `/groom/edit/{id}` — the
/// editor re-opens with the TYPED text (not the original card content)
/// and no recovery banner; Save persists the typed text.
#[tokio::test]
#[ignore = "browser"]
async fn editor_survives_reload_with_draft() -> Result<()> {
    let h = TestHarness::start().await?;
    let (store, user_id) = seed_store(&h).await?;
    seed_card(
        &store,
        user_id,
        "card-edit",
        "Seed prompt",
        "Seed solution",
        now_ms() + 60_000,
    )
    .await?;

    h.goto("/groom").await?;
    h.wait_for_selector("#edit-card-edit", TIMEOUT).await?;
    h.click("#edit-card-edit").await?;
    h.wait_for_selector("#editor-prompt", TIMEOUT).await?;
    append_text(&h, "#editor-prompt", " v2").await?;
    h.wait_for_text("#draft-indicator", "draft saved", AUTOSAVE_TIMEOUT)
        .await?;
    wait_for_path(&h, "/groom/edit/card-edit").await?;

    // Reload at the editor URL: the editor re-opens prefilled with the
    // draft (the typed text), and the banner stays away — the draft is
    // recovered inline.
    h.goto("/groom/edit/card-edit").await?;
    h.wait_for_selector("#editor-prompt", TIMEOUT).await?;
    let prompt = field_value(&h, "#editor-prompt").await?;
    if prompt != "Seed prompt v2" {
        return Err(Error::message(format!(
            "restored editor should hold the typed draft, shows: {prompt:?}"
        )));
    }
    let solution = field_value(&h, "#editor-solution").await?;
    if solution != "Seed solution" {
        return Err(Error::message(format!(
            "restored editor should hold the draft solution, shows: {solution:?}"
        )));
    }
    wait_for_path(&h, "/groom/edit/card-edit").await?;
    assert_absent(&h, "#recovery-banner").await?;
    h.screenshot("09_state_restore/editor-restored-with-draft")
        .await?;

    // Save persists the typed text.
    h.click("#editor-save").await?;
    h.wait_for_selector("#groom-search", TIMEOUT).await?;
    h.wait_for_text("#groom-results", "Seed prompt v2", TIMEOUT)
        .await?;
    let card = store
        .get_card(user_id, "card-edit")
        .await
        .map_err(store_err)?
        .ok_or_else(|| Error::message("card-edit vanished"))?;
    if card.prompt != "Seed prompt v2" {
        return Err(Error::message(format!(
            "store prompt should be the typed text, is {:?}",
            card.prompt
        )));
    }
    Ok(())
}

/// (d) Like (c), but the card is deleted before the reload: the edit
/// URL's fetch 404s, the app falls back to the Groom tab (URL `/groom`)
/// and the orphaned draft still prompts the recovery banner.
#[tokio::test]
#[ignore = "browser"]
async fn editor_reload_deleted_card_falls_back() -> Result<()> {
    let h = TestHarness::start().await?;
    let (store, user_id) = seed_store(&h).await?;
    seed_card(
        &store,
        user_id,
        "card-doom",
        "Doom prompt",
        "Doom solution",
        now_ms() + 60_000,
    )
    .await?;

    h.goto("/groom").await?;
    h.wait_for_selector("#edit-card-doom", TIMEOUT).await?;
    h.click("#edit-card-doom").await?;
    h.wait_for_selector("#editor-prompt", TIMEOUT).await?;
    append_text(&h, "#editor-prompt", " v2").await?;
    h.wait_for_text("#draft-indicator", "draft saved", AUTOSAVE_TIMEOUT)
        .await?;
    wait_for_path(&h, "/groom/edit/card-doom").await?;

    // White-box: the card vanishes while the draft survives.
    if !store
        .delete_card(user_id, "card-doom")
        .await
        .map_err(store_err)?
    {
        return Err(Error::message("seeded card-doom should have been deleted"));
    }

    // Reload at the now-dangling edit URL: 404 → Groom tab, URL
    // rewritten to /groom, and the draft prompts the banner.
    h.goto("/groom/edit/card-doom").await?;
    h.wait_for_selector("#groom-search", TIMEOUT).await?;
    wait_for_path(&h, "/groom").await?;
    h.wait_for_selector("#recovery-banner", TIMEOUT).await?;
    if h.eval::<bool>("!!document.querySelector('#editor-prompt')")
        .await?
    {
        return Err(Error::message(
            "the editor must not open for a deleted card",
        ));
    }
    h.screenshot("09_state_restore/deleted-card-banner").await?;
    Ok(())
}

/// (e) The Add card tab's editor is prefilled with a matching
/// (new-card) draft on a fresh load of `/add` — no recovery banner.
#[tokio::test]
#[ignore = "browser"]
async fn add_tab_draft_prefill() -> Result<()> {
    let h = TestHarness::start().await?;

    h.goto("/add").await?;
    h.wait_for_selector("#new-prompt", TIMEOUT).await?;
    h.type_into("#new-prompt", "Add draft prompt").await?;
    h.type_into("#new-solution", "Add draft solution").await?;
    h.wait_for_text("#draft-indicator", "draft saved", AUTOSAVE_TIMEOUT)
        .await?;

    // Reload at /add: the fields hold the draft, the banner stays away.
    h.goto("/add").await?;
    h.wait_for_selector("#new-prompt", TIMEOUT).await?;
    let prompt = field_value(&h, "#new-prompt").await?;
    if prompt != "Add draft prompt" {
        return Err(Error::message(format!(
            "Add tab should be prefilled with the draft, prompt shows: {prompt:?}"
        )));
    }
    let solution = field_value(&h, "#new-solution").await?;
    if solution != "Add draft solution" {
        return Err(Error::message(format!(
            "Add tab should be prefilled with the draft, solution shows: {solution:?}"
        )));
    }
    assert_absent(&h, "#recovery-banner").await?;
    h.screenshot("09_state_restore/add-tab-prefilled").await?;
    Ok(())
}

/// (f) A draft for an existing card, unrelated to the loaded route,
/// still prompts the banner; Recover opens the editor with the draft.
#[tokio::test]
#[ignore = "browser"]
async fn unrelated_draft_still_banners() -> Result<()> {
    let h = TestHarness::start().await?;
    let (store, user_id) = seed_store(&h).await?;
    seed_card(
        &store,
        user_id,
        "card-x",
        "X prompt",
        "X solution",
        now_ms() + 60_000,
    )
    .await?;
    store
        .put_autosave(user_id, Some("card-x"), "Draft P", "Draft S", now_ms())
        .await
        .map_err(store_err)?;

    // Fresh load at /quiz: the draft matches no restored editor, so
    // the banner is the recovery surface.
    h.goto("/quiz").await?;
    h.wait_for_selector("#recovery-banner", TIMEOUT).await?;
    h.click("#recover-draft").await?;
    h.wait_for_selector("#editor-prompt", TIMEOUT).await?;
    let prompt = field_value(&h, "#editor-prompt").await?;
    if prompt != "Draft P" {
        return Err(Error::message(format!(
            "recovered editor should hold the draft, shows: {prompt:?}"
        )));
    }
    let solution = field_value(&h, "#editor-solution").await?;
    if solution != "Draft S" {
        return Err(Error::message(format!(
            "recovered editor should hold the draft solution, shows: {solution:?}"
        )));
    }
    // The recovered edit of an existing card is a real route.
    wait_for_path(&h, "/groom/edit/card-x").await?;
    h.screenshot("09_state_restore/banner-recovered-editor")
        .await?;
    Ok(())
}

/// (g) An unauthenticated fresh load of `/groom/edit/{id}` shows the
/// auth screen; after register + login the editor for that card opens
/// (the deep link survives the auth flow).
#[tokio::test]
#[ignore = "browser"]
async fn auth_mode_editor_deep_link() -> Result<()> {
    let h = TestHarness::start_with_auth().await?;
    h.add_virtual_authenticator().await?;

    // Seed the claimable user and its card before the first run (the
    // open bootstrap claims an existing passkey-less user by name).
    let store = h.seed_store().await.map_err(store_err)?;
    let user = store.upsert_user("deeplink").await.map_err(store_err)?;
    seed_card(
        &store,
        user.id,
        "card-deep",
        "Deep prompt",
        "Deep solution",
        now_ms() + 60_000,
    )
    .await?;

    h.goto("/groom/edit/card-deep").await?;
    // First run: the register variant of the auth screen.
    h.wait_for_selector("#register-username", TIMEOUT).await?;
    h.type_into("#register-username", "deeplink").await?;
    h.click("#create-passkey").await?;
    h.wait_for_selector("#sign-in", TIMEOUT).await?;
    h.click("#sign-in").await?;

    // Lands in the editor for the deep-linked card, not on a bare tab.
    h.wait_for_selector("#editor-prompt", TIMEOUT).await?;
    let prompt = field_value(&h, "#editor-prompt").await?;
    if prompt != "Deep prompt" {
        return Err(Error::message(format!(
            "editor should open on the deep-linked card, prompt shows: {prompt:?}"
        )));
    }
    wait_for_path(&h, "/groom/edit/card-deep").await?;
    h.screenshot("09_state_restore/auth-deep-link-editor")
        .await?;
    Ok(())
}

/// (h) Back/Forward ONTO an editor URL must re-open the editor on the
/// same card (F1) — not flatten to the groom list while the URL still
/// says `/groom/edit/{id}`: open Edit, Back (closes just the overlay),
/// Forward (re-opens the editor).
#[tokio::test]
#[ignore = "browser"]
async fn back_forward_onto_editor_reopens_it() -> Result<()> {
    let h = TestHarness::start().await?;
    let (store, user_id) = seed_store(&h).await?;
    seed_card(
        &store,
        user_id,
        "card-nav",
        "Nav prompt",
        "Nav solution",
        now_ms() + 60_000,
    )
    .await?;

    h.goto("/groom").await?;
    h.wait_for_selector("#edit-card-nav", TIMEOUT).await?;
    h.click("#edit-card-nav").await?;
    h.wait_for_selector("#editor-prompt", TIMEOUT).await?;
    wait_for_path(&h, "/groom/edit/card-nav").await?;

    // Back pops the overlay's entry: editor closed, groom list shown.
    let _: bool = h.eval("(() => { history.back(); return true; })()").await?;
    wait_for_path(&h, "/groom").await?;
    h.wait_for_selector("#groom-search", TIMEOUT).await?;
    assert_absent(&h, "#editor-prompt").await?;

    // Forward lands back ON the editor URL: the editor re-opens on the
    // same card, prefilled with the card content.
    let _: bool = h
        .eval("(() => { history.forward(); return true; })()")
        .await?;
    wait_for_path(&h, "/groom/edit/card-nav").await?;
    h.wait_for_selector("#editor-prompt", TIMEOUT).await?;
    let prompt = field_value(&h, "#editor-prompt").await?;
    if prompt != "Nav prompt" {
        return Err(Error::message(format!(
            "Forward onto the editor URL should re-open the editor on card-nav, \
             prompt shows: {prompt:?}"
        )));
    }
    h.screenshot("09_state_restore/forward-reopens-editor")
        .await?;
    Ok(())
}

/// (i) Back from another tab ONTO the editor URL also re-opens the
/// editor (F1): open Edit, switch to the quiz tab, Back.
#[tokio::test]
#[ignore = "browser"]
async fn back_from_other_tab_reopens_editor() -> Result<()> {
    let h = TestHarness::start().await?;
    let (store, user_id) = seed_store(&h).await?;
    seed_card(
        &store,
        user_id,
        "card-tab",
        "Tab prompt",
        "Tab solution",
        now_ms() + 60_000,
    )
    .await?;

    h.goto("/groom").await?;
    h.wait_for_selector("#edit-card-tab", TIMEOUT).await?;
    h.click("#edit-card-tab").await?;
    h.wait_for_selector("#editor-prompt", TIMEOUT).await?;
    wait_for_path(&h, "/groom/edit/card-tab").await?;

    // Tab switch away (no typing, so no draft is orphaned).
    h.click("#tab-quiz").await?;
    wait_for_path(&h, "/quiz").await?;
    h.wait_for_selector("#quiz-done, #quiz-prompt", TIMEOUT)
        .await?;
    assert_absent(&h, "#editor-prompt").await?;

    // Back onto the editor URL: the editor re-opens on the same card.
    let _: bool = h.eval("(() => { history.back(); return true; })()").await?;
    wait_for_path(&h, "/groom/edit/card-tab").await?;
    h.wait_for_selector("#editor-prompt", TIMEOUT).await?;
    let prompt = field_value(&h, "#editor-prompt").await?;
    if prompt != "Tab prompt" {
        return Err(Error::message(format!(
            "Back onto the editor URL should re-open the editor on card-tab, \
             prompt shows: {prompt:?}"
        )));
    }
    Ok(())
}

/// (j) A stale `/quiz/solution` link (the retired reveal route) must
/// NOT restore a revealed quiz: it falls back to the quiz tab like any
/// unknown path and shows the prompt collapsed.
#[tokio::test]
#[ignore = "browser"]
async fn legacy_solution_url_starts_collapsed() -> Result<()> {
    let h = TestHarness::start().await?;
    let (store, user_id) = seed_store(&h).await?;
    seed_card(
        &store,
        user_id,
        "card-a",
        "Prompt A",
        "Solution A",
        now_ms() - 60_000,
    )
    .await?;

    h.goto("/quiz/solution").await?;
    h.wait_for_text("#quiz-prompt", "Prompt A", TIMEOUT).await?;
    h.wait_for_selector("#show-solution", TIMEOUT).await?;
    if h.eval::<bool>("!!document.querySelector('#quiz-solution, #rate-ok, #rate-failed')")
        .await?
    {
        return Err(Error::message(
            "a stale /quiz/solution link must not show the solution",
        ));
    }
    Ok(())
}

/// (l) Tab-switching away and back while revealed also collapses the
/// quiz (the tab remounts, the reveal is in-memory only) and the card
/// stays unrated — it is still the next due card.
#[tokio::test]
#[ignore = "browser"]
async fn tab_switch_collapses_reveal() -> Result<()> {
    let h = TestHarness::start().await?;
    let (store, user_id) = seed_store(&h).await?;
    seed_card(
        &store,
        user_id,
        "card-a",
        "Prompt A",
        "Solution A",
        now_ms() - 60_000,
    )
    .await?;

    h.goto("/quiz").await?;
    h.wait_for_text("#quiz-prompt", "Prompt A", TIMEOUT).await?;
    h.click("#show-solution").await?;
    h.wait_for_text("#quiz-solution", "Solution A", TIMEOUT)
        .await?;

    h.click("#tab-groom").await?;
    h.wait_for_selector("#groom-search", TIMEOUT).await?;
    h.click("#tab-quiz").await?;
    h.wait_for_text("#quiz-prompt", "Prompt A", TIMEOUT).await?;
    h.wait_for_selector("#show-solution", TIMEOUT).await?;
    if h.eval::<bool>("!!document.querySelector('#quiz-solution, #rate-ok, #rate-failed')")
        .await?
    {
        return Err(Error::message(
            "a tab switch must collapse the quiz back to the prompt",
        ));
    }

    // White-box: the card was never rated.
    let card = store
        .get_card(user_id, "card-a")
        .await
        .map_err(store_err)?
        .ok_or_else(|| Error::message("card-a vanished"))?;
    if card.state != CardState::New {
        return Err(Error::message(format!(
            "the card must stay unrated, state is {:?}",
            card.state
        )));
    }
    Ok(())
}

/// (k) A mid-session 401 on the Add tab must not orphan the draft (F2):
/// after re-login the first-load restore gate has re-engaged, so /add
/// mounts PREFILLED with the draft and no recovery banner. The expiry
/// is simulated by deleting the live session server-side (identified
/// via the browser's cookie) and letting the next autosave tick hit the
/// 401 — the natural "session expired" path, just faster.
#[tokio::test]
#[ignore = "browser"]
async fn relogin_on_add_restores_draft_prefill() -> Result<()> {
    let h = TestHarness::start_with_auth().await?;
    h.add_virtual_authenticator().await?;

    // First run, straight onto /add: register + login.
    h.goto("/add").await?;
    h.wait_for_selector("#register-username", TIMEOUT).await?;
    h.type_into("#register-username", "relogin").await?;
    h.click("#create-passkey").await?;
    h.wait_for_selector("#sign-in", TIMEOUT).await?;
    h.click("#sign-in").await?;
    h.wait_for_selector("#new-prompt", TIMEOUT).await?;

    // Type a draft and wait for the autosave round-trip.
    h.type_into("#new-prompt", "Relogin draft prompt").await?;
    h.wait_for_text("#draft-indicator", "draft saved", AUTOSAVE_TIMEOUT)
        .await?;

    // The session "expires": delete its row server-side.
    let token = h
        .session_token()
        .await?
        .ok_or_else(|| Error::message("no session cookie after login"))?;
    let store = h.seed_store().await.map_err(store_err)?;
    if !store.delete_session(&token).await.map_err(store_err)? {
        return Err(Error::message("the live session should have been deleted"));
    }

    // Dirty the editor so the next autosave tick PUTs, hits the 401 and
    // bounces the app to the auth screen.
    h.type_into("#new-prompt", "!").await?;
    h.wait_for_selector("#sign-in", AUTOSAVE_TIMEOUT).await?;

    // Re-login: /add must come back prefilled with the orphaned draft
    // (the "!" was never saved), and the banner must stay away — the
    // draft is recovered inline, not prompted and not invisible.
    h.click("#sign-in").await?;
    h.wait_for_selector("#new-prompt", TIMEOUT).await?;
    let prompt = field_value(&h, "#new-prompt").await?;
    if prompt != "Relogin draft prompt" {
        return Err(Error::message(format!(
            "after re-login the Add editor should be prefilled with the draft, \
             shows: {prompt:?}"
        )));
    }
    assert_absent(&h, "#recovery-banner").await?;
    h.screenshot("09_state_restore/relogin-add-prefilled")
        .await?;
    Ok(())
}
