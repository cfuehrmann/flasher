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
    replace_input_text(&h, "#rename-input", "Laptop").await?;
    h.click("#rename-save").await?;
    h.wait_for_text("#passkeys-list", "Laptop", TIMEOUT).await?;
    h.screenshot("07_auth/04_passkeys_renamed").await?;

    // Delete it behind the confirm modal: the row goes.
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
