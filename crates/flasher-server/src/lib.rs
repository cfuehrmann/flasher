//! Axum HTTP server for Flasher.
//!
//! Serves the JSON API under `/api` (unknown `/api/*` paths are a plain
//! 404) and falls back to static hosting of the built frontend (`dist`
//! directory produced by Trunk), with SPA-style fallback to `index.html`
//! for unknown non-API paths.
//!
//! # Cards API (internal contract, see `flasher-types`)
//!
//! | Route                            | Success                | Errors      |
//! |----------------------------------|------------------------|-------------|
//! | `GET /api/cards/next`            | 200 `CardResponse` or 200 `null` | 500 |
//! | `GET /api/cards`                 | 200 `FindCardsResponse` | 400, 500   |
//! | `POST /api/cards`                | 201 `CardResponse`     | 422, 500    |
//! | `GET /api/cards/{id}`            | 200 `CardResponse`     | 404, 500    |
//! | `PATCH /api/cards/{id}`          | 200 `CardResponse`     | 404, 422, 500 |
//! | `DELETE /api/cards/{id}`         | 204                    | 404, 500    |
//! | `DELETE /api/history/{id}`       | 200 `CardResponse`     | 404, 500    |
//! | `POST /api/cards/{id}/set-ok`    | 200 `CardResponse`     | 404, 500    |
//! | `POST /api/cards/{id}/set-failed`| 200 `CardResponse`     | 404, 500    |
//! | `PUT /api/autosave`              | 200 `AutoSaveResponse` | 500         |
//! | `GET /api/autosave`              | 200 `AutoSaveResponse` or 200 `null` | 500 |
//! | `DELETE /api/autosave`           | 204                    | 500         |
//!
//! `GET /api/cards` takes optional query params `search_text` (substring
//! match over prompt and solution) and `skip` (default 0); the page size
//! is the server's configured `page_size` (`FLASHER_PAGE_SIZE`, default
//! [`DEFAULT_PAGE_SIZE`]).
//!
//! `DELETE /api/history/{id}` ports `HistoryHandler.Delete`: it resets the
//! card to state `new` with `change_time = now` and
//! `next_time = now + NewCardWaitingTime`, as if the card had just been
//! created (except it keeps its `disabled` flag).
//!
//! # Auth API (passkeys; full JSON contract in the `flasher-auth` crate docs)
//!
//! | Route                              | Success                          | Errors              |
//! |------------------------------------|----------------------------------|---------------------|
//! | `GET /api/auth/bootstrap`          | 200 `BootstrapResponse`          | 500                 |
//! | `GET /api/auth/session`            | 200 `SessionResponse` or `null`  | —                 |
//! | `POST /api/auth/register/start`    | 200 `WebAuthn` creation options    | 401, 403, 422, 500, 503 |
//! | `POST /api/auth/register/finish`   | 201 `PasskeyResponse`            | 400, 401, 409, 422, 500 |
//! | `POST /api/auth/login/start`       | 200 `WebAuthn` request options     | 500, 503           |
//! | `POST /api/auth/login/finish`      | 200 `SessionResponse` + cookie   | 400, 401, 500       |
//! | `POST /api/auth/logout`            | 204 + cookie cleared             | 500                 |
//! | `GET /api/auth/passkeys`           | 200 `[PasskeyResponse]`          | 401, 500            |
//! | `PATCH /api/auth/passkeys/{id}`    | 200 `PasskeyResponse`            | 401, 404, 422, 500  |
//! | `DELETE /api/auth/passkeys/{id}`   | 204                              | 401, 404, 409, 500  |
//!
//! # Auth modes
//!
//! - **Dev bypass** (`FLASHER_USER` set, [`AppState::dev_bypass`]): every
//!   request acts as that user; no session is required anywhere. The auth
//!   routes stay mounted (register adds a passkey to the dev user).
//! - **Auth mode** (`FLASHER_USER` unset, [`AppState::new`]): all `/api/*`
//!   routes except `/api/health` and `/api/auth/*` require a valid
//!   `__Host-session` cookie (else 401). `register/start` without a
//!   session is open only while the system has zero passkeys (bootstrap).
//!
//! Sessions are server-side rows in the `sessions` table: an opaque
//! 256-bit token in the cookie, a fixed 7-day expiry from creation (no
//! sliding renewal), deleted on logout and swept on startup. The token is
//! stored **plain** — acceptable for this personal app: anyone with read
//! access to the database file already owns all its content.
//!
//! The `/api/autosave` routes port `AutoSaveHandler`: one draft per user,
//! upserted by `PUT` (the store keeps `updated_at` when the content is
//! unchanged), returned by `GET` (`null` when absent) and cleared by
//! `DELETE`. The draft's `card_id` is the card being edited (the old
//! `AutoSave.Id`); `null` means a draft for a brand-new card.
//!
//! All payloads are `snake_case` JSON; timestamps are unix epoch millis.
//! `POST /api/cards` ports `CardsHandler.Create`: the card is created in
//! state `new`, `disabled = true`, `change_time = now` and
//! `next_time = now + NewCardWaitingTime` (30 min). The set-ok/set-failed
//! handlers apply the scheduling rules of `CardsHandler.SetState` via
//! `flasher-core` — with one deliberate difference: the C# `SetState`
//! returned the *next* due card, while these handlers return the *rated*
//! card and the frontend refetches the next card itself.

use std::convert::Infallible;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use axum::extract::{FromRequestParts, Path, Query, Request, State};
use axum::http::header::{
    CACHE_CONTROL, CONTENT_SECURITY_POLICY, CONTENT_TYPE, COOKIE, SET_COOKIE,
    X_CONTENT_TYPE_OPTIONS,
};
use axum::http::request::Parts;
use axum::http::{HeaderValue, StatusCode};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::{
    Json, Router, routing::delete, routing::get, routing::patch, routing::post, routing::put,
};
use flasher_auth::{Auth, Passkey, PublicKeyCredential, RegisterPublicKeyCredential};
use flasher_core::SrsConfig;
use flasher_store::{AutoSave, Card, CardState, NewCard, Store, User};
use flasher_types::{
    AutoSaveResponse, BootstrapResponse, CardResponse, CardUpdateRequest, CreateCardRequest,
    FindCardsResponse, GetAutoSaveResponse, HealthResponse, NextCardResponse, PasskeyResponse,
    PutAutoSaveRequest, RegisterStartRequest, RenamePasskeyRequest, SessionResponse,
};
use serde::Deserialize;
use tokio::net::TcpListener;
use tower_http::compression::CompressionLayer;
use tower_http::services::{ServeDir, ServeFile};

/// Default page size of `GET /api/cards`, matching the old
/// `CardsOptions.PageSize`.
pub const DEFAULT_PAGE_SIZE: u32 = 10;

