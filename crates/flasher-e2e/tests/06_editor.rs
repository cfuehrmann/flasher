//! Card editor e2e tests (Phase 4C): the split-pane editor with live
//! Markdown preview, the 5 s autosave draft with its indicator, and the
//! crash-recovery banner — all click-driven through the browser, with
//! the database only used for seeding and white-box verification (and,
//! in one test, for deleting the edited card out from under a draft).

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

/// Seeds one enabled state=new card due in the future (never quizzable
/// during the test, so the Quiz tab stays out of the way).
async fn seed_card(
    store: &Store,
    user_id: i64,
    id: &str,
    prompt: &str,
    solution: &str,
) -> Result<()> {
    let now = now_ms();
    store
        .insert_card(&NewCard {
            user_id,
            id: id.to_owned(),
            prompt: prompt.to_owned(),
            solution: solution.to_owned(),
            state: CardState::New,
            change_time: now,
            next_time: now + 60_000,
            labels: vec!["Enabled".to_owned()],
        })
        .await
        .map_err(store_err)
}

/// The `.value` of a textarea/input.
async fn field_value(h: &TestHarness, sel: &str) -> Result<String> {
    h.eval::<String>(&format!("document.querySelector({sel:?}).value"))
        .await
}

/// Appends text to a single-line field like a user: click to focus, `End`
/// to jump past the prefilled content, then type.
async fn append_text(h: &TestHarness, sel: &str, text: &str) -> Result<()> {
    h.click(sel).await?;
    let el = h.page.find_element(sel).await.map_err(Error::Cdp)?;
    el.press_key("End").await.map_err(Error::Cdp)?;
    el.type_str(text).await.map_err(Error::Cdp)?;
    Ok(())
}

/// Polls a JS boolean expression until it holds or the deadline elapses.
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

/// Types prompt+solution into the new-card editor and waits for the 5 s
/// autosave to persist the draft.
async fn make_new_card_draft(h: &TestHarness, prompt: &str, solution: &str) -> Result<()> {
    h.goto("/").await?;
    h.click("#tab-add-card").await?;
    h.wait_for_selector("#new-prompt", TIMEOUT).await?;
    h.type_into("#new-prompt", prompt).await?;
    h.type_into("#new-solution", solution).await?;
    h.wait_for_text("#draft-indicator", "draft saved", AUTOSAVE_TIMEOUT)
        .await
}

/// Mints a label in the new-card editor's picker (mandatory since
/// 2026-08-01: cards get their labels at creation time) so Create
/// enables.
async fn mint_editor_label(h: &TestHarness, name: &str) -> Result<()> {
    h.type_into("#new-label-input", name).await?;
    h.click("#new-label-add").await?;
    h.wait_for_selector(&format!("#new-label-{name}"), TIMEOUT)
        .await
}

