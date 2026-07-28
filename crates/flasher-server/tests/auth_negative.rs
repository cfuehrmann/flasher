//! Auth negative-path tests: handler-level coverage of the auth surface
//! against a real in-memory store, driving full `WebAuthn` ceremonies
//! with the `softpasskey` software authenticator.
//!
//! These tests are the doctrine's sanctioned auth-security exception. The
//! project doctrine (see `flasher-e2e`, "e2e-primary doctrine") is that
//! the browser is the only valid public test surface and the HTTP API
//! must not be driven directly; auth security is the deliberate
//! exception, because the adversarial cases here (cross-user ceremony
//! confusion, credential/handle mismatch, bootstrap-window races, cookie
//! shadowing) cannot be produced through a well-behaved browser at all.
//!
//! Coverage: (a) session-user ≠ ceremony-user in register/finish,
//! (b) userHandle ↔ credential-owner mismatch in login/finish,
//! (c) bootstrap-token rejections, (d) expired sessions, (e) other-user
//! passkey rename/delete, (f) last-passkey delete, plus the
//! bootstrap-window re-check, the duplicate-credential 409, cookie
//! last-match, ceremony-cookie clearing, the challenge-store cap, session
//! revocation on passkey delete, and the step-up ("sudo mode") gate on
//! sensitive operations.

use flasher_auth::Auth;
use flasher_server::{AppState, CEREMONY_COOKIE, SESSION_COOKIE, serve};
use flasher_store::Store;
use tokio::net::TcpListener;
use webauthn_authenticator_rs::WebauthnAuthenticator;
use webauthn_authenticator_rs::prelude::Url;
use webauthn_authenticator_rs::softpasskey::SoftPasskey;

type TestResult = Result<(), Box<dyn std::error::Error>>;
type ServerHandle = tokio::task::JoinHandle<std::io::Result<()>>;
type SoftToken = WebauthnAuthenticator<SoftPasskey>;

const ORIGIN: &str = "http://localhost:3000";

fn test_auth() -> Result<Auth, Box<dyn std::error::Error>> {
    Ok(Auth::new("localhost", ORIGIN, "flasher")?)
}

async fn start(state: AppState) -> Result<(String, ServerHandle), Box<dyn std::error::Error>> {
    let dist = std::env::temp_dir().join(format!("flasher-neg-dist-{}", std::process::id()));
    std::fs::create_dir_all(&dist)?;
    std::fs::write(dist.join("index.html"), "<h1>flasher</h1>")?;
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let addr = listener.local_addr()?;
    let server = tokio::spawn(serve(listener, dist, state));
    Ok((format!("http://{addr}"), server))
}

fn session_cookie(token: &str) -> String {
    format!("{SESSION_COOKIE}={token}")
}

/// The `flasher-ceremony=<id>` pair out of a response's `Set-Cookie`
/// header, ready for the `Cookie` request header.
fn ceremony_pair(resp: &reqwest::Response) -> Result<String, Box<dyn std::error::Error>> {
    resp.headers()
        .get_all(reqwest::header::SET_COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .filter_map(|v| v.split(';').next())
        .find(|pair| pair.starts_with(&format!("{CEREMONY_COOKIE}=")))
        .map(str::to_owned)
        .ok_or_else(|| "response has no ceremony cookie".into())
}

/// POST register/start; returns the ceremony cookie pair and the options
/// JSON. `session` is the session token when the call is authenticated.
async fn register_start(
    base: &str,
    session: Option<&str>,
    username: &str,
) -> Result<(String, serde_json::Value), Box<dyn std::error::Error>> {
    let mut request = reqwest::Client::new()
        .post(format!("{base}/api/auth/register/start"))
        .json(&serde_json::json!({"username": username}));
    if let Some(session) = session {
        request = request.header(reqwest::header::COOKIE, session_cookie(session));
    }
    let resp = request.send().await?;
    assert_eq!(resp.status(), 200, "register/start must succeed");
    let ceremony = ceremony_pair(&resp)?;
    Ok((ceremony, resp.json().await?))
}

/// POST register/finish with the credential JSON; the caller asserts on
/// the response. `session` is the session token when authenticated. Both
/// cookies ride in ONE `Cookie` header (two headers would let the server
/// see only the first).
async fn register_finish(
    base: &str,
    session: Option<&str>,
    ceremony: &str,
    credential: &serde_json::Value,
) -> Result<reqwest::Response, Box<dyn std::error::Error>> {
    let cookie = match session {
        Some(session) => format!("{ceremony}; {}", session_cookie(session)),
        None => ceremony.to_owned(),
    };
    Ok(reqwest::Client::new()
        .post(format!("{base}/api/auth/register/finish"))
        .header(reqwest::header::COOKIE, cookie)
        .json(credential)
        .send()
        .await?)
}

/// The browser-side half of a registration: runs the options through the
/// software authenticator and returns the credential JSON.
///
/// The soft token cannot create resident keys, so it gets a COPY of the
/// options with residence downgraded (a browser happily creates one; the
/// production options say "required" — same pattern as flasher-auth's
/// tests).
fn do_registration(
    token: &mut SoftToken,
    options: &serde_json::Value,
) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let mut downgraded = options.clone();
    downgraded["publicKey"]["authenticatorSelection"]["residentKey"] =
        serde_json::Value::from("discouraged");
    downgraded["publicKey"]["authenticatorSelection"]["requireResidentKey"] =
        serde_json::Value::from(false);
    let ccr: flasher_auth::CreationChallengeResponse = serde_json::from_value(downgraded)?;
    let reg = token.do_registration(Url::parse(ORIGIN)?, ccr)?;
    Ok(serde_json::to_value(reg)?)
}

