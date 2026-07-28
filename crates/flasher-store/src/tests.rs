//! Observable-behavior tests for `Store`, run against real `SQLite`
//! (in-memory, plus one tempfile-backed test for `connect`).

use super::{Card, CardState, Error, NewCard, Store};

type TestResult = Result<(), Box<dyn std::error::Error>>;

/// Independent wall clock for tests (deliberately not `now_millis`, so a
/// `now_millis` mutant can't move both the reference and the value under test).
fn wall_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_millis()).unwrap_or(i64::MAX))
}

fn card(id: &str, prompt: &str, solution: &str, state: CardState, change: i64, next: i64) -> Card {
    Card {
        id: id.to_owned(),
        prompt: prompt.to_owned(),
        solution: solution.to_owned(),
        state,
        change_time: change,
        next_time: next,
        disabled: false,
    }
}

fn new_card(user_id: i64, id: &str) -> NewCard {
    let c = card(
        id,
        &format!("prompt {id}"),
        &format!("solution {id}"),
        CardState::New,
        1_000,
        2_000,
    );
    NewCard {
        user_id,
        id: c.id,
        prompt: c.prompt,
        solution: c.solution,
        state: c.state,
        change_time: c.change_time,
        next_time: c.next_time,
        disabled: c.disabled,
    }
}

#[tokio::test]
async fn create_and_get_user() -> TestResult {
    let store = Store::connect_in_memory().await?;

    let user = store.create_user("Alice").await?;
    assert_eq!(user.username, "Alice");
    assert!(user.id > 0);
    assert!(user.created_at > 0);

    // Lookup is case-insensitive (COLLATE NOCASE).
    let fetched = store.get_user_by_name("alice").await?;
    assert_eq!(fetched, Some(user));

    assert_eq!(store.get_user_by_name("bob").await?, None);
    assert_eq!(store.count_users().await?, 1);

    // Duplicate name (even with different case) is rejected.
    assert!(store.create_user("ALICE").await.is_err());
    Ok(())
}

#[tokio::test]
async fn get_user_by_id_finds_existing_and_misses_unknown() -> TestResult {
    let store = Store::connect_in_memory().await?;
    let user = store.create_user("alice").await?;

    let id = user.id;
    assert_eq!(store.get_user_by_id(id).await?, Some(user));
    assert_eq!(store.get_user_by_id(id + 1).await?, None);
    Ok(())
}

#[tokio::test]
async fn upsert_user_is_idempotent() -> TestResult {
    let store = Store::connect_in_memory().await?;

    let first = store.upsert_user_at("bob", 42).await?;
    assert_eq!(first.created_at, 42);

    // Re-upserting keeps the original row, including created_at.
    let second = store.upsert_user_at("bob", 99).await?;
    assert_eq!(second, first);
    assert_eq!(store.count_users().await?, 1);

    // The clock-based variant works too.
    let third = store.upsert_user("carol").await?;
    assert!(third.created_at > 0);
    assert_eq!(store.count_users().await?, 2);
    Ok(())
}

#[tokio::test]
async fn insert_and_get_card() -> TestResult {
    let store = Store::connect_in_memory().await?;
    let user = store.create_user("alice").await?;

    for (id, state) in [
        ("c-new", CardState::New),
        ("c-ok", CardState::Ok),
        ("c-failed", CardState::Failed),
    ] {
        let mut c = new_card(user.id, id);
        c.state = state;
        c.disabled = id == "c-failed";
        store.insert_card(&c).await?;

        let fetched = store.get_card(user.id, id).await?;
        let mut expected = card(
            id,
            &format!("prompt {id}"),
            &format!("solution {id}"),
            state,
            1_000,
            2_000,
        );
        expected.disabled = c.disabled;
        assert_eq!(fetched, Some(expected));
    }

    // Another user does not see the card.
    let other = store.create_user("bob").await?;
    assert_eq!(store.get_card(other.id, "c-new").await?, None);
    assert_eq!(store.get_card(user.id, "nope").await?, None);
    Ok(())
}

#[tokio::test]
async fn upsert_card_replaces_all_fields() -> TestResult {
    let store = Store::connect_in_memory().await?;
    let user = store.create_user("alice").await?;

    store
        .upsert_card(user.id, &card("c1", "old p", "old s", CardState::New, 1, 2))
        .await?;
    let mut updated = card("c1", "new p", "new s", CardState::Ok, 3, 4);
    updated.disabled = true;
    store.upsert_card(user.id, &updated).await?;

    assert_eq!(store.get_card(user.id, "c1").await?, Some(updated));
    Ok(())
}

