//! Browser journeys for target-scoped card drafts and the shared editor.

use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use flasher_e2e::{E2E_USER, Error, Result, TestHarness};
use flasher_store::{CardState, NewCard, Store};

const TIMEOUT: Duration = Duration::from_secs(15);
const AUTOSAVE_TIMEOUT: Duration = Duration::from_secs(15);

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| {
            i64::try_from(duration.as_millis()).unwrap_or(0)
        })
}

#[allow(clippy::needless_pass_by_value)]
fn store_err(err: flasher_store::Error) -> Error {
    Error::message(format!("store error: {err}"))
}

async fn seed_store(h: &TestHarness) -> Result<(Store, i64)> {
    let store = h.seed_store().await.map_err(store_err)?;
    let user = store
        .get_user_by_name(E2E_USER)
        .await
        .map_err(store_err)?
        .ok_or_else(|| Error::message("e2e user missing"))?;
    Ok((store, user.id))
}

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
            labels: vec!["A".to_owned()],
        })
        .await
        .map_err(store_err)
}

async fn field_value(h: &TestHarness, selector: &str) -> Result<String> {
    h.eval::<String>(&format!("document.querySelector({selector:?}).value"))
        .await
}

async fn append_text(h: &TestHarness, selector: &str, text: &str) -> Result<()> {
    h.click(selector).await?;
    let element = h.page.find_element(selector).await.map_err(Error::Cdp)?;
    element.press_key("End").await.map_err(Error::Cdp)?;
    element.type_str(text).await.map_err(Error::Cdp)?;
    Ok(())
}

async fn wait_for_js(h: &TestHarness, expression: &str) -> Result<()> {
    let deadline = Instant::now() + TIMEOUT;
    loop {
        if h.eval::<bool>(expression).await.unwrap_or(false) {
            return Ok(());
        }
        if Instant::now() >= deadline {
            return Err(Error::message(format!(
                "timed out waiting for {expression}"
            )));
        }
        tokio::task::yield_now().await;
    }
}

async fn field_checked(h: &TestHarness, selector: &str) -> Result<bool> {
    h.eval::<bool>(&format!("document.querySelector({selector:?}).checked"))
        .await
}

async fn button_disabled(h: &TestHarness, selector: &str) -> Result<bool> {
    h.eval::<bool>(&format!("document.querySelector({selector:?}).disabled"))
        .await
}

async fn seed_labels(store: &Store, user_id: i64) -> Result<()> {
    store.create_label(user_id, "A").await.map_err(store_err)?;
    store.create_label(user_id, "B").await.map_err(store_err)?;
    Ok(())
}

