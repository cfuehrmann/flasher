//! URL routing e2e tests (Phase 6.5): F5 keeps the current tab, deep
//! links land on their tab (also through the auth flow), browser
//! back/forward walks the tab history, and the groom paging bar sits
//! above the card list. The router is hand-rolled on the History API
//! (pushState on tab switch, popstate for back/forward, one replaceState
//! canonicalizing `/` to `/quiz`) — deliberately no `leptos_router`.

use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use flasher_e2e::{E2E_USER, Error, Result, TestHarness};
use flasher_store::{CardState, NewCard, Store};

/// Timeout for every DOM wait; generous because the wasm bundle has to
/// download and boot first (same reasoning as the harness default).
const TIMEOUT: Duration = Duration::from_secs(15);

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

/// Seeds `n` enabled state=new cards `P01..Pn` (`card-p01..`) with
/// ascending `next_time` values, so the list order is deterministic.
async fn seed_page_cards(store: &Store, user_id: i64, n: usize) -> Result<()> {
    let now = now_ms();
    for i in 1..=n {
        let offset = i64::try_from(i).unwrap_or(i64::MAX) * 60_000;
        store
            .insert_card(&NewCard {
                user_id,
                id: format!("card-p{i:02}"),
                prompt: format!("P{i:02}"),
                solution: format!("S{i:02}"),
                state: CardState::New,
                change_time: now,
                next_time: now + offset,
                labels: vec!["Enabled".to_owned()],
            })
            .await
            .map_err(store_err)?;
    }
    Ok(())
}

/// Waits until the groom viewport-fit calibration has run and persisted
/// the fitted page size, and returns it (paging expectations are
/// computed from the measured fit, never hard-coded).
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

/// The "showing first–last of count" line for page `page` (0-based) of
/// `count` cards at page size `fit`.
fn showing(page: usize, fit: usize, count: usize) -> String {
    let first = page * fit + 1;
    let last = ((page + 1) * fit).min(count);
    format!("showing {first}–{last} of {count}")
}

/// Polls `location.pathname` until it equals `path` (a tab switch's
/// `pushState` and `history.back()` are fire-and-forget).
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

/// Asserts the tab button `sel` carries the `active` class.
async fn assert_tab_active(h: &TestHarness, sel: &str) -> Result<()> {
    // Content can be restored before the nav buttons are patched after a
    // reload; wait for the element before reading its class list.
    h.wait_for_selector(sel, TIMEOUT).await?;
    let active: bool = h
        .eval(&format!(
            "document.querySelector({sel:?}).classList.contains('active')"
        ))
        .await?;
    if !active {
        return Err(Error::message(format!("{sel} should be active")));
    }
    Ok(())
}

/// The shared shell keeps the current top-level page title beside the logo.
async fn assert_page_heading(h: &TestHarness, title: &str) -> Result<()> {
    h.wait_for_text("header.top h1", title, TIMEOUT).await?;
    let rendered: String = h
        .eval("document.querySelector('header.top h1')?.textContent?.trim() ?? ''")
        .await?;
    if rendered != title {
        return Err(Error::message(format!(
            "page heading is {rendered:?}, expected {title:?}"
        )));
    }
    Ok(())
}

/// Reloads the app at `path` (a real navigation, equivalent to F5) and
/// waits until the tab content marker and the active tab button prove
/// the reload kept the tab.
async fn reload_and_expect_tab(
    h: &TestHarness,
    path: &str,
    content_sel: &str,
    tab_sel: &str,
) -> Result<()> {
    h.goto(path).await?;
    wait_for_path(h, path).await?;
    h.wait_for_selector(content_sel, TIMEOUT).await?;
    assert_tab_active(h, tab_sel).await?;
    let title = match path {
        "/add" => "Add card",
        "/groom" => "Groom",
        "/labels" => "Labels",
        "/account" => "Account",
        _ => "Quiz",
    };
    assert_page_heading(h, title).await
}

