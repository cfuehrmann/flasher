//! Quiz vertical-slice e2e tests: the full review round, SRS
//! rescheduling, the empty-quiz done state, and card creation (including
//! validation) — all driven through the browser
//! the way a user would, with the database only used for seeding and
//! white-box verification.

// The scheduling assertions compare wall-clock-derived intervals in
// floating point with generous tolerances; the pedantic cast lints add
// no value here (same reasoning as in the harness itself).
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss
)]

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use flasher_e2e::{E2E_USER, Error, Result, TestHarness};
use flasher_store::{CardState, NewCard, Store};

/// Timeout for every DOM wait; generous because the wasm bundle has to
/// download and boot first (same reasoning as the harness default).
const TIMEOUT: Duration = Duration::from_secs(15);

/// Milliseconds of wall-clock drift tolerated in scheduling assertions.
const TIMING_TOLERANCE_MS: i64 = 2_000;

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

/// Inserts one card with exact scheduling fields.
#[allow(clippy::too_many_arguments)]
async fn seed_card(
    store: &Store,
    user_id: i64,
    id: &str,
    prompt: &str,
    solution: &str,
    state: CardState,
    change_time: i64,
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
            change_time,
            next_time,
            disabled,
        })
        .await
        .map_err(store_err)
}

/// Seeds the two due cards of the standard round: card A (state ok, due
/// earlier, `change_time` one hour ago) and card B (state new, due later,
/// `change_time` half an hour ago). Returns the seed `change_time` values.
async fn seed_round(store: &Store, user_id: i64) -> Result<(i64, i64)> {
    let now = now_ms();
    let change_a = now - 3_600_000;
    let change_b = now - 1_800_000;
    seed_card(
        store,
        user_id,
        "card-a",
        "Prompt A",
        "Solution A",
        CardState::Ok,
        change_a,
        now - 60_000,
        false,
    )
    .await?;
    seed_card(
        store,
        user_id,
        "card-b",
        "Prompt B",
        "Solution B",
        CardState::New,
        change_b,
        now - 30_000,
        false,
    )
    .await?;
    Ok((change_a, change_b))
}

/// Drives the standard round through the UI: show A's solution, rate OK,
/// show B's solution, rate Failed, end in the done state.
async fn drive_round(h: &TestHarness) -> Result<()> {
    h.wait_for_text("#quiz-prompt", "Prompt A", TIMEOUT).await?;
    h.click("#show-solution").await?;
    h.wait_for_text("#quiz-solution", "Solution A", TIMEOUT)
        .await?;
    h.click("#rate-ok").await?;
    h.wait_for_text("#quiz-prompt", "Prompt B", TIMEOUT).await?;
    h.click("#show-solution").await?;
    h.wait_for_text("#quiz-solution", "Solution B", TIMEOUT)
        .await?;
    h.click("#rate-failed").await?;
    h.wait_for_selector("#quiz-done", TIMEOUT).await?;
    Ok(())
}

/// Asserts `actual ≈ multiplier × interval` within the tolerance.
fn assert_multiplier(label: &str, interval: i64, multiplier: f64, actual: i64) -> Result<()> {
    let expected = (interval as f64 * multiplier).round() as i64;
    if (actual - expected).abs() > TIMING_TOLERANCE_MS {
        return Err(Error::message(format!(
            "{label}: expected next_time - change_time ≈ {expected} ms \
             ({multiplier} × interval {interval} ms), got {actual} ms"
        )));
    }
    Ok(())
}

/// Full quiz round: two due cards are reviewed in due order — prompt,
/// solution, OK; prompt, solution, Failed — and the done state appears
/// once nothing is left.
#[tokio::test]
#[ignore = "browser"]
async fn quiz_round_walks_two_cards_to_done() -> Result<()> {
    let h = TestHarness::start().await?;
    let (store, user_id) = seed_store(&h).await?;
    seed_round(&store, user_id).await?;

    h.goto("/").await?;
    h.wait_for_text("#quiz-prompt", "Prompt A", TIMEOUT).await?;
    h.screenshot("03_quiz/quiz-prompt").await?;

    h.click("#show-solution").await?;
    h.wait_for_text("#quiz-solution", "Solution A", TIMEOUT)
        .await?;
    h.screenshot("03_quiz/quiz-solution").await?;

    h.click("#rate-ok").await?;
    h.wait_for_text("#quiz-prompt", "Prompt B", TIMEOUT).await?;
    let body = h.page_text().await?;
    if body.contains("Prompt A") {
        return Err(Error::message(format!(
            "card A should be gone after rating it, page shows: {body:?}"
        )));
    }

    h.click("#show-solution").await?;
    h.wait_for_text("#quiz-solution", "Solution B", TIMEOUT)
        .await?;
    h.click("#rate-failed").await?;
    h.wait_for_text("#quiz-done", "All done", TIMEOUT).await?;
    h.screenshot("03_quiz/quiz-done").await?;
    Ok(())
}