#[tokio::test]
async fn update_card_fields_is_partial() -> TestResult {
    let store = Store::connect_in_memory().await?;
    let user = store.create_user("alice").await?;
    store.insert_card(&new_card(user.id, "c1")).await?;

    let updated = store
        .update_card_fields(user.id, "c1", Some("changed"), None, None)
        .await?;
    assert_eq!(updated.as_ref().map(|c| c.prompt.as_str()), Some("changed"));
    assert_eq!(
        updated.as_ref().map(|c| c.solution.as_str()),
        Some("solution c1")
    );

    let updated = store
        .update_card_fields(user.id, "c1", None, None, Some(true))
        .await?;
    assert_eq!(updated.as_ref().map(|c| c.disabled), Some(true));

    // Updating nothing still returns the card, unchanged.
    let updated = store
        .update_card_fields(user.id, "c1", None, None, None)
        .await?;
    assert_eq!(updated.as_ref().map(|c| c.prompt.as_str()), Some("changed"));

    // Unknown card / wrong user.
    assert_eq!(
        store
            .update_card_fields(user.id, "nope", Some("x"), None, None)
            .await?,
        None
    );
    let other = store.create_user("bob").await?;
    assert_eq!(
        store
            .update_card_fields(other.id, "c1", Some("x"), None, None)
            .await?,
        None
    );
    Ok(())
}

#[tokio::test]
async fn set_card_state_updates_state_and_times() -> TestResult {
    let store = Store::connect_in_memory().await?;
    let user = store.create_user("alice").await?;
    store.insert_card(&new_card(user.id, "c1")).await?;

    let updated = store
        .set_card_state(user.id, "c1", CardState::Failed, 5_000, 9_000)
        .await?;
    assert_eq!(
        updated,
        Some(card(
            "c1",
            "prompt c1",
            "solution c1",
            CardState::Failed,
            5_000,
            9_000
        ))
    );

    // Unknown card and wrong user both yield None and change nothing.
    assert_eq!(
        store
            .set_card_state(user.id, "nope", CardState::Ok, 0, 0)
            .await?,
        None
    );
    let other = store.create_user("bob").await?;
    assert_eq!(
        store
            .set_card_state(other.id, "c1", CardState::Ok, 0, 0)
            .await?,
        None
    );
    Ok(())
}

#[tokio::test]
async fn delete_card_only_for_owner() -> TestResult {
    let store = Store::connect_in_memory().await?;
    let user = store.create_user("alice").await?;
    let other = store.create_user("bob").await?;
    store.insert_card(&new_card(user.id, "c1")).await?;

    assert!(!store.delete_card(other.id, "c1").await?);
    assert!(store.delete_card(user.id, "c1").await?);
    assert!(!store.delete_card(user.id, "c1").await?);
    assert_eq!(store.get_card(user.id, "c1").await?, None);
    Ok(())
}