/// Name of the session cookie (`__Host-` prefix: `Secure`, `Path=/`, no
/// `Domain`). The value is an opaque 256-bit hex token.
pub const SESSION_COOKIE: &str = "__Host-session";

/// Name of the one-shot ceremony cookie set by register/start and
/// login/start (not `__Host-` prefixed: it is scoped to `Path=/api/auth`).
pub const CEREMONY_COOKIE: &str = "flasher-ceremony";

/// Session lifetime: fixed 7 days from creation, no sliding renewal.
const SESSION_TTL_MS: i64 = 7 * 24 * 60 * 60 * 1000;

/// Ceremony cookie lifetime, matching `flasher_auth::CHALLENGE_TTL`
/// (5 minutes).
const CEREMONY_COOKIE_MAX_AGE: u32 = 300;

/// Shared application state.
#[derive(Debug, Clone)]
pub struct AppState {
    store: Store,
    /// The passkey ceremony driver (always present; in dev-bypass mode
    /// [`CurrentUser`] short-circuits before any session check).
    auth: std::sync::Arc<Auth>,
    /// Dev-bypass user: when `Some`, every request is attributed to this
    /// user and no session is required (`FLASHER_USER`).
    dev_user: Option<i64>,
    /// Optional bootstrap token (`FLASHER_BOOTSTRAP_TOKEN`): when set,
    /// the open (session-less, zero-passkey) register/start must carry it.
    bootstrap_token: Option<String>,
    srs: SrsConfig,
    /// Page size of `GET /api/cards` (the old `CardsOptions.PageSize`).
    page_size: u32,
}

impl AppState {
    /// Auth-mode state: requests resolve their user from the session
    /// cookie. Default SRS scheduling parameters and default page size.
    #[must_use]
    pub fn new(store: Store, auth: Auth) -> Self {
        Self {
            store,
            auth: std::sync::Arc::new(auth),
            dev_user: None,
            bootstrap_token: None,
            srs: SrsConfig::default(),
            page_size: DEFAULT_PAGE_SIZE,
        }
    }

    /// Dev-bypass state: every request acts as `user_id`, no session
    /// required (`FLASHER_USER` behavior; the browser e2e harness relies
    /// on this). The `auth` driver is still used by the `/api/auth/*`
    /// routes, which stay mounted.
    #[must_use]
    pub fn dev_bypass(store: Store, auth: Auth, user_id: i64) -> Self {
        Self::new(store, auth).with_dev_user(user_id)
    }

    /// Marks this state as dev-bypass for `user_id`.
    #[must_use]
    pub fn with_dev_user(mut self, user_id: i64) -> Self {
        self.dev_user = Some(user_id);
        self
    }

    /// Sets the bootstrap token required for the open (zero-passkey)
    /// registration (`FLASHER_BOOTSTRAP_TOKEN`).
    #[must_use]
    pub fn with_bootstrap_token(mut self, token: Option<String>) -> Self {
        self.bootstrap_token = token;
        self
    }

    /// Overrides the SRS scheduling parameters (from the `FLASHER_*`
    /// environment variables in `main`).
    #[must_use]
    pub fn with_srs_config(mut self, srs: SrsConfig) -> Self {
        self.srs = srs;
        self
    }

    /// Overrides the page size of `GET /api/cards` (from
    /// `FLASHER_PAGE_SIZE` in `main`).
    #[must_use]
    pub fn with_page_size(mut self, page_size: u32) -> Self {
        self.page_size = page_size;
        self
    }
}

/// The user the current request acts for.
///
/// Dev bypass: the configured `FLASHER_USER`. Auth mode: the user behind
/// the `__Host-session` cookie, else 401.
#[derive(Debug, Clone, Copy)]
pub struct CurrentUser(pub i64);

impl FromRequestParts<AppState> for CurrentUser {
    type Rejection = StatusCode;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        resolve_user(parts, state)
            .await
            .map(|user| Self(user.id))
            .ok_or(StatusCode::UNAUTHORIZED)
    }
}

/// Like [`CurrentUser`] but never rejects: `None` when there is no valid
/// session. Used by the routes whose behavior depends on whether a
/// session exists (`/api/auth/session`, register/start).
#[derive(Debug, Clone)]
pub struct MaybeUser(pub Option<User>);

impl FromRequestParts<AppState> for MaybeUser {
    type Rejection = Infallible;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        Ok(Self(resolve_user(parts, state).await))
    }
}

/// Resolves the request's user: the dev-bypass user, or the user behind
/// a valid (unexpired) session cookie.
async fn resolve_user(parts: &Parts, state: &AppState) -> Option<User> {
    if let Some(user_id) = state.dev_user {
        return state.store.get_user_by_id(user_id).await.ok().flatten();
    }
    let token = cookie_value(parts, SESSION_COOKIE)?;
    state
        .store
        .get_session_user(&token, now_millis())
        .await
        .ok()
        .flatten()
}

/// Reads one cookie from the `Cookie` header. When the name appears more
/// than once, the LAST match wins (hence the reverse scan): a cookie
/// planted by a sibling subdomain is serialized ahead of the real one, so
/// taking the first match would let the planted value shadow it.
fn cookie_value(parts: &Parts, name: &str) -> Option<String> {
    let header = parts.headers.get(COOKIE)?.to_str().ok()?;
    header.rsplit(';').find_map(|pair| {
        let (key, value) = pair.trim().split_once('=')?;
        (key == name).then(|| value.to_owned())
    })
}