/// F5 keeps the current tab: switch to each tab, reload at its URL, and
/// the same tab (content marker + active nav button) comes back.
#[tokio::test]
#[ignore = "browser"]
async fn reload_keeps_tab() -> Result<()> {
    let h = TestHarness::start().await?;

    // Groom.
    h.click("#tab-groom").await?;
    h.wait_for_selector("#groom-search", TIMEOUT).await?;
    wait_for_path(&h, "/groom").await?;
    reload_and_expect_tab(&h, "/groom", "#groom-search", "#tab-groom").await?;

    // Add card (the editor in new-card mode).
    h.click("#tab-add-card").await?;
    h.wait_for_selector("#new-prompt", TIMEOUT).await?;
    wait_for_path(&h, "/add").await?;
    reload_and_expect_tab(&h, "/add", "#new-prompt", "#tab-add-card").await?;

    // Account (dev-bypass mode: no logout button, but the identity card).
    h.click("#tab-account").await?;
    h.wait_for_selector("#account-username", TIMEOUT).await?;
    wait_for_path(&h, "/account").await?;
    reload_and_expect_tab(&h, "/account", "#account-username", "#tab-account").await?;

    // Labels uses the same shared heading, with no duplicate in-page h1.
    h.click("#tab-labels").await?;
    h.wait_for_selector("#labels-page", TIMEOUT).await?;
    wait_for_path(&h, "/labels").await?;
    reload_and_expect_tab(&h, "/labels", "#labels-page", "#tab-labels").await?;
    let duplicate_heading: bool = h
        .eval("!!document.querySelector('#labels-page h1')")
        .await?;
    if duplicate_heading {
        return Err(Error::message(
            "Labels should use the shared top heading instead of a duplicate in-page h1",
        ));
    }

    // And `/` still canonicalizes to the quiz.
    h.goto("/").await?;
    wait_for_path(&h, "/quiz").await?;
    assert_tab_active(&h, "#tab-quiz").await?;
    Ok(())
}

/// A fresh page load of `/groom` lands directly on the Groom tab: the
/// quiz is never rendered (the initial tab comes from the URL, not from
/// a post-boot redirect).
#[tokio::test]
#[ignore = "browser"]
async fn deep_link_lands_on_tab() -> Result<()> {
    let h = TestHarness::start().await?;
    h.goto("/groom").await?;
    h.wait_for_selector("#groom-search", TIMEOUT).await?;
    wait_for_path(&h, "/groom").await?;
    assert_tab_active(&h, "#tab-groom").await?;
    let quiz_rendered: bool = h
        .eval("!!document.querySelector('#quiz-prompt, #quiz-done, #quiz-loading')")
        .await?;
    if quiz_rendered {
        return Err(Error::message(
            "deep link to /groom must not render the quiz",
        ));
    }
    // Unknown paths serve the SPA and fall back to the quiz tab.
    h.goto("/no-such-route").await?;
    h.wait_for_selector("#tab-quiz", TIMEOUT).await?;
    assert_tab_active(&h, "#tab-quiz").await?;
    Ok(())
}

/// Browser Back/Forward walk the tab history: quiz → groom → account by
/// clicks, then Back to groom, Back to quiz, Forward to groom.
#[tokio::test]
#[ignore = "browser"]
async fn back_forward_navigation() -> Result<()> {
    let h = TestHarness::start().await?;
    h.wait_for_selector("#tab-quiz", TIMEOUT).await?;
    // `/` was canonicalized with replaceState, so the stack starts /quiz.
    wait_for_path(&h, "/quiz").await?;

    h.click("#tab-groom").await?;
    wait_for_path(&h, "/groom").await?;
    h.click("#tab-account").await?;
    wait_for_path(&h, "/account").await?;
    h.wait_for_selector("#account-username", TIMEOUT).await?;

    // Back → groom. (`history.back()` returns undefined, which the CDP
    // evaluate path cannot hand back — wrap it in a boolean.)
    let _: bool = h.eval("(() => { history.back(); return true; })()").await?;
    wait_for_path(&h, "/groom").await?;
    h.wait_for_selector("#groom-search", TIMEOUT).await?;
    assert_tab_active(&h, "#tab-groom").await?;

    // Back → quiz.
    let _: bool = h.eval("(() => { history.back(); return true; })()").await?;
    wait_for_path(&h, "/quiz").await?;
    assert_tab_active(&h, "#tab-quiz").await?;

    // Forward → groom.
    let _: bool = h
        .eval("(() => { history.forward(); return true; })()")
        .await?;
    wait_for_path(&h, "/groom").await?;
    h.wait_for_selector("#groom-search", TIMEOUT).await?;
    assert_tab_active(&h, "#tab-groom").await?;
    Ok(())
}

/// An unauthenticated deep link shows the auth screen first; after
/// register + login the app lands on the requested tab, not the quiz.
#[tokio::test]
#[ignore = "browser"]
async fn auth_mode_deep_link() -> Result<()> {
    let h = TestHarness::start_with_auth().await?;
    h.add_virtual_authenticator().await?;

    h.goto("/account").await?;
    // First run: the register variant of the auth screen.
    h.wait_for_selector("#register-username", TIMEOUT).await?;
    h.type_into("#register-username", "deepuser").await?;
    h.click("#create-passkey").await?;
    h.wait_for_selector("#sign-in", TIMEOUT).await?;
    h.click("#sign-in").await?;

    // Lands on Account (the stashed deep link), not the quiz.
    h.wait_for_text("#account-username", "deepuser", TIMEOUT)
        .await?;
    wait_for_path(&h, "/account").await?;
    assert_tab_active(&h, "#tab-account").await?;
    let quiz_rendered: bool = h
        .eval("!!document.querySelector('#quiz-prompt, #quiz-done, #quiz-loading')")
        .await?;
    if quiz_rendered {
        return Err(Error::message(
            "login after a deep link must land on the requested tab",
        ));
    }
    h.screenshot("08_routing/auth-deep-link-account").await?;
    Ok(())
}

