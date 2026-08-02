//! Smoke tests for the target-scoped draft routes. Full behavior is driven
//! through the browser e2e suite; these only prove the internal endpoints
//! are mounted and round-trip their contracts.

use flasher_server::{AppState, serve};
use flasher_store::Store;
use flasher_types::{
    CardEditDraftResponse, NewCardDraftResponse, PutCardEditDraftRequest, PutNewCardDraftRequest,
    SaveCardEditRequest,
};
use tokio::net::TcpListener;

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

async fn start_test_server() -> TestResult<(String, tokio::task::JoinHandle<std::io::Result<()>>)> {
    let dist = std::env::temp_dir().join(format!("flasher-test-dist-{}", std::process::id()));
    std::fs::create_dir_all(&dist)?;
    std::fs::write(dist.join("index.html"), "<h1>flasher</h1>")?;

    let store = Store::connect_in_memory().await?;
    let user = store.upsert_user("test").await?;
    let auth = flasher_auth::Auth::new("localhost", "http://localhost:3000", "flasher")?;
    let state = AppState::dev_bypass(store, auth, user.id);

    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let addr = listener.local_addr()?;
    let server = tokio::spawn(serve(listener, dist, state));
    Ok((format!("http://{addr}"), server))
}

#[tokio::test]
async fn new_card_draft_roundtrip() -> TestResult {
    let (base, server) = start_test_server().await?;
    let client = reqwest::Client::new();
    let response = client
        .put(format!("{base}/api/new-card-draft"))
        .json(&PutNewCardDraftRequest {
            prompt: "Q?".to_owned(),
            solution: "A.".to_owned(),
        })
        .send()
        .await?;
    assert_eq!(response.status(), 200);
    let draft: NewCardDraftResponse = response.json().await?;
    assert_eq!(draft.prompt, "Q?");

    let fetched: Option<NewCardDraftResponse> = client
        .get(format!("{base}/api/new-card-draft"))
        .send()
        .await?
        .json()
        .await?;
    assert_eq!(fetched, Some(draft));
    assert_eq!(
        client
            .delete(format!("{base}/api/new-card-draft"))
            .send()
            .await?
            .status(),
        204
    );
    server.abort();
    Ok(())
}

#[tokio::test]
async fn edit_draft_is_committed_by_the_matching_card_route() -> TestResult {
    let (base, server) = start_test_server().await?;
    let client = reqwest::Client::new();
    client
        .post(format!("{base}/api/labels"))
        .json(&serde_json::json!({ "name": "A" }))
        .send()
        .await?;
    let card: flasher_types::CardResponse = client
        .post(format!("{base}/api/cards"))
        .json(&serde_json::json!({ "prompt": "Q?", "solution": "A.", "labels": ["A"] }))
        .send()
        .await?
        .json()
        .await?;

    let draft: CardEditDraftResponse = client
        .put(format!("{base}/api/cards/{}/draft", card.id))
        .json(&PutCardEditDraftRequest {
            base_revision: card.revision,
            prompt: "Q2?".to_owned(),
            solution: "A2.".to_owned(),
            labels: vec!["A".to_owned()],
        })
        .send()
        .await?
        .json()
        .await?;
    assert_eq!(draft.card_id, card.id);

    let saved = client
        .put(format!("{base}/api/cards/{}", card.id))
        .json(&SaveCardEditRequest {
            expected_revision: draft.base_revision,
            prompt: draft.prompt,
            solution: draft.solution,
            labels: draft.labels,
        })
        .send()
        .await?;
    assert_eq!(saved.status(), 200);
    server.abort();
    Ok(())
}