/// Constant-time byte-wise string equality for the bootstrap-token
/// compare: a plain `!=` bails out at the first differing byte and leaks
/// the length of the matching prefix through timing. (The length itself
/// is not compared in constant time — token length is not a secret.)
fn constant_time_eq(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    a.len() == b.len() && a.iter().zip(b).fold(0_u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

/// The `Set-Cookie` value creating the session cookie (`Secure` is
/// accepted by browsers on localhost, which is a trustworthy origin).
fn session_set_cookie(token: &str) -> String {
    format!(
        "{SESSION_COOKIE}={token}; Path=/; HttpOnly; Secure; SameSite=Strict; Max-Age={}",
        SESSION_TTL_MS / 1000
    )
}

/// The `Set-Cookie` value deleting the session cookie.
fn session_clear_cookie() -> String {
    format!("{SESSION_COOKIE}=; Path=/; HttpOnly; Secure; SameSite=Strict; Max-Age=0")
}

/// The `Set-Cookie` value creating the one-shot ceremony cookie.
fn ceremony_set_cookie(ceremony: &str) -> String {
    format!(
        "{CEREMONY_COOKIE}={ceremony}; Path=/api/auth; HttpOnly; Secure; SameSite=Strict; Max-Age={CEREMONY_COOKIE_MAX_AGE}"
    )
}

/// The `Set-Cookie` value deleting the one-shot ceremony cookie (sent by
/// register/finish and login/finish: the ceremony is consumed either way,
/// so the cookie must not linger).
fn ceremony_clear_cookie() -> String {
    format!("{CEREMONY_COOKIE}=; Path=/api/auth; HttpOnly; Secure; SameSite=Strict; Max-Age=0")
}

/// Errors of the API handlers.
#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    /// No card with this id exists for the current user.
    #[error("card not found")]
    CardNotFound,
    /// The prompt of a new card must not be empty (or only whitespace).
    #[error("prompt must not be empty")]
    EmptyPrompt,
    /// A card update must change at least one field.
    #[error("update must set at least one of prompt, solution, disabled")]
    EmptyUpdate,
    /// The autosave written by `PUT /api/autosave` could not be read
    /// back (can only happen on a concurrent delete in between).
    #[error("autosave disappeared after write")]
    AutosaveGone,
    /// No session (or an expired one) where one is required.
    #[error("authentication required")]
    Unauthorized,
    /// The bootstrap token sent to register/start did not match
    /// `FLASHER_BOOTSTRAP_TOKEN`.
    #[error("invalid bootstrap token")]
    InvalidBootstrapToken,
    /// The username failed validation (1–64 chars after trimming).
    #[error("username must be 1-64 characters")]
    InvalidUsername,
    /// The passkey name failed validation (1–64 chars after trimming).
    #[error("name must be 1-64 characters")]
    InvalidPasskeyName,
    /// The passkey id does not belong to the current user.
    #[error("passkey not found")]
    PasskeyNotFound,
    /// Deleting the user's last passkey would lock them out.
    #[error("cannot delete your last passkey")]
    LastPasskey,
    /// The credential is already registered (to any user).
    #[error("credential already registered")]
    CredentialRegistered,
    /// Too many in-flight ceremonies (`flasher_auth`'s challenge-store
    /// cap); the client may retry.
    #[error("too many in-flight ceremonies; try again later")]
    TooManyChallenges,
    /// A client-correctable auth failure (unknown/expired ceremony,
    /// malformed ceremony payload, failed verification).
    #[error("{0}")]
    BadAuthRequest(String),
    /// A database operation failed.
    #[error(transparent)]
    Store(#[from] flasher_store::Error),
    /// An internal failure that should not be detailed to the client
    /// (logged instead).
    #[error("internal error: {0}")]
    Internal(String),
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        match self {
            Self::CardNotFound => StatusCode::NOT_FOUND.into_response(),
            Self::EmptyPrompt
            | Self::EmptyUpdate
            | Self::InvalidUsername
            | Self::InvalidPasskeyName => {
                (StatusCode::UNPROCESSABLE_ENTITY, self.to_string()).into_response()
            }
            Self::AutosaveGone => {
                tracing::error!("autosave read-back after upsert failed");
                StatusCode::INTERNAL_SERVER_ERROR.into_response()
            }
            Self::Unauthorized => StatusCode::UNAUTHORIZED.into_response(),
            Self::InvalidBootstrapToken => {
                (StatusCode::FORBIDDEN, self.to_string()).into_response()
            }
            Self::PasskeyNotFound => (StatusCode::NOT_FOUND, self.to_string()).into_response(),
            Self::LastPasskey | Self::CredentialRegistered => {
                (StatusCode::CONFLICT, self.to_string()).into_response()
            }
            // 503 (not 429): the challenge-store cap is a server resource
            // limit, not per-client rate limiting; Retry-After semantics
            // of 503 fit better.
            Self::TooManyChallenges => {
                (StatusCode::SERVICE_UNAVAILABLE, self.to_string()).into_response()
            }
            Self::BadAuthRequest(message) => (StatusCode::BAD_REQUEST, message).into_response(),
            Self::Store(err) => {
                tracing::error!(%err, "store error");
                StatusCode::INTERNAL_SERVER_ERROR.into_response()
            }
            Self::Internal(message) => {
                tracing::error!(%message, "internal error");
                StatusCode::INTERNAL_SERVER_ERROR.into_response()
            }
        }
    }
}

/// Build the application router serving the frontend from `dist_dir`.
pub fn app(dist_dir: PathBuf, state: AppState) -> Router {
    let index = dist_dir.join("index.html");
    let auth = Router::new()
        .route("/bootstrap", get(auth_bootstrap))
        .route("/session", get(auth_session))
        .route("/register/start", post(register_start))
        .route("/register/finish", post(register_finish))
        .route("/login/start", post(login_start))
        .route("/login/finish", post(login_finish))
        .route("/logout", post(logout))
        .route("/passkeys", get(list_passkeys))
        .route(
            "/passkeys/{id}",
            patch(rename_passkey).delete(delete_passkey),
        );
    let api = Router::new()
        .route("/health", get(health))
        .nest("/auth", auth)
        .route("/cards/next", get(next_card))
        .route("/cards", post(create_card).get(find_cards))
        .route(
            "/cards/{id}",
            get(get_card).patch(patch_card).delete(delete_card),
        )
        .route("/history/{id}", delete(delete_history))
        .route("/cards/{id}/set-ok", post(set_ok))
        .route("/cards/{id}/set-failed", post(set_failed))
        .route(
            "/autosave",
            put(put_autosave).get(get_autosave).delete(delete_autosave),
        )
        // Unknown /api/* paths must be a 404, never the SPA's index.html.
        .fallback(api_fallback);
    Router::new()
        .nest("/api", api)
        .fallback_service(ServeDir::new(dist_dir).fallback(ServeFile::new(index.clone())))
        .layer(middleware::from_fn(cache_headers))
        // Security headers (CSP, nosniff) on every response, static + API.
        .layer(middleware::from_fn_with_state(
            std::sync::Arc::new(content_security_policy(&index)),
            security_headers,
        ))
        // Outermost layer: gzip/brotli for static assets and API JSON alike.
        .layer(CompressionLayer::new())
        .with_state(state)
}

/// Fallback for unknown `/api/*` paths: 404 with an empty body (only
/// client-side routes may fall through to the SPA).
async fn api_fallback() -> StatusCode {
    StatusCode::NOT_FOUND
}

/// Adds the security headers to every response: a restrictive
/// `Content-Security-Policy` and `X-Content-Type-Options: nosniff`. The
/// CSP comes in as router state because it is computed once at startup
/// (see [`content_security_policy`]).
async fn security_headers(
    State(csp): State<std::sync::Arc<String>>,
    request: Request,
    next: Next,
) -> Response {
    let mut response = next.run(request).await;
    if let Ok(value) = HeaderValue::from_str(&csp) {
        response
            .headers_mut()
            .insert(CONTENT_SECURITY_POLICY, value);
    }
    response
        .headers_mut()
        .insert(X_CONTENT_TYPE_OPTIONS, HeaderValue::from_static("nosniff"));
    response
}

/// Builds the `Content-Security-Policy` for the app. The base policy is
/// strict — `script-src 'self' 'wasm-unsafe-eval'` (the wasm bundle needs
/// the eval allowance to instantiate; `style-src` keeps `unsafe-inline`
/// for the inlined loading-skeleton `<style>` and `KaTeX`'s inline style
/// attributes; everything else is self-only or off).
///
/// Trunk injects an inline bootstrap `<script type="module">` into
/// `index.html` that imports the hashed js/wasm bundle — its content
/// changes with every build, so a static hash list is impossible. Since
/// `script-src` deliberately has no `unsafe-inline`, the exact inline
/// snippets of the served `index.html` are allow-listed by sha-256 hash,
/// computed once here at startup.
fn content_security_policy(index_html: &std::path::Path) -> String {
    const BASE: &str = "default-src 'self'; script-src 'self' 'wasm-unsafe-eval'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; font-src 'self'; connect-src 'self'; base-uri 'none'; frame-ancestors 'none'";
    let hashes = inline_script_hashes(index_html);
    if hashes.is_empty() {
        return BASE.to_owned();
    }
    BASE.replacen(
        "'wasm-unsafe-eval'",
        &format!("'wasm-unsafe-eval' {}", hashes.join(" ")),
        1,
    )
}

/// The CSP sha-256 hashes (`'sha256-…'`) of every inline `<script>` in
/// the given HTML file (external scripts with a `src` attribute are
/// covered by `'self'` and skipped). The hash covers the exact text
/// between the tags, whitespace included — that is what the browser
/// hashes. A missing/unreadable file yields no hashes (the CSP then
/// blocks inline scripts; only the real app's Trunk loader needs one).
fn inline_script_hashes(index_html: &std::path::Path) -> Vec<String> {
    use base64::Engine as _;
    use sha2::Digest as _;

    let Ok(html) = std::fs::read_to_string(index_html) else {
        return Vec::new();
    };
    let mut hashes = Vec::new();
    let mut rest = html.as_str();
    while let Some(open) = rest.find("<script") {
        let Some(tag_end) = rest[open..].find('>') else {
            break;
        };
        let tag = &rest[open..open + tag_end];
        let body = &rest[open + tag_end + 1..];
        let Some(body_end) = body.find("</script>") else {
            break;
        };
        if !tag.contains("src=") {
            let digest = sha2::Sha256::digest(&body.as_bytes()[..body_end]);
            let encoded = base64::engine::general_purpose::STANDARD.encode(digest);
            hashes.push(format!("'sha256-{encoded}'"));
        }
        rest = &body[body_end..];
    }
    hashes
}

/// `Cache-Control` for every response: content-hashed Trunk assets
/// (`*-<16 hex chars>.js|wasm|css`) are immutable for a year; HTML (incl.
/// the SPA fallback) and the API are always revalidated. The immutable
/// arm requires a non-HTML response: a missing hashed-looking path falls
/// through to the SPA fallback (200 `text/html`) and must get `no-cache`
/// like any other HTML, or the browser would pin the error page for a
/// year.
async fn cache_headers(request: Request, next: Next) -> Response {
    let path = request.uri().path().to_owned();
    let mut response = next.run(request).await;
    let value = if is_hashed_asset(&path) && !is_html(&response) {
        "public, max-age=31536000, immutable"
    } else if path.starts_with("/api/") || is_html(&response) {
        "no-cache"
    } else {
        // Unhashed static files (robots.txt, llms.txt, ...): short cache.
        "public, max-age=3600"
    };
    if let Ok(value) = HeaderValue::from_str(value) {
        response.headers_mut().insert(CACHE_CONTROL, value);
    }
    response
}

/// True for HTML responses (the SPA fallback also serves `index.html` for
/// unknown paths, so the content type — not the URL — is the signal).
fn is_html(response: &Response) -> bool {
    response
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.starts_with("text/html"))
}