/// Editing an existing card: prefill, live preview of typed Markdown,
/// autosave indicator, Save back to Groom — and the draft is gone
/// server-side afterwards (PATCH deletes it).
#[tokio::test]
#[ignore = "browser"]
async fn edit_existing_card_with_preview() -> Result<()> {
    let h = TestHarness::start().await?;
    let (store, user_id) = seed_store(&h).await?;
    seed_card(&store, user_id, "card-edit", "Seed prompt", "Seed solution").await?;

    h.goto("/").await?;
    h.click("#tab-groom").await?;
    h.wait_for_selector("#edit-card-edit", TIMEOUT).await?;
    h.click("#edit-card-edit").await?;
    h.wait_for_selector("#editor-prompt", TIMEOUT).await?;

    // Pre-filled from the card.
    if field_value(&h, "#editor-prompt").await? != "Seed prompt" {
        return Err(Error::message(format!(
            "editor should be pre-filled, prompt shows: {:?}",
            field_value(&h, "#editor-prompt").await?
        )));
    }
    if field_value(&h, "#editor-solution").await? != "Seed solution" {
        return Err(Error::message("editor solution should be pre-filled"));
    }

    // Typing Markdown renders it live in the preview pane.
    append_text(&h, "#editor-prompt", " with **bold edit**").await?;
    wait_for_js(
        &h,
        "document.querySelector('#editor-preview-prompt').innerHTML.includes('<strong>bold edit</strong>')",
        TIMEOUT,
    )
    .await?;
    h.wait_for_text("#draft-indicator", "unsaved changes", TIMEOUT)
        .await?;
    h.screenshot("06_editor/edit-split-view").await?;

    // The 5 s autosave flips the indicator to "draft saved HH:MM:SS".
    h.wait_for_text("#draft-indicator", "draft saved", AUTOSAVE_TIMEOUT)
        .await?;
    // Taller viewport so the indicator/Save bar above the fold is captured.
    h.set_viewport(1400, 1100).await?;
    h.screenshot("06_editor/draft-saved").await?;

    // Save → back to Groom with the updated prompt in the list.
    h.click("#editor-save").await?;
    h.wait_for_selector("#groom-search", TIMEOUT).await?;
    h.wait_for_text("#groom-results", "bold edit", TIMEOUT)
        .await?;
    h.screenshot("06_editor/back-in-groom").await?;

    let card = store
        .get_card(user_id, "card-edit")
        .await
        .map_err(store_err)?
        .ok_or_else(|| Error::message("card-edit vanished"))?;
    if !card.prompt.contains("**bold edit**") {
        return Err(Error::message(format!(
            "store prompt should contain the edit, is {:?}",
            card.prompt
        )));
    }
    if store
        .get_autosave(user_id)
        .await
        .map_err(store_err)?
        .is_some()
    {
        return Err(Error::message(
            "autosave draft should be deleted by the content PATCH",
        ));
    }
    Ok(())
}

/// The round-trip: draft a new card, reload (crash), recover from the
/// banner, save — the card lands in the store and the draft is gone.
#[tokio::test]
#[ignore = "browser"]
async fn autosave_and_recovery_roundtrip() -> Result<()> {
    let h = TestHarness::start().await?;
    make_new_card_draft(&h, "Draft prompt", "Draft solution").await?;
    // Taller viewport so the draft indicator above the fold is captured.
    h.set_viewport(1400, 1100).await?;
    h.screenshot("06_editor/new-card-draft-saved").await?;

    // Crash simulation: a full reload loses all client state.
    h.goto("/").await?;
    h.wait_for_selector("#recovery-banner", TIMEOUT).await?;
    let banner = h.text_content("#recovery-banner").await?;
    if !banner.contains("unsaved draft") {
        return Err(Error::message(format!(
            "banner should mention the unsaved draft, shows: {banner:?}"
        )));
    }
    h.screenshot("06_editor/recovery-banner").await?;

    h.click("#recover-draft").await?;
    h.wait_for_selector("#new-prompt", TIMEOUT).await?;
    if field_value(&h, "#new-prompt").await? != "Draft prompt" {
        return Err(Error::message(format!(
            "recovered editor should hold the draft prompt, shows: {:?}",
            field_value(&h, "#new-prompt").await?
        )));
    }
    if field_value(&h, "#new-solution").await? != "Draft solution" {
        return Err(Error::message(
            "recovered editor should hold the draft solution",
        ));
    }
    h.screenshot("06_editor/recovered-editor").await?;

    mint_editor_label(&h, "recovered").await?;
    h.click("#create-card").await?;
    h.wait_for_text("#add-card-confirmation", "Card created", TIMEOUT)
        .await?;

    let (store, user_id) = seed_store(&h).await?;
    let (cards, count) = store
        .search_cards(user_id, Some("Draft prompt"), None, 0, 10)
        .await
        .map_err(store_err)?;
    if count != 1 || cards.first().map(|c| c.solution.as_str()) != Some("Draft solution") {
        return Err(Error::message(format!(
            "expected exactly the recovered card, count={count}"
        )));
    }
    if store
        .get_autosave(user_id)
        .await
        .map_err(store_err)?
        .is_some()
    {
        return Err(Error::message("draft should be deleted after create"));
    }
    Ok(())
}