#[tokio::test]
async fn search_cards_filters_orders_and_pages() -> TestResult {
    let store = Store::connect_in_memory().await?;
    let user = store.create_user("alice").await?;

    // Ordering reference (old `CardStore.Find`): enabled before
    // disabled, `next_time` ascending within each group.
    let mut c1 = new_card(user.id, "c1");
    c1.prompt = "Rust borrow checker".to_owned();
    c1.solution = "lifetimes".to_owned();
    c1.next_time = 200;
    store.insert_card(&c1).await?;

    let mut c2 = new_card(user.id, "c2");
    c2.prompt = "SQL joins".to_owned();
    c2.solution = "combine tables".to_owned();
    c2.next_time = 100;
    store.insert_card(&c2).await?;

    let mut c3 = new_card(user.id, "c3");
    c3.prompt = "ownership".to_owned();
    c3.solution = "RUST moves values".to_owned();
    c3.next_time = 50;
    c3.disabled = true;
    store.insert_card(&c3).await?;

    let mut c4 = new_card(user.id, "c4");
    c4.prompt = "unrelated".to_owned();
    c4.solution = "nothing".to_owned();
    c4.next_time = 10;
    store.insert_card(&c4).await?;

    // Case-insensitive substring on prompt or solution; c1 (enabled)
    // sorts before c3 (disabled) regardless of next_time.
    let (hits, count) = store.search_cards(user.id, Some("rust"), 0, 10).await?;
    assert_eq!(count, 2);
    assert_eq!(
        hits.iter().map(|c| c.id.as_str()).collect::<Vec<_>>(),
        ["c1", "c3"]
    );

    // Paging: page 1 (skip 0, limit 1) and page 2 (skip 1, limit 1).
    let (page1, count) = store.search_cards(user.id, Some("rust"), 0, 1).await?;
    assert_eq!(count, 2);
    assert_eq!(page1.len(), 1);
    assert_eq!(page1[0].id, "c1");
    let (page2, _) = store.search_cards(user.id, Some("rust"), 1, 1).await?;
    assert_eq!(page2.len(), 1);
    assert_eq!(page2[0].id, "c3");

    // No search matches everything: enabled by next_time asc
    // (c4, c2, c1), then disabled (c3).
    let (all, count) = store.search_cards(user.id, None, 0, 10).await?;
    assert_eq!(count, 4);
    assert_eq!(
        all.iter().map(|c| c.id.as_str()).collect::<Vec<_>>(),
        ["c4", "c2", "c1", "c3"]
    );

    // Empty search behaves like None.
    let (all, count) = store.search_cards(user.id, Some(""), 0, 10).await?;
    assert_eq!(count, 4);
    assert_eq!(all.len(), 4);

    // Old-SQL LIKE metacharacters have no special meaning anymore.
    let (hits, count) = store.search_cards(user.id, Some("100%"), 0, 10).await?;
    assert_eq!(count, 0);
    assert!(hits.is_empty());

    // Other users see nothing.
    let other = store.create_user("bob").await?;
    let (_, count) = store.search_cards(other.id, None, 0, 10).await?;
    assert_eq!(count, 0);
    Ok(())
}

#[tokio::test]
async fn search_cards_folds_full_unicode_case() -> TestResult {
    let store = Store::connect_in_memory().await?;
    let user = store.create_user("alice").await?;

    let mut c1 = new_card(user.id, "c1");
    c1.prompt = "Äpfel und Birnen".to_owned();
    store.insert_card(&c1).await?;

    // SQLite LIKE folds ASCII only and would miss this; the reference
    // semantics (OrdinalIgnoreCase) fold full Unicode.
    let (hits, count) = store.search_cards(user.id, Some("äpfel"), 0, 10).await?;
    assert_eq!(count, 1);
    assert_eq!(hits[0].id, "c1");
    let (_, count) = store.search_cards(user.id, Some("ÄPFEL"), 0, 10).await?;
    assert_eq!(count, 1);
    Ok(())
}

#[tokio::test]
async fn next_card_picks_earliest_due_enabled_card() -> TestResult {
    let store = Store::connect_in_memory().await?;
    let user = store.create_user("alice").await?;

    // due, second-earliest
    let mut due_later = new_card(user.id, "due-later");
    due_later.next_time = 500;
    store.insert_card(&due_later).await?;

    // due, earliest -> winner
    let mut due_first = new_card(user.id, "due-first");
    due_first.next_time = 100;
    store.insert_card(&due_first).await?;

    // due but disabled
    let mut disabled = new_card(user.id, "disabled");
    disabled.next_time = 50;
    disabled.disabled = true;
    store.insert_card(&disabled).await?;

    // enabled but not yet due
    let mut future = new_card(user.id, "future");
    future.next_time = 10_000;
    store.insert_card(&future).await?;

    let next = store.next_card(user.id, 1_000).await?;
    assert_eq!(next.map(|c| c.id), Some("due-first".to_owned()));

    // Before the earliest next_time there is nothing to review.
    assert_eq!(store.next_card(user.id, 99).await?, None);

    // state does not matter, only disabled/next_time.
    store
        .set_card_state(user.id, "due-first", CardState::Ok, 600, 5_000)
        .await?;
    let next = store.next_card(user.id, 1_000).await?;
    assert_eq!(next.map(|c| c.id), Some("due-later".to_owned()));

    // A 'new' card is treated like any other.
    store
        .set_card_state(user.id, "due-later", CardState::New, 600, 700)
        .await?;
    let next = store.next_card(user.id, 1_000).await?;
    assert_eq!(next.map(|c| c.id), Some("due-later".to_owned()));
    Ok(())
}

