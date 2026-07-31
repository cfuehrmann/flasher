//! Thin smoke tests of the groom API surface (find, patch, delete,
//! history-reset). Behavior is covered end-to-end in `flasher-e2e`, not
//! here.

use flasher_server::{AppState, DEFAULT_PAGE_SIZE, serve};
use flasher_store::Store;
use flasher_types::{CardResponse, CardState, CardUpdateRequest, FindCardsResponse};
use tokio::net::TcpListener;

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

struct TestServer {
    base: String,
    server: tokio::task::JoinHandle<std::io::Result<()>>,
    store: Store,
    user_id: i64,
}

async fn start_test_server() -> Result<TestServer, Box<dyn std::error::Error>> {
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
        store,
        user_id: user.id,
    })
}

/// Creates a card via the API and returns its id.
async fn create_card(base: &str, prompt: &str, solution: &str) -> TestResult<String> {
    let card: CardResponse = reqwest::Client::new()
        .post(format!("{base}/api/cards"))
        .json(&serde_json::json!({ "prompt": prompt, "solution": solution }))
        .send()
        .await?
        .json()
        .await?;
    Ok(card.id)
}

#[tokio::test]
async fn find_returns_seeded_card_and_count() -> TestResult {
    let TestServer { base, server, .. } = start_test_server().await?;
    // The filter assertions live at API level on purpose (doctrine
    // allows smoke-level only): the frontend always SENDS
    // `disabled_filter`, so the absent-param default is observable only
    // here, and the three wire values are pinned in one request each.
    // A freshly created card starts disabled: the default filter (`all`)
    // finds it, `enabled` hides it.
    let id = create_card(&base, "Capital of France?", "Paris").await?;

    let default_filtered: FindCardsResponse = reqwest::get(format!("{base}/api/cards"))
        .await?
        .json()
        .await?;
    assert_eq!(default_filtered.count, 1);
    let only_enabled: FindCardsResponse =
        reqwest::get(format!("{base}/api/cards?disabled_filter=enabled"))
            .await?
            .json()
            .await?;
    assert_eq!(only_enabled.count, 0);

    let found: FindCardsResponse = reqwest::get(format!("{base}/api/cards?disabled_filter=all"))
        .await?
        .json()
        .await?;
    assert_eq!(found.count, 1);
    assert_eq!(found.cards.len(), 1);
    assert_eq!(found.cards[0].id, id);
    // The response echoes the server's configured page size.
    assert_eq!(found.page_size, i64::from(DEFAULT_PAGE_SIZE));

    // `search_text` filters, `skip` pages past the only card.
    let hit: FindCardsResponse = reqwest::get(format!(
        "{base}/api/cards?disabled_filter=all&search_text=france&skip=0"
    ))
    .await?
    .json()
    .await?;
    assert_eq!(hit.count, 1);
    let miss: FindCardsResponse = reqwest::get(format!(
        "{base}/api/cards?disabled_filter=all&search_text=berlin"
    ))
    .await?
    .json()
    .await?;
    assert_eq!(miss.count, 0);
    assert!(miss.cards.is_empty());
    let paged_out: FindCardsResponse =
        reqwest::get(format!("{base}/api/cards?disabled_filter=all&skip=1"))
            .await?
            .json()
            .await?;
    assert_eq!(paged_out.count, 1);
    assert!(paged_out.cards.is_empty());

    server.abort();
    Ok(())
}

#[tokio::test]
async fn patch_toggles_disabled_and_rejects_empty_or_unknown() -> TestResult {
    let TestServer { base, server, .. } = start_test_server().await?;
    let client = reqwest::Client::new();
    let id = create_card(&base, "Q?", "A.").await?;

    let resp = client
        .patch(format!("{base}/api/cards/no-such-id"))
        .json(&CardUpdateRequest {
            prompt: None,
            solution: None,
            disabled: Some(false),
        })
        .send()
        .await?;
    assert_eq!(resp.status(), 404);

    let resp = client
        .patch(format!("{base}/api/cards/{id}"))
        .json(&serde_json::json!({}))
        .send()
        .await?;
    assert_eq!(resp.status(), 422);

    let updated: CardResponse = client
        .patch(format!("{base}/api/cards/{id}"))
        .json(&CardUpdateRequest {
            prompt: None,
            solution: None,
            disabled: Some(false),
        })
        .send()
        .await?
        .json()
        .await?;
    assert!(!updated.disabled);
    assert_eq!(updated.prompt, "Q?");

    server.abort();
    Ok(())
}

