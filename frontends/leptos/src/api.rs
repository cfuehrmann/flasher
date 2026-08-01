//! Thin async wrappers around the Flasher JSON API.
//!
//! The real implementations use `gloo-net` and only exist in the `csr`
//! (browser) build. Under `ssr` every function returns an error instead:
//! they are only ever invoked from effects and event handlers that never
//! run on the server, but having the same signatures in both builds keeps
//! the components themselves cfg-free.

use flasher_types::{
    AutoSaveResponse, CardResponse, DisabledFilter, FindCardsResponse, GetAutoSaveResponse,
    HealthResponse, NextCardResponse,
};
#[cfg(feature = "csr")]
use flasher_types::{
    CardUpdateRequest, CreateCardRequest, PutAutoSaveRequest, SetCardStateRequest,
};

/// The error every non-`csr` stub returns.
#[cfg(not(feature = "csr"))]
const SSR_STUB_ERROR: &str = "the API is only available in the browser build";

/// `GET /api/health`.
#[cfg(feature = "csr")]
pub async fn health() -> Result<HealthResponse, String> {
    let response = gloo_net::http::Request::get("/api/health")
        .send()
        .await
        .map_err(|err| err.to_string())?;
    if response.status() != 200 {
        return Err(error_message(response).await);
    }
    response
        .json::<HealthResponse>()
        .await
        .map_err(|err| err.to_string())
}

/// `GET /api/health` (ssr stub, never called — the health effect is
/// csr-only, so nothing references this in an ssr build).
#[cfg(not(feature = "csr"))]
#[allow(clippy::unused_async, dead_code)]
pub async fn health() -> Result<HealthResponse, String> {
    Err(SSR_STUB_ERROR.to_owned())
}

/// `GET /api/cards/next` — the next due card, or `None`.
#[cfg(feature = "csr")]
pub async fn next_card() -> Result<NextCardResponse, String> {
    let response = gloo_net::http::Request::get("/api/cards/next")
        .send()
        .await
        .map_err(|err| err.to_string())?;
    if response.status() != 200 {
        return Err(error_message(response).await);
    }
    response
        .json::<NextCardResponse>()
        .await
        .map_err(|err| err.to_string())
}

/// `GET /api/cards/next` (ssr stub, never called).
#[cfg(not(feature = "csr"))]
#[allow(clippy::unused_async)]
pub async fn next_card() -> Result<NextCardResponse, String> {
    Err(SSR_STUB_ERROR.to_owned())
}

/// `POST /api/cards` — create a card (it starts out disabled).
#[cfg(feature = "csr")]
pub async fn create_card(prompt: &str, solution: &str) -> Result<CardResponse, String> {
    let request = gloo_net::http::Request::post("/api/cards")
        .json(&CreateCardRequest {
            prompt: prompt.to_owned(),
            solution: solution.to_owned(),
        })
        .map_err(|err| err.to_string())?;
    let response = request.send().await.map_err(|err| err.to_string())?;
    if response.status() != 201 {
        return Err(error_message(response).await);
    }
    response
        .json::<CardResponse>()
        .await
        .map_err(|err| err.to_string())
}

/// `POST /api/cards` (ssr stub, never called).
#[cfg(not(feature = "csr"))]
#[allow(clippy::unused_async)]
pub async fn create_card(_prompt: &str, _solution: &str) -> Result<CardResponse, String> {
    Err(SSR_STUB_ERROR.to_owned())
}

/// `POST /api/cards/{id}/set-ok`. `change_time` is the value of the card
/// as rendered; the server rejects the rating with 409 when the card has
/// moved since (see [`set_state`]).
#[cfg(feature = "csr")]
pub async fn set_ok(id: &str, change_time: i64) -> Result<(), String> {
    set_state(id, "set-ok", change_time).await
}

