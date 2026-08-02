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
            labels: vec![if disabled {
                "Disabled".to_owned()
            } else {
                "Enabled".to_owned()
            }],
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

/// Regression test for issue #124: rapid double-rating must apply
/// exactly one rating. A synchronous burst of OK clicks on the only due
/// card (the review's repro: double-taps within the response window)
/// must leave the card rescheduled by 1.8 × its last interval — not
/// collapsed to ~now by a second request racing the first — and the
/// quiz must reach the done state instead of showing the card again.
#[tokio::test]
#[ignore = "browser"]
async fn rapid_double_rating_applies_exactly_once() -> Result<()> {
    let h = TestHarness::start().await?;
    let (store, user_id) = seed_store(&h).await?;
    let now = now_ms();
    let change_a = now - 3_600_000;
    seed_card(
        &store,
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

    h.goto("/").await?;
    h.wait_for_text("#quiz-prompt", "Prompt A", TIMEOUT).await?;
    h.click("#show-solution").await?;
    h.wait_for_text("#quiz-solution", "Solution A", TIMEOUT)
        .await?;

    // The double-tap burst: ten clicks dispatched synchronously, before
    // any response can arrive. The in-flight guard (and the disabled
    // buttons) must swallow everything after the first.
    h.eval::<bool>(
        "(() => { for (let i = 0; i < 10; i++) \
         document.querySelector('#rate-ok').click(); return true; })()",
    )
    .await?;

    // Exactly one rating was applied: the card is gone and stays gone.
    h.wait_for_selector("#quiz-done", TIMEOUT).await?;

    let card = store
        .get_card(user_id, "card-a")
        .await
        .map_err(store_err)?
        .ok_or_else(|| Error::message("card-a vanished"))?;
    if card.state != CardState::Ok {
        return Err(Error::message(format!(
            "card-a should be ok, is {:?}",
            card.state
        )));
    }
    if card.next_time <= now_ms() {
        return Err(Error::message(format!(
            "card-a next_time collapsed to the past ({} <= now) — \
             a second rating recomputed the interval off the first",
            card.next_time
        )));
    }
    assert_multiplier(
        "card-a",
        card.change_time - change_a,
        1.8,
        card.next_time - card.change_time,
    )?;
    Ok(())
}

/// Label names carry no semantics: a due card is quizzed whatever its
/// labels are (until the user filters), while a future-due card waits.
#[tokio::test]
#[ignore = "browser"]
async fn quiz_shows_due_cards_regardless_of_labels() -> Result<()> {
    let h = TestHarness::start().await?;
    let (store, user_id) = seed_store(&h).await?;
    let now = now_ms();
    // Not yet due.
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
    // Due, carrying the fixture's "disabled-style" label — an arbitrary
    // name with no effect on quizzability.
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
    h.wait_for_text("#quiz-prompt", "Disabled prompt", TIMEOUT)
        .await?;
    let body = h.page_text().await?;
    if body.contains("Future prompt") {
        return Err(Error::message(format!(
            "the future-due card must not appear in the quiz, page shows: {body:?}"
        )));
    }
    h.screenshot("03_quiz/due-regardless-of-labels").await?;
    Ok(())
}

/// Add card: the label picker is mandatory (owner decision 2026-08-01) —
/// Create stays disabled until at least one existing label is chosen, and
/// the created card carries exactly the chosen label (state=new,
/// 30-minute initial wait). Label creation belongs to the Labels page.
/// An empty prompt keeps Create disabled and creates nothing.
#[tokio::test]
#[ignore = "browser"]
async fn add_card_creates_card_with_chosen_labels() -> Result<()> {
    let h = TestHarness::start().await?;
    let (store, user_id) = seed_store(&h).await?;
    store
        .ensure_label(user_id, "grammar")
        .await
        .map_err(store_err)?;
    h.goto("/").await?;

    h.click("#tab-add-card").await?;
    h.wait_for_selector("#new-prompt", TIMEOUT).await?;
    h.type_into("#new-prompt", "e2e prompt").await?;
    h.type_into("#new-solution", "e2e solution").await?;

    // Create stays disabled until an existing label is chosen.
    if !h
        .eval::<bool>("document.querySelector('#create-card').disabled")
        .await?
    {
        return Err(Error::message(
            "Create should be disabled while no label is chosen",
        ));
    }
    h.screenshot("03_quiz/add-card").await?;
    h.click("#editor-label-grammar").await?;
    if h.eval::<bool>("document.querySelector('#create-card').disabled")
        .await?
    {
        return Err(Error::message(
            "Create should enable once a label is chosen",
        ));
    }
    h.click("#create-card").await?;
    h.wait_for_text("#add-card-confirmation", "Card created", TIMEOUT)
        .await?;
    h.screenshot("03_quiz/add-card-created").await?;

    let (cards, count) = store
        .search_cards(user_id, Some("e2e prompt"), None, 0, 10)
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
    if card.labels != ["grammar".to_owned()] {
        return Err(Error::message(format!(
            "new card should carry exactly the chosen label, carries {:?}",
            card.labels
        )));
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

    // The success cleared the form; Create stays disabled while the prompt
    // is empty, so an empty submission cannot create another card.
    if !h
        .eval::<bool>("document.querySelector('#create-card').disabled")
        .await?
    {
        return Err(Error::message(
            "Create should be disabled after the form is cleared",
        ));
    }
    h.screenshot("03_quiz/add-card-empty-disabled").await?;
    let (_all, total) = store
        .search_cards(user_id, None, None, 0, 100)
        .await
        .map_err(store_err)?;
    if total != 1 {
        return Err(Error::message(format!(
            "empty-prompt submit must not create a card; store holds {total}"
        )));
    }
    Ok(())
}

/// Sets exactly the given labels checked in a label filter (union
/// semantics), clicking like a user: open the panel, toggle each
/// checkbox into the wanted state, close via the backdrop (same helper
/// shape as the groom suite's).
async fn set_label_filter(h: &TestHarness, id_prefix: &str, only: &[&str]) -> Result<()> {
    h.click(&format!("#{id_prefix}-label-filter-button"))
        .await?;
    h.wait_for_selector(&format!("#{id_prefix}-label-filter-panel"), TIMEOUT)
        .await?;
    // The labels list fetches after mount; wait for the checkboxes to
    // exist before probing their state.
    h.wait_for_selector(
        &format!("#{id_prefix}-label-filter-panel .label-filter-item input"),
        TIMEOUT,
    )
    .await?;
    // The fixture labels (arbitrary names; a label's checkbox exists
    // only once the name is used in the database, so absent ones are
    // simply skipped).
    for name in ["Enabled", "Disabled"] {
        let box_sel = format!("#{id_prefix}-label-{name}");
        let exists = h
            .eval::<bool>(&format!("!!document.querySelector('{box_sel}')"))
            .await?;
        if !exists {
            continue;
        }
        let want = only.contains(&name);
        let is = h
            .eval::<bool>(&format!("document.querySelector('{box_sel}').checked"))
            .await?;
        if is != want {
            h.click(&box_sel).await?;
        }
    }
    h.click(".label-filter-backdrop").await
}

/// The quiz's label filter (owner decision 2026-08-01): selects which
/// labels may be quizzed (union semantics, default Enabled-only), is
/// persisted across a real reload, and is independent of the groom
/// tab's own persisted selection.
#[tokio::test]
#[ignore = "browser"]
async fn quiz_label_filter_selects_persists_and_is_independent() -> Result<()> {
    let h = TestHarness::start().await?;
    let (store, user_id) = seed_store(&h).await?;
    let now = now_ms();
    // An enabled due card (earliest) and a disabled due card.
    seed_card(
        &store,
        user_id,
        "card-enabled",
        "Enabled prompt",
        "Enabled solution",
        CardState::New,
        now - 120_000,
        now - 2_000,
        false,
    )
    .await?;
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
    // First-usage default: everything selected (label names carry no
    // semantics, so there is no name-based default) — both selection
    // badges show next to the button.
    h.wait_for_text("#quiz-prompt", "Enabled prompt", TIMEOUT)
        .await?;
    for name in ["Enabled", "Disabled"] {
        h.wait_for_selector(&format!("#quiz-selected-{name}"), TIMEOUT)
            .await?;
    }

    // The open panel (new UI — the CV check reads this PNG).
    h.click("#quiz-label-filter-button").await?;
    h.wait_for_selector("#quiz-label-filter-panel", TIMEOUT)
        .await?;
    h.screenshot("03_quiz/quiz-label-panel").await?;
    h.click(".label-filter-backdrop").await?;

    // Disabled only: the next card is the disabled one.
    set_label_filter(&h, "quiz", &["Disabled"]).await?;
    h.wait_for_text("#quiz-prompt", "Disabled prompt", TIMEOUT)
        .await?;

    // Both: due order wins (the enabled card is due earliest).
    set_label_filter(&h, "quiz", &["Enabled", "Disabled"]).await?;
    h.wait_for_text("#quiz-prompt", "Enabled prompt", TIMEOUT)
        .await?;

    // Nothing: the done state, with the filter-aware hint.
    set_label_filter(&h, "quiz", &[]).await?;
    h.wait_for_text("#quiz-done", "no due cards match", TIMEOUT)
        .await?;

    // Back to Disabled-only, then a real reload: the selection persists.
    set_label_filter(&h, "quiz", &["Disabled"]).await?;
    h.wait_for_text("#quiz-prompt", "Disabled prompt", TIMEOUT)
        .await?;
    h.goto("/").await?;
    h.wait_for_text("#quiz-prompt", "Disabled prompt", TIMEOUT)
        .await?;

    // Independence: the groom filter has its own persisted selection and
    // does not touch the quiz's (nor vice versa).
    h.click("#tab-groom").await?;
    h.wait_for_selector("#groom-search", TIMEOUT).await?;
    set_label_filter(&h, "groom", &["Enabled"]).await?;
    h.wait_for_text("#groom-page-info", "of 1", TIMEOUT).await?;
    h.click("#tab-quiz").await?;
    h.wait_for_text("#quiz-prompt", "Disabled prompt", TIMEOUT)
        .await?;
    let groom_persisted = h
        .eval::<String>("localStorage.getItem('flasher-groom-labels') ?? ''")
        .await?;
    let quiz_persisted = h
        .eval::<String>("localStorage.getItem('flasher-quiz-labels') ?? ''")
        .await?;
    if groom_persisted != "id:1" || quiz_persisted != "id:2" {
        return Err(Error::message(format!(
            "selections should persist independently, groom={groom_persisted:?} quiz={quiz_persisted:?}"
        )));
    }
    h.screenshot("03_quiz/quiz-label-filter").await?;
    Ok(())
}