#[tokio::test]
async fn autosave_roundtrip() -> TestResult {
    let store = Store::connect_in_memory().await?;
    let user = store.create_user("alice").await?;

    assert_eq!(store.get_autosave(user.id).await?, None);
    assert!(!store.delete_autosave(user.id).await?);

    store.put_autosave(user.id, None, "p", "s", 1_000).await?;
    let autosave = store.get_autosave(user.id).await?;
    assert_eq!(
        autosave.as_ref().map(|a| {
            (
                a.card_id.as_deref(),
                a.prompt.as_str(),
                a.solution.as_str(),
                a.updated_at,
            )
        }),
        Some((None, "p", "s", 1_000))
    );

    // Same content again: updated_at is kept (idempotent re-apply).
    store.put_autosave(user.id, None, "p", "s", 2_000).await?;
    let autosave = store.get_autosave(user.id).await?;
    assert_eq!(autosave.as_ref().map(|a| a.updated_at), Some(1_000));

    // Changed content: updated_at is bumped.
    store.put_autosave(user.id, None, "p2", "s", 3_000).await?;
    let autosave = store.get_autosave(user.id).await?;
    assert_eq!(
        autosave.as_ref().map(|a| (a.prompt.as_str(), a.updated_at)),
        Some(("p2", 3_000))
    );

    assert!(store.delete_autosave(user.id).await?);
    assert_eq!(store.get_autosave(user.id).await?, None);
    Ok(())
}

#[tokio::test]
async fn autosave_card_id_roundtrip_and_change_detection() -> TestResult {
    let store = Store::connect_in_memory().await?;
    let user = store.create_user("alice").await?;

    // A draft tied to an existing card (the old AutoSave.Id semantics).
    store
        .put_autosave(user.id, Some("card-1"), "p", "s", 1_000)
        .await?;
    let autosave = store.get_autosave(user.id).await?;
    assert_eq!(
        autosave
            .as_ref()
            .map(|a| (a.card_id.as_deref(), a.updated_at)),
        Some((Some("card-1"), 1_000))
    );

    // Identical re-apply keeps updated_at, card_id included.
    store
        .put_autosave(user.id, Some("card-1"), "p", "s", 2_000)
        .await?;
    let autosave = store.get_autosave(user.id).await?;
    assert_eq!(autosave.as_ref().map(|a| a.updated_at), Some(1_000));

    // Switching only the card is a content change: updated_at bumps.
    store
        .put_autosave(user.id, Some("card-2"), "p", "s", 3_000)
        .await?;
    let autosave = store.get_autosave(user.id).await?;
    assert_eq!(
        autosave
            .as_ref()
            .map(|a| (a.card_id.as_deref(), a.updated_at)),
        Some((Some("card-2"), 3_000))
    );

    // Some -> None is a change as well.
    store.put_autosave(user.id, None, "p", "s", 4_000).await?;
    let autosave = store.get_autosave(user.id).await?;
    assert_eq!(
        autosave
            .as_ref()
            .map(|a| (a.card_id.as_deref(), a.updated_at)),
        Some((None, 4_000))
    );
    Ok(())
}

#[tokio::test]
async fn connect_creates_dirs_and_persists() -> TestResult {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("nested/deeper/flasher.sqlite");

    let user_id;
    {
        let store = Store::connect(&path).await?;
        let user = store.create_user("alice").await?;
        user_id = user.id;
        store.insert_card(&new_card(user.id, "c1")).await?;
    }

    assert!(path.exists());

    // Reopening the file keeps the data (WAL checkpointed on close).
    let store = Store::connect(&path).await?;
    let fetched = store.get_card(user_id, "c1").await?;
    assert_eq!(fetched.map(|c| c.prompt), Some("prompt c1".to_owned()));
    Ok(())
}

#[tokio::test]
async fn reconnect_backs_up_existing_database_before_migrations() -> TestResult {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("flasher.sqlite");

    // First connect creates the database; nothing to back up yet.
    let store = Store::connect(&path).await?;
    store.create_user("alice").await?;
    assert!(!dir.path().join("backups").exists());
    drop(store);

    // Second connect sees a non-empty database and copies it aside.
    let _store = Store::connect(&path).await?;
    let backups = std::fs::read_dir(dir.path().join("backups"))?.collect::<Result<Vec<_>, _>>()?;
    assert_eq!(backups.len(), 1);
    let name = backups[0].file_name();
    let name = name.to_str().ok_or("non-UTF-8 backup name")?;
    assert!(
        name.starts_with("flasher.sqlite-")
            && std::path::Path::new(name)
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("db")),
        "unexpected backup name: {name}"
    );
    assert!(backups[0].metadata()?.len() > 0);
    Ok(())
}