#[tokio::test]
#[ignore = "browser"]
async fn editor_requires_prompt_and_at_least_one_label() -> Result<()> {
    let h = TestHarness::start().await?;
    let (store, user_id) = seed_store(&h).await?;
    seed_labels(&store, user_id).await?;
    seed_card(
        &store,
        user_id,
        "card-validation",
        "Existing prompt",
        "Existing solution",
        now_ms() + 60_000,
    )
    .await?;

    h.goto("/add").await?;
    h.wait_for_selector("#new-prompt", TIMEOUT).await?;
    if !button_disabled(&h, "#create-card").await? {
        return Err(Error::message(
            "Create must be disabled while the prompt is empty",
        ));
    }
    if !h
        .eval::<bool>(
            "document.querySelector('#editor-label-requirement').textContent.includes('Choose at least one label.')",
        )
        .await?
    {
        return Err(Error::message(
            "Add card does not explain that at least one label is required",
        ));
    }
    h.type_into("#new-prompt", "A prompt").await?;
    if !button_disabled(&h, "#create-card").await? {
        return Err(Error::message(
            "Create must remain disabled until a label is selected",
        ));
    }
    h.click("#editor-label-A").await?;
    if button_disabled(&h, "#create-card").await? {
        return Err(Error::message(
            "Create should enable after a non-empty prompt and label",
        ));
    }
    h.click("#editor-discard").await?;
    h.wait_for_text("#quiz-done", "All done", TIMEOUT).await?;

    h.goto("/groom").await?;
    h.wait_for_selector("#edit-card-validation", TIMEOUT)
        .await?;
    h.click("#edit-card-validation").await?;
    h.wait_for_selector("#editor-prompt", TIMEOUT).await?;
    h.wait_for_selector("#editor-label-A", TIMEOUT).await?;
    wait_for_js(&h, "!document.querySelector('#editor-save').disabled").await?;
    if !h
        .eval::<bool>(
            "document.querySelector('#editor-label-requirement').textContent.includes('Choose at least one label.')",
        )
        .await?
    {
        return Err(Error::message(
            "Edit card does not explain that at least one label is required",
        ));
    }
    if button_disabled(&h, "#editor-save").await? {
        return Err(Error::message("An initially valid edit should be saveable"));
    }
    h.click("#editor-label-A").await?;
    if !button_disabled(&h, "#editor-save").await? {
        return Err(Error::message(
            "Save must be disabled when all labels are cleared",
        ));
    }
    h.click("#editor-label-A").await?;
    let prompt = h
        .page
        .find_element("#editor-prompt")
        .await
        .map_err(Error::Cdp)?;
    prompt.click().await.map_err(Error::Cdp)?;
    prompt.press_key("End").await.map_err(Error::Cdp)?;
    for _ in "Existing prompt".chars() {
        prompt.press_key("Backspace").await.map_err(Error::Cdp)?;
    }
    if !button_disabled(&h, "#editor-save").await? {
        return Err(Error::message(
            "Save must be disabled when the prompt is empty",
        ));
    }
    h.click("#editor-discard").await?;
    h.wait_for_selector("#groom-search", TIMEOUT).await?;
    Ok(())
}

#[tokio::test]
#[ignore = "browser"]
async fn edit_restores_its_draft_and_saves_labels() -> Result<()> {
    let h = TestHarness::start().await?;
    let (store, user_id) = seed_store(&h).await?;
    seed_labels(&store, user_id).await?;
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
    append_text(&h, "#editor-prompt", " with **bold edit**").await?;
    h.click("#editor-label-B").await?;
    wait_for_js(
        &h,
        "document.querySelector('#editor-preview-prompt').innerHTML.includes('<strong>bold edit</strong>')",
    )
    .await?;
    h.wait_for_text("#draft-indicator", "draft saved", AUTOSAVE_TIMEOUT)
        .await?;

    let draft = store
        .get_card_edit_draft(user_id, "card-edit")
        .await
        .map_err(store_err)?
        .ok_or_else(|| Error::message("edit draft was not persisted"))?;
    if !draft.labels.contains(&"B".to_owned()) {
        return Err(Error::message(
            "edit draft did not persist the label change",
        ));
    }

    // Close retains the draft; entering the same card restores it directly.
    h.click("#editor-close").await?;
    h.wait_for_selector("#groom-search", TIMEOUT).await?;
    h.click("#edit-card-edit").await?;
    h.wait_for_selector("#editor-prompt", TIMEOUT).await?;
    h.wait_for_text("#draft-indicator", "draft saved", AUTOSAVE_TIMEOUT)
        .await?;
    if !field_value(&h, "#editor-prompt")
        .await?
        .contains("bold edit")
    {
        return Err(Error::message("Edit did not restore its own draft"));
    }
    if !field_checked(&h, "#editor-label-B").await? {
        return Err(Error::message("Edit did not restore draft labels"));
    }
    h.click("#editor-save").await?;
    h.wait_for_selector("#groom-search", TIMEOUT).await?;

    let saved = store
        .get_card(user_id, "card-edit")
        .await
        .map_err(store_err)?
        .ok_or_else(|| Error::message("edited card vanished"))?;
    if !saved.prompt.contains("bold edit") || !saved.labels.contains(&"B".to_owned()) {
        return Err(Error::message(
            "Save did not commit the shared editor fields",
        ));
    }
    if store
        .get_card_edit_draft(user_id, "card-edit")
        .await
        .map_err(store_err)?
        .is_some()
    {
        return Err(Error::message("Save did not delete the edit draft"));
    }
    Ok(())
}