/// True for Trunk's content-hashed file names, e.g.
/// `flasher-leptos-c37081118a308d1d.js`, `...-c37081118a308d1d_bg.wasm`
/// or `app-3d0f8d9643f50d5a.css`.
fn is_hashed_asset(path: &str) -> bool {
    let Some(name) = path.rsplit('/').next() else {
        return false;
    };
    let Some((stem, ext)) = name.rsplit_once('.') else {
        return false;
    };
    if !matches!(ext, "js" | "wasm" | "css") {
        return false;
    }
    let stem = stem.strip_suffix("_bg").unwrap_or(stem);
    let Some(hash) = stem.rsplit('-').next() else {
        return false;
    };
    hash.len() >= 16 && hash.bytes().all(|b| b.is_ascii_hexdigit())
}

/// Serve the application on `listener` until ctrl-c (graceful shutdown).
///
/// # Errors
///
/// Returns any I/O error raised while accepting connections.
pub async fn serve(
    listener: TcpListener,
    dist_dir: PathBuf,
    state: AppState,
) -> std::io::Result<()> {
    axum::serve(listener, app(dist_dir, state))
        .with_graceful_shutdown(shutdown_signal())
        .await
}

async fn shutdown_signal() {
    if let Err(err) = tokio::signal::ctrl_c().await {
        tracing::error!(%err, "failed to listen for ctrl-c");
    }
}

async fn health() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok".to_owned(),
        version: env!("CARGO_PKG_VERSION").to_owned(),
    })
}

// --------------------------------------------------------------------- auth

/// `GET /api/auth/bootstrap` — `registration_open` while the system has
/// zero passkeys (any user); `token_required` when the open bootstrap is
/// gated by `FLASHER_BOOTSTRAP_TOKEN` (the register screen then asks for
/// the token).
async fn auth_bootstrap(
    State(state): State<AppState>,
) -> Result<Json<BootstrapResponse>, ApiError> {
    let registration_open = state.store.count_passkeys().await? == 0;
    Ok(Json(BootstrapResponse {
        registration_open,
        token_required: state.bootstrap_token.is_some(),
    }))
}

/// `GET /api/auth/session` — 200 with the current user, 200 `null` without
/// a valid session (dev bypass always answers with the user). A 200-null
/// keeps the normal logged-out probe out of the browser console as an
/// "error" (Lighthouse best-practices); protected endpoints still 401.
async fn auth_session(MaybeUser(user): MaybeUser) -> Json<Option<SessionResponse>> {
    Json(user.map(|user| SessionResponse {
        username: user.username,
    }))
}