/// Scheduling white-box check: after the standard round, card A (rated
/// OK) must be rescheduled to 1.8 × its last interval in the future, and
/// card B (rated Failed) to 0.5555 × its interval.
#[tokio::test]
#[ignore = "browser"]
async fn rating_reschedules_cards_by_srs_multipliers() -> Result<()> {
    let h = TestHarness::start().await?;
    let (store, user_id) = seed_store(&h).await?;
    let (change_a, change_b) = seed_round(&store, user_id).await?;

    h.goto("/").await?;
    drive_round(&h).await?;

    let card_a = store
        .get_card(user_id, "card-a")
        .await
        .map_err(store_err)?
        .ok_or_else(|| Error::message("card-a vanished"))?;
    if card_a.state != CardState::Ok {
        return Err(Error::message(format!(
            "card-a should be ok, is {:?}",
            card_a.state
        )));
    }
    if card_a.next_time <= now_ms() {
        return Err(Error::message(format!(
            "card-a next_time should be in the future: {} <= now",
            card_a.next_time
        )));
    }
    assert_multiplier(
        "card-a",
        card_a.change_time - change_a,
        1.8,
        card_a.next_time - card_a.change_time,
    )?;

    let card_b = store
        .get_card(user_id, "card-b")
        .await
        .map_err(store_err)?
        .ok_or_else(|| Error::message("card-b vanished"))?;
    if card_b.state != CardState::Failed {
        return Err(Error::message(format!(
            "card-b should be failed, is {:?}",
            card_b.state
        )));
    }
    assert_multiplier(
        "card-b",
        card_b.change_time - change_b,
        0.5555,
        card_b.next_time - card_b.change_time,
    )?;
    Ok(())
}

/// Nothing due: only future-due and disabled cards exist, so the done
/// state shows immediately.
#[tokio::test]
#[ignore = "browser"]
async fn quiz_shows_done_when_nothing_is_due() -> Result<()> {
    let h = TestHarness::start().await?;
    let (store, user_id) = seed_store(&h).await?;
    let now = now_ms();
    // Enabled but not yet due.
    seed_card(
        &store,
        user_id,
        "card-future",
        "Future prompt",
        "Future solution",
        CardState::Ok,
        now,
        now + 3_600_000,
        false,
    )
    .await?;
    // Due, but disabled (new cards start out disabled).
    seed_card(
        &store,
        user_id,
        "card-disabled",
        "Disabled prompt",
        "Disabled solution",
        CardState::New,
        now - 60_000,
        now - 1_000,
        true,
    )
    .await?;

    h.goto("/").await?;
    h.wait_for_text("#quiz-done", "All done", TIMEOUT).await?;
    let body = h.page_text().await?;
    for hidden in ["Future prompt", "Disabled prompt"] {
        if body.contains(hidden) {
            return Err(Error::message(format!(
                "{hidden:?} must not appear in the quiz, page shows: {body:?}"
            )));
        }
    }
    h.screenshot("03_quiz/done-immediate").await?;
    Ok(())
}

/// Add card: the form creates a state=new, disabled card with a 30-minute
/// initial waiting time; submitting an empty prompt is rejected
/// client-side and creates nothing.
#[tokio::test]
#[ignore = "browser"]
async fn add_card_creates_disabled_new_card() -> Result<()> {
    let h = TestHarness::start().await?;
    h.goto("/").await?;

    h.click("#tab-add-card").await?;
    h.wait_for_selector("#new-prompt", TIMEOUT).await?;
    h.type_into("#new-prompt", "e2e prompt").await?;
    h.type_into("#new-solution", "e2e solution").await?;
    h.screenshot("03_quiz/add-card").await?;
    h.click("#create-card").await?;
    h.wait_for_text("#add-card-confirmation", "Card created", TIMEOUT)
        .await?;
    h.screenshot("03_quiz/add-card-created").await?;

    let (store, user_id) = seed_store(&h).await?;
    let (cards, count) = store
        .search_cards(user_id, Some("e2e prompt"), 0, 10)
        .await
        .map_err(store_err)?;
    if count != 1 || cards.len() != 1 {
        return Err(Error::message(format!(
            "expected exactly 1 card with the new prompt, got {count}"
        )));
    }
    let card = &cards[0];
    if card.state != CardState::New {
        return Err(Error::message(format!(
            "new card should be state new, is {:?}",
            card.state
        )));
    }
    if !card.disabled {
        return Err(Error::message("new card should start out disabled"));
    }
    if card.solution != "e2e solution" {
        return Err(Error::message(format!(
            "solution mismatch: {:?}",
            card.solution
        )));
    }
    let waiting = card.next_time - card.change_time;
    if (waiting - 1_800_000).abs() > TIMING_TOLERANCE_MS {
        return Err(Error::message(format!(
            "new card waiting time should be ≈ 1800000 ms, got {waiting} ms"
        )));
    }

    // The success cleared the form; submitting again now exercises the
    // empty-prompt validation, which must create nothing.
    h.click("#create-card").await?;
    h.wait_for_text("#add-card-validation", "Prompt must not be empty", TIMEOUT)
        .await?;
    h.screenshot("03_quiz/add-card-validation").await?;
    let (_all, total) = store
        .search_cards(user_id, None, 0, 100)
        .await
        .map_err(store_err)?;
    if total != 1 {
        return Err(Error::message(format!(
            "empty-prompt submit must not create a card; store holds {total}"
        )));
    }
    Ok(())
}