#[tokio::test]
#[ignore = "browser"]
async fn closing_immediately_after_typing_persists_the_edit_draft() -> Result<()> {
    let h = TestHarness::start().await?;
    let (store, user_id) = seed_store(&h).await?;
    seed_labels(&store, user_id).await?;
    seed_card(
        &store,
        user_id,
        "card-close",
        "Seed prompt",
        "Seed solution",
        now_ms() + 60_000,
    )
    .await?;

    h.goto("/groom").await?;
    h.wait_for_selector("#edit-card-close", TIMEOUT).await?;
    h.click("#edit-card-close").await?;
    h.wait_for_selector("#editor-prompt", TIMEOUT).await?;

    // Do not wait for the 750 ms idle debounce. Close must finish the
    // in-memory snapshot's server save before unmounting the editor.
    append_text(&h, "#editor-prompt", " immediately").await?;
    h.wait_for_text("#draft-indicator", "unsaved changes", TIMEOUT)
        .await?;
    h.click("#editor-close").await?;
    h.wait_for_selector("#groom-search", TIMEOUT).await?;

    if store
        .get_card_edit_draft(user_id, "card-close")
        .await
        .map_err(store_err)?
        .is_none()
    {
        return Err(Error::message(
            "Closing a dirty editor did not persist its draft before returning",
        ));
    }

    h.click("#edit-card-close").await?;
    h.wait_for_selector("#editor-prompt", TIMEOUT).await?;
    h.wait_for_text("#draft-indicator", "draft saved", AUTOSAVE_TIMEOUT)
        .await?;
    if !field_value(&h, "#editor-prompt")
        .await?
        .contains("immediately")
    {
        return Err(Error::message(
            "Closing a dirty editor lost the pre-debounce draft",
        ));
    }
    if store
        .get_card_edit_draft(user_id, "card-close")
        .await
        .map_err(store_err)?
        .is_none()
    {
        return Err(Error::message(
            "Closing a dirty editor did not persist its draft",
        ));
    }
    h.click("#editor-discard").await?;
    h.wait_for_selector("#groom-search", TIMEOUT).await?;
    Ok(())
}

#[tokio::test]
#[ignore = "browser"]
async fn navigation_waits_for_and_then_completes_a_dirty_new_card_draft() -> Result<()> {
    let h = TestHarness::start().await?;
    let (store, user_id) = seed_store(&h).await?;
    seed_labels(&store, user_id).await?;

    h.goto("/add").await?;
    h.wait_for_selector("#new-prompt", TIMEOUT).await?;
    h.type_into("#new-prompt", "Navigate after saving").await?;
    h.wait_for_text("#draft-indicator", "unsaved changes", TIMEOUT)
        .await?;

    // One click is enough: the shell remembers the requested destination
    // while the editor completes its server-side autosave.
    h.click("#tab-groom").await?;
    h.wait_for_selector("#groom-search", AUTOSAVE_TIMEOUT)
        .await?;
    if store
        .get_new_card_draft(user_id)
        .await
        .map_err(store_err)?
        .is_none()
    {
        return Err(Error::message(
            "Dirty navigation completed without saving the new-card draft",
        ));
    }
    Ok(())
}