#[tokio::test]
async fn delete_and_history_reset_smoke() -> TestResult {
    let TestServer { base, server, .. } = start_test_server().await?;
    let client = reqwest::Client::new();
    let id = create_card(&base, "Q?", "A.").await?;

    // History reset puts the card back to state `new` (and 404s after).
    let reset: CardResponse = client
        .delete(format!("{base}/api/history/{id}"))
        .send()
        .await?
        .json()
        .await?;
    assert_eq!(reset.state, CardState::New);
    let resp = client
        .delete(format!("{base}/api/history/no-such-id"))
        .send()
        .await?;
    assert_eq!(resp.status(), 404);

    // Delete: 204 first, 404 on the second attempt.
    let resp = client
        .delete(format!("{base}/api/cards/{id}"))
        .send()
        .await?;
    assert_eq!(resp.status(), 204);
    let resp = client
        .delete(format!("{base}/api/cards/{id}"))
        .send()
        .await?;
    assert_eq!(resp.status(), 404);

    server.abort();
    Ok(())
}

#[tokio::test]
async fn get_card_by_id_smoke() -> TestResult {
    let TestServer { base, server, .. } = start_test_server().await?;
    let id = create_card(&base, "Capital of France?", "Paris").await?;

    let card: CardResponse = reqwest::get(format!("{base}/api/cards/{id}"))
        .await?
        .json()
        .await?;
    assert_eq!(card.id, id);
    assert_eq!(card.prompt, "Capital of France?");
    assert_eq!(card.solution, "Paris");
    assert!(card.disabled, "a freshly created card starts disabled");

    // Unknown id → 404. `/api/cards/next` must still resolve to the
    // static route, not the `{id}` param.
    let resp = reqwest::get(format!("{base}/api/cards/no-such-id")).await?;
    assert_eq!(resp.status(), 404);
    let resp = reqwest::get(format!("{base}/api/cards/next")).await?;
    assert_eq!(resp.status(), 200);

    server.abort();
    Ok(())
}

/// Ported `CardsHandler.Update` side effect: a PATCH that changes content
/// (prompt and/or solution) deletes the user's autosave; a pure
/// `disabled` toggle keeps it.
#[tokio::test]
async fn patch_with_content_invalidates_autosave() -> TestResult {
    let TestServer {
        base,
        server,
        store,
        user_id,
    } = start_test_server().await?;
    let client = reqwest::Client::new();
    let id = create_card(&base, "Q?", "A.").await?;

    // A pure disabled toggle leaves the autosave alone.
    store
        .put_autosave(user_id, Some(&id), "draft p", "draft s", 1)
        .await?;
    let resp = client
        .patch(format!("{base}/api/cards/{id}"))
        .json(&CardUpdateRequest {
            prompt: None,
            solution: None,
            disabled: Some(false),
        })
        .send()
        .await?;
    assert_eq!(resp.status(), 200);
    assert!(store.get_autosave(user_id).await?.is_some());

    // A content change (prompt and/or solution) invalidates it.
    store
        .put_autosave(user_id, Some(&id), "draft p", "draft s", 2)
        .await?;
    let resp = client
        .patch(format!("{base}/api/cards/{id}"))
        .json(&CardUpdateRequest {
            prompt: Some("Q2?".to_owned()),
            solution: None,
            disabled: None,
        })
        .send()
        .await?;
    assert_eq!(resp.status(), 200);
    assert_eq!(store.get_autosave(user_id).await?, None);

    server.abort();
    Ok(())
}