/// Cancel abandons the session AND deletes the draft (reference-app
/// Cancel/Abandon semantics): the store has no autosave afterwards and a
/// reload shows no recovery banner.
#[tokio::test]
#[ignore = "browser"]
async fn cancel_deletes_draft() -> Result<()> {
    let h = TestHarness::start().await?;
    make_new_card_draft(&h, "Cancelled prompt", "Cancelled solution").await?;

    // The editor closes only after the DELETE landed.
    h.click("#editor-cancel").await?;
    h.wait_for_text("#quiz-done", "All done", TIMEOUT).await?;

    let (store, user_id) = seed_store(&h).await?;
    if store
        .get_autosave(user_id)
        .await
        .map_err(store_err)?
        .is_some()
    {
        return Err(Error::message("cancel should delete the draft"));
    }

    // A fresh start must not prompt for recovery.
    h.goto("/").await?;
    h.wait_for_text("#quiz-done", "All done", TIMEOUT).await?;
    // Give the one-shot draft check a beat to (not) show the banner.
    tokio::time::sleep(Duration::from_millis(500)).await;
    if h.eval::<bool>("!!document.querySelector('#recovery-banner')")
        .await?
    {
        return Err(Error::message("banner must not appear after cancel"));
    }
    Ok(())
}

/// Discard: the banner disappears, the draft is deleted server-side, and
/// the next reload does not prompt again.
#[tokio::test]
#[ignore = "browser"]
async fn discard_recovery() -> Result<()> {
    let h = TestHarness::start().await?;
    make_new_card_draft(&h, "Doomed draft", "Doomed solution").await?;

    h.goto("/").await?;
    h.wait_for_selector("#recovery-banner", TIMEOUT).await?;
    h.click("#discard-draft").await?;
    wait_until_gone(&h, "#recovery-banner", TIMEOUT).await?;

    let (store, user_id) = seed_store(&h).await?;
    if store
        .get_autosave(user_id)
        .await
        .map_err(store_err)?
        .is_some()
    {
        return Err(Error::message("discard should delete the draft"));
    }

    // A fresh start with no draft must not prompt.
    h.goto("/").await?;
    h.wait_for_text("#quiz-done", "All done", TIMEOUT).await?;
    // Give the one-shot draft check a beat to (not) show the banner.
    tokio::time::sleep(Duration::from_millis(500)).await;
    if h.eval::<bool>("!!document.querySelector('#recovery-banner')")
        .await?
    {
        return Err(Error::message("banner must not reappear after discard"));
    }
    Ok(())
}

/// The edited card was deleted after the draft was written: recovery
/// falls back to new-card mode with the draft text, and saving creates a
/// brand-new card.
#[tokio::test]
#[ignore = "browser"]
async fn recover_deleted_card_falls_back_to_new() -> Result<()> {
    let h = TestHarness::start().await?;
    let (store, user_id) = seed_store(&h).await?;
    seed_card(&store, user_id, "card-doom", "Doom prompt", "Doom solution").await?;

    h.goto("/").await?;
    h.click("#tab-groom").await?;
    h.wait_for_selector("#edit-card-doom", TIMEOUT).await?;
    h.click("#edit-card-doom").await?;
    h.wait_for_selector("#editor-prompt", TIMEOUT).await?;
    append_text(&h, "#editor-prompt", " v2").await?;
    h.wait_for_text("#draft-indicator", "draft saved", AUTOSAVE_TIMEOUT)
        .await?;

    // White-box: the card vanishes while the draft survives.
    if !store
        .delete_card(user_id, "card-doom")
        .await
        .map_err(store_err)?
    {
        return Err(Error::message("seeded card-doom should have been deleted"));
    }

    h.goto("/").await?;
    h.wait_for_selector("#recovery-banner", TIMEOUT).await?;
    h.click("#recover-draft").await?;

    // New-card mode (the card is gone), but the draft text is kept.
    h.wait_for_text("#editor-heading", "New card", TIMEOUT)
        .await?;
    if field_value(&h, "#new-prompt").await? != "Doom prompt v2" {
        return Err(Error::message(format!(
            "fallback editor should hold the draft text, shows: {:?}",
            field_value(&h, "#new-prompt").await?
        )));
    }
    h.screenshot("06_editor/recovered-as-new-card").await?;

    mint_editor_label(&h, "doomed").await?;
    h.click("#create-card").await?;
    h.wait_for_text("#add-card-confirmation", "Card created", TIMEOUT)
        .await?;

    if store
        .get_card(user_id, "card-doom")
        .await
        .map_err(store_err)?
        .is_some()
    {
        return Err(Error::message("card-doom must stay deleted"));
    }
    let (cards, count) = store
        .search_cards(user_id, Some("Doom prompt"), None, 0, 10)
        .await
        .map_err(store_err)?;
    if count != 1 || cards.first().map(|c| c.prompt.as_str()) != Some("Doom prompt v2") {
        return Err(Error::message(format!(
            "expected one new card with the draft content, count={count}"
        )));
    }
    if store
        .get_autosave(user_id)
        .await
        .map_err(store_err)?
        .is_some()
    {
        return Err(Error::message("draft should be deleted after create"));
    }
    Ok(())
}