/// `POST /api/cards/{id}/set-ok` (ssr stub, never called).
#[cfg(not(feature = "csr"))]
#[allow(clippy::unused_async)]
pub async fn set_ok(_id: &str, _change_time: i64) -> Result<(), String> {
    Err(SSR_STUB_ERROR.to_owned())
}

/// `POST /api/cards/{id}/set-failed`. See [`set_ok`] for `change_time`.
#[cfg(feature = "csr")]
pub async fn set_failed(id: &str, change_time: i64) -> Result<(), String> {
    set_state(id, "set-failed", change_time).await
}

/// `POST /api/cards/{id}/set-failed` (ssr stub, never called).
#[cfg(not(feature = "csr"))]
#[allow(clippy::unused_async)]
pub async fn set_failed(_id: &str, _change_time: i64) -> Result<(), String> {
    Err(SSR_STUB_ERROR.to_owned())
}

/// Shared body of the two rating endpoints; the updated card in the
/// response is irrelevant because the next card is re-fetched afterwards.
/// A 409 means a concurrent/duplicated rating already moved the card
/// (issue #124): that is not an error for the quiz — the next fetch
/// simply re-reads the current state — so it is folded into `Ok(())`.
#[cfg(feature = "csr")]
async fn set_state(id: &str, action: &str, change_time: i64) -> Result<(), String> {
    let request = gloo_net::http::Request::post(&format!("/api/cards/{id}/{action}"))
        .json(&SetCardStateRequest { change_time })
        .map_err(|err| err.to_string())?;
    let response = request.send().await.map_err(|err| err.to_string())?;
    if response.status() != 200 && response.status() != 409 {
        return Err(error_message(response).await);
    }
    Ok(())
}

/// `GET /api/cards` — one page of the groom list plus the total match
/// count (before paging) and the effective page size. An empty
/// `search_text` lists all cards; `filter` restricts the list by the
/// `disabled` flag (groom filter, issue #127); `take` is the requested
/// page size (the groom tab sizes it to its viewport; the server clamps
/// and echoes it).
#[cfg(feature = "csr")]
pub async fn find_cards(
    search_text: &str,
    filter: DisabledFilter,
    skip: u32,
    take: u32,
) -> Result<FindCardsResponse, String> {
    let skip = skip.to_string();
    let take = take.to_string();
    let mut query = vec![
        ("skip", skip.as_str()),
        ("take", take.as_str()),
        ("disabled_filter", filter.as_str()),
    ];
    if !search_text.is_empty() {
        query.push(("search_text", search_text));
    }
    let response = gloo_net::http::Request::get("/api/cards")
        .query(query)
        .send()
        .await
        .map_err(|err| err.to_string())?;
    if response.status() != 200 {
        return Err(error_message(response).await);
    }
    response
        .json::<FindCardsResponse>()
        .await
        .map_err(|err| err.to_string())
}

/// `GET /api/cards` (ssr stub, never called).
#[cfg(not(feature = "csr"))]
#[allow(clippy::unused_async)]
pub async fn find_cards(
    _search_text: &str,
    _filter: DisabledFilter,
    _skip: u32,
    _take: u32,
) -> Result<FindCardsResponse, String> {
    Err(SSR_STUB_ERROR.to_owned())
}

/// `PATCH /api/cards/{id}` — update prompt and solution. The server
/// deletes the user's autosave draft as a side effect of the content
/// change, so a successful save needs no separate draft cleanup.
#[cfg(feature = "csr")]
pub async fn update_card(id: &str, prompt: &str, solution: &str) -> Result<CardResponse, String> {
    let request = gloo_net::http::Request::patch(&format!("/api/cards/{id}"))
        .json(&CardUpdateRequest {
            prompt: Some(prompt.to_owned()),
            solution: Some(solution.to_owned()),
            disabled: None,
        })
        .map_err(|err| err.to_string())?;
    let response = request.send().await.map_err(|err| err.to_string())?;
    if response.status() != 200 {
        return Err(error_message(response).await);
    }
    response
        .json::<CardResponse>()
        .await
        .map_err(|err| err.to_string())
}

