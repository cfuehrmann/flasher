//! Groom tab e2e tests: search-as-you-type (with full unicode case
//! folding), the clear button, the enabled/disabled/all status filter,
//! paging, the enable/disable toggle, the row "⋯" overflow
//! menu (incl. its aria-expanded state and the one-line meta row at
//! mobile width), delete with a confirm modal (including the
//! last-item-on-page fallback), progress reset, and
//! the cross-feature round "disabled card becomes quizzable once enabled"
//! — all click-driven through the browser, with the database only used
//! for seeding and white-box verification.

use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use chromiumoxide::cdp::browser_protocol::input::InsertTextParams;
use flasher_e2e::{E2E_USER, Error, Result, TestHarness};
use flasher_store::{CardState, DisabledFilter, NewCard, Store};

/// Timeout for every DOM wait; generous because the wasm bundle has to
/// download and boot first (same reasoning as the harness default).
const TIMEOUT: Duration = Duration::from_secs(15);

/// Layout probe for the groom meta row: reports whether the state badge
/// and the ⋯ trigger share one line (a wrapped line would put the
/// button below the badge) plus the row's client/scroll width and the
/// per-child widths, so a failure says by how much it misses.
const META_PROBE: &str = "(() => {
    const meta = document.querySelector('#groom-row-card-menu .groom-meta');
    const a = document.querySelector('#state-card-menu').getBoundingClientRect();
    const b = document.querySelector('#menu-card-menu').getBoundingClientRect();
    return JSON.stringify({
        same_line: a.top < b.bottom && b.top < a.bottom,
        client: meta.clientWidth, scroll: meta.scrollWidth, vw: window.innerWidth,
        kids: [...meta.children].map(c =>
            c.className + ':' + Math.round(c.getBoundingClientRect().width)),
    });
})()";

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

/// Seeds `n` enabled state=new cards `P01..Pn` (`card-p01..`) with
/// ascending `next_time` values, so the list order is deterministic.
async fn seed_page_cards(store: &Store, user_id: i64, n: usize, id_prefix: &str) -> Result<()> {
    let now = now_ms();
    for i in 1..=n {
        let offset = i64::try_from(i).unwrap_or(i64::MAX) * 60_000;
        seed_card(
            store,
            user_id,
            &format!("{id_prefix}{i:02}"),
            &format!("P{i:02}"),
            &format!("S{i:02}"),
            CardState::New,
            now,
            now + offset,
            false,
        )
        .await?;
    }
    Ok(())
}

/// Opens the app and switches to the Groom tab, waiting until the search
/// input is there.
async fn goto_groom(h: &TestHarness) -> Result<()> {
    h.goto("/").await?;
    h.click("#tab-groom").await?;
    h.wait_for_selector("#groom-search", TIMEOUT).await
}

/// Number of card rows currently rendered.
async fn row_count(h: &TestHarness) -> Result<usize> {
    h.eval::<usize>("document.querySelectorAll('.groom-row').length")
        .await
}