#[tokio::test]
async fn backup_rotation_keeps_only_the_newest_ten() -> TestResult {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("flasher.sqlite");
    let store = Store::connect(&path).await?;
    drop(store);

    // 12 stale backups with chronological names, plus one backup of an
    // unrelated database that rotation must not touch.
    let backups = dir.path().join("backups");
    std::fs::create_dir_all(&backups)?;
    for i in 0..12 {
        std::fs::write(
            backups.join(format!("flasher.sqlite-20200101-0000{i:02}.db")),
            b"old",
        )?;
    }
    std::fs::write(backups.join("other.db-20200101-000000.db"), b"other")?;

    // Reconnecting writes one fresh backup and prunes the oldest of ours.
    let _store = Store::connect(&path).await?;
    let mut names: Vec<String> = std::fs::read_dir(&backups)?
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .filter_map(|entry| entry.file_name().into_string().ok())
        .collect();
    names.sort();
    assert_eq!(names.len(), 11, "10 of ours + the unrelated one");
    assert_eq!(names[0], "flasher.sqlite-20200101-000003.db");
    assert!(names.iter().any(|n| n == "other.db-20200101-000000.db"));
    assert!(
        names
            .iter()
            .any(|n| n.starts_with("flasher.sqlite-2") && !n.starts_with("flasher.sqlite-2020")),
        "the fresh backup survived: {names:?}"
    );
    Ok(())
}

#[tokio::test]
async fn list_users_returns_all_users_in_id_order() -> TestResult {
    let store = Store::connect_in_memory().await?;

    let before = wall_millis();
    let alice = store.create_user("alice").await?;
    let bob = store.upsert_user("bob").await?;
    let after = wall_millis();

    // Clock-based timestamps must land within the call window (pins now_millis).
    for user in [&alice, &bob] {
        assert!(
            (before..=after).contains(&user.created_at),
            "created_at {} outside [{before}, {after}]",
            user.created_at
        );
    }

    // Content AND order are pinned (id order), not just the count.
    assert_eq!(store.list_users().await?, vec![alice, bob]);
    Ok(())
}

// ------------------------------------------------------------- passkeys

/// Inserts a passkey row with minimal ceremony; returns its row id.
async fn insert_test_passkey(
    store: &Store,
    user_id: i64,
    credential_id: &str,
    name: &str,
) -> Result<i64, Box<dyn std::error::Error>> {
    Ok(store
        .insert_passkey(user_id, credential_id, name, "{}", 1_000)
        .await?)
}

#[tokio::test]
async fn passkey_insert_and_lookup_by_credential_id() -> TestResult {
    let store = Store::connect_in_memory().await?;
    let user = store.create_user("alice").await?;

    let id = insert_test_passkey(&store, user.id, "cred-1", "Passkey 1").await?;
    let row = store
        .get_passkey_by_credential_id("cred-1")
        .await?
        .ok_or("passkey not found")?;
    assert_eq!(row.id, id);
    assert_eq!(row.user_id, user.id);
    assert_eq!(row.credential_id, "cred-1");
    assert_eq!(row.name, "Passkey 1");
    assert_eq!(row.data, "{}");
    assert_eq!(row.created_at, 1_000);
    assert_eq!(row.last_used_at, None);

    assert_eq!(store.get_passkey_by_credential_id("unknown").await?, None);
    Ok(())
}

#[tokio::test]
async fn passkey_credential_id_is_globally_unique() -> TestResult {
    let store = Store::connect_in_memory().await?;
    let alice = store.create_user("alice").await?;
    let bob = store.create_user("bob").await?;

    insert_test_passkey(&store, alice.id, "cred-1", "Passkey 1").await?;
    // Same credential id for another user must violate the UNIQUE constraint.
    assert!(
        insert_test_passkey(&store, bob.id, "cred-1", "Passkey 1")
            .await
            .is_err()
    );
    Ok(())
}