/// `PATCH /api/cards/{id}` (ssr stub, never called).
#[cfg(not(feature = "csr"))]
#[allow(clippy::unused_async)]
pub async fn update_card(
    _id: &str,
    _prompt: &str,
    _solution: &str,
) -> Result<CardResponse, String> {
    Err(SSR_STUB_ERROR.to_owned())
}

/// `PUT /api/autosave` — upserts the current user's draft; returns the
/// stored draft (`updated_at` only bumps on a real content change).
#[cfg(feature = "csr")]
pub async fn put_autosave(
    card_id: Option<&str>,
    prompt: &str,
    solution: &str,
) -> Result<AutoSaveResponse, String> {
    let request = gloo_net::http::Request::put("/api/autosave")
        .json(&PutAutoSaveRequest {
            card_id: card_id.map(str::to_owned),
            prompt: prompt.to_owned(),
            solution: solution.to_owned(),
        })
        .map_err(|err| err.to_string())?;
    let response = request.send().await.map_err(|err| err.to_string())?;
    if response.status() != 200 {
        return Err(error_message(response).await);
    }
    response
        .json::<AutoSaveResponse>()
        .await
        .map_err(|err| err.to_string())
}

/// `PUT /api/autosave` (ssr stub, never called).
#[cfg(not(feature = "csr"))]
#[allow(clippy::unused_async, dead_code)]
pub async fn put_autosave(
    _card_id: Option<&str>,
    _prompt: &str,
    _solution: &str,
) -> Result<AutoSaveResponse, String> {
    Err(SSR_STUB_ERROR.to_owned())
}

/// `GET /api/autosave` — the current user's draft, or `None`.
#[cfg(feature = "csr")]
pub async fn get_autosave() -> Result<GetAutoSaveResponse, String> {
    let response = gloo_net::http::Request::get("/api/autosave")
        .send()
        .await
        .map_err(|err| err.to_string())?;
    if response.status() != 200 {
        return Err(error_message(response).await);
    }
    response
        .json::<GetAutoSaveResponse>()
        .await
        .map_err(|err| err.to_string())
}

/// `GET /api/autosave` (ssr stub, never called).
#[cfg(not(feature = "csr"))]
#[allow(clippy::unused_async, dead_code)]
pub async fn get_autosave() -> Result<GetAutoSaveResponse, String> {
    Err(SSR_STUB_ERROR.to_owned())
}

/// `DELETE /api/autosave` — drops the current user's draft (204).
#[cfg(feature = "csr")]
pub async fn delete_autosave() -> Result<(), String> {
    let response = gloo_net::http::Request::delete("/api/autosave")
        .send()
        .await
        .map_err(|err| err.to_string())?;
    if response.status() != 204 {
        return Err(error_message(response).await);
    }
    Ok(())
}

/// `DELETE /api/autosave` (ssr stub, never called).
#[cfg(not(feature = "csr"))]
#[allow(clippy::unused_async)]
pub async fn delete_autosave() -> Result<(), String> {
    Err(SSR_STUB_ERROR.to_owned())
}

/// `GET /api/cards/{id}` — one card by id, `Ok(None)` on 404 (unknown
/// or deleted meanwhile). Used by the editor deep-link restore (Phase
/// 6.6) and by draft recovery to fall back to new-card mode when the
/// edited card no longer exists.
#[cfg(feature = "csr")]
pub async fn get_card(id: &str) -> Result<Option<CardResponse>, String> {
    let response = gloo_net::http::Request::get(&format!("/api/cards/{id}"))
        .send()
        .await
        .map_err(|err| err.to_string())?;
    if response.status() == 404 {
        return Ok(None);
    }
    if response.status() != 200 {
        return Err(error_message(response).await);
    }
    response
        .json::<CardResponse>()
        .await
        .map(Some)
        .map_err(|err| err.to_string())
}