/// Back/Forward on the auth screen must not clobber the stashed deep
/// link. (`popstate` only fires for history entries of the SAME document
/// — a full-page Back to a previous load just reloads and re-reads the
/// URL — so the walk is built with `pushState` inside the page: visit
/// /quiz, return to /account, then Back to /quiz.) After register +
/// login the app lands on Account — the tab comes from the location
/// re-read at login, not from a popstate-clobbered signal.
#[tokio::test]
#[ignore = "browser"]
async fn auth_screen_popstate_keeps_deep_link() -> Result<()> {
    let h = TestHarness::start_with_auth().await?;
    h.add_virtual_authenticator().await?;

    // The deep link: boot on /account, auth screen shows (stash: Account).
    h.goto("/account").await?;
    h.wait_for_selector("#register-username", TIMEOUT).await?;

    // Same-document history walk: /account → /quiz → /account, then Back
    // to /quiz. The popstate must be ignored while unauthenticated.
    let _: bool = h
        .eval(
            "(() => { history.pushState(null, '', '/quiz'); \
             history.pushState(null, '', '/account'); return true; })()",
        )
        .await?;
    wait_for_path(&h, "/account").await?;
    let _: bool = h.eval("(() => { history.back(); return true; })()").await?;
    wait_for_path(&h, "/quiz").await?;
    // Restore the deep link's URL without firing popstate.
    let _: bool = h
        .eval("(() => { history.pushState(null, '', '/account'); return true; })()")
        .await?;
    wait_for_path(&h, "/account").await?;

    // Register + login: lands on /account's tab, not where Back pointed.
    h.type_into("#register-username", "stashuser").await?;
    h.click("#create-passkey").await?;
    h.wait_for_selector("#sign-in", TIMEOUT).await?;
    h.click("#sign-in").await?;

    h.wait_for_text("#account-username", "stashuser", TIMEOUT)
        .await?;
    wait_for_path(&h, "/account").await?;
    assert_tab_active(&h, "#tab-account").await?;
    let quiz_rendered: bool = h
        .eval("!!document.querySelector('#quiz-prompt, #quiz-done, #quiz-loading')")
        .await?;
    if quiz_rendered {
        return Err(Error::message(
            "login after Back on the auth screen must land on the stashed deep link",
        ));
    }
    h.screenshot("08_routing/auth-popstate-kept-account")
        .await?;
    Ok(())
}

/// The groom paging bar sits ABOVE the card list (owner complaint: on a
/// full page it used to be a scroll away below it) — and paging through
/// it still works.
#[tokio::test]
#[ignore = "browser"]
async fn groom_paging_bar_on_top() -> Result<()> {
    let h = TestHarness::start().await?;
    let (store, user_id) = seed_store(&h).await?;
    seed_page_cards(&store, user_id, 12).await?;

    h.click("#tab-groom").await?;
    let fit = wait_for_calibration(&h).await?;
    if fit >= 12 {
        return Err(Error::message(format!(
            "test premise: the calibrated page size {fit} must leave the 12 cards paged"
        )));
    }
    h.wait_for_text("#groom-page-info", &showing(0, fit, 12), TIMEOUT)
        .await?;

    // DOM order: the paging bar precedes the first card row.
    let bar_before_list: bool = h
        .eval(
            "(() => { \
             const bar = document.querySelector('.groom-paging'); \
             const row = document.querySelector('.groom-row'); \
             const search = document.querySelector('#groom-search'); \
             return !!(bar && row && search \
             && (bar.compareDocumentPosition(row) & Node.DOCUMENT_POSITION_FOLLOWING) \
             && (search.compareDocumentPosition(bar) & Node.DOCUMENT_POSITION_FOLLOWING)); \
             })()",
        )
        .await?;
    if !bar_before_list {
        return Err(Error::message(
            "paging bar should sit between the search field and the first card row",
        ));
    }
    h.screenshot("08_routing/groom-paging-top").await?;

    // Paging from the top bar still works.
    h.click("#groom-next").await?;
    h.wait_for_text("#groom-page-info", &showing(1, fit, 12), TIMEOUT)
        .await?;
    let rows: usize = h
        .eval("document.querySelectorAll('.groom-row').length")
        .await?;
    let want = fit.min(12 - fit);
    if rows != want {
        return Err(Error::message(format!(
            "page 2 should show {want} rows, shows {rows}"
        )));
    }
    h.screenshot("08_routing/groom-paging-top-page2").await?;
    Ok(())
}