#[tokio::test]
async fn passkeys_for_user_are_ordered_and_scoped() -> TestResult {
    let store = Store::connect_in_memory().await?;
    let alice = store.create_user("alice").await?;
    let bob = store.create_user("bob").await?;

    let a1 = insert_test_passkey(&store, alice.id, "cred-a1", "Passkey 1").await?;
    insert_test_passkey(&store, bob.id, "cred-b1", "Passkey 1").await?;
    let a2 = insert_test_passkey(&store, alice.id, "cred-a2", "Passkey 2").await?;

    let rows = store.get_passkeys_for_user(alice.id).await?;
    assert_eq!(rows.iter().map(|r| r.id).collect::<Vec<_>>(), vec![a1, a2]);
    assert_eq!(store.count_passkeys().await?, 3);
    assert_eq!(store.count_passkeys_for_user(alice.id).await?, 2);
    assert_eq!(store.count_passkeys_for_user(bob.id).await?, 1);
    Ok(())
}

#[tokio::test]
async fn rename_and_delete_passkey_are_user_scoped() -> TestResult {
    let store = Store::connect_in_memory().await?;
    let alice = store.create_user("alice").await?;
    let bob = store.create_user("bob").await?;
    let id = insert_test_passkey(&store, alice.id, "cred-1", "Passkey 1").await?;
    // A second passkey: the delete below must not hit the last-passkey
    // guard (covered by its own test).
    insert_test_passkey(&store, alice.id, "cred-2", "Passkey 2").await?;

    // Another user cannot rename or delete it.
    assert!(!store.rename_passkey(bob.id, id, "hijack").await?);
    assert!(!store.delete_passkey(bob.id, id).await?);
    assert_eq!(
        store
            .get_passkey_by_credential_id("cred-1")
            .await?
            .map(|r| r.name)
            .as_deref(),
        Some("Passkey 1")
    );

    // The owner can.
    assert!(store.rename_passkey(alice.id, id, "Yubikey").await?);
    assert_eq!(
        store
            .get_passkey_by_credential_id("cred-1")
            .await?
            .map(|r| r.name)
            .as_deref(),
        Some("Yubikey")
    );
    assert!(store.delete_passkey(alice.id, id).await?);
    assert!(!store.delete_passkey(alice.id, id).await?);
    assert_eq!(store.count_passkeys().await?, 1);
    Ok(())
}

#[tokio::test]
async fn delete_passkey_refuses_the_users_last_passkey() -> TestResult {
    let store = Store::connect_in_memory().await?;
    let alice = store.create_user("alice").await?;
    let first = insert_test_passkey(&store, alice.id, "cred-1", "Passkey 1").await?;
    let second = insert_test_passkey(&store, alice.id, "cred-2", "Passkey 2").await?;

    // Two passkeys: deleting one is fine.
    assert!(store.delete_passkey(alice.id, first).await?);
    // The remaining one is the last: refused, and the row survives.
    assert!(!store.delete_passkey(alice.id, second).await?);
    assert_eq!(store.count_passkeys_for_user(alice.id).await?, 1);
    Ok(())
}

#[tokio::test]
async fn duplicate_credential_id_is_a_unique_violation() -> TestResult {
    let store = Store::connect_in_memory().await?;
    let alice = store.create_user("alice").await?;
    insert_test_passkey(&store, alice.id, "cred-1", "Passkey 1").await?;

    let result = store
        .insert_passkey(alice.id, "cred-1", "Passkey 2", "{}", 2_000)
        .await;
    let Err(err) = result else {
        return Err("duplicate credential id must fail".into());
    };
    assert!(err.is_unique_violation(), "got: {err}");
    Ok(())
}

#[test]
fn non_database_error_is_not_a_unique_violation() {
    // The false arm matters: flasher-server maps is_unique_violation() to
    // 409 vs 500, so a non-UNIQUE failure must not look like a duplicate.
    let err = Error::from(std::io::Error::other("boom"));
    assert!(!err.is_unique_violation(), "got: {err}");
}