/// `POST /api/auth/register/start` — begins a passkey registration.
///
/// With a session: adds a passkey to the session's user (`username` is
/// ignored). Without a session: the open bootstrap, allowed only while
/// the system has zero passkeys; `username` claims an existing user
/// (case-insensitive — the migrated no-passkeys case) or creates a new
/// one. The bootstrap optionally requires `FLASHER_BOOTSTRAP_TOKEN`.
async fn register_start(
    State(state): State<AppState>,
    MaybeUser(session_user): MaybeUser,
    Json(request): Json<RegisterStartRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let user = if let Some(user) = session_user {
        user
    } else {
        if state.store.count_passkeys().await? > 0 {
            return Err(ApiError::Unauthorized);
        }
        if let Some(expected) = &state.bootstrap_token
            && !request
                .token
                .as_deref()
                .is_some_and(|token| constant_time_eq(token, expected))
        {
            return Err(ApiError::InvalidBootstrapToken);
        }
        let username = validate_name(&request.username).ok_or(ApiError::InvalidUsername)?;
        match state.store.get_user_by_name(&username).await? {
            // Claiming an existing (passkey-less) user: safe, because
            // this branch only runs while NO passkey exists at all,
            // so no account can be authenticated as yet.
            Some(user) => user,
            None => state.store.create_user(&username).await?,
        }
    };
    let existing = load_passkeys(&state, user.id).await?;
    let (ccr, ceremony) = state
        .auth
        .start_registration(Auth::user_handle_for(user.id), &user.username, &existing)
        .map_err(start_ceremony_error)?;
    Ok(([(SET_COOKIE, ceremony_set_cookie(&ceremony))], Json(ccr)))
}

/// `POST /api/auth/register/finish` — verifies the attestation and stores
/// the new passkey (auto-named `Passkey N`), attached to the user the
/// ceremony was started for. 201 with the created passkey.
async fn register_finish(
    State(state): State<AppState>,
    MaybeUser(session_user): MaybeUser,
    parts: Parts,
    Json(body): Json<serde_json::Value>,
) -> Result<impl IntoResponse, ApiError> {
    let ceremony = cookie_value(&parts, CEREMONY_COOKIE)
        .ok_or_else(|| ApiError::BadAuthRequest("missing ceremony cookie".to_owned()))?;
    let reg: RegisterPublicKeyCredential = serde_json::from_value(body)
        .map_err(|err| ApiError::BadAuthRequest(format!("malformed credential json: {err}")))?;
    let (passkey, user_handle) = state
        .auth
        .finish_registration(&ceremony, &reg)
        .map_err(auth_ceremony_error)?;
    let user_id = Auth::user_id_from_handle(&user_handle)
        .ok_or_else(|| ApiError::Internal("foreign user handle in ceremony".to_owned()))?;
    if let Some(session_user) = &session_user
        && session_user.id != user_id
    {
        return Err(ApiError::BadAuthRequest(
            "ceremony was started for a different user".to_owned(),
        ));
    }
    let user = state
        .store
        .get_user_by_id(user_id)
        .await?
        .ok_or_else(|| ApiError::Internal(format!("user {user_id} of ceremony not found")))?;
    let credential_id = flasher_auth::base64url_string(passkey.cred_id());
    // The open (session-less) bootstrap was admitted at start time while
    // the system had zero passkeys; re-check the window immediately before
    // the insert: if another registration landed in between, the bootstrap
    // is closed (401, the same answer register/start gives then).
    if session_user.is_none() && state.store.count_passkeys().await? > 0 {
        return Err(ApiError::Unauthorized);
    }
    let count = state.store.count_passkeys_for_user(user.id).await?;
    let name = format!("Passkey {}", count + 1);
    let data = serde_json::to_string(&passkey).map_err(auth_internal)?;
    let created_at = now_millis();
    let id = match state
        .store
        .insert_passkey(user.id, &credential_id, &name, &data, created_at)
        .await
    {
        Ok(id) => id,
        // credential_id is globally UNIQUE: a duplicate (double submit,
        // replay, or a credential already registered to another user) is
        // a 409, never a 500.
        Err(err) if err.is_unique_violation() => return Err(ApiError::CredentialRegistered),
        Err(err) => return Err(ApiError::Store(err)),
    };
    Ok((
        StatusCode::CREATED,
        // The ceremony is consumed: clear the one-shot cookie.
        axum::response::AppendHeaders([(SET_COOKIE, ceremony_clear_cookie())]),
        Json(PasskeyResponse {
            id,
            name,
            created_at,
            last_used_at: None,
        }),
    ))
}

/// `POST /api/auth/login/start` — begins a username-less (discoverable)
/// authentication: no body, `allowCredentials: []`.
async fn login_start(State(state): State<AppState>) -> Result<impl IntoResponse, ApiError> {
    let (rcr, ceremony) = state
        .auth
        .start_authentication()
        .map_err(start_ceremony_error)?;
    Ok(([(SET_COOKIE, ceremony_set_cookie(&ceremony))], Json(rcr)))
}

/// `POST /api/auth/login/finish` — verifies the assertion, identifies the
/// user by credential handle, creates the session, sets the cookie.
async fn login_finish(
    State(state): State<AppState>,
    parts: Parts,
    Json(body): Json<serde_json::Value>,
) -> Result<impl IntoResponse, ApiError> {
    let ceremony = cookie_value(&parts, CEREMONY_COOKIE)
        .ok_or_else(|| ApiError::BadAuthRequest("missing ceremony cookie".to_owned()))?;
    let assertion: PublicKeyCredential = serde_json::from_value(body)
        .map_err(|err| ApiError::BadAuthRequest(format!("malformed credential json: {err}")))?;
    let (user_handle, credential_id) = state
        .auth
        .identify_authentication(&assertion)
        .map_err(|_| ApiError::Unauthorized)?;
    let row = state
        .store
        .get_passkey_by_credential_id(&credential_id)
        .await?
        .ok_or(ApiError::Unauthorized)?;
    if Auth::user_id_from_handle(&user_handle) != Some(row.user_id) {
        return Err(ApiError::Unauthorized);
    }
    let mut passkey: Passkey = serde_json::from_str(&row.data).map_err(auth_internal)?;
    let result = state
        .auth
        .finish_authentication(&ceremony, &assertion, &passkey)
        .map_err(|err| match err {
            flasher_auth::Error::UnknownCeremony | flasher_auth::Error::CeremonyKind => {
                auth_ceremony_error(err)
            }
            _ => ApiError::Unauthorized,
        })?;
    // Persist counter/backup-flag updates and stamp the usage time.
    let _ = passkey.update_credential(&result);
    let data = serde_json::to_string(&passkey).map_err(auth_internal)?;
    state
        .store
        .update_passkey_after_auth(row.user_id, row.id, &data, now_millis())
        .await?;
    let user = state
        .store
        .get_user_by_id(row.user_id)
        .await?
        .ok_or_else(|| ApiError::Internal(format!("user {} of passkey not found", row.user_id)))?;
    let token = Auth::generate_token();
    state
        .store
        .create_session(&token, user.id, now_millis() + SESSION_TTL_MS)
        .await?;
    Ok((
        // The ceremony is consumed: clear the one-shot cookie.
        axum::response::AppendHeaders([
            (SET_COOKIE, session_set_cookie(&token)),
            (SET_COOKIE, ceremony_clear_cookie()),
        ]),
        Json(SessionResponse {
            username: user.username,
        }),
    ))
}

