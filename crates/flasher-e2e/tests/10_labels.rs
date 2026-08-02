//! Label-management e2e coverage: the dedicated page is navigable at its
//! own URL, labels can be created and renamed, and deleting a used label
//! reports the exact affected-card count before requiring a second explicit
//! confirmation.

use std::time::{Duration, Instant};

use flasher_e2e::{E2E_USER, Error, Result, TestHarness};
use flasher_store::{CardState, NewCard, Store};

const TIMEOUT: Duration = Duration::from_secs(15);

#[allow(clippy::needless_pass_by_value)]
fn store_err(err: flasher_store::Error) -> Error {
    Error::message(format!("store error: {err}"))
}

async fn seed_user(h: &TestHarness) -> Result<(Store, i64)> {
    let store = h.seed_store().await.map_err(store_err)?;
    let user = store
        .get_user_by_name(E2E_USER)
        .await
        .map_err(store_err)?
        .ok_or_else(|| Error::message("e2e user missing"))?;
    Ok((store, user.id))
}

async fn wait_for_path(h: &TestHarness, expected: &str) -> Result<()> {
    let deadline = Instant::now() + TIMEOUT;
    loop {
        let path: String = h.eval("location.pathname").await?;
        if path == expected {
            return Ok(());
        }
        if Instant::now() >= deadline {
            return Err(Error::message(format!(
                "path is {path:?}, expected {expected:?}"
            )));
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

#[tokio::test]
#[ignore = "browser"]
async fn label_crud_and_used_label_delete_confirmation() -> Result<()> {
    let h = TestHarness::start().await?;

    h.goto("/labels").await?;
    h.wait_for_selector("#labels-page", TIMEOUT).await?;
    h.wait_for_selector("#tab-labels.active", TIMEOUT).await?;
    wait_for_path(&h, "/labels").await?;

    h.type_into("#new-label-name", "Rust").await?;
    h.click("#create-label").await?;
    h.wait_for_text("#labels-list", "Rust", TIMEOUT).await?;

    h.type_into("#new-label-name", "Unused").await?;
    h.click("#create-label").await?;
    h.wait_for_text("#labels-list", "Unused", TIMEOUT).await?;

    // Seed through the store, never through the internal HTTP API. The
    // label was created through the user-facing page, so this card proves
    // the delete warning describes real existing usage.
    let (store, user_id) = seed_user(&h).await?;
    store
        .insert_card(&NewCard {
            user_id,
            id: "card-rust".to_owned(),
            prompt: "Rust card".to_owned(),
            solution: "ownership".to_owned(),
            state: CardState::New,
            change_time: 1_000,
            next_time: 2_000,
            labels: vec!["Rust".to_owned()],
        })
        .await
        .map_err(store_err)?;

    h.click("#rename-label-1").await?;
    h.click("#label-rename-input").await?;
    let rename_input = h
        .page
        .find_element("#label-rename-input")
        .await
        .map_err(Error::Cdp)?;
    for _ in 0..4 {
        rename_input
            .press_key("Backspace")
            .await
            .map_err(Error::Cdp)?;
    }
    rename_input.type_str("Systems").await.map_err(Error::Cdp)?;
    h.click("#save-label-rename").await?;
    h.wait_for_text("#labels-list", "Systems", TIMEOUT).await?;

    h.click("#delete-label-1").await?;
    h.wait_for_selector("#label-delete-modal", TIMEOUT).await?;
    h.click("#confirm-delete-label").await?;
    h.wait_for_text("#label-delete-warning", "attached to 1 card", TIMEOUT)
        .await?;
    h.screenshot("10_labels/delete-warning").await?;

    // The first confirmation only discovers usage; the row must remain and
    // the modal must name the exact count until the user confirms again.
    let row_still_exists: bool = h.eval("!!document.querySelector('#label-row-1')").await?;
    if !row_still_exists {
        return Err(Error::message(
            "a used label must remain until the explicit destructive confirmation",
        ));
    }
    h.click("#confirm-delete-label").await?;
    let deadline = std::time::Instant::now() + TIMEOUT;
    loop {
        let gone: bool = h.eval("!document.querySelector('#label-row-1')").await?;
        if gone {
            break;
        }
        if std::time::Instant::now() >= deadline {
            return Err(Error::message("deleted label row did not disappear"));
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }

    let card = store
        .get_card(user_id, "card-rust")
        .await
        .map_err(store_err)?
        .ok_or_else(|| Error::message("seeded card disappeared with its label"))?;
    if !card.labels.is_empty() {
        return Err(Error::message(format!(
            "deleted label association remains: {:?}",
            card.labels
        )));
    }

    // The narrow drawer keeps all labels in one deliberate overflow surface
    // instead of widening the page's persistent navigation bar.
    h.set_viewport(390, 844).await?;
    h.click("#nav-toggle").await?;
    h.wait_for_selector("#tab-labels", TIMEOUT).await?;
    let nav_width: f64 = h
        .eval("document.querySelector('#primary-nav').getBoundingClientRect().width")
        .await?;
    if nav_width > 300.0 {
        return Err(Error::message(format!(
            "mobile navigation drawer is unexpectedly wide: {nav_width}px"
        )));
    }
    h.click("#tab-labels").await?;
    h.wait_for_selector("#labels-page", TIMEOUT).await?;
    Ok(())
}

/// Counts include unused labels and are refreshed after the card's label set
/// changes through the user-facing Groom editor.
#[tokio::test]
#[ignore = "browser"]
async fn label_card_counts_refresh_after_relabeling() -> Result<()> {
    let h = TestHarness::start().await?;

    h.goto("/labels").await?;
    h.wait_for_selector("#labels-page", TIMEOUT).await?;
    h.type_into("#new-label-name", "Used").await?;
    h.click("#create-label").await?;
    h.wait_for_text("#labels-list", "Used", TIMEOUT).await?;
    h.type_into("#new-label-name", "Unused").await?;
    h.click("#create-label").await?;
    h.wait_for_text("#labels-list", "Unused", TIMEOUT).await?;

    let (store, user_id) = seed_user(&h).await?;
    store
        .insert_card(&NewCard {
            user_id,
            id: "card-counts".to_owned(),
            prompt: "Count me".to_owned(),
            solution: "card counts".to_owned(),
            state: CardState::New,
            change_time: 1_000,
            next_time: 2_000,
            labels: vec!["Used".to_owned()],
        })
        .await
        .map_err(store_err)?;

    // The page fetch is fresh on mount, so the labels created above now show
    // one used label and one deliberately unused label.
    h.goto("/labels").await?;
    h.wait_for_selector("#labels-page", TIMEOUT).await?;
    h.wait_for_text("#label-card-count-1", "1", TIMEOUT).await?;
    h.wait_for_text("#label-card-count-2", "0", TIMEOUT).await?;

    // Change the association through the browser, not the internal API.
    h.goto("/groom").await?;
    h.wait_for_selector("#groom-row-card-counts", TIMEOUT)
        .await?;
    h.click("#edit-card-counts").await?;
    h.wait_for_selector("#editor-prompt", TIMEOUT).await?;
    h.wait_for_selector("#editor-label-Used", TIMEOUT).await?;
    h.click("#editor-label-Used").await?;
    h.click("#editor-label-Unused").await?;
    h.click("#editor-save").await?;
    h.wait_for_selector("#groom-search", TIMEOUT).await?;
    h.wait_for_text("#groom-row-card-counts", "Unused", TIMEOUT)
        .await?;

    h.goto("/labels").await?;
    h.wait_for_selector("#labels-page", TIMEOUT).await?;
    h.wait_for_text("#label-card-count-1", "0", TIMEOUT).await?;
    h.wait_for_text("#label-card-count-2", "1", TIMEOUT).await?;
    Ok(())
}

/// Renaming a selected label keeps both page filters selected by the stable
/// database ID and updates their visible badge to the new name.
#[tokio::test]
#[ignore = "browser"]
async fn renaming_selected_label_preserves_quiz_and_groom_filters() -> Result<()> {
    let h = TestHarness::start().await?;

    h.goto("/labels").await?;
    h.wait_for_selector("#labels-page", TIMEOUT).await?;
    h.type_into("#new-label-name", "Rust").await?;
    h.click("#create-label").await?;
    h.wait_for_text("#labels-list", "Rust", TIMEOUT).await?;
    h.type_into("#new-label-name", "Other").await?;
    h.click("#create-label").await?;
    h.wait_for_text("#labels-list", "Other", TIMEOUT).await?;

    let (store, user_id) = seed_user(&h).await?;
    for (id, prompt, next_time, labels) in [
        ("card-rust", "Rust card", 1_000_i64, vec!["Rust"]),
        ("card-other", "Other card", 2_000_i64, vec!["Other"]),
    ] {
        store
            .insert_card(&NewCard {
                user_id,
                id: id.to_owned(),
                prompt: prompt.to_owned(),
                solution: "solution".to_owned(),
                state: CardState::New,
                change_time: 1,
                next_time,
                labels: labels.into_iter().map(str::to_owned).collect(),
            })
            .await
            .map_err(store_err)?;
    }

    // Select Rust only in Quiz, then verify the same one-card result in Groom.
    h.goto("/").await?;
    h.wait_for_text("#quiz-prompt", "Rust card", TIMEOUT)
        .await?;
    h.click("#quiz-label-filter-button").await?;
    h.wait_for_selector("#quiz-label-Other", TIMEOUT).await?;
    h.click("#quiz-label-Other").await?;
    h.click(".label-filter-backdrop").await?;
    h.wait_for_text("#quiz-prompt", "Rust card", TIMEOUT)
        .await?;

    h.click("#tab-groom").await?;
    h.wait_for_selector("#groom-search", TIMEOUT).await?;
    h.click("#groom-label-filter-button").await?;
    h.wait_for_selector("#groom-label-Other", TIMEOUT).await?;
    h.click("#groom-label-Other").await?;
    h.click(".label-filter-backdrop").await?;
    h.wait_for_text("#groom-page-info", "of 1", TIMEOUT).await?;
    h.wait_for_text("#groom-results", "Rust card", TIMEOUT)
        .await?;

    // Rename the first-created label while both pages have it selected.
    h.click("#tab-labels").await?;
    h.wait_for_selector("#labels-page", TIMEOUT).await?;
    h.click("#rename-label-1").await?;
    h.click("#label-rename-input").await?;
    let rename_input = h
        .page
        .find_element("#label-rename-input")
        .await
        .map_err(Error::Cdp)?;
    for _ in 0..4 {
        rename_input
            .press_key("Backspace")
            .await
            .map_err(Error::Cdp)?;
    }
    rename_input.type_str("Systems").await.map_err(Error::Cdp)?;
    h.click("#save-label-rename").await?;
    h.wait_for_text("#labels-list", "Systems", TIMEOUT).await?;

    h.click("#tab-quiz").await?;
    h.wait_for_selector("#quiz-selected-Systems", TIMEOUT)
        .await?;
    h.wait_for_text("#quiz-prompt", "Rust card", TIMEOUT)
        .await?;
    h.click("#tab-groom").await?;
    h.wait_for_selector("#groom-selected-Systems", TIMEOUT)
        .await?;
    h.wait_for_text("#groom-page-info", "of 1", TIMEOUT).await?;
    h.wait_for_text("#groom-results", "Rust card", TIMEOUT)
        .await?;
    Ok(())
}