#[tokio::test]
async fn update_passkey_after_auth_replaces_blob_and_stamps_use() -> TestResult {
    let store = Store::connect_in_memory().await?;
    let alice = store.create_user("alice").await?;
    let id = insert_test_passkey(&store, alice.id, "cred-1", "Passkey 1").await?;

    assert!(
        store
            .update_passkey_after_auth(alice.id, id, "{\"counter\":2}", 2_000)
            .await?
    );
    let row = store
        .get_passkey_by_credential_id("cred-1")
        .await?
        .ok_or("passkey not found")?;
    assert_eq!(row.data, "{\"counter\":2}");
    assert_eq!(row.last_used_at, Some(2_000));

    // Scoped: wrong user does not touch the row.
    assert!(
        !store
            .update_passkey_after_auth(999, id, "evil", 3_000)
            .await?
    );
    let row = store
        .get_passkey_by_credential_id("cred-1")
        .await?
        .ok_or("passkey not found")?;
    assert_eq!(row.data, "{\"counter\":2}");
    Ok(())
}

// ------------------------------------------------------------- sessions

#[tokio::test]
async fn session_round_trip_and_logout() -> TestResult {
    let store = Store::connect_in_memory().await?;
    let user = store.create_user("alice").await?;

    store.create_session("tok-1", user.id, 10_000, 1_000).await?;
    let session_user = store
        .get_session_user("tok-1", 5_000)
        .await?
        .ok_or("session not found")?;
    assert_eq!(session_user, user);

    // The verified_at stamp: set at creation, re-stamped by touch.
    assert_eq!(store.get_session_verified_at("tok-1").await?, Some(1_000));
    assert!(store.touch_session_verified("tok-1", 6_000).await?);
    assert_eq!(store.get_session_verified_at("tok-1").await?, Some(6_000));
    assert!(!store.touch_session_verified("unknown", 6_000).await?);
    assert_eq!(store.get_session_verified_at("unknown").await?, None);

    assert_eq!(store.get_session_user("unknown", 5_000).await?, None);

    assert!(store.delete_session("tok-1").await?);
    assert!(!store.delete_session("tok-1").await?);
    assert_eq!(store.get_session_user("tok-1", 5_000).await?, None);
    Ok(())
}

#[tokio::test]
async fn expired_session_is_not_returned_and_is_deleted() -> TestResult {
    let store = Store::connect_in_memory().await?;
    let user = store.create_user("alice").await?;

    store.create_session("tok-1", user.id, 10_000, 1_000).await?;
    // At exactly expires_at the session is over.
    assert_eq!(store.get_session_user("tok-1", 10_000).await?, None);
    // The expired row was deleted eagerly: a later lookup at an earlier
    // clock (time travel) must not resurrect it.
    assert_eq!(store.get_session_user("tok-1", 1_000).await?, None);
    Ok(())
}

#[tokio::test]
async fn delete_expired_sessions_keeps_live_ones() -> TestResult {
    let store = Store::connect_in_memory().await?;
    let user = store.create_user("alice").await?;

    store.create_session("old", user.id, 10_000, 1_000).await?;
    store.create_session("live", user.id, 20_000, 1_000).await?;

    assert_eq!(store.delete_expired_sessions(15_000).await?, 1);
    assert_eq!(store.get_session_user("old", 5_000).await?, None);
    assert!(store.get_session_user("live", 5_000).await?.is_some());
    assert_eq!(store.delete_expired_sessions(15_000).await?, 0);
    Ok(())
}

#[tokio::test]
async fn delete_other_sessions_keeps_only_the_current_one() -> TestResult {
    let store = Store::connect_in_memory().await?;
    let alice = store.create_user("alice").await?;
    let bob = store.create_user("bob").await?;

    store.create_session("tok-current", alice.id, 10_000, 1_000).await?;
    store.create_session("tok-phone", alice.id, 10_000, 1_000).await?;
    store.create_session("tok-laptop", alice.id, 10_000, 1_000).await?;
    store.create_session("tok-bob", bob.id, 10_000, 1_000).await?;

    // Only alice's other sessions go; hers survives, bob's is untouched.
    assert_eq!(store.delete_other_sessions(alice.id, "tok-current").await?, 2);
    assert!(
        store
            .get_session_user("tok-current", 5_000)
            .await?
            .is_some()
    );
    assert_eq!(store.get_session_user("tok-phone", 5_000).await?, None);
    assert_eq!(store.get_session_user("tok-laptop", 5_000).await?, None);
    assert!(store.get_session_user("tok-bob", 5_000).await?.is_some());

    // Idempotent: nothing left to delete.
    assert_eq!(store.delete_other_sessions(alice.id, "tok-current").await?, 0);
    Ok(())
}