/// `POST /api/auth/logout` — deletes the session row (if any) and clears
/// the cookie. Always 204, even without a session (idempotent).
async fn logout(
    State(state): State<AppState>,
    parts: Parts,
) -> Result<impl IntoResponse, ApiError> {
    if let Some(token) = cookie_value(&parts, SESSION_COOKIE) {
        state.store.delete_session(&token).await?;
    }
    Ok((
        [(SET_COOKIE, session_clear_cookie())],
        StatusCode::NO_CONTENT,
    ))
}

/// `GET /api/auth/passkeys` — the current user's passkeys, id order.
async fn list_passkeys(
    State(state): State<AppState>,
    CurrentUser(user_id): CurrentUser,
) -> Result<Json<Vec<PasskeyResponse>>, ApiError> {
    let rows = state.store.get_passkeys_for_user(user_id).await?;
    Ok(Json(rows.into_iter().map(passkey_response).collect()))
}

/// `PATCH /api/auth/passkeys/{id}` — renames an own passkey.
async fn rename_passkey(
    State(state): State<AppState>,
    CurrentUser(user_id): CurrentUser,
    Path(id): Path<i64>,
    Json(request): Json<RenamePasskeyRequest>,
) -> Result<Json<PasskeyResponse>, ApiError> {
    let name = validate_name(&request.name).ok_or(ApiError::InvalidPasskeyName)?;
    if !state.store.rename_passkey(user_id, id, &name).await? {
        return Err(ApiError::PasskeyNotFound);
    }
    let row = state
        .store
        .get_passkeys_for_user(user_id)
        .await?
        .into_iter()
        .find(|row| row.id == id)
        .ok_or_else(|| ApiError::Internal(format!("passkey {id} vanished after rename")))?;
    Ok(Json(passkey_response(row)))
}

/// `DELETE /api/auth/passkeys/{id}` — deletes an own passkey, refusing
/// the user's last one (409). The last-passkey guard is atomic in the
/// store's DELETE statement; a zero-row result is disambiguated here by
/// checking whether the passkey (still) exists.
async fn delete_passkey(
    State(state): State<AppState>,
    CurrentUser(user_id): CurrentUser,
    Path(id): Path<i64>,
) -> Result<StatusCode, ApiError> {
    if state.store.delete_passkey(user_id, id).await? {
        return Ok(StatusCode::NO_CONTENT);
    }
    let exists = state
        .store
        .get_passkeys_for_user(user_id)
        .await?
        .iter()
        .any(|row| row.id == id);
    Err(if exists {
        ApiError::LastPasskey
    } else {
        ApiError::PasskeyNotFound
    })
}

/// Loads and deserializes a user's passkeys (for `excludeCredentials`).
async fn load_passkeys(state: &AppState, user_id: i64) -> Result<Vec<Passkey>, ApiError> {
    state
        .store
        .get_passkeys_for_user(user_id)
        .await?
        .iter()
        .map(|row| serde_json::from_str(&row.data).map_err(auth_internal))
        .collect()
}

fn passkey_response(row: flasher_store::PasskeyRow) -> PasskeyResponse {
    PasskeyResponse {
        id: row.id,
        name: row.name,
        created_at: row.created_at,
        last_used_at: row.last_used_at,
    }
}

/// Trims and validates a username/passkey name: 1–64 chars.
fn validate_name(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    (1..=64)
        .contains(&trimmed.chars().count())
        .then(|| trimmed.to_owned())
}

/// Maps an auth-crate failure during a finish step to a client error.
fn auth_ceremony_error(err: flasher_auth::Error) -> ApiError {
    match err {
        flasher_auth::Error::UnknownCeremony | flasher_auth::Error::CeremonyKind => {
            ApiError::BadAuthRequest(err.to_string())
        }
        flasher_auth::Error::Webauthn(_) => {
            ApiError::BadAuthRequest("passkey verification failed".to_owned())
        }
        other => auth_internal(other),
    }
}

/// Maps an auth-crate failure while STARTING a ceremony: the challenge
/// store cap is a 503, everything else a logged 500.
fn start_ceremony_error(err: flasher_auth::Error) -> ApiError {
    match err {
        flasher_auth::Error::TooManyChallenges => ApiError::TooManyChallenges,
        other => auth_internal(other),
    }
}

/// Maps an internal auth-crate failure to a logged 500.
fn auth_internal(err: impl std::fmt::Display) -> ApiError {
    ApiError::Internal(format!("auth: {err}"))
}

/// `GET /api/cards/next` — the next due enabled card, or `null`.
async fn next_card(
    State(state): State<AppState>,
    CurrentUser(user_id): CurrentUser,
) -> Result<Json<NextCardResponse>, ApiError> {
    let card = state.store.next_card(user_id, now_millis()).await?;
    Ok(Json(card.map(card_response)))
}

/// `POST /api/cards` — port of `CardsHandler.Create`: state `new`,
/// `disabled = true`, `change_time = now`,
/// `next_time = now + NewCardWaitingTime`.
async fn create_card(
    State(state): State<AppState>,
    CurrentUser(user_id): CurrentUser,
    Json(request): Json<CreateCardRequest>,
) -> Result<(StatusCode, Json<CardResponse>), ApiError> {
    if request.prompt.trim().is_empty() {
        return Err(ApiError::EmptyPrompt);
    }
    let now = now_millis();
    let card = NewCard {
        user_id,
        id: uuid::Uuid::new_v4().to_string(),
        prompt: request.prompt,
        solution: request.solution,
        state: CardState::New,
        change_time: now,
        next_time: flasher_core::next_time_for_new_card(now, &state.srs),
        disabled: true,
    };
    state.store.insert_card(&card).await?;
    let response = CardResponse {
        id: card.id,
        prompt: card.prompt,
        solution: card.solution,
        state: card.state,
        change_time: card.change_time,
        next_time: card.next_time,
        disabled: card.disabled,
    };
    Ok((StatusCode::CREATED, Json(response)))
}

