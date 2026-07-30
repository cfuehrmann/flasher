//! Thin smoke tests of the rating API surface (set-ok, set-failed),
//! including the conditional-update 409 on a stale `change_time`
//! (issue #124). Behavior is covered end-to-end in `flasher-e2e`, not
//! here.

use flasher_server::{AppState, serve};
use flasher_store::Store;
use flasher_types::{CardResponse, CardState, SetCardStateRequest};
use tokio::net::TcpListener;

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

struct TestServer {
    base: String,
    server: tokio::task::JoinHandle<std::io::Result<()>>,
}

async fn start_test_server() -> TestResult<TestServer> {
    let dist = std::env::temp_dir().join(format!("flasher-test-dist-{}", std::process::id()));
    std::fs::create_dir_all(&dist)?;
    std::fs::write(dist.join("index.html"), "<h1>flasher</h1>")?;

    let store = Store::connect_in_memory().await?;
    let user = store.upsert_user("test").await?;
    let auth = flasher_auth::Auth::new("localhost", "http://localhost:3000", "flasher")?;
    let state = AppState::dev_bypass(store.clone(), auth, user.id);

    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let addr = listener.local_addr()?;
    let server = tokio::spawn(serve(listener, dist.clone(), state));
    Ok(TestServer {
        base: format!("http://{addr}"),
        server,
    })
}

/// Creates a card via the API and returns it (state `new`, disabled).
async fn create_card(base: &str, prompt: &str, solution: &str) -> TestResult<CardResponse> {
    let card = reqwest::Client::new()
        .post(format!("{base}/api/cards"))
        .json(&serde_json::json!({ "prompt": prompt, "solution": solution }))
        .send()
        .await?
        .json()
        .await?;
    Ok(card)
}

#[tokio::test]
async fn set_ok_and_set_failed_rate_the_card() -> TestResult {
    let TestServer { base, server } = start_test_server().await?;
    let client = reqwest::Client::new();

    let card = create_card(&base, "Q?", "A.").await?;
    let rated: CardResponse = client
        .post(format!("{base}/api/cards/{}/set-ok", card.id))
        .json(&SetCardStateRequest {
            change_time: card.change_time,
        })
        .send()
        .await?
        .json()
        .await?;
    assert_eq!(rated.state, CardState::Ok);
    assert!(rated.change_time > card.change_time);
    assert!(rated.next_time > rated.change_time);

    let failed: CardResponse = client
        .post(format!("{base}/api/cards/{}/set-failed", card.id))
        .json(&SetCardStateRequest {
            change_time: rated.change_time,
        })
        .send()
        .await?
        .json()
        .await?;
    assert_eq!(failed.state, CardState::Failed);

    server.abort();
    Ok(())
}

/// Issue #124: a rating based on a `change_time` the card no longer has
/// (a duplicated/concurrent request) is rejected with 409 and leaves the
/// stored schedule untouched; an unknown id is still a 404.
#[tokio::test]
async fn rating_with_stale_change_time_is_rejected() -> TestResult {
    let TestServer { base, server } = start_test_server().await?;
    let client = reqwest::Client::new();

    let card = create_card(&base, "Q?", "A.").await?;
    let rated: CardResponse = client
        .post(format!("{base}/api/cards/{}/set-ok", card.id))
        .json(&SetCardStateRequest {
            change_time: card.change_time,
        })
        .send()
        .await?
        .json()
        .await?;

    // Replaying the same rating (the double-tap case) now conflicts.
    let resp = client
        .post(format!("{base}/api/cards/{}/set-ok", card.id))
        .json(&SetCardStateRequest {
            change_time: card.change_time,
        })
        .send()
        .await?;
    assert_eq!(resp.status(), 409);

    // The schedule is the one the first rating wrote.
    let current: CardResponse = client
        .get(format!("{base}/api/cards/{}", card.id))
        .send()
        .await?
        .json()
        .await?;
    assert_eq!(current, rated);

    let resp = client
        .post(format!("{base}/api/cards/no-such-id/set-failed"))
        .json(&SetCardStateRequest { change_time: 0 })
        .send()
        .await?;
    assert_eq!(resp.status(), 404);

    server.abort();
    Ok(())
}
