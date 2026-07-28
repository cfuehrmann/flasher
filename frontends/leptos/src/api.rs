//! Thin async wrappers around the Flasher JSON API.
//!
//! The real implementations use `gloo-net` and only exist in the `csr`
//! (browser) build. Under `ssr` every function returns an error instead:
//! they are only ever invoked from effects and event handlers that never
//! run on the server, but having the same signatures in both builds keeps
//! the components themselves cfg-free.

use flasher_types::{
    AutoSaveResponse, CardResponse, FindCardsResponse, GetAutoSaveResponse, HealthResponse,
    NextCardResponse,
};
#[cfg(feature = "csr")]
use flasher_types::{CardUpdateRequest, CreateCardRequest, PutAutoSaveRequest};

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

/// `POST /api/cards/{id}/set-ok`.
#[cfg(feature = "csr")]
pub async fn set_ok(id: &str) -> Result<(), String> {
    set_state(id, "set-ok").await
}

/// `POST /api/cards/{id}/set-ok` (ssr stub, never called).
#[cfg(not(feature = "csr"))]
#[allow(clippy::unused_async)]
pub async fn set_ok(_id: &str) -> Result<(), String> {
    Err(SSR_STUB_ERROR.to_owned())
}

/// `POST /api/cards/{id}/set-failed`.
#[cfg(feature = "csr")]
pub async fn set_failed(id: &str) -> Result<(), String> {
    set_state(id, "set-failed").await
}

/// `POST /api/cards/{id}/set-failed` (ssr stub, never called).
#[cfg(not(feature = "csr"))]
#[allow(clippy::unused_async)]
pub async fn set_failed(_id: &str) -> Result<(), String> {
    Err(SSR_STUB_ERROR.to_owned())
}

/// Shared body of the two rating endpoints; the updated card in the
/// response is irrelevant because the next card is re-fetched afterwards.
#[cfg(feature = "csr")]
async fn set_state(id: &str, action: &str) -> Result<(), String> {
    let response = gloo_net::http::Request::post(&format!("/api/cards/{id}/{action}"))
        .send()
        .await
        .map_err(|err| err.to_string())?;
    if response.status() != 200 {
        return Err(error_message(response).await);
    }
    Ok(())
}

/// `GET /api/cards` — one page of the groom list plus the total match
/// count (before paging) and the server's configured page size. An empty
/// `search_text` lists all cards.
#[cfg(feature = "csr")]
pub async fn find_cards(search_text: &str, skip: u32) -> Result<FindCardsResponse, String> {
    let skip = skip.to_string();
    let mut query = vec![("skip", skip.as_str())];
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
pub async fn find_cards(_search_text: &str, _skip: u32) -> Result<FindCardsResponse, String> {
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
/// A 401 anywhere means the session expired mid-use: the registered
/// unauthorized hook bounces the app back to the auth screen (in
/// dev-bypass mode 401s never occur). The auth ceremony endpoints that
/// expect 401s as a normal outcome (`session`, `login/finish`) handle the
/// status themselves before reaching this.
#[cfg(feature = "csr")]
async fn error_message(response: gloo_net::http::Response) -> String {
    let status = response.status();
    if status == 401 {
        notify_unauthorized();
    }
    match response.text().await {
        Ok(body) if !body.trim().is_empty() => format!("{status}: {body}"),
        _ => format!("request failed with status {status}"),
    }
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

/// `POST /api/auth/register/start` — returns the raw options JSON text
/// (the `webauthn` module converts it for `navigator.credentials`).
/// `token` is the bootstrap token, sent only on the open first-run
/// registration when the server requires it. A 403 (bad/missing token)
/// is surfaced as the server's plain message without the status prefix —
/// it is an expected outcome of a mistyped token, not a failure.
#[cfg(feature = "csr")]
pub async fn register_start(username: &str, token: Option<&str>) -> Result<String, String> {
    let request = gloo_net::http::Request::post("/api/auth/register/start")
        .json(&flasher_types::RegisterStartRequest {
            username: username.to_owned(),
            token: token.map(str::to_owned),
        })
        .map_err(|err| err.to_string())?;
    let response = request.send().await.map_err(|err| err.to_string())?;
    if response.status() == 403 {
        return match response.text().await {
            Ok(body) if !body.trim().is_empty() => Err(body),
            _ => Err("invalid bootstrap token".to_owned()),
        };
    }
    if response.status() != 200 {
        return Err(error_message(response).await);
    }
    response.text().await.map_err(|err| err.to_string())
}

/// `register_start` (ssr stub, never called).
#[cfg(not(feature = "csr"))]
#[allow(clippy::unused_async, dead_code)]
pub async fn register_start(_username: &str, _token: Option<&str>) -> Result<String, String> {
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
/// logged-in username (and the session cookie rides along).
#[cfg(feature = "csr")]
pub async fn login_finish(assertion_json: &str) -> Result<String, String> {
    let request = gloo_net::http::Request::post("/api/auth/login/finish")
        .header("Content-Type", "application/json")
        .body(assertion_json)
        .map_err(|err| err.to_string())?;
    let response = request.send().await.map_err(|err| err.to_string())?;
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
/// answers 409 "cannot delete your last passkey" for the final one; the
/// caller surfaces that message.
#[cfg(feature = "csr")]
pub async fn delete_passkey(id: i64) -> Result<(), String> {
    let response = gloo_net::http::Request::delete(&format!("/api/auth/passkeys/{id}"))
        .send()
        .await
        .map_err(|err| err.to_string())?;
    if response.status() != 204 {
        return Err(error_message(response).await);
    }
    Ok(())
}

/// `delete_passkey` (ssr stub, never called).
#[cfg(not(feature = "csr"))]
#[allow(clippy::unused_async)]
pub async fn delete_passkey(_id: i64) -> Result<(), String> {
    Err(SSR_STUB_ERROR.to_owned())
}