/// Query parameters of `GET /api/cards` (`snake_case`; the old route used
/// `searchText`, but this API is internal with no compat constraints).
#[derive(Debug, Deserialize)]
struct FindCardsQuery {
    search_text: Option<String>,
    skip: Option<u32>,
}

/// `GET /api/cards` — port of `CardsHandler.Find`: full-Unicode
/// case-insensitive substring match over prompt and solution, enabled
/// cards first, then `next_time` ascending; `take` is the configured
/// page size. Returns the page plus the total match count.
async fn find_cards(
    State(state): State<AppState>,
    CurrentUser(user_id): CurrentUser,
    Query(query): Query<FindCardsQuery>,
) -> Result<Json<FindCardsResponse>, ApiError> {
    let (cards, count) = state
        .store
        .search_cards(
            user_id,
            query.search_text.as_deref(),
            query.skip.unwrap_or(0),
            state.page_size,
        )
        .await?;
    Ok(Json(FindCardsResponse {
        cards: cards.into_iter().map(card_response).collect(),
        count,
        page_size: i64::from(state.page_size),
    }))
}

/// `GET /api/cards/{id}` — one card by id, 404 for unknown/other-user
/// ids. Exists for the editor's deep-link restore (Phase 6.6): a fresh
/// load of `/groom/edit/{id}` fetches exactly this card instead of
/// paging through `GET /api/cards`.
async fn get_card(
    State(state): State<AppState>,
    CurrentUser(user_id): CurrentUser,
    Path(id): Path<String>,
) -> Result<Json<CardResponse>, ApiError> {
    let card = state
        .store
        .get_card(user_id, &id)
        .await?
        .ok_or(ApiError::CardNotFound)?;
    Ok(Json(card_response(card)))
}

/// `PATCH /api/cards/{id}` — port of `CardsHandler.Update`: an
/// all-optional partial update of prompt/solution/disabled (the old
/// request had no `disabled`, it was toggled via Enable/Disable — here
/// one endpoint covers all three). Like the old handler, there is no
/// prompt-emptiness guard on update; an all-absent body is a 422.
/// Side effect ported from `CardsHandler.Update`: when the request
/// changes content (prompt and/or solution), the user's autosave draft is
/// deleted; a pure `disabled` toggle keeps it (the old Enable/Disable
/// endpoints never touched the autosave). Returns the updated card, 404
/// for unknown/other-user ids.
///
/// Deliberate deviation from `CardsHandler.Update`: the C# handler
/// deleted the draft before checking whether the card exists. Here the
/// draft is deleted only AFTER the card was found and the update applied
/// — deleting it in the failure case would destroy the crash-recovery
/// net exactly when the edit did not land.
async fn patch_card(
    State(state): State<AppState>,
    CurrentUser(user_id): CurrentUser,
    Path(id): Path<String>,
    Json(request): Json<CardUpdateRequest>,
) -> Result<Json<CardResponse>, ApiError> {
    if request.prompt.is_none() && request.solution.is_none() && request.disabled.is_none() {
        return Err(ApiError::EmptyUpdate);
    }
    let content_changed = request.prompt.is_some() || request.solution.is_some();
    let updated = state
        .store
        .update_card_fields(
            user_id,
            &id,
            request.prompt.as_deref(),
            request.solution.as_deref(),
            request.disabled,
        )
        .await?
        .ok_or(ApiError::CardNotFound)?;
    if content_changed {
        state.store.delete_autosave(user_id).await?;
    }
    Ok(Json(card_response(updated)))
}

/// `DELETE /api/cards/{id}` — port of `CardsHandler.Delete`: 204 on
/// success, 404 for unknown/other-user ids.
async fn delete_card(
    State(state): State<AppState>,
    CurrentUser(user_id): CurrentUser,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    if state.store.delete_card(user_id, &id).await? {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(ApiError::CardNotFound)
    }
}

/// `DELETE /api/history/{id}` — port of `HistoryHandler.Delete`: resets
/// the learning history by putting the card back to state `new` with
/// `change_time = now` and `next_time = now + NewCardWaitingTime`
/// (the `disabled` flag is untouched). Returns the updated card, 404 for
/// unknown/other-user ids.
async fn delete_history(
    State(state): State<AppState>,
    CurrentUser(user_id): CurrentUser,
    Path(id): Path<String>,
) -> Result<Json<CardResponse>, ApiError> {
    let now = now_millis();
    let next_time = flasher_core::next_time_for_new_card(now, &state.srs);
    let updated = state
        .store
        .set_card_state(user_id, &id, CardState::New, now, next_time)
        .await?
        .ok_or(ApiError::CardNotFound)?;
    Ok(Json(card_response(updated)))
}

/// `POST /api/cards/{id}/set-ok`.
async fn set_ok(
    State(state): State<AppState>,
    CurrentUser(user_id): CurrentUser,
    Path(id): Path<String>,
) -> Result<Json<CardResponse>, ApiError> {
    set_state(&state, user_id, &id, CardState::Ok).await
}

/// `POST /api/cards/{id}/set-failed`.
async fn set_failed(
    State(state): State<AppState>,
    CurrentUser(user_id): CurrentUser,
    Path(id): Path<String>,
) -> Result<Json<CardResponse>, ApiError> {
    set_state(&state, user_id, &id, CardState::Failed).await
}

/// Shared body of set-ok/set-failed, applying the scheduling rules of
/// `CardsHandler.SetState`: 404 for unknown/other-user cards,
/// `change_time` becomes `now` and `next_time` is rescheduled by
/// `flasher-core`. Deliberate difference to the C# handler, which
/// returned the *next* due card: this returns the *rated* card and the
/// frontend refetches the next card itself.
async fn set_state(
    state: &AppState,
    user_id: i64,
    id: &str,
    card_state: CardState,
) -> Result<Json<CardResponse>, ApiError> {
    let card = state
        .store
        .get_card(user_id, id)
        .await?
        .ok_or(ApiError::CardNotFound)?;
    let now = now_millis();
    let next_time = match card_state {
        CardState::Ok => flasher_core::next_time_after_ok(card.change_time, now, &state.srs),
        CardState::New | CardState::Failed => {
            flasher_core::next_time_after_failed(card.change_time, now, &state.srs)
        }
    };
    let updated = state
        .store
        .set_card_state(user_id, id, card_state, now, next_time)
        .await?
        .ok_or(ApiError::CardNotFound)?;
    Ok(Json(card_response(updated)))
}

