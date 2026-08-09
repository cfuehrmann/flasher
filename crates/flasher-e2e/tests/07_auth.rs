//! Passkey authentication e2e tests (Phase 5B): the full `WebAuthn`
//! ceremony driven through the browser against a CDP virtual
//! authenticator, plus the account tab's passkey management and the
//! auth-mode API gate. The server runs in auth mode
//! ([`TestHarness::start_with_auth`], `FLASHER_USER` unset) on a
//! localhost origin — `WebAuthn` needs a registrable domain matching the
//! relying-party config.

use std::time::{Duration, Instant};

use flasher_e2e::{Error, Result, TestHarness};

/// Timeout for every DOM wait; generous because the wasm bundle has to
/// download and boot first (same reasoning as the harness default).
const TIMEOUT: Duration = Duration::from_secs(15);

// The error is only formatted, but `map_err` needs an owned receiver.
#[allow(clippy::needless_pass_by_value)]
fn store_err(err: flasher_store::Error) -> Error {
    Error::message(format!("store error: {err}"))
}

/// Drives the full first-run flow: register a passkey for `username`,
/// then sign in with it. Lands in the app (Quiz tab). Returns the
/// virtual authenticator's id (callers adding a second passkey need it,
/// see [`swap_to_fresh_authenticator`]).
async fn register_and_login(h: &TestHarness, username: &str) -> Result<String> {
    let authenticator = h.add_virtual_authenticator().await?;
    h.wait_for_selector("#register-username", TIMEOUT).await?;
    h.type_into("#register-username", username).await?;
    h.click("#create-passkey").await?;
    // Register/finish does not log in: the screen flips to login.
    h.wait_for_selector("#sign-in", TIMEOUT).await?;
    h.click("#sign-in").await?;
    h.wait_for_selector("#tab-quiz", TIMEOUT).await?;
    Ok(authenticator)
}

/// Replaces the authenticator that holds the first passkey with a fresh
/// one. An authenticator refuses `create()` when it holds a credential
/// listed in `excludeCredentials` (the server sends the existing
/// passkeys there in the add-another flow), so the second passkey must
/// be created on a different authenticator.
async fn swap_to_fresh_authenticator(h: &TestHarness, old_id: &str) -> Result<()> {
    h.remove_virtual_authenticator(old_id).await?;
    h.add_virtual_authenticator().await?;
    Ok(())
}

/// Selects the whole content of the input matching `sel` and types
/// `text`, replacing it (plain typing would append to the current
/// value).
async fn replace_input_text(h: &TestHarness, sel: &str, text: &str) -> Result<()> {
    h.wait_for_selector(sel, TIMEOUT).await?;
    let _: bool = h
        .eval(&format!(
            "(() => {{ const el = document.querySelector({sel:?}); \
             el.focus(); el.select(); return true; }})()"
        ))
        .await?;
    let el = h.page.find_element(sel).await.map_err(Error::Cdp)?;
    el.type_str(text).await.map_err(Error::Cdp)?;
    Ok(())
}