/// The `.value` of a textarea/input.
async fn field_value(h: &TestHarness, sel: &str) -> Result<String> {
    h.eval::<String>(&format!("document.querySelector({sel:?}).value"))
        .await
}

/// Clicking the already-active tab must be a no-op while no editor
/// overlay is open (F3): the Add tab's editor keeps everything typed
/// (no remount wiping the unsaved state) and no recovery banner pops up
/// from the orphaned-draft re-check.
#[tokio::test]
#[ignore = "browser"]
async fn active_tab_click_preserves_add_draft() -> Result<()> {
    let h = TestHarness::start().await?;

    h.goto("/add").await?;
    h.wait_for_selector("#new-prompt", TIMEOUT).await?;
    h.type_into("#new-prompt", "Typed but unsaved").await?;

    // Click the active Add card tab: nothing may change.
    h.click("#tab-add-card").await?;
    tokio::time::sleep(Duration::from_millis(500)).await;
    let prompt = field_value(&h, "#new-prompt").await?;
    if prompt != "Typed but unsaved" {
        return Err(Error::message(format!(
            "clicking the active Add tab must not wipe the editor, shows: {prompt:?}"
        )));
    }
    wait_for_path(&h, "/add").await?;
    if h.eval::<bool>("!!document.querySelector('#recovery-banner')")
        .await?
    {
        return Err(Error::message(
            "clicking the active Add tab must not prompt a recovery banner",
        ));
    }
    Ok(())
}

/// Same guard on the Groom tab (F3): an active-tab click must not
/// remount the view — the search text and the current page survive.
#[tokio::test]
#[ignore = "browser"]
async fn active_tab_click_preserves_groom_state() -> Result<()> {
    let h = TestHarness::start().await?;
    let (store, user_id) = seed_store(&h).await?;
    // Twelve cards matching the search "x" plus three non-matching, so
    // the filtered page info ("…of 12") is distinguishable from the
    // unfiltered one ("…of 15") and the debounced search fetch has
    // provably landed before the test pages.
    let now = now_ms();
    for i in 1..=12_usize {
        let offset = i64::try_from(i).unwrap_or(i64::MAX) * 60_000;
        store
            .insert_card(&NewCard {
                user_id,
                id: format!("card-x{i:02}"),
                prompt: format!("xP{i:02}"),
                solution: format!("xS{i:02}"),
                state: CardState::New,
                change_time: now,
                next_time: now + offset,
                labels: vec!["Enabled".to_owned()],
            })
            .await
            .map_err(store_err)?;
    }
    for i in 1..=3_usize {
        let offset = i64::try_from(i).unwrap_or(i64::MAX) * 60_000;
        store
            .insert_card(&NewCard {
                user_id,
                id: format!("card-y{i:02}"),
                prompt: format!("yP{i:02}"),
                solution: format!("yS{i:02}"),
                state: CardState::New,
                change_time: now,
                next_time: now + offset,
                labels: vec!["Enabled".to_owned()],
            })
            .await
            .map_err(store_err)?;
    }

    h.goto("/groom").await?;
    h.wait_for_selector("#groom-search", TIMEOUT).await?;
    let fit = wait_for_calibration(&h).await?;
    if fit >= 12 {
        return Err(Error::message(format!(
            "test premise: the calibrated page size {fit} must leave the 12 matching cards paged"
        )));
    }
    h.type_into("#groom-search", "x").await?;
    h.wait_for_text("#groom-page-info", &showing(0, fit, 12), TIMEOUT)
        .await?;
    h.click("#groom-next").await?;
    h.wait_for_text("#groom-page-info", &showing(1, fit, 12), TIMEOUT)
        .await?;

    // Click the active Groom tab: search text and page survive.
    h.click("#tab-groom").await?;
    tokio::time::sleep(Duration::from_millis(700)).await;
    let search = field_value(&h, "#groom-search").await?;
    if search != "x" {
        return Err(Error::message(format!(
            "clicking the active Groom tab must not clear the search, shows: {search:?}"
        )));
    }
    h.wait_for_text("#groom-page-info", &showing(1, fit, 12), TIMEOUT)
        .await?;
    wait_for_path(&h, "/groom").await?;
    Ok(())
}
