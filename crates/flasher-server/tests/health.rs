//! Integration test: `/api/health` round-trip, SPA static fallback, and
//! a thin smoke check of the cards API (`next` is `null` on an empty
//! database). Behavior of the cards API is covered end-to-end in
//! `flasher-e2e`, not here.

use flasher_server::{AppState, serve};
use flasher_store::Store;
use flasher_types::{HealthResponse, NextCardResponse};
use tokio::net::TcpListener;

type TestResult = Result<(), Box<dyn std::error::Error>>;

async fn start_test_server()
-> Result<(String, tokio::task::JoinHandle<std::io::Result<()>>), Box<dyn std::error::Error>> {
    let dist = std::env::temp_dir().join(format!("flasher-test-dist-{}", std::process::id()));
    std::fs::create_dir_all(&dist)?;
    std::fs::write(dist.join("index.html"), "<h1>flasher</h1>")?;

    let store = Store::connect_in_memory().await?;
    let user = store.upsert_user("test").await?;
    let auth = flasher_auth::Auth::new("localhost", "http://localhost:3000", "flasher")?;
    let state = AppState::dev_bypass(store, auth, user.id);

    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let addr = listener.local_addr()?;
    let server = tokio::spawn(serve(listener, dist.clone(), state));
    Ok((format!("http://{addr}"), server))
}

#[tokio::test]
async fn health_endpoint_returns_ok_json() -> TestResult {
    let (base, server) = start_test_server().await?;

    let health: HealthResponse = reqwest::get(format!("{base}/api/health"))
        .await?
        .json()
        .await?;
    assert_eq!(health.status, "ok");
    assert_eq!(health.version, env!("CARGO_PKG_VERSION"));

    // SPA fallback: unknown paths are served index.html with status 200.
    let resp = reqwest::get(format!("{base}/some/client/route")).await?;
    assert_eq!(resp.status(), 200);
    let body = resp.text().await?;
    assert!(body.contains("flasher"));

    // Unknown /api/* paths must NOT fall through to the SPA: plain 404.
    let resp = reqwest::get(format!("{base}/api/nonsense")).await?;
    assert_eq!(resp.status(), 404);
    let content_type = resp
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .map(str::to_owned);
    assert!(
        content_type
            .as_deref()
            .is_none_or(|v| !v.starts_with("text/html")),
        "api fallback must not serve HTML, got {content_type:?}"
    );

    server.abort();
    Ok(())
}

#[tokio::test]
async fn next_card_is_null_on_empty_database() -> TestResult {
    let (base, server) = start_test_server().await?;

    let resp = reqwest::get(format!("{base}/api/cards/next")).await?;
    assert_eq!(resp.status(), 200);
    let next: NextCardResponse = resp.json().await?;
    assert_eq!(next, None);

    server.abort();
    Ok(())
}