/// `get_card` (ssr stub, never called).
#[cfg(not(feature = "csr"))]
#[allow(clippy::unused_async, dead_code)]
pub async fn get_card(_id: &str) -> Result<Option<CardResponse>, String> {
    Err(SSR_STUB_ERROR.to_owned())
}

/// `PATCH /api/cards/{id}` — toggle only the `disabled` flag; prompt and
/// solution stay untouched.
#[cfg(feature = "csr")]
pub async fn set_disabled(id: &str, disabled: bool) -> Result<CardResponse, String> {
    let request = gloo_net::http::Request::patch(&format!("/api/cards/{id}"))
        .json(&CardUpdateRequest {
            prompt: None,
            solution: None,
            disabled: Some(disabled),
        })
        .map_err(|err| err.to_string())?;
    let response = request.send().await.map_err(|err| err.to_string())?;
    if response.status() != 200 {
        return Err(error_message(response).await);
    }
    response
        .json::<CardResponse>()
        .await
        .map_err(|err| err.to_string())
}

/// `PATCH /api/cards/{id}` (ssr stub, never called).
#[cfg(not(feature = "csr"))]
#[allow(clippy::unused_async)]
pub async fn set_disabled(_id: &str, _disabled: bool) -> Result<CardResponse, String> {
    Err(SSR_STUB_ERROR.to_owned())
}

/// `DELETE /api/cards/{id}` — deletes the card entirely (204).
#[cfg(feature = "csr")]
pub async fn delete_card(id: &str) -> Result<(), String> {
    let response = gloo_net::http::Request::delete(&format!("/api/cards/{id}"))
        .send()
        .await
        .map_err(|err| err.to_string())?;
    if response.status() != 204 {
        return Err(error_message(response).await);
    }
    Ok(())
}

/// `DELETE /api/cards/{id}` (ssr stub, never called).
#[cfg(not(feature = "csr"))]
#[allow(clippy::unused_async)]
pub async fn delete_card(_id: &str) -> Result<(), String> {
    Err(SSR_STUB_ERROR.to_owned())
}

/// `DELETE /api/history/{id}` — resets the learning progress: the card
/// goes back to state `new` with `next_time = now + 30 min`. Returns the
/// updated card.
#[cfg(feature = "csr")]
pub async fn delete_history(id: &str) -> Result<CardResponse, String> {
    let response = gloo_net::http::Request::delete(&format!("/api/history/{id}"))
        .send()
        .await
        .map_err(|err| err.to_string())?;
    if response.status() != 200 {
        return Err(error_message(response).await);
    }
    response
        .json::<CardResponse>()
        .await
        .map_err(|err| err.to_string())
}

/// `DELETE /api/history/{id}` (ssr stub, never called).
#[cfg(not(feature = "csr"))]
#[allow(clippy::unused_async)]
pub async fn delete_history(_id: &str) -> Result<CardResponse, String> {
    Err(SSR_STUB_ERROR.to_owned())
}

/// Builds an error from a non-success response: the server's plain-text
/// body when present, otherwise the bare status code.
///
/// A 401 on a DATA endpoint means the session expired mid-use: the
/// registered unauthorized hook bounces the app back to the auth screen
/// (in dev-bypass mode 401s never occur). A 401 on an auth CEREMONY
/// endpoint (`/api/auth/register/*`, `/api/auth/login/*`,
/// `/api/auth/step-up/*`) is about the ceremony — an unknown or
/// server-side-deleted passkey — NOT about the session: the session may
/// be perfectly valid (a failed step-up must not log the user out), and
/// on the login screen there is no session to lose. Those 401s surface
/// as a plain error where the action started.
#[cfg(feature = "csr")]
async fn error_message(response: gloo_net::http::Response) -> String {
    let status = response.status();
    if status == 401 && !is_ceremony_url(&response) {
        notify_unauthorized();
    }
    match response.text().await {
        Ok(body) if !body.trim().is_empty() => format!("{status}: {body}"),
        _ => format!("request failed with status {status}"),
    }
}