#[tokio::test]
#[ignore = "browser"]
async fn new_card_draft_survives_navigation_and_discard_is_explicit() -> Result<()> {
    let h = TestHarness::start().await?;
    let (store, user_id) = seed_store(&h).await?;
    seed_labels(&store, user_id).await?;

    h.goto("/add").await?;
    h.wait_for_selector("#new-prompt", TIMEOUT).await?;
    h.type_into("#new-prompt", "Draft prompt").await?;
    h.type_into("#new-solution", "Draft solution").await?;
    h.click("#editor-label-A").await?;
    h.wait_for_text("#draft-indicator", "draft saved", AUTOSAVE_TIMEOUT)
        .await?;

    // Normal navigation has no global recovery banner.
    h.click("#tab-groom").await?;
    h.wait_for_selector("#groom-search", TIMEOUT).await?;
    if h.eval::<bool>("!!document.querySelector('#recovery-banner')")
        .await?
    {
        return Err(Error::message("normal navigation must not show recovery"));
    }
    h.click("#tab-add-card").await?;
    h.wait_for_selector("#new-prompt", TIMEOUT).await?;
    h.wait_for_text("#draft-indicator", "draft saved", AUTOSAVE_TIMEOUT)
        .await?;
    if field_value(&h, "#new-prompt").await? != "Draft prompt" {
        return Err(Error::message("Add card did not restore its own draft"));
    }
    if field_value(&h, "#new-solution").await? != "Draft solution" {
        return Err(Error::message("Add card did not restore its solution"));
    }
    if field_checked(&h, "#editor-label-A").await? {
        return Err(Error::message(
            "Add-card autosave must not persist label selection",
        ));
    }

    h.click("#editor-discard").await?;
    h.wait_for_text("#quiz-done", "All done", TIMEOUT).await?;
    if store
        .get_new_card_draft(user_id)
        .await
        .map_err(store_err)?
        .is_some()
    {
        return Err(Error::message("Discard did not delete the new-card draft"));
    }
    Ok(())
}

#[tokio::test]
#[ignore = "browser"]
async fn quiz_uses_the_persisted_card_not_an_edit_draft() -> Result<()> {
    let h = TestHarness::start().await?;
    let (store, user_id) = seed_store(&h).await?;
    seed_labels(&store, user_id).await?;
    seed_card(
        &store,
        user_id,
        "card-quiz",
        "Original prompt",
        "Original solution",
        now_ms() - 1_000,
    )
    .await?;
    let card = store
        .get_card(user_id, "card-quiz")
        .await
        .map_err(store_err)?
        .ok_or_else(|| Error::message("quiz card vanished"))?;
    store
        .put_card_edit_draft(
            user_id,
            "card-quiz",
            card.revision,
            "Draft prompt",
            "Draft solution",
            &["A".to_owned()],
            now_ms(),
        )
        .await
        .map_err(store_err)?;

    h.goto("/quiz").await?;
    h.wait_for_text("#quiz-prompt", "Original prompt", TIMEOUT)
        .await?;
    if h.eval::<bool>("document.querySelector('#quiz-prompt').textContent.includes('Draft prompt')")
        .await?
    {
        return Err(Error::message("Quiz displayed pending draft content"));
    }
    Ok(())
}

#[tokio::test]
#[ignore = "browser"]
async fn stale_edit_is_rejected_without_silent_overwrite() -> Result<()> {
    let h = TestHarness::start().await?;
    let (store, user_id) = seed_store(&h).await?;
    seed_labels(&store, user_id).await?;
    seed_card(
        &store,
        user_id,
        "card-stale",
        "Original",
        "Answer",
        now_ms() + 60_000,
    )
    .await?;

    h.goto("/groom").await?;
    h.wait_for_selector("#edit-card-stale", TIMEOUT).await?;
    h.click("#edit-card-stale").await?;
    h.wait_for_selector("#editor-prompt", TIMEOUT).await?;
    append_text(&h, "#editor-prompt", " draft").await?;
    h.wait_for_text("#draft-indicator", "draft saved", AUTOSAVE_TIMEOUT)
        .await?;
    store
        .update_card_fields(user_id, "card-stale", Some("Changed elsewhere"), None)
        .await
        .map_err(store_err)?;
    h.click("#editor-save").await?;
    h.wait_for_text("#editor-validation", "card changed", TIMEOUT)
        .await?;

    let card = store
        .get_card(user_id, "card-stale")
        .await
        .map_err(store_err)?
        .ok_or_else(|| Error::message("stale card vanished"))?;
    if card.prompt != "Changed elsewhere" {
        return Err(Error::message("stale Save silently overwrote the card"));
    }
    Ok(())
}
