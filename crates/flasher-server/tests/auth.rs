//! Integration tests for the passkey auth surface: auth-mode 401s,
//! bootstrap gating (open while zero passkeys, optional token), session
//! resolution/logout, and the passkey-management rules. The full
//! `WebAuthn` ceremony crypto round-trip lives in `flasher-auth`'s unit
//! tests; the browser ceremony is covered by the e2e suite.

use flasher_auth::Auth;
use flasher_server::{AppState, CEREMONY_COOKIE, SESSION_COOKIE, serve};
use flasher_store::Store;
use tokio::net::TcpListener;

type TestResult = Result<(), Box<dyn std::error::Error>>;
type ServerHandle = tokio::task::JoinHandle<std::io::Result<()>>;

const ORIGIN: &str = "http://localhost:3000";

fn test_auth() -> Result<Auth, Box<dyn std::error::Error>> {
    Ok(Auth::new("localhost", ORIGIN, "flasher")?)
}

async fn start(state: AppState) -> Result<(String, ServerHandle), Box<dyn std::error::Error>> {
    let dist = std::env::temp_dir().join(format!("flasher-test-dist-{}", std::process::id()));
    std::fs::create_dir_all(&dist)?;
    std::fs::write(dist.join("index.html"), "<h1>flasher</h1>")?;
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let addr = listener.local_addr()?;
    let server = tokio::spawn(serve(listener, dist, state));
    Ok((format!("http://{addr}"), server))
}

/// Auth-mode state (no dev user): sessions decide.
async fn start_auth_mode(
    store: Store,
) -> Result<(String, ServerHandle), Box<dyn std::error::Error>> {
    start(AppState::new(store, test_auth()?)).await
}

fn session_cookie(token: &str) -> String {
    format!("{SESSION_COOKIE}={token}")
}

#[tokio::test]
async fn auth_mode_requires_session_for_api_but_not_for_auth_routes() -> TestResult {
    let store = Store::connect_in_memory().await?;
    let (base, server) = start_auth_mode(store).await?;

    // Everything /api/* outside /api/health and /api/auth/* needs a session.
    for path in ["/api/cards", "/api/cards/next", "/api/autosave"] {
        let resp = reqwest::get(format!("{base}{path}")).await?;
        assert_eq!(resp.status(), 401, "{path} must require a session");
    }
    // Public routes.
    let resp = reqwest::get(format!("{base}/api/health")).await?;
    assert_eq!(resp.status(), 200);
    let bootstrap: serde_json::Value = reqwest::get(format!("{base}/api/auth/bootstrap"))
        .await?
        .json()
        .await?;
    assert_eq!(
        bootstrap,
        serde_json::json!({"registration_open": true, "token_required": false})
    );
    let resp = reqwest::get(format!("{base}/api/auth/session")).await?;
    assert_eq!(resp.status(), 200);
    let session: serde_json::Value = resp.json().await?;
    assert_eq!(session, serde_json::Value::Null, "no session → 200 null");

    server.abort();
    Ok(())
}

#[tokio::test]
async fn valid_session_unlocks_api_expired_does_not() -> TestResult {
    let store = Store::connect_in_memory().await?;
    let user = store.create_user("alice").await?;
    store
        .create_session("good-token", user.id, i64::MAX)
        .await?;
    store.create_session("dead-token", user.id, 10_000).await?;
    let (base, server) = start_auth_mode(store).await?;

    let client = reqwest::Client::new();
    let resp = client
        .get(format!("{base}/api/cards"))
        .header(reqwest::header::COOKIE, session_cookie("good-token"))
        .send()
        .await?;
    assert_eq!(resp.status(), 200);

    let session: serde_json::Value = client
        .get(format!("{base}/api/auth/session"))
        .header(reqwest::header::COOKIE, session_cookie("good-token"))
        .send()
        .await?
        .json()
        .await?;
    assert_eq!(session, serde_json::json!({"username": "alice"}));

    // Expired sessions (checked against the wall clock) are rejected.
    for token in ["dead-token", "never-existed"] {
        let resp = client
            .get(format!("{base}/api/cards"))
            .header(reqwest::header::COOKIE, session_cookie(token))
            .send()
            .await?;
        assert_eq!(resp.status(), 401, "{token} must be rejected");
    }

    server.abort();
    Ok(())
}