/// A fresh database has no draft, so no recovery banner appears.
#[tokio::test]
#[ignore = "browser"]
async fn no_draft_no_banner() -> Result<()> {
    let h = TestHarness::start().await?;
    h.goto("/").await?;
    h.wait_for_text("#quiz-done", "All done", TIMEOUT).await?;
    // Give the one-shot draft check a beat to (not) show the banner.
    tokio::time::sleep(Duration::from_millis(500)).await;
    if h.eval::<bool>("!!document.querySelector('#recovery-banner')")
        .await?
    {
        return Err(Error::message("no draft, so no banner expected"));
    }
    Ok(())
}

/// Browser Back while editing closes just the editor overlay — the app
/// stays on the Groom tab (the overlay's `<tab>/edit` history entry is
/// popped, nothing else). That close was neither Save nor Cancel, so the
/// orphaned server-side draft re-arms the recovery banner immediately
/// (no reload needed), and Recover opens the editor with the draft text.
#[tokio::test]
#[ignore = "browser"]
async fn back_closes_editor_and_rearms_recovery_banner() -> Result<()> {
    let h = TestHarness::start().await?;
    let (store, user_id) = seed_store(&h).await?;
    seed_card(
        &store,
        user_id,
        "card-orphan",
        "Orphan prompt",
        "Orphan solution",
    )
    .await?;

    h.goto("/").await?;
    h.click("#tab-groom").await?;
    h.wait_for_selector("#edit-card-orphan", TIMEOUT).await?;
    h.click("#edit-card-orphan").await?;
    h.wait_for_selector("#editor-prompt", TIMEOUT).await?;
    // The overlay pushed its own history entry on top of the tab path
    // (Phase 6.6: the real, reload-safe per-card edit route).
    wait_for_path(&h, "/groom/edit/card-orphan").await?;

    append_text(&h, "#editor-prompt", " v2").await?;
    h.wait_for_text("#draft-indicator", "draft saved", AUTOSAVE_TIMEOUT)
        .await?;

    // Browser Back: the editor closes, the tab is NOT left.
    let _: bool = h.eval("(() => { history.back(); return true; })()").await?;
    wait_for_path(&h, "/groom").await?;
    h.wait_for_selector("#groom-search", TIMEOUT).await?;
    wait_until_gone(&h, "#editor-prompt", TIMEOUT).await?;

    // The orphaned draft re-arms the banner without a reload; Recover
    // brings the draft text back into the editor (edit mode: the card
    // still exists).
    h.wait_for_selector("#recovery-banner", TIMEOUT).await?;
    h.screenshot("06_editor/orphaned-draft-banner").await?;
    h.click("#recover-draft").await?;
    h.wait_for_selector("#editor-prompt", TIMEOUT).await?;
    let recovered = field_value(&h, "#editor-prompt").await?;
    if recovered != "Orphan prompt v2" {
        return Err(Error::message(format!(
            "recovered editor should hold the orphaned draft, shows: {recovered:?}"
        )));
    }
    Ok(())
}
