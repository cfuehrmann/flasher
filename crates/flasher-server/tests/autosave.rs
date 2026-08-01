//! Thin smoke tests of the autosave API surface (put, get, delete).
//! Behavior is covered end-to-end in `flasher-e2e`, not here.

use flasher_server::{AppState, serve};
use flasher_store::Store;
use flasher_types::{AutoSaveResponse, GetAutoSaveResponse, PutAutoSaveRequest};
use tokio::net::TcpListener;

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

/// Spins up the server on an in-memory store; returns the base URL and
/// the server task.
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

async fn put_draft(base: &str, request: &PutAutoSaveRequest) -> TestResult<AutoSaveResponse> {
    let response = reqwest::Client::new()
        .put(format!("{base}/api/autosave"))
        .json(request)
        .send()
        .await?;
    assert_eq!(response.status(), 200);
    Ok(response.json().await?)
}

async fn get_draft(base: &str) -> TestResult<GetAutoSaveResponse> {
    let response = reqwest::get(format!("{base}/api/autosave")).await?;
    assert_eq!(response.status(), 200);
    Ok(response.json().await?)
}

#[tokio::test]
async fn put_get_delete_roundtrip() -> TestResult {
    let (base, server) = start_test_server().await?;
    let client = reqwest::Client::new();

    // No draft yet: GET is 200 null, DELETE is already 204.
    assert_eq!(get_draft(&base).await?, None);
    let response = client.delete(format!("{base}/api/autosave")).send().await?;
    assert_eq!(response.status(), 204);

    // Draft for a brand-new card (no card_id).
    let draft = put_draft(
        &base,
        &PutAutoSaveRequest {
            card_id: None,
            prompt: "Q?".to_owned(),
            solution: "A.".to_owned(),
        },
    )
    .await?;
    assert_eq!(draft.card_id, None);
    assert_eq!(draft.prompt, "Q?");
    assert_eq!(get_draft(&base).await?, Some(draft.clone()));

    // Upsert: the draft now belongs to an existing card edit session.
    let updated = put_draft(
        &base,
        &PutAutoSaveRequest {
            card_id: Some("card-1".to_owned()),
            prompt: "Q2?".to_owned(),
            solution: "A.".to_owned(),
        },
    )
    .await?;
    assert_eq!(updated.card_id.as_deref(), Some("card-1"));
    assert_eq!(updated.prompt, "Q2?");
    assert!(updated.updated_at >= draft.updated_at);
    assert_eq!(get_draft(&base).await?, Some(updated));

    // DELETE clears it; GET is null again.
    let response = client.delete(format!("{base}/api/autosave")).send().await?;
    assert_eq!(response.status(), 204);
    assert_eq!(get_draft(&base).await?, None);

    server.abort();
    Ok(())
}

#[tokio::test]
async fn unchanged_reput_keeps_updated_at() -> TestResult {
    let (base, server) = start_test_server().await?;
    let request = PutAutoSaveRequest {
        card_id: Some("card-1".to_owned()),
        prompt: "Q?".to_owned(),
        solution: "A.".to_owned(),
    };

    let first = put_draft(&base, &request).await?;
    let second = put_draft(&base, &request).await?;
    assert_eq!(second, first);

    server.abort();
    Ok(())
}

/// The PATCH → draft-deletion side effect (see `patch_card` docs): the
/// draft is deleted only after the card was found and the update applied,
/// and only when the request changes content.
#[tokio::test]
async fn patch_deletes_draft_only_after_successful_content_update() -> TestResult {
    let (base, server) = start_test_server().await?;
    let client = reqwest::Client::new();
    let draft = PutAutoSaveRequest {
        card_id: None,
        prompt: "Draft Q?".to_owned(),
        solution: "Draft A.".to_owned(),
    };

    // PATCH of an unknown id is a 404 and must NOT delete the draft —
    // that is exactly the failure case the recovery net exists for.
    put_draft(&base, &draft).await?;
    let response = client
        .patch(format!("{base}/api/cards/no-such-card"))
        .json(&serde_json::json!({ "prompt": "Q2?" }))
        .send()
        .await?;
    assert_eq!(response.status(), 404);
    assert_eq!(
        get_draft(&base).await?.map(|d| d.prompt),
        Some(draft.prompt.clone())
    );

    // A label-only toggle keeps the draft (like the old
    // Enable/Disable endpoints, which never touched the autosave).
    let card: flasher_types::CardResponse = client
        .post(format!("{base}/api/cards"))
        .json(&serde_json::json!({ "prompt": "Q?", "solution": "A.", "labels": ["A"] }))
        .send()
        .await?
        .json()
        .await?;
    let response = client
        .patch(format!("{base}/api/cards/{}", card.id))
        .json(&serde_json::json!({ "labels": ["A"] }))
        .send()
        .await?;
    assert_eq!(response.status(), 200);
    assert_eq!(
        get_draft(&base).await?.map(|d| d.prompt),
        Some(draft.prompt.clone())
    );

    // A content update of an existing card deletes the draft.
    let response = client
        .patch(format!("{base}/api/cards/{}", card.id))
        .json(&serde_json::json!({ "prompt": "Q2?" }))
        .send()
        .await?;
    assert_eq!(response.status(), 200);
    assert_eq!(get_draft(&base).await?, None);

    server.abort();
    Ok(())
}