/// Waits until the viewport-fit calibration has run and persisted the
/// fitted page size, and returns it (owner wish 2026-07-31: the groom
/// page size is the number of rows that fit the viewport, so paging
/// expectations are computed from the measured fit, never hard-coded).
async fn wait_for_calibration(h: &TestHarness) -> Result<usize> {
    let deadline = Instant::now() + TIMEOUT;
    loop {
        // `?? ''`: a JS null (missing key) does not deserialize over CDP.
        let persisted = h
            .eval::<String>("localStorage.getItem('flasher-groom-take') ?? ''")
            .await?;
        if let Ok(fit) = persisted.parse::<usize>() {
            return Ok(fit);
        }
        if Instant::now() >= deadline {
            return Err(Error::message(
                "the viewport-fit calibration did not persist a page size",
            ));
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

/// Waits until a resize re-fit persists a page size different from
/// `previous`, and returns the new one.
async fn wait_for_refit(h: &TestHarness, previous: usize) -> Result<usize> {
    let deadline = Instant::now() + TIMEOUT;
    loop {
        let persisted = h
            .eval::<String>("localStorage.getItem('flasher-groom-take') ?? ''")
            .await?;
        if let Ok(fit) = persisted.parse::<usize>()
            && fit != previous
        {
            return Ok(fit);
        }
        if Instant::now() >= deadline {
            return Err(Error::message(format!(
                "no re-fit away from page size {previous} happened"
            )));
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

/// The "showing first–last of count" line for page `page` (0-based) of
/// `count` cards at page size `fit`.
fn showing(page: usize, fit: usize, count: usize) -> String {
    showing_at(page * fit, fit, count)
}

/// The "showing first–last of count" line for a window of `fit` cards
/// starting at offset `skip`.
fn showing_at(skip: usize, fit: usize, count: usize) -> String {
    let first = skip + 1;
    let last = (skip + fit).min(count);
    format!("showing {first}–{last} of {count}")
}

/// Waits until the page is actually scrolled (a `scrollTo` has landed).
async fn wait_scrolled(h: &TestHarness) -> Result<()> {
    let deadline = Instant::now() + TIMEOUT;
    loop {
        if h.eval::<f64>("window.scrollY").await? > 0.0 {
            return Ok(());
        }
        if Instant::now() >= deadline {
            return Err(Error::message("the page did not scroll"));
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

/// Picks a status-filter option like a user: sets the select's value and
/// fires the bubbling `change` event its `on:change` handler listens to
/// (CDP has no realistic "choose a native select option" input path).
async fn set_filter(h: &TestHarness, value: &str) -> Result<()> {
    let applied = h
        .eval::<bool>(&format!(
            "(() => {{
            const s = document.querySelector('#groom-filter');
            s.value = '{value}';
            s.dispatchEvent(new Event('change', {{ bubbles: true }}));
            return s.value === '{value}';
        }})()"
        ))
        .await?;
    if !applied {
        return Err(Error::message(format!(
            "filter option {value:?} not selectable"
        )));
    }
    Ok(())
}

/// Polls until no element matches `sel` (row/badge/modal removal).
async fn wait_until_gone(h: &TestHarness, sel: &str, timeout: Duration) -> Result<()> {
    let deadline = Instant::now() + timeout;
    loop {
        let exists: bool = h
            .eval(&format!("!!document.querySelector('{sel}')"))
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

/// Opens the row's "⋯" overflow menu and clicks the given item
/// (`#reset-<card>` / `#delete-<card>`), waiting for the menu to render.
async fn click_row_menu_item(h: &TestHarness, card_id: &str, item_sel: &str) -> Result<()> {
    h.click(&format!("#menu-{card_id}")).await?;
    h.wait_for_selector(item_sel, TIMEOUT).await?;
    h.click(item_sel).await
}

/// Search-as-you-type: typing `äpfel` (lowercase) matches `Äpfel und
/// Birnen` via full unicode case folding and hides `Zebra`; clearing the
/// input brings both cards back.
#[tokio::test]
#[ignore = "browser"]
async fn search_filters_with_unicode_folding() -> Result<()> {
    let h = TestHarness::start().await?;
    let (store, user_id) = seed_store(&h).await?;
    let now = now_ms();
    seed_card(
        &store,
        user_id,
        "card-aepfel",
        "Äpfel und Birnen",
        "Obst",
        CardState::New,
        now,
        now + 60_000,
        false,
    )
    .await?;
    seed_card(
        &store,
        user_id,
        "card-zebra",
        "Zebra",
        "Tier",
        CardState::New,
        now,
        now + 120_000,
        false,
    )
    .await?;

    goto_groom(&h).await?;
    h.wait_for_text("#groom-page-info", "of 2", TIMEOUT).await?;

    // chromiumoxide's `type_str` maps every char to a physical key and
    // has no key for `ä`; `Input.insertText` is the IME-style path real
    // unicode input takes, and it fires the same input events.
    h.click("#groom-search").await?;
    h.page
        .execute(InsertTextParams::new("äpfel"))
        .await
        .map_err(Error::Cdp)?;
    h.wait_for_text("#groom-page-info", "of 1", TIMEOUT).await?;
    let results = h.text_content("#groom-results").await?;
    if !results.contains("Äpfel und Birnen") {
        return Err(Error::message(format!(
            "filtered list should contain Äpfel und Birnen, shows: {results:?}"
        )));
    }
    if results.contains("Zebra") {
        return Err(Error::message(format!(
            "filtered list must not contain Zebra, shows: {results:?}"
        )));
    }
    h.screenshot("04_groom/search-filtered").await?;

    // Clear the input like a user: one Backspace per typed character.
    let input = h
        .page
        .find_element("#groom-search")
        .await
        .map_err(Error::Cdp)?;
    for _ in 0.."äpfel".chars().count() {
        input.press_key("Backspace").await.map_err(Error::Cdp)?;
    }
    h.wait_for_text("#groom-page-info", "of 2", TIMEOUT).await?;
    let results = h.text_content("#groom-results").await?;
    for prompt in ["Äpfel und Birnen", "Zebra"] {
        if !results.contains(prompt) {
            return Err(Error::message(format!(
                "cleared search should list {prompt:?}, shows: {results:?}"
            )));
        }
    }
    h.screenshot("04_groom/search-cleared").await?;
    Ok(())
}

/// Clear button: disabled while the box is empty; one click empties the
/// box and restores the full list immediately (no debounce — a click is
/// a deliberate act, unlike a keystroke).
#[tokio::test]
#[ignore = "browser"]
async fn clear_button_resets_search() -> Result<()> {
    let h = TestHarness::start().await?;
    let (store, user_id) = seed_store(&h).await?;
    seed_page_cards(&store, user_id, 3, "card-clear-").await?;

    goto_groom(&h).await?;
    h.wait_for_text("#groom-page-info", "of 3", TIMEOUT).await?;
    let disabled = h
        .eval::<bool>("document.querySelector('#groom-clear').disabled")
        .await?;
    if !disabled {
        return Err(Error::message(
            "clear button must be disabled while the search box is empty",
        ));
    }

    h.type_into("#groom-search", "P01").await?;
    h.wait_for_text("#groom-page-info", "of 1", TIMEOUT).await?;
    h.click("#groom-clear").await?;
    h.wait_for_text("#groom-page-info", "of 3", TIMEOUT).await?;
    let value = h
        .eval::<String>("document.querySelector('#groom-search').value")
        .await?;
    if !value.is_empty() {
        return Err(Error::message(format!(
            "clear button should empty the search box, shows {value:?}"
        )));
    }
    let disabled = h
        .eval::<bool>("document.querySelector('#groom-clear').disabled")
        .await?;
    if !disabled {
        return Err(Error::message(
            "clear button must be disabled again once the box is empty",
        ));
    }
    Ok(())
}

/// Status filter (issue #127): the first-usage default `all` lists every
/// card, `enabled` hides disabled ones, `disabled` lists only those —
/// and switching the filter resets to page 0 (checked from page 2 of the
/// enabled list).
#[tokio::test]
#[ignore = "browser"]
async fn filter_enabled_disabled_all() -> Result<()> {
    let h = TestHarness::start().await?;
    let (store, user_id) = seed_store(&h).await?;
    // 11 enabled cards plus 1 disabled card.
    seed_page_cards(&store, user_id, 11, "card-f").await?;
    let now = now_ms();
    seed_card(
        &store,
        user_id,
        "card-disabled",
        "Disabled prompt",
        "Disabled solution",
        CardState::New,
        now,
        now + 30_000,
        true,
    )
    .await?;

    goto_groom(&h).await?;
    let fit = wait_for_calibration(&h).await?;
    if fit >= 11 {
        return Err(Error::message(format!(
            "test premise: the calibrated page size {fit} must leave the 11 enabled cards paged"
        )));
    }
    // First usage (fresh browser profile, nothing persisted): the filter
    // defaults to `all` (owner decision 2026-07-31) — all 12 cards.
    h.wait_for_text("#groom-page-info", &showing(0, fit, 12), TIMEOUT)
        .await?;
    if h.eval::<String>("document.querySelector('#groom-filter').value")
        .await?
        != "all"
    {
        return Err(Error::message(
            "the filter should default to all on first use",
        ));
    }
    let results = h.text_content("#groom-results").await?;
    if !results.contains("P01") || !results.contains("Disabled prompt") {
        return Err(Error::message(format!(
            "all filter should list enabled and disabled cards, shows: {results:?}"
        )));
    }
    h.screenshot("04_groom/filter-all").await?;

    // `enabled`: the disabled card hides, the 11 enabled ones paginate.
    set_filter(&h, "enabled").await?;
    h.wait_for_text("#groom-page-info", &showing(0, fit, 11), TIMEOUT)
        .await?;
    let results = h.text_content("#groom-results").await?;
    if results.contains("Disabled prompt") {
        return Err(Error::message(format!(
            "enabled filter must hide the disabled card, shows: {results:?}"
        )));
    }
    h.screenshot("04_groom/filter-enabled").await?;

    // Go to page 2, then switch the filter: it must reset to page 0.
    h.click("#groom-next").await?;
    h.wait_for_text("#groom-page-info", &showing(1, fit, 11), TIMEOUT)
        .await?;
    set_filter(&h, "disabled").await?;
    h.wait_for_text("#groom-page-info", "showing 1–1 of 1", TIMEOUT)
        .await?;
    let results = h.text_content("#groom-results").await?;
    if !results.contains("Disabled prompt") || results.contains("P01") {
        return Err(Error::message(format!(
            "disabled filter should list only the disabled card, shows: {results:?}"
        )));
    }
    h.screenshot("04_groom/filter-disabled").await?;
    Ok(())
}

/// Asserts the restored groom state after a remount: the list is back
/// to the one persisted hit, and both controls show the persisted
/// values (`disabled` filter, `alpha` search).
async fn expect_restored(h: &TestHarness) -> Result<()> {
    h.wait_for_text("#groom-page-info", "of 1", TIMEOUT).await?;
    let filter = h
        .eval::<String>("document.querySelector('#groom-filter').value")
        .await?;
    if filter != "disabled" {
        return Err(Error::message(format!(
            "filter should be restored as disabled, is {filter:?}"
        )));
    }
    let search = h
        .eval::<String>("document.querySelector('#groom-search').value")
        .await?;
    if search != "alpha" {
        return Err(Error::message(format!(
            "search text should be restored as alpha, is {search:?}"
        )));
    }
    let results = h.text_content("#groom-results").await?;
    if !results.contains("Alpha one") {
        return Err(Error::message(format!(
            "restored list should show Alpha one, shows: {results:?}"
        )));
    }
    Ok(())
}

/// Filter and search text persist (owner wish 2026-07-31): both survive
/// a tab switch (which remounts the groom tab) and a full browser
/// refresh, and the restored state drives the very first fetch after
/// remount.
#[tokio::test]
#[ignore = "browser"]
async fn filter_and_search_survive_tab_switch_and_reload() -> Result<()> {
    let h = TestHarness::start().await?;
    let (store, user_id) = seed_store(&h).await?;
    let now = now_ms();
    for (id, prompt, disabled) in [
        ("card-persist-a", "Alpha one", true),
        ("card-persist-b", "Beta two", true),
        ("card-persist-c", "Gamma three", false),
    ] {
        seed_card(
            &store,
            user_id,
            id,
            prompt,
            "S",
            CardState::New,
            now,
            now + 60_000,
            disabled,
        )
        .await?;
    }

    goto_groom(&h).await?;
    h.wait_for_text("#groom-page-info", "of 3", TIMEOUT).await?;

    // Filter `disabled`, then search `alpha`: exactly the one matching
    // disabled card remains.
    set_filter(&h, "disabled").await?;
    h.wait_for_text("#groom-page-info", "of 2", TIMEOUT).await?;
    h.type_into("#groom-search", "alpha").await?;
    h.wait_for_text("#groom-page-info", "of 1", TIMEOUT).await?;
    let results = h.text_content("#groom-results").await?;
    if !results.contains("Alpha one") || results.contains("Beta two") {
        return Err(Error::message(format!(
            "filter + search should leave only Alpha one, shows: {results:?}"
        )));
    }

    // A tab switch remounts the groom tab: filter + search come back.
    h.click("#tab-quiz").await?;
    h.wait_for_selector("#tab-quiz.active", TIMEOUT).await?;
    h.click("#tab-groom").await?;
    h.wait_for_selector("#groom-search", TIMEOUT).await?;
    expect_restored(&h).await?;

    // A full browser refresh (real navigation): same restore.
    h.goto("/groom").await?;
    h.wait_for_selector("#groom-search", TIMEOUT).await?;
    expect_restored(&h).await?;
    h.screenshot("04_groom/filter-search-restored").await?;
    Ok(())
}

/// Toggling the last row of a page beyond the first out of the active
/// filter steps back one page (same fallback as delete) instead of
/// stranding the user on a false "No cards match." without a paging bar
/// (adversarial review, issue #127). The single-row second page is
/// constructed relative to the calibrated viewport fit: seed plenty,
/// measure the fit, trim to fit+1, reload.
#[tokio::test]
#[ignore = "browser"]
async fn toggle_last_item_out_of_filter_goes_back() -> Result<()> {
    let h = TestHarness::start().await?;
    let (store, user_id) = seed_store(&h).await?;
    seed_page_cards(&store, user_id, 30, "card-t").await?;

    goto_groom(&h).await?;
    let fit = wait_for_calibration(&h).await?;
    // Trim to exactly fit+1 cards, so the second page holds one row.
    for i in (fit + 2)..=30 {
        store
            .delete_card(user_id, &format!("card-t{i:02}"))
            .await
            .map_err(store_err)?;
    }
    h.goto("/groom").await?;
    h.wait_for_selector("#groom-search", TIMEOUT).await?;

    // The first-usage default is `all`; the fallback this test pins
    // needs the `enabled` filter so the toggle drops the row out.
    set_filter(&h, "enabled").await?;
    h.wait_for_text("#groom-page-info", &showing(0, fit, fit + 1), TIMEOUT)
        .await?;
    h.click("#groom-next").await?;
    h.wait_for_text("#groom-page-info", &showing(1, fit, fit + 1), TIMEOUT)
        .await?;
    if row_count(&h).await? != 1 {
        return Err(Error::message("page 2 should show exactly 1 row"));
    }

    // Disabling the single row of page 2 drops it out of the `enabled`
    // filter: the UI must land back on page 1 with the refreshed count.
    let last_id = format!("card-t{:02}", fit + 1);
    h.click(&format!("#toggle-disabled-{last_id}")).await?;
    h.wait_for_text("#groom-page-info", &showing(0, fit, fit), TIMEOUT)
        .await?;
    if row_count(&h).await? != fit {
        return Err(Error::message("should be back on page 1 with a full page"));
    }
    let card = store
        .get_card(user_id, &last_id)
        .await
        .map_err(store_err)?
        .ok_or_else(|| Error::message(format!("{last_id} vanished")))?;
    if !card.disabled {
        return Err(Error::message("store row should be disabled=true"));
    }
    h.screenshot("04_groom/toggle-last-item-back").await?;
    Ok(())
}

/// Paging: 12 enabled cards spread over several viewport-fitted pages;
/// the prev/next buttons and the "showing X–Y of Z" line stay in sync.
/// Page-size expectations are computed from the calibrated fit (owner
/// wish 2026-07-31: the page size is the number of rows that fit the
/// viewport), never hard-coded.
#[tokio::test]
#[ignore = "browser"]
async fn paging_walks_two_pages() -> Result<()> {
    let h = TestHarness::start().await?;
    let (store, user_id) = seed_store(&h).await?;
    seed_page_cards(&store, user_id, 12, "card-p").await?;

    goto_groom(&h).await?;
    let fit = wait_for_calibration(&h).await?;
    if fit >= 12 {
        return Err(Error::message(format!(
            "test premise: the calibrated page size {fit} must leave the 12 cards paged"
        )));
    }
    h.wait_for_text("#groom-page-info", &showing(0, fit, 12), TIMEOUT)
        .await?;
    if row_count(&h).await? != fit {
        return Err(Error::message("page 1 should show a full fitted page"));
    }
    if !h
        .eval::<bool>("document.querySelector('#groom-prev').disabled")
        .await?
    {
        return Err(Error::message("prev should be disabled on page 1"));
    }
    let results = h.text_content("#groom-results").await?;
    let last = format!("P{fit:02}");
    let beyond = format!("P{:02}", fit + 1);
    if !results.contains("P01") || !results.contains(&last) || results.contains(&beyond) {
        return Err(Error::message(format!(
            "page 1 should show P01..{last} and not {beyond}, shows: {results:?}"
        )));
    }
    h.screenshot("04_groom/paging-page1").await?;

    // Walk forward to the last page: the line and the buttons stay in
    // sync on every step.
    let last_page = 12_usize.div_ceil(fit) - 1;
    for p in 1..=last_page {
        h.click("#groom-next").await?;
        h.wait_for_text("#groom-page-info", &showing(p, fit, 12), TIMEOUT)
            .await?;
    }
    if !h
        .eval::<bool>("document.querySelector('#groom-next').disabled")
        .await?
    {
        return Err(Error::message("next should be disabled on the last page"));
    }
    let rows = row_count(&h).await?;
    if rows != 12 - last_page * fit {
        return Err(Error::message(format!(
            "the last page should show {} rows, shows {rows}",
            12 - last_page * fit
        )));
    }
    h.screenshot("04_groom/paging-page2").await?;

    h.click("#groom-prev").await?;
    h.wait_for_text(
        "#groom-page-info",
        &showing(last_page - 1, fit, 12),
        TIMEOUT,
    )
    .await?;
    Ok(())
}

/// Enable/disable toggle: the store row follows each click immediately.
/// Under the `enabled` filter disabling a card removes its row
/// (it no longer matches the filter); the `disabled` filter then shows it
/// with the badge and the `Enable` action, and enabling it there removes
/// it again.
#[tokio::test]
#[ignore = "browser"]
async fn disable_enable_toggle() -> Result<()> {
    let h = TestHarness::start().await?;
    let (store, user_id) = seed_store(&h).await?;
    let now = now_ms();
    seed_card(
        &store,
        user_id,
        "card-toggle",
        "Toggle prompt",
        "Toggle solution",
        CardState::New,
        now,
        now + 60_000,
        false,
    )
    .await?;

    goto_groom(&h).await?;
    // The first-usage default is `all`; this test is about the
    // filter interplay, so it starts from `enabled`.
    set_filter(&h, "enabled").await?;
    h.wait_for_selector("#toggle-disabled-card-toggle", TIMEOUT)
        .await?;

    // Disable: the row leaves the default `enabled` filter, and the
    // store row follows.
    h.click("#toggle-disabled-card-toggle").await?;
    wait_until_gone(&h, "#groom-row-card-toggle", TIMEOUT).await?;
    let card = store
        .get_card(user_id, "card-toggle")
        .await
        .map_err(store_err)?
        .ok_or_else(|| Error::message("card-toggle vanished"))?;
    if !card.disabled {
        return Err(Error::message("store row should be disabled=true"));
    }

    // The `disabled` filter shows the card: badge, `Enable` label.
    set_filter(&h, "disabled").await?;
    h.wait_for_selector("#disabled-card-toggle", TIMEOUT)
        .await?;
    if h.text_content("#toggle-disabled-card-toggle").await? != "Enable" {
        return Err(Error::message("toggle should read Enable once disabled"));
    }
    h.screenshot("04_groom/disabled-badge").await?;

    // Enable again: the row leaves the `disabled` filter, the store row
    // follows, and the `enabled` filter shows the card badge-free.
    h.click("#toggle-disabled-card-toggle").await?;
    wait_until_gone(&h, "#groom-row-card-toggle", TIMEOUT).await?;
    let card = store
        .get_card(user_id, "card-toggle")
        .await
        .map_err(store_err)?
        .ok_or_else(|| Error::message("card-toggle vanished"))?;
    if card.disabled {
        return Err(Error::message("store row should be disabled=false"));
    }
    set_filter(&h, "enabled").await?;
    h.wait_for_selector("#toggle-disabled-card-toggle", TIMEOUT)
        .await?;
    if h.eval::<bool>("!!document.querySelector('#disabled-card-toggle')")
        .await?
    {
        return Err(Error::message(
            "enabled card must not show a disabled badge",
        ));
    }
    if h.text_content("#toggle-disabled-card-toggle").await? != "Disable" {
        return Err(Error::message("toggle should read Disable once enabled"));
    }
    Ok(())
}

/// Delete: the confirm modal shows the prompt, Cancel keeps the card,
/// Delete removes it from the list and the store.
#[tokio::test]
#[ignore = "browser"]
async fn delete_with_confirm_modal() -> Result<()> {
    let h = TestHarness::start().await?;
    let (store, user_id) = seed_store(&h).await?;
    let now = now_ms();
    seed_card(
        &store,
        user_id,
        "card-delete",
        "Doomed prompt",
        "Doomed solution",
        CardState::New,
        now,
        now + 60_000,
        false,
    )
    .await?;

    goto_groom(&h).await?;
    h.wait_for_selector("#groom-row-card-delete", TIMEOUT)
        .await?;

    click_row_menu_item(&h, "card-delete", "#delete-card-delete").await?;
    h.wait_for_selector("#groom-modal", TIMEOUT).await?;
    let modal = h.text_content("#groom-modal").await?;
    if !modal.contains("Really delete this card?") || !modal.contains("Doomed prompt") {
        return Err(Error::message(format!(
            "modal should ask about the card, shows: {modal:?}"
        )));
    }
    h.screenshot("04_groom/delete-modal").await?;

    h.click("#modal-cancel").await?;
    wait_until_gone(&h, "#groom-modal", TIMEOUT).await?;
    h.wait_for_selector("#groom-row-card-delete", TIMEOUT)
        .await?;

    click_row_menu_item(&h, "card-delete", "#delete-card-delete").await?;
    h.wait_for_selector("#groom-modal", TIMEOUT).await?;
    h.click("#modal-confirm").await?;
    h.wait_for_text("#groom-empty", "No cards match", TIMEOUT)
        .await?;
    h.screenshot("04_groom/delete-done").await?;
    if store
        .get_card(user_id, "card-delete")
        .await
        .map_err(store_err)?
        .is_some()
    {
        return Err(Error::message("store row should be gone after delete"));
    }
    Ok(())
}

/// Deleting the last row of page 2 lands back on page 1 with the
/// refreshed count. The single-row second page is constructed relative
/// to the calibrated viewport fit (seed plenty, measure, trim to fit+1,
/// reload).
#[tokio::test]
#[ignore = "browser"]
async fn delete_last_item_on_page_goes_back() -> Result<()> {
    let h = TestHarness::start().await?;
    let (store, user_id) = seed_store(&h).await?;
    seed_page_cards(&store, user_id, 30, "card-q").await?;

    goto_groom(&h).await?;
    let fit = wait_for_calibration(&h).await?;
    for i in (fit + 2)..=30 {
        store
            .delete_card(user_id, &format!("card-q{i:02}"))
            .await
            .map_err(store_err)?;
    }
    h.goto("/groom").await?;
    h.wait_for_selector("#groom-search", TIMEOUT).await?;

    h.wait_for_text("#groom-page-info", &showing(0, fit, fit + 1), TIMEOUT)
        .await?;
    h.click("#groom-next").await?;
    h.wait_for_text("#groom-page-info", &showing(1, fit, fit + 1), TIMEOUT)
        .await?;
    if row_count(&h).await? != 1 {
        return Err(Error::message("page 2 should show exactly 1 row"));
    }

    let last_id = format!("card-q{:02}", fit + 1);
    click_row_menu_item(&h, &last_id, &format!("#delete-{last_id}")).await?;
    h.wait_for_selector("#groom-modal", TIMEOUT).await?;
    h.click("#modal-confirm").await?;
    h.wait_for_text("#groom-page-info", &showing(0, fit, fit), TIMEOUT)
        .await?;
    if row_count(&h).await? != fit {
        return Err(Error::message("should be back on page 1 with a full page"));
    }
    let (_cards, count) = store
        .search_cards(user_id, None, DisabledFilter::All, 0, 100)
        .await
        .map_err(store_err)?;
    if count != i64::try_from(fit).unwrap_or(i64::MAX) {
        return Err(Error::message(format!(
            "store should hold {fit} cards after the delete, holds {count}"
        )));
    }
    h.screenshot("04_groom/delete-last-item-back").await?;
    Ok(())
}

/// Reset progress: confirm modal, badge flips to `new`, and the store row
/// is rescheduled to `now + 30 min`.
#[tokio::test]
#[ignore = "browser"]
async fn reset_progress_with_confirm_modal() -> Result<()> {
    let h = TestHarness::start().await?;
    let (store, user_id) = seed_store(&h).await?;
    let now = now_ms();
    seed_card(
        &store,
        user_id,
        "card-reset",
        "Reset prompt",
        "Reset solution",
        CardState::Ok,
        now - 3_600_000,
        now + 3_600_000,
        false,
    )
    .await?;

    goto_groom(&h).await?;
    h.wait_for_text("#state-card-reset", "ok", TIMEOUT).await?;

    click_row_menu_item(&h, "card-reset", "#reset-card-reset").await?;
    h.wait_for_selector("#groom-modal", TIMEOUT).await?;
    let modal = h.text_content("#groom-modal").await?;
    if !modal.contains("Reset learning progress for this card?") || !modal.contains("Reset prompt")
    {
        return Err(Error::message(format!(
            "modal should ask about the reset, shows: {modal:?}"
        )));
    }
    h.screenshot("04_groom/reset-modal").await?;

    h.click("#modal-confirm").await?;
    h.wait_for_text("#state-card-reset", "new", TIMEOUT).await?;
    h.screenshot("04_groom/reset-done").await?;

    let card = store
        .get_card(user_id, "card-reset")
        .await
        .map_err(store_err)?
        .ok_or_else(|| Error::message("card-reset vanished"))?;
    if card.state != CardState::New {
        return Err(Error::message(format!(
            "state should be new after reset, is {:?}",
            card.state
        )));
    }
    // The server sets change_time = now and next_time = now + 30 min with
    // the same clock reading, so the difference is exact up to rounding.
    let waiting = card.next_time - card.change_time;
    if (waiting - 1_800_000).abs() > 3_000 {
        return Err(Error::message(format!(
            "next_time should be ≈ change_time + 1800000 ms, difference is {waiting} ms"
        )));
    }
    if card.next_time <= now_ms() {
        return Err(Error::message("next_time should be in the future"));
    }
    Ok(())
}

/// The money test of the slice: a disabled due card is not quizzable;
/// enabling it in Groom makes it appear in the Quiz tab.
#[tokio::test]
#[ignore = "browser"]
async fn disabled_card_not_quizzable_until_enabled() -> Result<()> {
    let h = TestHarness::start().await?;
    let (store, user_id) = seed_store(&h).await?;
    let now = now_ms();
    seed_card(
        &store,
        user_id,
        "card-cross",
        "Cross feature prompt",
        "Cross feature solution",
        CardState::New,
        now - 60_000,
        now - 1_000,
        true,
    )
    .await?;

    h.goto("/").await?;
    h.wait_for_text("#quiz-done", "All done", TIMEOUT).await?;
    h.screenshot("04_groom/cross-quiz-done").await?;

    h.click("#tab-groom").await?;
    // The card starts disabled; the first-usage default filter `all`
    // lists it for the toggle.
    h.wait_for_selector("#toggle-disabled-card-cross", TIMEOUT)
        .await?;
    h.click("#toggle-disabled-card-cross").await?;
    wait_until_gone(&h, "#disabled-card-cross", TIMEOUT).await?;

    h.click("#tab-quiz").await?;
    h.wait_for_text("#quiz-prompt", "Cross feature prompt", TIMEOUT)
        .await?;
    h.screenshot("04_groom/cross-quizzable").await?;
    Ok(())
}

/// Owner decision (2026-07-27): full delete stays available for learned
/// cards, but the confirmation must surface the existing progress. A new
/// (unlearned) card's modal shows no such warning.
#[tokio::test]
#[ignore = "browser"]
async fn delete_modal_warns_about_existing_progress() -> Result<()> {
    let h = TestHarness::start().await?;
    let (store, user_id) = seed_store(&h).await?;
    let now = now_ms();
    seed_card(
        &store,
        user_id,
        "card-learned",
        "Learned prompt",
        "Learned solution",
        CardState::Ok,
        now - 3_600_000,
        now + 86_400_000,
        false,
    )
    .await?;
    seed_card(
        &store,
        user_id,
        "card-fresh",
        "Fresh prompt",
        "Fresh solution",
        CardState::New,
        now,
        now + 60_000,
        false,
    )
    .await?;

    goto_groom(&h).await?;
    h.wait_for_selector("#groom-row-card-fresh", TIMEOUT)
        .await?;

    // Learned card: warning names the state and the permanence.
    click_row_menu_item(&h, "card-learned", "#delete-card-learned").await?;
    h.wait_for_selector("#modal-progress-warning", TIMEOUT)
        .await?;
    let warning = h.text_content("#modal-progress-warning").await?;
    if !warning.contains("learning progress") || !warning.contains("ok") {
        return Err(Error::message(format!(
            "warning should mention progress and state, shows: {warning:?}"
        )));
    }
    h.screenshot("04_groom/delete-modal-progress-warning")
        .await?;
    h.click("#modal-cancel").await?;
    wait_until_gone(&h, "#groom-modal", TIMEOUT).await?;

    // Fresh card: same modal, no warning.
    click_row_menu_item(&h, "card-fresh", "#delete-card-fresh").await?;
    h.wait_for_selector("#groom-modal", TIMEOUT).await?;
    let modal = h.text_content("#groom-modal").await?;
    if modal.contains("learning progress") {
        return Err(Error::message(format!(
            "new card's modal must not warn about progress, shows: {modal:?}"
        )));
    }
    h.click("#modal-cancel").await?;
    Ok(())
}

/// Row "⋯" overflow menu: closed initially, opens on the trigger with
/// both destructive actions (and tracks aria-expanded), dismisses on a
/// backdrop click without arming the modal, and closes again when an
/// item is chosen (which arms the confirm modal). Also pins the owner
/// requirement behind the redesign: the meta line — badges + due +
/// actions — stays on ONE line even at mobile width for the common
/// worst case (new + disabled; fresh cards start disabled).
#[tokio::test]
#[ignore = "browser"]
async fn row_menu_opens_and_dismisses() -> Result<()> {
    let h = TestHarness::start().await?;
    let (store, user_id) = seed_store(&h).await?;
    let now = now_ms();
    seed_card(
        &store,
        user_id,
        "card-menu",
        "Menu prompt",
        "Menu solution",
        CardState::New,
        now,
        now + 60_000,
        true,
    )
    .await?;

    goto_groom(&h).await?;
    // Wait out the viewport-fit calibration: its corrective refetch
    // briefly replaces the rows with the Loading state, which would
    // race the one-shot evals below.
    let fit = wait_for_calibration(&h).await?;
    // The card is seeded disabled (new + disabled is the widest badge
    // combination); the first-usage default filter `all` lists it.
    h.wait_for_selector("#menu-card-menu", TIMEOUT).await?;
    if h.eval::<bool>("!!document.querySelector('.groom-menu')")
        .await?
    {
        return Err(Error::message("menu should start closed"));
    }
    if h.eval::<String>(
        "document.querySelector('#menu-card-menu').getAttribute('aria-expanded') ?? '<missing>'",
    )
    .await?
        != "false"
    {
        return Err(Error::message("aria-expanded should be false while closed"));
    }

    // One-line meta row, checked at desktop width and at 390px mobile
    // (new + disabled is the common widest combination; fresh cards
    // start disabled).
    let probe = h.eval::<String>(META_PROBE).await?;
    if !probe.contains("\"same_line\":true") {
        return Err(Error::message(format!(
            "meta line wrapped at desktop width: {probe}"
        )));
    }
    h.set_viewport(390, 844).await?;
    // The viewport change makes the groom tab re-fit its page size (a
    // refetch briefly replaces the rows with the Loading state): wait
    // for the re-fit to settle before probing the row layout.
    wait_for_refit(&h, fit).await?;
    h.wait_for_selector("#groom-row-card-menu", TIMEOUT).await?;
    let probe = h.eval::<String>(META_PROBE).await?;
    if !probe.contains("\"same_line\":true") {
        return Err(Error::message(format!(
            "meta line wrapped at 390px for a new + disabled card: {probe}"
        )));
    }

    // Open: both destructive actions show, aria-expanded follows.
    h.click("#menu-card-menu").await?;
    h.wait_for_selector("#reset-card-menu", TIMEOUT).await?;
    h.wait_for_selector("#delete-card-menu", TIMEOUT).await?;
    if h.eval::<String>(
        "document.querySelector('#menu-card-menu').getAttribute('aria-expanded') ?? '<missing>'",
    )
    .await?
        != "true"
    {
        return Err(Error::message("aria-expanded should be true while open"));
    }
    h.screenshot("04_groom/row-menu-open").await?;

    // A backdrop click dismisses the menu without arming any modal.
    h.click(".groom-menu-backdrop").await?;
    wait_until_gone(&h, ".groom-menu", TIMEOUT).await?;
    if h.eval::<bool>("!!document.querySelector('#groom-modal')")
        .await?
    {
        return Err(Error::message("backdrop dismiss must not arm the modal"));
    }

    // Choosing an item closes the menu and arms the confirm modal.
    h.click("#menu-card-menu").await?;
    h.wait_for_selector("#delete-card-menu", TIMEOUT).await?;
    h.click("#delete-card-menu").await?;
    wait_until_gone(&h, ".groom-menu", TIMEOUT).await?;
    h.wait_for_selector("#groom-modal", TIMEOUT).await?;
    h.click("#modal-cancel").await?;
    Ok(())
}

/// Viewport fit (owner wish 2026-07-31): the groom page size is the
/// number of rows that fill the viewport — the page needs no vertical
/// scrolling AND does not underfill (the leftover below the last row is
/// less than one row pitch).
#[tokio::test]
#[ignore = "browser"]
async fn viewport_fit_fills_page_without_scroll_or_underfill() -> Result<()> {
    let h = TestHarness::start().await?;
    let (store, user_id) = seed_store(&h).await?;
    seed_page_cards(&store, user_id, 30, "card-fit-").await?;

    goto_groom(&h).await?;
    let fit = wait_for_calibration(&h).await?;
    if !(2..30).contains(&fit) {
        return Err(Error::message(format!(
            "test premise: the calibrated page size should be 2..30, is {fit}"
        )));
    }
    h.wait_for_text("#groom-page-info", &showing(0, fit, 30), TIMEOUT)
        .await?;
    if row_count(&h).await? != fit {
        return Err(Error::message(
            "the first page should show exactly the fitted rows",
        ));
    }
    // No vertical scrolling needed (1 px of sub-pixel slack allowed).
    if !h
        .eval::<bool>("document.scrollingElement.scrollHeight - window.innerHeight <= 1")
        .await?
    {
        return Err(Error::message(
            "the fitted page should not need vertical scrolling",
        ));
    }
    // No underfill: the slack below the last row (beyond the normal app
    // gap) cannot hold another row.
    let pitch = h
        .eval::<f64>(
            "(() => { \
             const rows = document.querySelectorAll('.groom-row'); \
             return rows[1].getBoundingClientRect().top - rows[0].getBoundingClientRect().top; \
             })()",
        )
        .await?;
    let slack = h
        .eval::<f64>(
            "(() => { \
             const rows = document.querySelectorAll('.groom-row'); \
             const last = rows[rows.length - 1].getBoundingClientRect(); \
             const footer = document.querySelector('.bottom').getBoundingClientRect(); \
             const gap = parseFloat(getComputedStyle(document.querySelector('.app')).rowGap); \
             return footer.top - last.bottom - gap; \
             })()",
        )
        .await?;
    if slack >= pitch {
        return Err(Error::message(format!(
            "underfill: another row (pitch {pitch}px) would fit into the slack {slack}px"
        )));
    }
    h.screenshot("04_groom/viewport-fit").await?;
    Ok(())
}

/// The fit sums per-row heights (prompts clamp at two lines, so rows
/// come in two heights): a page mixing one- and two-line prompts still
/// fills the viewport exactly — no scroll, and the leftover holds no
/// further row of any height (adversarial review 2026-07-31:
/// uniform-only seeding could not see the off-by-one).
#[tokio::test]
#[ignore = "browser"]
async fn viewport_fit_handles_mixed_row_heights() -> Result<()> {
    let h = TestHarness::start().await?;
    let (store, user_id) = seed_store(&h).await?;
    // Every third prompt carries a newline, so its row is one text line
    // taller than its neighbors'.
    let now = now_ms();
    for i in 1..=30_usize {
        let offset = i64::try_from(i).unwrap_or(i64::MAX) * 60_000;
        let prompt = if i % 3 == 0 {
            format!("Tall {i:02}\nsecond line")
        } else {
            format!("Short {i:02}")
        };
        seed_card(
            &store,
            user_id,
            &format!("card-m{i:02}"),
            &prompt,
            "S",
            CardState::New,
            now,
            now + offset,
            false,
        )
        .await?;
    }

    goto_groom(&h).await?;
    let fit = wait_for_calibration(&h).await?;
    if !(2..30).contains(&fit) {
        return Err(Error::message(format!(
            "test premise: the calibrated page size should be 2..30, is {fit}"
        )));
    }
    h.wait_for_text("#groom-page-info", &showing(0, fit, 30), TIMEOUT)
        .await?;
    if row_count(&h).await? != fit {
        return Err(Error::message(
            "the first page should show exactly the fitted rows",
        ));
    }
    if !h
        .eval::<bool>("document.scrollingElement.scrollHeight - window.innerHeight <= 1")
        .await?
    {
        return Err(Error::message(
            "the fitted mixed-height page should not need vertical scrolling",
        ));
    }
    // The leftover holds no further row of any height. (Every third row
    // is tall, so the tallest rendered pitch is an upper bound on the
    // next row's height.)
    let max_pitch = h
        .eval::<f64>(
            "(() => { \
             const rows = [...document.querySelectorAll('.groom-row')] \
             .map(r => r.getBoundingClientRect().height); \
             const gap = parseFloat(getComputedStyle(document.getElementById('groom-results')).rowGap); \
             return Math.max(...rows) + gap; \
             })()",
        )
        .await?;
    let slack = h
        .eval::<f64>(
            "(() => { \
             const rows = document.querySelectorAll('.groom-row'); \
             const last = rows[rows.length - 1].getBoundingClientRect(); \
             const footer = document.querySelector('.bottom').getBoundingClientRect(); \
             const gap = parseFloat(getComputedStyle(document.querySelector('.app')).rowGap); \
             return footer.top - last.bottom - gap; \
             })()",
        )
        .await?;
    if slack >= max_pitch {
        return Err(Error::message(format!(
            "underfill: another row (tallest pitch {max_pitch}px) would fit into the slack {slack}px"
        )));
    }
    h.screenshot("04_groom/viewport-fit-mixed").await?;
    Ok(())
}

/// Resizing the window re-fits the page size (debounced) — and keeps the
/// EXACT top card: the list is offset-based, so a re-fit only changes
/// how many cards show below the top one, never which card is on top
/// (owner feedback 2026-07-31).
#[tokio::test]
#[ignore = "browser"]
async fn resize_refits_and_keeps_anchor() -> Result<()> {
    let h = TestHarness::start().await?;
    let (store, user_id) = seed_store(&h).await?;
    seed_page_cards(&store, user_id, 30, "card-r").await?;

    goto_groom(&h).await?;
    let fit1 = wait_for_calibration(&h).await?;
    if fit1 < 3 {
        return Err(Error::message(format!(
            "test premise: the initial fit should be at least 3 rows, is {fit1}"
        )));
    }
    // Step one window forward, then shrink the window: the fit shrinks,
    // the offset stays, so the top card is exactly the same.
    h.click("#groom-next").await?;
    h.wait_for_text("#groom-page-info", &showing(1, fit1, 30), TIMEOUT)
        .await?;
    let anchor = format!("P{:02}", fit1 + 1);

    h.set_viewport(1280, 500).await?;
    let fit2 = wait_for_refit(&h, fit1).await?;
    if fit2 >= fit1 {
        return Err(Error::message(format!(
            "shrinking the window should shrink the fit ({fit1} -> {fit2})"
        )));
    }
    h.wait_for_text("#groom-page-info", &showing_at(fit1, fit2, 30), TIMEOUT)
        .await?;
    if row_count(&h).await? != fit2 {
        return Err(Error::message(
            "the re-fitted page should show exactly fit2 rows",
        ));
    }
    let top = h
        .eval::<String>("document.querySelector('.groom-row .groom-prompt').textContent ?? ''")
        .await?;
    if top != anchor {
        return Err(Error::message(format!(
            "the top card must stay {anchor} after the re-fit, is {top:?}"
        )));
    }
    if !h
        .eval::<bool>("document.scrollingElement.scrollHeight - window.innerHeight <= 1")
        .await?
    {
        return Err(Error::message(
            "the re-fitted page should not need vertical scrolling",
        ));
    }
    Ok(())
}

/// Sticky chrome (owner wish 2026-07-31): the header (logo + tabs) is
/// pinned on every page, and the groom search/filter + paging bar stay
/// directly below it — so the rare page that still scrolls (here forced
/// with a tiny viewport) keeps every control reachable.
#[tokio::test]
#[ignore = "browser"]
async fn sticky_chrome_keeps_header_and_controls_pinned() -> Result<()> {
    let h = TestHarness::start().await?;
    h.set_viewport(1280, 280).await?;
    let (store, user_id) = seed_store(&h).await?;
    seed_page_cards(&store, user_id, 6, "card-s").await?;

    goto_groom(&h).await?;
    let fit = wait_for_calibration(&h).await?;
    // Premise: even the fitted page overflows the tiny viewport, so the
    // page genuinely scrolls.
    if !h
        .eval::<bool>("document.scrollingElement.scrollHeight > window.innerHeight + 1")
        .await?
    {
        return Err(Error::message(
            "test premise: the tiny viewport should overflow even after the fit",
        ));
    }
    h.eval::<bool>("window.scrollTo(0, document.scrollingElement.scrollHeight); true")
        .await?;
    wait_scrolled(&h).await?;
    let probe = h
        .eval::<Vec<f64>>(
            "(() => { \
             const top = document.querySelector('.top').getBoundingClientRect(); \
             const head = document.querySelector('.groom-head').getBoundingClientRect(); \
             return [top.top, top.bottom, head.top]; \
             })()",
        )
        .await?;
    if probe[0].abs() > 1.0 {
        return Err(Error::message(format!(
            "the header should stay pinned at the viewport top, is at {}px",
            probe[0]
        )));
    }
    if (probe[2] - probe[1]).abs() > 1.0 {
        return Err(Error::message(format!(
            "the groom chrome should sit directly below the header (header bottom {}px, chrome top {}px)",
            probe[1], probe[2]
        )));
    }
    // The pinned paging buttons still work while scrolled to the bottom.
    h.click("#groom-next").await?;
    h.wait_for_text("#groom-page-info", &showing(1, fit, 6), TIMEOUT)
        .await?;
    h.screenshot("04_groom/sticky-scrolled").await?;

    // The header is sticky on the other tabs too: the Add card editor
    // overflows this viewport as well.
    h.click("#tab-add-card").await?;
    h.wait_for_selector("#new-prompt", TIMEOUT).await?;
    if !h
        .eval::<bool>("document.scrollingElement.scrollHeight > window.innerHeight + 1")
        .await?
    {
        return Err(Error::message(
            "test premise: the editor should overflow the tiny viewport",
        ));
    }
    h.eval::<bool>("window.scrollTo(0, document.scrollingElement.scrollHeight); true")
        .await?;
    wait_scrolled(&h).await?;
    let top_top = h
        .eval::<f64>("document.querySelector('.top').getBoundingClientRect().top")
        .await?;
    if top_top.abs() > 1.0 {
        return Err(Error::message(format!(
            "the header should stay pinned on the Add tab too, is at {top_top}px"
        )));
    }
    Ok(())
}