/// The browser-side half of a login: the soft token needs an
/// `allowCredentials` hint to find its credential (a browser's resident
/// key does not), injected into a COPY of the request options — the
/// signature ends up over the same challenge, so server-side verification
/// is unaffected. Returns the assertion JSON.
fn do_authentication(
    token: &mut SoftToken,
    options: &serde_json::Value,
    credential_id: &str,
) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let mut hinted = options.clone();
    hinted["publicKey"]["allowCredentials"] = serde_json::json!([{
        "type": "public-key",
        "id": credential_id,
    }]);
    let rcr: flasher_auth::RequestChallengeResponse = serde_json::from_value(hinted)?;
    let assertion = token.do_authentication(Url::parse(ORIGIN)?, rcr)?;
    Ok(serde_json::to_value(assertion)?)
}

/// POST login/start; returns the ceremony cookie pair and the options.
async fn login_start(
    base: &str,
) -> Result<(String, serde_json::Value), Box<dyn std::error::Error>> {
    let resp = reqwest::Client::new()
        .post(format!("{base}/api/auth/login/start"))
        .send()
        .await?;
    assert_eq!(resp.status(), 200, "login/start must succeed");
    let ceremony = ceremony_pair(&resp)?;
    Ok((ceremony, resp.json().await?))
}

/// POST login/finish with the assertion JSON.
async fn login_finish(
    base: &str,
    ceremony: &str,
    assertion: &serde_json::Value,
) -> Result<reqwest::Response, Box<dyn std::error::Error>> {
    Ok(reqwest::Client::new()
        .post(format!("{base}/api/auth/login/finish"))
        .header(reqwest::header::COOKIE, ceremony)
        .json(assertion)
        .send()
        .await?)
}

/// The base64url user handle the server derives for a database user id
/// (the `response.userHandle` of a login assertion).
fn user_handle_b64(user_id: i64) -> String {
    flasher_auth::base64url_string(Auth::user_handle_for(user_id).as_bytes())
}