#[tokio::test]
async fn logout_deletes_the_session_and_clears_the_cookie() -> TestResult {
    let store = Store::connect_in_memory().await?;
    let user = store.create_user("alice").await?;
    store
        .create_session("good-token", user.id, i64::MAX)
        .await?;
    let (base, server) = start_auth_mode(store).await?;

    let client = reqwest::Client::new();
    let resp = client
        .post(format!("{base}/api/auth/logout"))
        .header(reqwest::header::COOKIE, session_cookie("good-token"))
        .send()
        .await?;
    assert_eq!(resp.status(), 204);
    let set_cookie = resp
        .headers()
        .get(reqwest::header::SET_COOKIE)
        .and_then(|v| v.to_str().ok())
        .map(str::to_owned);
    assert!(
        set_cookie
            .as_deref()
            .is_some_and(|v| v.contains("Max-Age=0")),
        "logout must clear the cookie, got {set_cookie:?}"
    );

    let resp = client
        .get(format!("{base}/api/auth/session"))
        .header(reqwest::header::COOKIE, session_cookie("good-token"))
        .send()
        .await?;
    assert_eq!(resp.status(), 200);
    let session: serde_json::Value = resp.json().await?;
    assert_eq!(
        session,
        serde_json::Value::Null,
        "session must be gone after logout"
    );

    server.abort();
    Ok(())
}

#[tokio::test]
async fn bootstrap_closes_with_the_first_passkey() -> TestResult {
    let store = Store::connect_in_memory().await?;
    let (base, server) = start_auth_mode(store.clone()).await?;
    let client = reqwest::Client::new();

    // Open while zero passkeys: register/start works without a session.
    let resp = client
        .post(format!("{base}/api/auth/register/start"))
        .json(&serde_json::json!({"username": "kakimena"}))
        .send()
        .await?;
    assert_eq!(resp.status(), 200);
    let set_cookie = resp
        .headers()
        .get(reqwest::header::SET_COOKIE)
        .and_then(|v| v.to_str().ok())
        .map(str::to_owned);
    assert!(
        set_cookie
            .as_deref()
            .is_some_and(|v| v.starts_with(&format!("{CEREMONY_COOKIE}="))),
        "register/start must set the ceremony cookie, got {set_cookie:?}"
    );
    let options: serde_json::Value = resp.json().await?;
    assert!(options["publicKey"]["challenge"].is_string());
    assert_eq!(
        options["publicKey"]["authenticatorSelection"]["residentKey"],
        "required"
    );

    // A passkey appears (another browser finished registration): closed.
    let user = store.create_user("other").await?;
    store
        .insert_passkey(user.id, "cred-1", "Passkey 1", "{}", 1_000)
        .await?;
    let bootstrap: serde_json::Value = client
        .get(format!("{base}/api/auth/bootstrap"))
        .send()
        .await?
        .json()
        .await?;
    assert_eq!(
        bootstrap,
        serde_json::json!({"registration_open": false, "token_required": false})
    );
    let resp = client
        .post(format!("{base}/api/auth/register/start"))
        .json(&serde_json::json!({"username": "mallory"}))
        .send()
        .await?;
    assert_eq!(resp.status(), 401);

    server.abort();
    Ok(())
}

#[tokio::test]
async fn bootstrap_token_gates_the_open_registration() -> TestResult {
    let store = Store::connect_in_memory().await?;
    let state = AppState::new(store, test_auth()?).with_bootstrap_token(Some("s3cret".to_owned()));
    let (base, server) = start(state).await?;
    let client = reqwest::Client::new();

    for body in [
        serde_json::json!({"username": "alice"}),
        serde_json::json!({"username": "alice", "token": "wrong"}),
    ] {
        let resp = client
            .post(format!("{base}/api/auth/register/start"))
            .json(&body)
            .send()
            .await?;
        assert_eq!(resp.status(), 403, "{body} must be rejected");
    }
    let resp = client
        .post(format!("{base}/api/auth/register/start"))
        .json(&serde_json::json!({"username": "alice", "token": "s3cret"}))
        .send()
        .await?;
    assert_eq!(resp.status(), 200);

    server.abort();
    Ok(())
}

#[tokio::test]
async fn bootstrap_claims_an_existing_passkey_less_user() -> TestResult {
    let store = Store::connect_in_memory().await?;
    store.create_user("kakimena").await?; // the migrated case
    let (base, server) = start_auth_mode(store.clone()).await?;

    // Untrimmed, differently-cased name still claims the existing user.
    let resp = reqwest::Client::new()
        .post(format!("{base}/api/auth/register/start"))
        .json(&serde_json::json!({"username": "  Kakimena "}))
        .send()
        .await?;
    assert_eq!(resp.status(), 200);
    assert_eq!(
        store.count_users().await?,
        1,
        "claiming must not create a second user"
    );

    // Bad usernames are a 422.
    for username in ["", "   ", &"x".repeat(65)] {
        let resp = reqwest::Client::new()
            .post(format!("{base}/api/auth/register/start"))
            .json(&serde_json::json!({"username": username}))
            .send()
            .await?;
        assert_eq!(resp.status(), 422, "{username:?} must be a 422");
    }

    server.abort();
    Ok(())
}