/// True for the passkey ceremony endpoints, whose 401s must NOT bounce
/// the app to the auth screen (see [`error_message`]). The session-gated
/// passkey management endpoints (`/api/auth/passkeys*`) are deliberately
/// not included: a 401 there DOES mean the session is gone.
#[cfg(feature = "csr")]
fn is_ceremony_url(response: &gloo_net::http::Response) -> bool {
    let url = response.url();
    [
        "/api/auth/register/",
        "/api/auth/login/",
        "/api/auth/step-up/",
    ]
    .iter()
    .any(|prefix| url.contains(prefix))
}

// ---------------------------------------------------------------------
// auth (Phase 5B)
// ---------------------------------------------------------------------

// Hook fired when any API call answers 401 (session expired mid-use).
// Registered once by `App` on startup; wasm is single-threaded, so a
// thread-local `Rc` callback is enough.
#[cfg(feature = "csr")]
thread_local! {
    static UNAUTHORIZED_HOOK: std::cell::RefCell<Option<std::rc::Rc<dyn Fn()>>> =
        const { std::cell::RefCell::new(None) };
}

/// Registers the callback invoked on any mid-session 401. The callback
/// should switch the app back to the auth screen.
#[cfg(feature = "csr")]
pub fn on_unauthorized(hook: impl Fn() + 'static) {
    UNAUTHORIZED_HOOK.with(|slot| *slot.borrow_mut() = Some(std::rc::Rc::new(hook)));
}