/// Registers a passkey for `username` through the full HTTP ceremony
/// (open bootstrap). Returns the soft token holding the credential, its
/// (base64url) credential id and the finish response's status.
async fn bootstrap_register(
    base: &str,
    username: &str,
) -> Result<(SoftToken, String), Box<dyn std::error::Error>> {
    let mut token = WebauthnAuthenticator::new(SoftPasskey::new(true));
    let (ceremony, options) = register_start(base, None, username).await?;
    let credential = do_registration(&mut token, &options)?;
    let credential_id = credential["rawId"]
        .as_str()
        .ok_or("credential has no rawId")?
        .to_owned();
    let resp = register_finish(base, None, &ceremony, &credential).await?;
    assert_eq!(resp.status(), 201, "bootstrap register must succeed");
    // F7: a consumed ceremony clears its cookie.
    let cleared = resp
        .headers()
        .get_all(reqwest::header::SET_COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .any(|v| v.starts_with(&format!("{CEREMONY_COOKIE}=")) && v.contains("Max-Age=0"));
    assert!(cleared, "register/finish must clear the ceremony cookie");
    Ok((token, credential_id))
}

/// (a) register/finish with a session belonging to a DIFFERENT user than
/// the ceremony was started for is rejected.
#[tokio::test]
async fn register_finish_rejects_a_ceremony_bound_to_another_user() -> TestResult {
    let store = Store::connect_in_memory().await?;
    let alice = store.create_user("alice").await?;
    let bob = store.create_user("bob").await?;
    store
        .create_session("sess-alice", alice.id, i64::MAX, i64::MAX)
        .await?;
    store
        .create_session("sess-bob", bob.id, i64::MAX, i64::MAX)
        .await?;
    let (base, server) = start(AppState::new(store, test_auth()?)).await?;

    // Alice starts the ceremony (bound to her user handle)...
    let mut token = WebauthnAuthenticator::new(SoftPasskey::new(true));
    let (ceremony, options) = register_start(&base, Some("sess-alice"), "alice").await?;
    let credential = do_registration(&mut token, &options)?;
    // ...but Bob's session finishes it.
    let resp = register_finish(&base, Some("sess-bob"), &ceremony, &credential).await?;
    assert_eq!(resp.status(), 400, "cross-user finish must be rejected");

    server.abort();
    Ok(())
}

/// (b) login/finish with an assertion whose userHandle does not match the
/// credential's owner is rejected with 401. Constructed end-to-end: two
/// users, a credential of user A (registered through the full ceremony),
/// and an assertion tampered to carry user B's handle — `userHandle` is
/// an unsigned client-supplied field, so the server MUST catch this
/// before trusting the mapping.
#[tokio::test]
async fn login_finish_rejects_a_foreign_user_handle() -> TestResult {
    let store = Store::connect_in_memory().await?;
    let bob = store.create_user("bob").await?;
    let (base, server) = start(AppState::new(store, test_auth()?)).await?;

    let (mut token, credential_id) = bootstrap_register(&base, "alice").await?;
    let (ceremony, options) = login_start(&base).await?;
    let mut assertion = do_authentication(&mut token, &options, &credential_id)?;
    // Tamper: the assertion now claims to be Bob.
    assertion["response"]["userHandle"] = serde_json::Value::from(user_handle_b64(bob.id));
    let resp = login_finish(&base, &ceremony, &assertion).await?;
    assert_eq!(resp.status(), 401, "handle/owner mismatch must be a 401");

    server.abort();
    Ok(())
}

/// Happy-path login through the full ceremony: proves the soft-token
/// plumbing works (so the negative results above are meaningful) and pins
/// the F7 behavior — a consumed ceremony clears its cookie while the
/// session cookie is set.
#[tokio::test]
async fn login_finish_sets_the_session_and_clears_the_ceremony_cookie() -> TestResult {
    let store = Store::connect_in_memory().await?;
    let (base, server) = start(AppState::new(store.clone(), test_auth()?)).await?;

    let (mut token, credential_id) = bootstrap_register(&base, "alice").await?;
    let alice = store
        .get_user_by_name("alice")
        .await?
        .ok_or("alice must exist")?;
    let (ceremony, options) = login_start(&base).await?;
    let mut assertion = do_authentication(&mut token, &options, &credential_id)?;
    assertion["response"]["userHandle"] = serde_json::Value::from(user_handle_b64(alice.id));
    let resp = login_finish(&base, &ceremony, &assertion).await?;
    assert_eq!(resp.status(), 200, "valid login must succeed");
    let cookies: Vec<String> = resp
        .headers()
        .get_all(reqwest::header::SET_COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok().map(str::to_owned))
        .collect();
    assert!(
        cookies
            .iter()
            .any(|v| v.starts_with(&format!("{SESSION_COOKIE}=")) && !v.contains("Max-Age=0")),
        "login must set the session cookie, got {cookies:?}"
    );
    assert!(
        cookies
            .iter()
            .any(|v| v.starts_with(&format!("{CEREMONY_COOKIE}=")) && v.contains("Max-Age=0")),
        "login/finish must clear the ceremony cookie, got {cookies:?}"
    );

    // The created session is valid right away: its expiry was stamped in
    // the FUTURE (now + SESSION_TTL), not the past.
    let token = cookies
        .iter()
        .filter_map(|v| v.strip_prefix(&format!("{SESSION_COOKIE}=")))
        .filter_map(|v| v.split(';').next())
        .find(|v| !v.is_empty())
        .ok_or("no session cookie in the login response")?;
    let session: serde_json::Value = reqwest::Client::new()
        .get(format!("{base}/api/auth/session"))
        .header(reqwest::header::COOKIE, session_cookie(token))
        .send()
        .await?
        .json()
        .await?;
    assert_eq!(
        session,
        serde_json::json!({"username": "alice"}),
        "the session created by login must be valid"
    );

    server.abort();
    Ok(())
}

/// A ceremony cookie from a REGISTRATION ceremony used on login/finish
/// is a kind mismatch — a client error (400, the ceremony is consumed),
/// never an authentication failure (401).
#[tokio::test]
async fn login_finish_with_a_registration_ceremony_is_a_400() -> TestResult {
    let store = Store::connect_in_memory().await?;
    let (base, server) = start(AppState::new(store.clone(), test_auth()?)).await?;

    let (mut token, credential_id) = bootstrap_register(&base, "alice").await?;
    let alice = store
        .get_user_by_name("alice")
        .await?
        .ok_or("alice must exist")?;
    store
        .create_session("sess-alice", alice.id, i64::MAX, i64::MAX)
        .await?;
    // A REGISTRATION ceremony (the wrong kind for login/finish).
    let (reg_ceremony, _options) = register_start(&base, Some("sess-alice"), "alice").await?;
    // A valid assertion (signed over a login ceremony's challenge — the
    // signature is never verified: the kind check fires first).
    let (_login_ceremony, options) = login_start(&base).await?;
    let mut assertion = do_authentication(&mut token, &options, &credential_id)?;
    assertion["response"]["userHandle"] = serde_json::Value::from(user_handle_b64(alice.id));
    let resp = login_finish(&base, &reg_ceremony, &assertion).await?;
    assert_eq!(resp.status(), 400, "a wrong-kind ceremony must be a 400");

    server.abort();
    Ok(())
}

/// register/start for a user with existing passkeys must list them in
/// `excludeCredentials` — otherwise the same device could be registered
/// twice and the discoverable-login mapping would go ambiguous.
#[tokio::test]
async fn register_start_excludes_the_users_existing_credentials() -> TestResult {
    let store = Store::connect_in_memory().await?;
    let (base, server) = start(AppState::new(store.clone(), test_auth()?)).await?;

    let (_token, credential_id) = bootstrap_register(&base, "alice").await?;
    let alice = store
        .get_user_by_name("alice")
        .await?
        .ok_or("alice must exist")?;
    store
        .create_session("sess-alice", alice.id, i64::MAX, i64::MAX)
        .await?;

    let (_ceremony, options) = register_start(&base, Some("sess-alice"), "alice").await?;
    let excluded = options["publicKey"]["excludeCredentials"]
        .as_array()
        .ok_or("excludeCredentials must list the existing passkey")?;
    assert_eq!(excluded.len(), 1, "got: {excluded:?}");
    assert_eq!(excluded[0]["id"], serde_json::Value::from(credential_id));
    assert_eq!(excluded[0]["type"], "public-key");

    server.abort();
    Ok(())
}

/// The 409 mapping of register/finish is UNIQUE-violation-specific: a
/// generic database failure on the passkey insert is a logged 500, never
/// a 409. Fault injection: a trigger aborts every passkey INSERT with a
/// non-UNIQUE constraint error (`SQLITE_CONSTRAINT_TRIGGER`).
#[tokio::test]
async fn register_finish_maps_a_generic_store_failure_to_500() -> TestResult {
    let store = Store::connect_in_memory().await?;
    let alice = store.create_user("alice").await?;
    store
        .create_session("sess-alice", alice.id, i64::MAX, i64::MAX)
        .await?;
    let (base, server) = start(AppState::new(store.clone(), test_auth()?)).await?;

    let mut token = WebauthnAuthenticator::new(SoftPasskey::new(true));
    let (ceremony, options) = register_start(&base, Some("sess-alice"), "alice").await?;
    let credential = do_registration(&mut token, &options)?;
    sqlx::query(
        "CREATE TRIGGER sabotage_passkey_insert BEFORE INSERT ON passkeys \
         BEGIN SELECT RAISE(ABORT, 'sabotaged'); END",
    )
    .execute(store.pool())
    .await?;
    let resp = register_finish(&base, Some("sess-alice"), &ceremony, &credential).await?;
    assert_eq!(
        resp.status(),
        500,
        "a generic store failure must be a 500, never a 409"
    );

    server.abort();
    Ok(())
}

/// (c) With `FLASHER_BOOTSTRAP_TOKEN` configured, the open bootstrap
/// registration rejects a missing or wrong token with 403.
#[tokio::test]
async fn bootstrap_token_rejects_missing_and_wrong_token() -> TestResult {
    let store = Store::connect_in_memory().await?;
    let state = AppState::new(store, test_auth()?).with_bootstrap_token(Some("s3cret".to_owned()));
    let (base, server) = start(state).await?;
    let client = reqwest::Client::new();

    for body in [
        serde_json::json!({"username": "alice"}),
        serde_json::json!({"username": "alice", "token": "wrong"}),
        serde_json::json!({"username": "alice", "token": "s3cret-but-longer"}),
    ] {
        let resp = client
            .post(format!("{base}/api/auth/register/start"))
            .json(&body)
            .send()
            .await?;
        assert_eq!(resp.status(), 403, "{body} must be rejected");
        assert_eq!(resp.text().await?, "invalid bootstrap token");
    }

    server.abort();
    Ok(())
}

/// (d) An expired session is a 401, not a user.
#[tokio::test]
async fn expired_session_is_rejected() -> TestResult {
    let store = Store::connect_in_memory().await?;
    let alice = store.create_user("alice").await?;
    store
        .create_session("dead-token", alice.id, 10_000, 10_000)
        .await?;
    let (base, server) = start(AppState::new(store, test_auth()?)).await?;

    let resp = reqwest::Client::new()
        .get(format!("{base}/api/auth/session"))
        .header(reqwest::header::COOKIE, session_cookie("dead-token"))
        .send()
        .await?;
    assert_eq!(resp.status(), 200);
    let session: serde_json::Value = resp.json().await?;
    assert_eq!(
        session,
        serde_json::Value::Null,
        "expired session → 200 null"
    );

    // The same dead token is still rejected where it matters.
    let resp = reqwest::Client::new()
        .get(format!("{base}/api/cards"))
        .header(reqwest::header::COOKIE, session_cookie("dead-token"))
        .send()
        .await?;
    assert_eq!(resp.status(), 401);

    server.abort();
    Ok(())
}

/// (e) Renaming or deleting ANOTHER user's passkey id is a 404 (no
/// existence leak), not a success and not a 403.
#[tokio::test]
async fn rename_and_delete_of_another_users_passkey_are_not_found() -> TestResult {
    let store = Store::connect_in_memory().await?;
    let alice = store.create_user("alice").await?;
    let bob = store.create_user("bob").await?;
    let bob_passkey = store
        .insert_passkey(bob.id, "cred-bob", "Passkey 1", "{}", 1_000)
        .await?;
    store
        .create_session("sess-alice", alice.id, i64::MAX, i64::MAX)
        .await?;
    let (base, server) = start(AppState::new(store, test_auth()?)).await?;
    let client = reqwest::Client::new();

    let resp = client
        .patch(format!("{base}/api/auth/passkeys/{bob_passkey}"))
        .header(reqwest::header::COOKIE, session_cookie("sess-alice"))
        .json(&serde_json::json!({"name": "hijack"}))
        .send()
        .await?;
    assert_eq!(resp.status(), 404, "cross-user rename must be a 404");
    let resp = client
        .delete(format!("{base}/api/auth/passkeys/{bob_passkey}"))
        .header(reqwest::header::COOKIE, session_cookie("sess-alice"))
        .send()
        .await?;
    assert_eq!(resp.status(), 404, "cross-user delete must be a 404");

    server.abort();
    Ok(())
}

/// (f) Deleting the user's last passkey is a 409 at the API level (and
/// the guard holds in the store's atomic DELETE).
#[tokio::test]
async fn last_passkey_delete_is_a_conflict() -> TestResult {
    let store = Store::connect_in_memory().await?;
    let alice = store.create_user("alice").await?;
    let first = store
        .insert_passkey(alice.id, "cred-1", "Passkey 1", "{}", 1_000)
        .await?;
    let second = store
        .insert_passkey(alice.id, "cred-2", "Passkey 2", "{}", 2_000)
        .await?;
    store
        .create_session("sess-alice", alice.id, i64::MAX, i64::MAX)
        .await?;
    let (base, server) = start(AppState::new(store.clone(), test_auth()?)).await?;
    let client = reqwest::Client::new();

    // Two passkeys: deleting one is fine.
    let resp = client
        .delete(format!("{base}/api/auth/passkeys/{first}"))
        .header(reqwest::header::COOKIE, session_cookie("sess-alice"))
        .send()
        .await?;
    assert_eq!(resp.status(), 204);
    // The remaining one is the last: refused, and it survives.
    let resp = client
        .delete(format!("{base}/api/auth/passkeys/{second}"))
        .header(reqwest::header::COOKIE, session_cookie("sess-alice"))
        .send()
        .await?;
    assert_eq!(resp.status(), 409);
    assert_eq!(store.count_passkeys_for_user(alice.id).await?, 1);

    server.abort();
    Ok(())
}

/// Deleting a passkey kills the user's OTHER sessions (lost-device
/// reaction: the lost device's cookie must die with the passkey) but
/// keeps the session performing the deletion.
#[tokio::test]
async fn passkey_delete_revokes_other_sessions_but_keeps_the_current() -> TestResult {
    let store = Store::connect_in_memory().await?;
    let alice = store.create_user("alice").await?;
    let bob = store.create_user("bob").await?;
    let first = store
        .insert_passkey(alice.id, "cred-1", "Passkey 1", "{}", 1_000)
        .await?;
    store
        .insert_passkey(alice.id, "cred-2", "Passkey 2", "{}", 2_000)
        .await?;
    store
        .create_session("sess-current", alice.id, i64::MAX, i64::MAX)
        .await?;
    store
        .create_session("sess-phone", alice.id, i64::MAX, i64::MAX)
        .await?;
    store
        .create_session("sess-bob", bob.id, i64::MAX, i64::MAX)
        .await?;
    let (base, server) = start(AppState::new(store, test_auth()?)).await?;
    let client = reqwest::Client::new();

    let resp = client
        .delete(format!("{base}/api/auth/passkeys/{first}"))
        .header(reqwest::header::COOKIE, session_cookie("sess-current"))
        .send()
        .await?;
    assert_eq!(resp.status(), 204);

    // The lost device's session is dead...
    let resp = client
        .get(format!("{base}/api/cards"))
        .header(reqwest::header::COOKIE, session_cookie("sess-phone"))
        .send()
        .await?;
    assert_eq!(resp.status(), 401, "other session must be revoked");
    // ...the session doing the deletion is not logged out mid-action...
    let resp = client
        .get(format!("{base}/api/cards"))
        .header(reqwest::header::COOKIE, session_cookie("sess-current"))
        .send()
        .await?;
    assert_eq!(resp.status(), 200, "current session must survive");
    // ...and another user's session is untouched.
    let resp = client
        .get(format!("{base}/api/cards"))
        .header(reqwest::header::COOKIE, session_cookie("sess-bob"))
        .send()
        .await?;
    assert_eq!(resp.status(), 200, "other users must be unaffected");

    server.abort();
    Ok(())
}

/// F1b: the no-session register/finish re-checks the bootstrap window
/// immediately before inserting: a ceremony started while the system had
/// zero passkeys is rejected once a passkey exists (another registration
/// landed in between — simulated by inserting a passkey row directly).
#[tokio::test]
async fn bootstrap_finish_is_rejected_once_a_passkey_exists() -> TestResult {
    let store = Store::connect_in_memory().await?;
    let (base, server) = start(AppState::new(store.clone(), test_auth()?)).await?;

    // Open bootstrap: the ceremony starts fine...
    let mut token = WebauthnAuthenticator::new(SoftPasskey::new(true));
    let (ceremony, options) = register_start(&base, None, "mallory").await?;
    let credential = do_registration(&mut token, &options)?;
    // ...but another browser finished first: a passkey now exists.
    let other = store.create_user("other").await?;
    store
        .insert_passkey(other.id, "cred-other", "Passkey 1", "{}", 1_000)
        .await?;
    let resp = register_finish(&base, None, &ceremony, &credential).await?;
    assert_eq!(
        resp.status(),
        401,
        "the closed window must reject the finish"
    );

    server.abort();
    Ok(())
}

/// F4: a credential id that is already registered (UNIQUE violation on
/// insert — e.g. double submit or a credential registered to another
/// user) is a 409, never a 500.
#[tokio::test]
async fn duplicate_credential_finish_is_a_conflict() -> TestResult {
    let store = Store::connect_in_memory().await?;
    let alice = store.create_user("alice").await?;
    // A passkey already exists, so registration needs a session.
    let other = store.create_user("other").await?;
    store
        .insert_passkey(other.id, "cred-other", "Passkey 1", "{}", 1_000)
        .await?;
    store
        .create_session("sess-alice", alice.id, i64::MAX, i64::MAX)
        .await?;
    let (base, server) = start(AppState::new(store.clone(), test_auth()?)).await?;

    let mut token = WebauthnAuthenticator::new(SoftPasskey::new(true));
    let (ceremony, options) = register_start(&base, Some("sess-alice"), "alice").await?;
    let credential = do_registration(&mut token, &options)?;
    let credential_id = credential["rawId"]
        .as_str()
        .ok_or("credential has no rawId")?;
    // The credential id is already taken (registered to another user).
    store
        .insert_passkey(other.id, credential_id, "Passkey 2", "{}", 2_000)
        .await?;
    let resp = register_finish(&base, Some("sess-alice"), &ceremony, &credential).await?;
    assert_eq!(resp.status(), 409, "duplicate credential must be a 409");

    server.abort();
    Ok(())
}

/// F6: with duplicate cookie names the LAST match wins — a cookie planted
/// ahead of the real one (sibling-subdomain cookie) cannot shadow it.
#[tokio::test]
async fn a_planted_first_cookie_does_not_shadow_the_session() -> TestResult {
    let store = Store::connect_in_memory().await?;
    let alice = store.create_user("alice").await?;
    store
        .create_session("real-token", alice.id, i64::MAX, i64::MAX)
        .await?;
    let (base, server) = start(AppState::new(store, test_auth()?)).await?;
    let client = reqwest::Client::new();

    // Planted value first, real one last: the real session wins.
    let resp = client
        .get(format!("{base}/api/auth/session"))
        .header(
            reqwest::header::COOKIE,
            format!("{SESSION_COOKIE}=planted; {SESSION_COOKIE}=real-token"),
        )
        .send()
        .await?;
    assert_eq!(resp.status(), 200, "the last cookie must win");
    let session: serde_json::Value = resp.json().await?;
    assert_eq!(session, serde_json::json!({"username": "alice"}));
    // Sanity: a planted value in the winning (last) position is rejected.
    let resp = client
        .get(format!("{base}/api/auth/session"))
        .header(
            reqwest::header::COOKIE,
            format!("{SESSION_COOKIE}=real-token; {SESSION_COOKIE}=planted"),
        )
        .send()
        .await?;
    assert_eq!(resp.status(), 200);
    let session: serde_json::Value = resp.json().await?;
    assert_eq!(
        session,
        serde_json::Value::Null,
        "a planted value in the winning position is not a session"
    );

    server.abort();
    Ok(())
}

/// F3 (API mapping): the challenge-store cap surfaces as 503, not a 500.
#[tokio::test]
async fn challenge_store_cap_is_a_503() -> TestResult {
    let store = Store::connect_in_memory().await?;
    let (base, server) = start(AppState::new(store, test_auth()?)).await?;
    let client = reqwest::Client::new();

    for i in 0..flasher_auth::MAX_LIVE_CHALLENGES {
        let resp = client
            .post(format!("{base}/api/auth/login/start"))
            .send()
            .await?;
        assert_eq!(resp.status(), 200, "start #{i} must succeed");
    }
    let resp = client
        .post(format!("{base}/api/auth/login/start"))
        .send()
        .await?;
    assert_eq!(resp.status(), 503, "past the cap must be a 503");

    server.abort();
    Ok(())
}

// ------------------------------------------------------------- step-up

/// POST step-up/start with a session; returns the ceremony cookie pair
/// and the request options.
async fn step_up_start(
    base: &str,
    session: &str,
) -> Result<(String, serde_json::Value), Box<dyn std::error::Error>> {
    let resp = reqwest::Client::new()
        .post(format!("{base}/api/auth/step-up/start"))
        .header(reqwest::header::COOKIE, session_cookie(session))
        .send()
        .await?;
    assert_eq!(resp.status(), 200, "step-up/start must succeed");
    let ceremony = ceremony_pair(&resp)?;
    Ok((ceremony, resp.json().await?))
}

/// POST step-up/finish with session + ceremony in ONE `Cookie` header.
async fn step_up_finish(
    base: &str,
    session: &str,
    ceremony: &str,
    assertion: &serde_json::Value,
) -> Result<reqwest::Response, Box<dyn std::error::Error>> {
    Ok(reqwest::Client::new()
        .post(format!("{base}/api/auth/step-up/finish"))
        .header(
            reqwest::header::COOKIE,
            format!("{ceremony}; {}", session_cookie(session)),
        )
        .json(assertion)
        .send()
        .await?)
}

/// Sensitive operations (register/start with a session, passkey delete)
/// answer 403 "step-up required" when the session's `verified_at` stamp is
/// stale — the session cookie alone must not be enough to add or remove
/// passkeys.
#[tokio::test]
async fn sensitive_ops_require_a_recently_verified_session() -> TestResult {
    let store = Store::connect_in_memory().await?;
    let alice = store.create_user("alice").await?;
    let passkey = store
        .insert_passkey(alice.id, "cred-1", "Passkey 1", "{}", 1_000)
        .await?;
    store
        .insert_passkey(alice.id, "cred-2", "Passkey 2", "{}", 2_000)
        .await?;
    // Verified "at the epoch" = long ago = stale.
    store
        .create_session("sess-stale", alice.id, i64::MAX, 0)
        .await?;
    let (base, server) = start(AppState::new(store.clone(), test_auth()?)).await?;
    let client = reqwest::Client::new();

    let resp = client
        .post(format!("{base}/api/auth/register/start"))
        .header(reqwest::header::COOKIE, session_cookie("sess-stale"))
        .json(&serde_json::json!({"username": "alice"}))
        .send()
        .await?;
    assert_eq!(resp.status(), 403, "stale session must be a 403");
    assert_eq!(resp.text().await?, "step-up required");

    let resp = client
        .delete(format!("{base}/api/auth/passkeys/{passkey}"))
        .header(reqwest::header::COOKIE, session_cookie("sess-stale"))
        .send()
        .await?;
    assert_eq!(resp.status(), 403, "stale session must be a 403");
    assert_eq!(resp.text().await?, "step-up required");
    assert_eq!(
        store.count_passkeys_for_user(alice.id).await?,
        2,
        "a refused delete must not remove the passkey"
    );

    server.abort();
    Ok(())
}

/// The step-up ceremony re-stamps the session's `verified_at` and unlocks
/// the sensitive operations; it must NOT mint a new session.
#[tokio::test]
async fn step_up_refreshes_the_session_and_unlocks_sensitive_ops() -> TestResult {
    let store = Store::connect_in_memory().await?;
    let (base, server) = start(AppState::new(store.clone(), test_auth()?)).await?;

    let (mut token, credential_id) = bootstrap_register(&base, "alice").await?;
    let alice = store
        .get_user_by_name("alice")
        .await?
        .ok_or("alice must exist")?;
    // A stale session for alice (as if logged in days ago).
    store
        .create_session("sess-stale", alice.id, i64::MAX, 0)
        .await?;

    // register/start is refused for the stale session...
    let resp = reqwest::Client::new()
        .post(format!("{base}/api/auth/register/start"))
        .header(reqwest::header::COOKIE, session_cookie("sess-stale"))
        .json(&serde_json::json!({"username": "alice"}))
        .send()
        .await?;
    assert_eq!(resp.status(), 403);

    // ...the step-up ceremony runs against the same passkey...
    let (ceremony, options) = step_up_start(&base, "sess-stale").await?;
    let mut assertion = do_authentication(&mut token, &options, &credential_id)?;
    assertion["response"]["userHandle"] = serde_json::Value::from(user_handle_b64(alice.id));
    let resp = step_up_finish(&base, "sess-stale", &ceremony, &assertion).await?;
    assert_eq!(resp.status(), 204, "step-up/finish must succeed");
    let cookies: Vec<String> = resp
        .headers()
        .get_all(reqwest::header::SET_COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok().map(str::to_owned))
        .collect();
    assert!(
        !cookies
            .iter()
            .any(|v| v.starts_with(&format!("{SESSION_COOKIE}="))),
        "step-up must not mint a new session, got {cookies:?}"
    );
    assert!(
        cookies
            .iter()
            .any(|v| v.starts_with(&format!("{CEREMONY_COOKIE}=")) && v.contains("Max-Age=0")),
        "step-up/finish must clear the ceremony cookie, got {cookies:?}"
    );

    // ...and the previously refused operation now works.
    let (_ceremony, _options) = register_start(&base, Some("sess-stale"), "alice").await?;

    server.abort();
    Ok(())
}

/// step-up with ANOTHER user's passkey is a 401: proving possession of
/// someone else's credential proves nothing about this session.
#[tokio::test]
async fn step_up_with_another_users_passkey_is_rejected() -> TestResult {
    let store = Store::connect_in_memory().await?;
    let (base, server) = start(AppState::new(store.clone(), test_auth()?)).await?;

    let (mut token, credential_id) = bootstrap_register(&base, "alice").await?;
    let alice = store
        .get_user_by_name("alice")
        .await?
        .ok_or("alice must exist")?;
    let bob = store.create_user("bob").await?;
    store
        .create_session("sess-bob", bob.id, i64::MAX, 0)
        .await?;

    // Bob's session, alice's passkey in the ceremony.
    let (ceremony, options) = step_up_start(&base, "sess-bob").await?;
    let mut assertion = do_authentication(&mut token, &options, &credential_id)?;
    assertion["response"]["userHandle"] = serde_json::Value::from(user_handle_b64(alice.id));
    let resp = step_up_finish(&base, "sess-bob", &ceremony, &assertion).await?;
    assert_eq!(resp.status(), 401, "another user's passkey must be a 401");
    // Bob's session stays stale.
    assert_eq!(
        store.get_session_verified_at("sess-bob").await?,
        Some(0),
        "a failed step-up must not refresh the stamp"
    );

    server.abort();
    Ok(())
}

/// step-up requires a session at all: without one, start is a 401 (it is
/// a re-authentication, not a login).
#[tokio::test]
async fn step_up_requires_a_session() -> TestResult {
    let store = Store::connect_in_memory().await?;
    let (base, server) = start(AppState::new(store, test_auth()?)).await?;

    let resp = reqwest::Client::new()
        .post(format!("{base}/api/auth/step-up/start"))
        .send()
        .await?;
    assert_eq!(resp.status(), 401, "step-up without a session is a 401");

    server.abort();
    Ok(())
}

/// step-up/finish with an unknown (or expired) ceremony is a 400, not a
/// 401: the ceremony problem is client-correctable, not an authentication
/// failure (same mapping as login/finish).
#[tokio::test]
async fn step_up_finish_with_an_unknown_ceremony_is_a_400() -> TestResult {
    let store = Store::connect_in_memory().await?;
    let (base, server) = start(AppState::new(store.clone(), test_auth()?)).await?;

    let (mut token, credential_id) = bootstrap_register(&base, "alice").await?;
    let alice = store
        .get_user_by_name("alice")
        .await?
        .ok_or("alice must exist")?;
    store
        .create_session("sess-alice", alice.id, i64::MAX, i64::MAX)
        .await?;

    // Sign an assertion over SOME challenge (its verification never runs:
    // the unknown ceremony is rejected first).
    let (_login_ceremony, options) = login_start(&base).await?;
    let mut assertion = do_authentication(&mut token, &options, &credential_id)?;
    assertion["response"]["userHandle"] = serde_json::Value::from(user_handle_b64(alice.id));
    let resp = step_up_finish(&base, "sess-alice", "flasher-ceremony=bogus", &assertion).await?;
    assert_eq!(resp.status(), 400, "an unknown ceremony must be a 400");

    server.abort();
    Ok(())
}