/// Waits until no element matches `sel` anymore.
async fn wait_until_gone(h: &TestHarness, sel: &str) -> Result<()> {
    let deadline = Instant::now() + TIMEOUT;
    loop {
        let exists: bool = h
            .eval(&format!("!!document.querySelector({sel:?})"))
            .await
            .unwrap_or(false);
        if !exists {
            return Ok(());
        }
        if Instant::now() >= deadline {
            return Err(Error::message(format!(
                "element {sel:?} still present after {TIMEOUT:?}"
            )));
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

/// Waits for the responsive drawer's slide-out transition to finish after a
/// viewport change or backdrop click, so screenshots and geometry probes see
/// the settled page rather than an intermediate frame.
async fn wait_for_nav_closed(h: &TestHarness) -> Result<()> {
    let deadline = Instant::now() + TIMEOUT;
    loop {
        let right: f64 = h
            .eval("document.querySelector('#primary-nav')?.getBoundingClientRect().right ?? -1")
            .await?;
        if right <= 0.5 {
            return Ok(());
        }
        if Instant::now() >= deadline {
            return Err(Error::message("responsive navigation did not close"));
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
}

/// Checks the immediate focus and whole-value selection promised by the
/// inline rename editor.
async fn assert_rename_input_ready(h: &TestHarness, label: &str) -> Result<()> {
    let (focused, all_selected): (bool, bool) = h
        .eval(
            "(() => { const el = document.querySelector('#rename-input'); \
             return [document.activeElement === el, \
                     el.selectionStart === 0 && el.selectionEnd === el.value.length]; })()",
        )
        .await?;
    if focused && all_selected {
        return Ok(());
    }
    Err(Error::message(format!(
        "{label} rename input should be focused with its text selected (focused: {focused}, selected: {all_selected})"
    )))
}

/// Checks the shared destructive-dialog keyboard contract and dismisses it
/// with Escape so the caller can reopen it for the action assertion.
async fn exercise_confirmation_keyboard(
    h: &TestHarness,
    modal: &str,
    confirm: &str,
    cancel: &str,
    title_id: &str,
    description: &str,
) -> Result<()> {
    let dialog = format!("{modal} [role=dialog]");
    let labelled_by: String = h
        .eval(&format!(
            "document.querySelector({dialog:?}).getAttribute('aria-labelledby')"
        ))
        .await?;
    if labelled_by != title_id {
        return Err(Error::message(format!(
            "{description} confirmation should have an accessible name, got {labelled_by:?}"
        )));
    }
    let focused: String = h.eval("document.activeElement?.id ?? ''").await?;
    if focused != cancel.trim_start_matches('#') {
        return Err(Error::message(format!(
            "{description} confirmation should initially focus Cancel, got {focused:?}"
        )));
    }
    h.page
        .find_element(cancel)
        .await
        .map_err(Error::Cdp)?
        .press_key("Tab")
        .await
        .map_err(Error::Cdp)?;
    let focused: String = h.eval("document.activeElement?.id ?? ''").await?;
    if focused != confirm.trim_start_matches('#') {
        return Err(Error::message(format!(
            "{description} confirmation Tab should reach Delete, got {focused:?}"
        )));
    }
    h.page
        .find_element(confirm)
        .await
        .map_err(Error::Cdp)?
        .press_key("Escape")
        .await
        .map_err(Error::Cdp)?;
    wait_until_gone(h, modal).await
}

/// Full roundtrip: open bootstrap → register → login → app.
#[ignore = "browser"]
#[tokio::test]
async fn register_and_login_roundtrip() -> Result<()> {
    let h = TestHarness::start_with_auth().await?;
    h.add_virtual_authenticator().await?;

    // Zero passkeys in the system: the register variant shows.
    h.wait_for_selector("#register-username", TIMEOUT).await?;
    h.screenshot("07_auth/01_register_screen").await?;

    h.type_into("#register-username", "e2euser").await?;
    h.click("#create-passkey").await?;

    // Registration succeeded but does not log in: the login variant
    // appears with the success note.
    h.wait_for_text("body", "Passkey created", TIMEOUT).await?;
    h.screenshot("07_auth/02_login_screen_after_register")
        .await?;

    h.click("#sign-in").await?;
    h.wait_for_selector("#tab-quiz", TIMEOUT).await?;
    h.wait_for_selector("#quiz-loading, #quiz-done, #quiz-prompt", TIMEOUT)
        .await?;

    // The session now belongs to the registered user.
    let username: String = h
        .eval(
            "(async () => { \
             const r = await fetch('/api/auth/session'); \
             return (await r.json()).username; })()",
        )
        .await?;
    if username != "e2euser" {
        return Err(Error::message(format!(
            "session username is {username:?}, expected \"e2euser\""
        )));
    }
    Ok(())
}

/// Logout from the account tab returns to the auth screen (login
/// variant — a passkey exists, so registration is closed).
#[ignore = "browser"]
#[tokio::test]
async fn logout_returns_to_auth_screen() -> Result<()> {
    let h = TestHarness::start_with_auth().await?;
    register_and_login(&h, "logoutuser").await?;

    h.click("#tab-account").await?;
    h.wait_for_selector("#logout", TIMEOUT).await?;
    h.wait_for_text("#account-username", "logoutuser", TIMEOUT)
        .await?;
    h.screenshot("07_auth/03_account_tab").await?;
    h.click("#logout").await?;

    h.wait_for_selector("#sign-in", TIMEOUT).await?;
    // Registration is closed (the passkey exists): no username input.
    let has_register: bool = h
        .eval("!!document.querySelector('#register-username')")
        .await?;
    if has_register {
        return Err(Error::message(
            "register variant shown after logout although a passkey exists",
        ));
    }
    Ok(())
}

/// Passkey management: auto-name, inline rename, delete with confirm,
/// and the last-passkey delete guard surfaced as an error.
#[ignore = "browser"]
#[tokio::test]
async fn passkey_management() -> Result<()> {
    let h = TestHarness::start_with_auth().await?;
    let authenticator = register_and_login(&h, "manager").await?;

    h.click("#tab-account").await?;
    h.wait_for_text("#passkeys-list", "Passkey 1", TIMEOUT)
        .await?;

    // A second passkey is needed so the guard can be tested on the last
    // one.
    swap_to_fresh_authenticator(&h, &authenticator).await?;
    h.click("#add-passkey").await?;
    h.wait_for_text("#passkeys-list", "Passkey 2", TIMEOUT)
        .await?;

    // Inline rename of the first passkey.
    h.click("#rename-passkey-1").await?;
    // The rename input is focused immediately and the current name is
    // selected, so the new name can be typed over without an extra
    // click or keystroke (owner wish 2026-08-09).
    assert_rename_input_ready(&h, "first").await?;
    // CV checkpoint: the inline editor with the focused input.
    h.screenshot("07_auth/03b_passkey_rename_focused").await?;
    replace_input_text(&h, "#rename-input", "Laptop").await?;
    h.click("#rename-save").await?;
    h.wait_for_text("#passkeys-list", "Laptop", TIMEOUT).await?;
    h.screenshot("07_auth/04_passkeys_renamed").await?;

    // Re-entering rename after a completed edit gets a fresh input node;
    // focus and selection must work on this second edit as well. Check it at
    // mobile width too: the editor must preserve the row's two-column layout
    // and keep the action buttons inside the card.
    h.set_viewport(390, 844).await?;
    let nav_open: bool = h.eval("!!document.querySelector('#nav-backdrop')").await?;
    if nav_open {
        h.click("#nav-backdrop").await?;
        wait_for_nav_closed(&h).await?;
    }
    let (normal_button_font, normal_actions_right, delete_aria): (String, f64, String) = h
        .eval(
            "(() => { const rename = document.querySelector('#rename-passkey-1'); \
             const actions = document.querySelector('#passkey-row-1 .passkey-actions').getBoundingClientRect(); \
             return [getComputedStyle(rename).fontSize, actions.right, \
                     document.querySelector('#delete-passkey-1').getAttribute('aria-label')]; })()",
        )
        .await?;
    if delete_aria != "Delete passkey" {
        return Err(Error::message(format!(
            "passkey delete should expose its action name, got {delete_aria:?}"
        )));
    }
    h.click("#rename-passkey-1").await?;
    assert_rename_input_ready(&h, "second").await?;
    let (
        row_left,
        row_right,
        input_left,
        input_right,
        save_left,
        cancel_right,
        save_font,
        cancel_font,
    ): (f64, f64, f64, f64, f64, f64, String, String) = h
        .eval(
            "(() => { const row = document.querySelector('#passkey-row-1').getBoundingClientRect(); \
             const input = document.querySelector('#rename-input').getBoundingClientRect(); \
             const save = document.querySelector('#rename-save').getBoundingClientRect(); \
             const cancel = document.querySelector('#rename-cancel').getBoundingClientRect(); \
             return [row.left, row.right, input.left, input.right, save.left, cancel.right, \
                     getComputedStyle(document.querySelector('#rename-save')).fontSize, \
                     getComputedStyle(document.querySelector('#rename-cancel')).fontSize]; })()",
        )
        .await?;
    if input_left < row_left - 0.5
        || input_right > save_left
        || cancel_right > row_right + 0.5
        || (cancel_right - normal_actions_right).abs() > 0.5
        || save_font != normal_button_font
        || cancel_font != normal_button_font
    {
        return Err(Error::message(format!(
            "mobile rename layout should fit the row and match button sizing (row {row_left}..{row_right}, input {input_left}..{input_right}, actions {save_left}..{cancel_right}, fonts normal={normal_button_font}, save={save_font}, cancel={cancel_font})"
        )));
    }
    h.screenshot("07_auth/03c_passkey_rename_mobile").await?;
    h.click("#rename-cancel").await?;
    h.wait_for_selector("#rename-passkey-1", TIMEOUT).await?;

    // Delete it behind the confirm modal: the row goes.
    h.click("#delete-passkey-1").await?;
    h.wait_for_selector("#confirm-delete-modal", TIMEOUT)
        .await?;
    exercise_confirmation_keyboard(
        &h,
        "#confirm-delete-modal",
        "#confirm-delete",
        "#cancel-delete",
        "confirm-delete-title",
        "passkey",
    )
    .await?;

    h.click("#delete-passkey-1").await?;
    h.wait_for_selector("#confirm-delete-modal", TIMEOUT)
        .await?;
    h.click("#confirm-delete").await?;
    wait_until_gone(&h, "#passkey-row-1").await?;

    // The remaining passkey is the last one: the delete is refused, the
    // error shows inside the modal and the passkey survives.
    h.click("#delete-passkey-2").await?;
    h.wait_for_selector("#confirm-delete-modal", TIMEOUT)
        .await?;
    h.click("#confirm-delete").await?;
    h.wait_for_text("#delete-error", "cannot delete your last passkey", TIMEOUT)
        .await?;
    h.screenshot("07_auth/05_last_passkey_guard").await?;
    h.click("#cancel-delete").await?;
    h.wait_for_text("#passkeys-list", "Passkey 2", TIMEOUT)
        .await?;
    Ok(())
}

/// Step-up ("sudo mode"): a session whose last passkey proof is stale
/// must re-authenticate before a sensitive operation. The UI does it
/// transparently: the delete is refused (403 "step-up required"), the
/// client runs the step-up ceremony against the current authenticator
/// and retries — the row goes without any error surfacing.
#[ignore = "browser"]
#[tokio::test]
async fn stale_session_steps_up_before_delete() -> Result<()> {
    let h = TestHarness::start_with_auth().await?;
    let authenticator = register_and_login(&h, "stale").await?;

    // A second passkey, so the first may be deleted at all.
    h.click("#tab-account").await?;
    h.wait_for_text("#passkeys-list", "Passkey 1", TIMEOUT)
        .await?;
    swap_to_fresh_authenticator(&h, &authenticator).await?;
    h.click("#add-passkey").await?;
    h.wait_for_text("#passkeys-list", "Passkey 2", TIMEOUT)
        .await?;

    // Pretend the login happened long ago: stale the verified_at stamp.
    let token = h
        .session_token()
        .await?
        .ok_or_else(|| Error::message("no session cookie after login"))?;
    h.seed_store()
        .await
        .map_err(store_err)?
        .touch_session_verified(&token, 0)
        .await
        .map_err(store_err)?;

    // Delete passkey 1 behind the confirm modal: gated → step-up runs
    // against the CURRENT authenticator (which holds Passkey 2) → retry
    // succeeds.
    h.click("#delete-passkey-1").await?;
    h.wait_for_selector("#confirm-delete-modal", TIMEOUT)
        .await?;
    h.click("#confirm-delete").await?;
    wait_until_gone(&h, "#passkey-row-1").await?;
    h.wait_for_text("#passkeys-list", "Passkey 2", TIMEOUT)
        .await?;
    Ok(())
}

/// Adding a second passkey from the account tab auto-names it
/// "Passkey 2".
#[ignore = "browser"]
#[tokio::test]
async fn add_second_passkey_appears() -> Result<()> {
    let h = TestHarness::start_with_auth().await?;
    let authenticator = register_and_login(&h, "adder").await?;

    h.click("#tab-account").await?;
    h.wait_for_text("#passkeys-list", "Passkey 1", TIMEOUT)
        .await?;

    swap_to_fresh_authenticator(&h, &authenticator).await?;
    h.click("#add-passkey").await?;
    h.wait_for_text("#passkeys-list", "Passkey 2", TIMEOUT)
        .await?;
    h.screenshot("07_auth/06_second_passkey").await?;
    Ok(())
}

/// Auth mode without a session: the API answers 401; health stays open.
#[ignore = "browser"]
#[tokio::test]
async fn api_requires_auth() -> Result<()> {
    let h = TestHarness::start_with_auth().await?;
    // The app itself boots into the auth screen.
    h.wait_for_selector("#auth-screen", TIMEOUT).await?;
    let cards: u16 = h
        .eval("(async () => (await fetch('/api/cards')).status)()")
        .await?;
    let health: u16 = h
        .eval("(async () => (await fetch('/api/health')).status)()")
        .await?;
    if cards != 401 {
        return Err(Error::message(format!(
            "GET /api/cards without session: {cards}, expected 401"
        )));
    }
    if health != 200 {
        return Err(Error::message(format!(
            "GET /api/health: {health}, expected 200"
        )));
    }
    Ok(())
}