/// `on_unauthorized` (ssr stub: never fires — API calls never happen).
#[cfg(not(feature = "csr"))]
#[allow(dead_code)]
pub fn on_unauthorized(_hook: impl Fn() + 'static) {}

/// Fires the unauthorized hook, if registered.
#[cfg(feature = "csr")]
fn notify_unauthorized() {
    UNAUTHORIZED_HOOK.with(|slot| {
        if let Some(hook) = slot.borrow().as_ref() {
            hook();
        }
    });
}

/// `GET /api/auth/session` — `Ok(Some(username))` with a valid session,
/// `Ok(None)` on 200 `null` (the normal logged-out signal, no error).
///
/// `send_credentials = false` suppresses cookies; in dev-bypass mode the
/// endpoint still answers with the user, in auth mode with `null` — that
/// is how the UI tells the two modes apart without a dedicated indicator.
#[cfg(feature = "csr")]
pub async fn session(send_credentials: bool) -> Result<Option<String>, String> {
    let request = gloo_net::http::Request::get("/api/auth/session");
    let request = if send_credentials {
        request
    } else {
        request.credentials(web_sys::RequestCredentials::Omit)
    };
    let response = request.send().await.map_err(|err| err.to_string())?;
    match response.status() {
        200 => {
            let session = response
                .json::<Option<flasher_types::SessionResponse>>()
                .await
                .map_err(|err| err.to_string())?;
            Ok(session.map(|session| session.username))
        }
        _ => Err(error_message(response).await),
    }
}

/// `session` (ssr stub, never called).
#[cfg(not(feature = "csr"))]
#[allow(clippy::unused_async, dead_code)]
pub async fn session(_send_credentials: bool) -> Result<Option<String>, String> {
    Err(SSR_STUB_ERROR.to_owned())
}

/// `GET /api/auth/bootstrap` — whether the first passkey may be
/// registered without a session (zero passkeys in the system), and
/// whether the open bootstrap requires the `FLASHER_BOOTSTRAP_TOKEN`.
#[cfg(feature = "csr")]
pub async fn bootstrap() -> Result<flasher_types::BootstrapResponse, String> {
    let response = gloo_net::http::Request::get("/api/auth/bootstrap")
        .send()
        .await
        .map_err(|err| err.to_string())?;
    if response.status() != 200 {
        return Err(error_message(response).await);
    }
    response
        .json::<flasher_types::BootstrapResponse>()
        .await
        .map_err(|err| err.to_string())
}

/// `bootstrap` (ssr stub, never called).
#[cfg(not(feature = "csr"))]
#[allow(clippy::unused_async, dead_code)]
pub async fn bootstrap() -> Result<flasher_types::BootstrapResponse, String> {
    Err(SSR_STUB_ERROR.to_owned())
}

/// Outcome of an action the server may gate behind a recent passkey
/// proof (step-up, "sudo mode"): `NeedsStepUp` for exactly the
/// 403 "step-up required" response — the client runs the step-up ceremony
/// and retries. Any other error is a plain `Err`.
#[derive(Debug)]
#[cfg_attr(not(feature = "csr"), allow(dead_code))] // variants built in csr only
pub enum StepUp<T> {
    /// The action went through.
    Done(T),
    /// The server's step-up window had expired: re-authenticate, retry.
    NeedsStepUp,
}

/// The exact body the server sends with the step-up 403 (part of the
/// contract — see `ApiError::StepUpRequired` in flasher-server).
#[cfg(feature = "csr")]
const STEP_UP_BODY: &str = "step-up required";

/// Maps a response of a step-up-gated endpoint: the expected `success`
/// status → `Done`, the step-up 403 → `NeedsStepUp`, anything else an
/// error (via [`error_message`]).
#[cfg(feature = "csr")]
async fn step_up_outcome(
    response: gloo_net::http::Response,
    success: u16,
) -> Result<StepUp<()>, String> {
    if response.status() == success {
        return Ok(StepUp::Done(()));
    }
    if response.status() == 403 {
        let body = response.text().await.map_err(|err| err.to_string())?;
        if body.trim() == STEP_UP_BODY {
            return Ok(StepUp::NeedsStepUp);
        }
        return Err(format!("403: {}", body.trim()));
    }
    Err(error_message(response).await)
}

/// `POST /api/auth/register/start` — returns the raw options JSON text
/// (the `webauthn` module converts it for `navigator.credentials`).
/// `token` is the bootstrap token, sent only on the open first-run
/// registration when the server requires it. A 403 with a stale-session
/// body is `NeedsStepUp` (add-passkey flow); any other 403 (bad/missing
/// bootstrap token) is surfaced as the server's plain message without the
/// status prefix — an expected outcome of a mistyped token, not a failure.
#[cfg(feature = "csr")]
pub async fn register_start(username: &str, token: Option<&str>) -> Result<StepUp<String>, String> {
    let request = gloo_net::http::Request::post("/api/auth/register/start")
        .json(&flasher_types::RegisterStartRequest {
            username: username.to_owned(),
            token: token.map(str::to_owned),
        })
        .map_err(|err| err.to_string())?;
    let response = request.send().await.map_err(|err| err.to_string())?;
    if response.status() == 403 {
        return match response.text().await {
            Ok(body) if body.trim() == STEP_UP_BODY => Ok(StepUp::NeedsStepUp),
            Ok(body) if !body.trim().is_empty() => Err(body),
            _ => Err("invalid bootstrap token".to_owned()),
        };
    }
    if response.status() != 200 {
        return Err(error_message(response).await);
    }
    let options = response.text().await.map_err(|err| err.to_string())?;
    Ok(StepUp::Done(options))
}

/// `register_start` (ssr stub, never called).
#[cfg(not(feature = "csr"))]
#[allow(clippy::unused_async, dead_code)]
pub async fn register_start(
    _username: &str,
    _token: Option<&str>,
) -> Result<StepUp<String>, String> {
    Err(SSR_STUB_ERROR.to_owned())
}

/// `POST /api/auth/register/finish` — sends the credential JSON the
/// browser produced (201 on success).
#[cfg(feature = "csr")]
pub async fn register_finish(credential_json: &str) -> Result<(), String> {
    let request = gloo_net::http::Request::post("/api/auth/register/finish")
        .header("Content-Type", "application/json")
        .body(credential_json)
        .map_err(|err| err.to_string())?;
    let response = request.send().await.map_err(|err| err.to_string())?;
    if response.status() != 201 {
        return Err(error_message(response).await);
    }
    Ok(())
}

/// `register_finish` (ssr stub, never called).
#[cfg(not(feature = "csr"))]
#[allow(clippy::unused_async, dead_code)]
pub async fn register_finish(_credential_json: &str) -> Result<(), String> {
    Err(SSR_STUB_ERROR.to_owned())
}

/// `POST /api/auth/login/start` — returns the raw options JSON text.
#[cfg(feature = "csr")]
pub async fn login_start() -> Result<String, String> {
    let request = gloo_net::http::Request::post("/api/auth/login/start")
        .header("Content-Type", "application/json")
        .body("{}")
        .map_err(|err| err.to_string())?;
    let response = request.send().await.map_err(|err| err.to_string())?;
    if response.status() != 200 {
        return Err(error_message(response).await);
    }
    response.text().await.map_err(|err| err.to_string())
}

/// `login_start` (ssr stub, never called).
#[cfg(not(feature = "csr"))]
#[allow(clippy::unused_async, dead_code)]
pub async fn login_start() -> Result<String, String> {
    Err(SSR_STUB_ERROR.to_owned())
}

/// `POST /api/auth/login/finish` — sends the assertion JSON; returns the
/// logged-in username (and the session cookie rides along). A 401 (the
/// passkey is unknown server-side or the verification failed) gets a
/// friendly message — the body is empty there, and the user must know
/// their passkey was not recognized, not stare at a bare status code.
#[cfg(feature = "csr")]
pub async fn login_finish(assertion_json: &str) -> Result<String, String> {
    let request = gloo_net::http::Request::post("/api/auth/login/finish")
        .header("Content-Type", "application/json")
        .body(assertion_json)
        .map_err(|err| err.to_string())?;
    let response = request.send().await.map_err(|err| err.to_string())?;
    if response.status() == 401 {
        return Err("this passkey is not known to the server — try another one".to_owned());
    }
    if response.status() != 200 {
        return Err(error_message(response).await);
    }
    let session = response
        .json::<flasher_types::SessionResponse>()
        .await
        .map_err(|err| err.to_string())?;
    Ok(session.username)
}

/// `login_finish` (ssr stub, never called).
#[cfg(not(feature = "csr"))]
#[allow(clippy::unused_async, dead_code)]
pub async fn login_finish(_assertion_json: &str) -> Result<String, String> {
    Err(SSR_STUB_ERROR.to_owned())
}

/// `POST /api/auth/step-up/start` — re-authentication for sensitive
/// operations: returns the raw options JSON text (same shape as
/// login/start). Requires a session.
#[cfg(feature = "csr")]
pub async fn step_up_start() -> Result<String, String> {
    let response = gloo_net::http::Request::post("/api/auth/step-up/start")
        .send()
        .await
        .map_err(|err| err.to_string())?;
    if response.status() != 200 {
        return Err(error_message(response).await);
    }
    response.text().await.map_err(|err| err.to_string())
}

/// `step_up_start` (ssr stub, never called).
#[cfg(not(feature = "csr"))]
#[allow(clippy::unused_async, dead_code)]
pub async fn step_up_start() -> Result<String, String> {
    Err(SSR_STUB_ERROR.to_owned())
}

/// `POST /api/auth/step-up/finish` — sends the assertion JSON (204 on
/// success): re-stamps the session's `verified_at`, mints no new session.
#[cfg(feature = "csr")]
pub async fn step_up_finish(assertion_json: &str) -> Result<(), String> {
    let request = gloo_net::http::Request::post("/api/auth/step-up/finish")
        .header("Content-Type", "application/json")
        .body(assertion_json)
        .map_err(|err| err.to_string())?;
    let response = request.send().await.map_err(|err| err.to_string())?;
    if response.status() != 204 {
        return Err(error_message(response).await);
    }
    Ok(())
}

/// `step_up_finish` (ssr stub, never called).
#[cfg(not(feature = "csr"))]
#[allow(clippy::unused_async, dead_code)]
pub async fn step_up_finish(_assertion_json: &str) -> Result<(), String> {
    Err(SSR_STUB_ERROR.to_owned())
}

/// `POST /api/auth/logout` — deletes the session (204).
#[cfg(feature = "csr")]
pub async fn logout() -> Result<(), String> {
    let response = gloo_net::http::Request::post("/api/auth/logout")
        .send()
        .await
        .map_err(|err| err.to_string())?;
    if response.status() != 204 {
        return Err(error_message(response).await);
    }
    Ok(())
}

/// `logout` (ssr stub, never called).
#[cfg(not(feature = "csr"))]
#[allow(clippy::unused_async)]
pub async fn logout() -> Result<(), String> {
    Err(SSR_STUB_ERROR.to_owned())
}

/// `GET /api/auth/passkeys` — the current user's passkeys.
#[cfg(feature = "csr")]
pub async fn list_passkeys() -> Result<Vec<flasher_types::PasskeyResponse>, String> {
    let response = gloo_net::http::Request::get("/api/auth/passkeys")
        .send()
        .await
        .map_err(|err| err.to_string())?;
    if response.status() != 200 {
        return Err(error_message(response).await);
    }
    response
        .json::<Vec<flasher_types::PasskeyResponse>>()
        .await
        .map_err(|err| err.to_string())
}

/// `list_passkeys` (ssr stub, never called).
#[cfg(not(feature = "csr"))]
#[allow(clippy::unused_async)]
pub async fn list_passkeys() -> Result<Vec<flasher_types::PasskeyResponse>, String> {
    Err(SSR_STUB_ERROR.to_owned())
}

/// `PATCH /api/auth/passkeys/{id}` — renames a passkey (200).
#[cfg(feature = "csr")]
pub async fn rename_passkey(id: i64, name: &str) -> Result<(), String> {
    let request = gloo_net::http::Request::patch(&format!("/api/auth/passkeys/{id}"))
        .json(&flasher_types::RenamePasskeyRequest {
            name: name.to_owned(),
        })
        .map_err(|err| err.to_string())?;
    let response = request.send().await.map_err(|err| err.to_string())?;
    if response.status() != 200 {
        return Err(error_message(response).await);
    }
    Ok(())
}

/// `rename_passkey` (ssr stub, never called).
#[cfg(not(feature = "csr"))]
#[allow(clippy::unused_async)]
pub async fn rename_passkey(_id: i64, _name: &str) -> Result<(), String> {
    Err(SSR_STUB_ERROR.to_owned())
}

/// `DELETE /api/auth/passkeys/{id}` — deletes a passkey (204). The server
/// answers 409 "cannot delete your last passkey" for the final one (the
/// caller surfaces that message) and 403 "step-up required" for a stale
/// session (`NeedsStepUp` — re-authenticate and retry).
#[cfg(feature = "csr")]
pub async fn delete_passkey(id: i64) -> Result<StepUp<()>, String> {
    let response = gloo_net::http::Request::delete(&format!("/api/auth/passkeys/{id}"))
        .send()
        .await
        .map_err(|err| err.to_string())?;
    step_up_outcome(response, 204).await
}

/// `delete_passkey` (ssr stub, never called).
#[cfg(not(feature = "csr"))]
#[allow(clippy::unused_async)]
pub async fn delete_passkey(_id: i64) -> Result<StepUp<()>, String> {
    Err(SSR_STUB_ERROR.to_owned())
}