/// `PUT /api/autosave` — port of `AutoSaveHandler.Write`: upserts the
/// user's draft. The store keeps `updated_at` when the content is
/// unchanged, so the response is read back after the write.
async fn put_autosave(
    State(state): State<AppState>,
    CurrentUser(user_id): CurrentUser,
    Json(request): Json<PutAutoSaveRequest>,
) -> Result<Json<AutoSaveResponse>, ApiError> {
    state
        .store
        .put_autosave(
            user_id,
            request.card_id.as_deref(),
            &request.prompt,
            &request.solution,
            now_millis(),
        )
        .await?;
    // The upsert cannot fail to leave a row behind, so this read always
    // finds the draft just written.
    let autosave = state
        .store
        .get_autosave(user_id)
        .await?
        .ok_or(ApiError::AutosaveGone)?;
    Ok(Json(autosave_response(autosave)))
}

/// `GET /api/autosave` — the user's draft, or `null` when there is none.
async fn get_autosave(
    State(state): State<AppState>,
    CurrentUser(user_id): CurrentUser,
) -> Result<Json<GetAutoSaveResponse>, ApiError> {
    let autosave = state.store.get_autosave(user_id).await?;
    Ok(Json(autosave.map(autosave_response)))
}

/// `DELETE /api/autosave` — port of `AutoSaveHandler.Delete`: always 204
/// (the old handler also did not distinguish "no draft existed").
async fn delete_autosave(
    State(state): State<AppState>,
    CurrentUser(user_id): CurrentUser,
) -> Result<StatusCode, ApiError> {
    state.store.delete_autosave(user_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

fn autosave_response(autosave: AutoSave) -> AutoSaveResponse {
    AutoSaveResponse {
        card_id: autosave.card_id,
        prompt: autosave.prompt,
        solution: autosave.solution,
        updated_at: autosave.updated_at,
    }
}

fn card_response(card: Card) -> CardResponse {
    CardResponse {
        id: card.id,
        prompt: card.prompt,
        solution: card.solution,
        state: card.state,
        change_time: card.change_time,
        next_time: card.next_time,
        disabled: card.disabled,
    }
}

/// Current time as unix epoch millis, falling back to 0 if the system
/// clock is before the epoch.
fn now_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_millis()).unwrap_or(i64::MAX))
}

#[cfg(test)]
mod tests {
    //! Unit tests for the pure helpers (HTTP behavior lives in
    //! `tests/`).

    use super::*;

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    #[test]
    fn constant_time_eq_compares_byte_wise() {
        assert!(constant_time_eq("s3cret", "s3cret"));
        assert!(constant_time_eq("", ""));
        assert!(!constant_time_eq("s3cret", "s3creT"));
        // Two differing positions whose XORs cancel out: only an OR-fold
        // of the per-byte differences stays nonzero.
        assert!(!constant_time_eq("ab", "ba"));
        // Different lengths never match (and must not panic on zip).
        assert!(!constant_time_eq("s3cret", "s3cret-longer"));
        assert!(!constant_time_eq("s3cret-longer", "s3cret"));
        assert!(!constant_time_eq("", "x"));
    }

    #[test]
    fn session_ttl_is_seven_days_and_the_cookie_carries_it() {
        assert_eq!(SESSION_TTL_MS, 604_800_000);
        assert!(session_set_cookie("tok").contains("Max-Age=604800"));
    }

    #[test]
    fn is_html_goes_by_the_content_type_header() {
        let mut html = StatusCode::OK.into_response();
        html.headers_mut().insert(
            CONTENT_TYPE,
            HeaderValue::from_static("text/html; charset=utf-8"),
        );
        assert!(is_html(&html));
        let mut json = StatusCode::OK.into_response();
        json.headers_mut()
            .insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        assert!(!is_html(&json));
        // No Content-Type at all: not HTML.
        assert!(!is_html(&StatusCode::OK.into_response()));
    }

    #[test]
    fn is_hashed_asset_recognizes_trunk_file_names() {
        assert!(is_hashed_asset("/flasher-leptos-c37081118a308d1d.js"));
        assert!(is_hashed_asset("/flasher-leptos-c37081118a308d1d_bg.wasm"));
        assert!(is_hashed_asset("/assets/app-3d0f8d9643f50d5a.css"));
        // The boundary is 16 hex chars: exactly 16 counts, 15 does not.
        assert!(is_hashed_asset("/app-0123456789abcdef.js"));
        assert!(!is_hashed_asset("/app-0123456789abcde.js"));
        // Non-hex characters disqualify the hash.
        assert!(!is_hashed_asset("/app-gggggggggggggggg.js"));
        // A hash-like suffix on an unhashed extension does not count.
        assert!(!is_hashed_asset("/app-c37081118a308d1d.png"));
        assert!(!is_hashed_asset("/robots.txt"));
        assert!(!is_hashed_asset("/no-extension"));
    }

    #[test]
    fn inline_script_hashes_covers_inline_scripts_only() -> TestResult {
        let dir = std::env::temp_dir().join(format!("flasher-csp-test-{}", std::process::id()));
        let path = dir.join("index.html");
        let html = "<html><head><script src=\"/app.js\"></script>\
                    <script type=\"module\">\ninline();\n</script></head></html>";
        std::fs::create_dir_all(&dir)?;
        std::fs::write(&path, html)?;
        let hashes = inline_script_hashes(&path);
        assert_eq!(hashes.len(), 1, "only the inline script is hashed");
        assert!(
            hashes[0].starts_with("'sha256-") && hashes[0].ends_with('\''),
            "got: {}",
            hashes[0]
        );
        // The CSP embeds the hash in script-src and keeps the base policy.
        let csp = content_security_policy(&path);
        assert!(csp.contains("default-src 'self'"));
        assert!(csp.contains(&hashes[0]));
        // No inline scripts: the base policy is used unchanged.
        assert_eq!(
            content_security_policy(&dir.join("missing.html")),
            "default-src 'self'; script-src 'self' 'wasm-unsafe-eval'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; font-src 'self'; connect-src 'self'; base-uri 'none'; frame-ancestors 'none'"
        );
        std::fs::remove_dir_all(&dir)?;
        Ok(())
    }

    /// Cross-check the hash extraction against a known vector: sha-256 of
    /// `\ninline();\n` (the inline script body above), base64-encoded.
    #[test]
    fn inline_script_hash_matches_reference() -> TestResult {
        use base64::Engine as _;
        use sha2::Digest as _;

        let dir = std::env::temp_dir().join(format!("flasher-csp-ref-{}", std::process::id()));
        let path = dir.join("index.html");
        std::fs::create_dir_all(&dir)?;
        std::fs::write(&path, "<script>\ninline();\n</script>")?;
        let reference = format!(
            "'sha256-{}'",
            base64::engine::general_purpose::STANDARD
                .encode(sha2::Sha256::digest(b"\ninline();\n"))
        );
        assert_eq!(inline_script_hashes(&path), vec![reference]);
        std::fs::remove_dir_all(&dir)?;
        Ok(())
    }
}