#[tokio::test]
async fn login_start_returns_discoverable_options_and_a_ceremony_cookie() -> TestResult {
    let store = Store::connect_in_memory().await?;
    let (base, server) = start_auth_mode(store).await?;

    let resp = reqwest::Client::new()
        .post(format!("{base}/api/auth/login/start"))
        .send()
        .await?;
    assert_eq!(resp.status(), 200);
    let set_cookie = resp
        .headers()
        .get(reqwest::header::SET_COOKIE)
        .and_then(|v| v.to_str().ok())
        .map(str::to_owned);
    assert!(
        set_cookie
            .as_deref()
            .is_some_and(|v| v.starts_with(&format!("{CEREMONY_COOKIE}="))),
        "login/start must set the ceremony cookie, got {set_cookie:?}"
    );
    let options: serde_json::Value = resp.json().await?;
    assert_eq!(
        options["publicKey"]["allowCredentials"],
        serde_json::json!([])
    );
    assert_eq!(options["publicKey"]["rpId"], "localhost");
    assert!(options["publicKey"].get("mediation").is_none());

    server.abort();
    Ok(())
}

#[tokio::test]
async fn dev_bypass_answers_session_and_needs_no_cookie() -> TestResult {
    let store = Store::connect_in_memory().await?;
    let user = store.create_user("e2e").await?;
    let state = AppState::dev_bypass(store, test_auth()?, user.id);
    let (base, server) = start(state).await?;

    let resp = reqwest::get(format!("{base}/api/cards")).await?;
    assert_eq!(resp.status(), 200, "dev bypass needs no session");
    let session: serde_json::Value = reqwest::get(format!("{base}/api/auth/session"))
        .await?
        .json()
        .await?;
    assert_eq!(session, serde_json::json!({"username": "e2e"}));

    server.abort();
    Ok(())
}

#[tokio::test]
async fn passkey_management_rename_delete_and_last_passkey_guard() -> TestResult {
    let store = Store::connect_in_memory().await?;
    let user = store.create_user("alice").await?;
    let first = store
        .insert_passkey(user.id, "cred-1", "Passkey 1", "{}", 1_000)
        .await?;
    let state = AppState::dev_bypass(store.clone(), test_auth()?, user.id);
    let (base, server) = start(state).await?;
    let client = reqwest::Client::new();

    // The only passkey cannot be deleted.
    let resp = client
        .delete(format!("{base}/api/auth/passkeys/{first}"))
        .send()
        .await?;
    assert_eq!(resp.status(), 409);

    // Rename works and round-trips through the list endpoint.
    let renamed: serde_json::Value = client
        .patch(format!("{base}/api/auth/passkeys/{first}"))
        .json(&serde_json::json!({"name": "Yubikey"}))
        .send()
        .await?
        .json()
        .await?;
    assert_eq!(renamed["name"], "Yubikey");
    let list: serde_json::Value = client
        .get(format!("{base}/api/auth/passkeys"))
        .send()
        .await?
        .json()
        .await?;
    assert_eq!(list.as_array().map(Vec::len), Some(1));
    assert_eq!(list[0]["name"], "Yubikey");
    assert_eq!(list[0]["last_used_at"], serde_json::Value::Null);

    // Unknown ids are a 404 for both rename and delete.
    let resp = client
        .patch(format!("{base}/api/auth/passkeys/9999"))
        .json(&serde_json::json!({"name": "x"}))
        .send()
        .await?;
    assert_eq!(resp.status(), 404);
    let resp = client
        .delete(format!("{base}/api/auth/passkeys/9999"))
        .send()
        .await?;
    assert_eq!(resp.status(), 404);

    // With a second passkey, deleting the first succeeds; deleting the
    // remaining last one is refused again.
    store
        .insert_passkey(user.id, "cred-2", "Passkey 2", "{}", 2_000)
        .await?;
    let resp = client
        .delete(format!("{base}/api/auth/passkeys/{first}"))
        .send()
        .await?;
    assert_eq!(resp.status(), 204);
    let list: serde_json::Value = client
        .get(format!("{base}/api/auth/passkeys"))
        .send()
        .await?
        .json()
        .await?;
    assert_eq!(list.as_array().map(Vec::len), Some(1));
    assert_eq!(list[0]["name"], "Passkey 2");
    let remaining = list[0]["id"].as_i64().ok_or("id missing")?;
    let resp = client
        .delete(format!("{base}/api/auth/passkeys/{remaining}"))
        .send()
        .await?;
    assert_eq!(resp.status(), 409);

    server.abort();
    Ok(())
}
