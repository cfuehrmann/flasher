//! Golden tests: importing the synthetic `FileStore` fixture must produce
//! an exact, snapshot-tested database, be idempotent, leave the database
//! untouched in `--dry-run`, and fail on corrupt JSON.

use std::path::Path;
use std::process::Command;

use flasher_migrate::{dry_run, import, import_with_overwrite, render_report};
use flasher_store::{CardState, NewCard, Store};

/// Fixed import time so the snapshots need no redactions.
const NOW: i64 = 1_700_000_000_000;

const FIXTURE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/filestore");

/// A card of fixture user `alice`, in state `New` in the snapshot.
const ALICE_CARD: &str = "3f6a1c2e-9b4d-4e8a-a1c5-7d2f0b9e6a31";

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[derive(serde::Serialize, sqlx::FromRow)]
struct UserRow {
    id: i64,
    username: String,
    created_at: i64,
}

#[derive(serde::Serialize, sqlx::FromRow)]
struct CardRow {
    id: String,
    user_id: i64,
    prompt: String,
    solution: String,
    state: String,
    change_time: i64,
    next_time: i64,
}

#[derive(serde::Serialize, sqlx::FromRow)]
struct LabelRow {
    id: i64,
    user_id: i64,
    name: String,
}

#[derive(serde::Serialize, sqlx::FromRow)]
struct CardLabelRow {
    card_id: String,
    name: String,
}

#[derive(serde::Serialize, sqlx::FromRow)]
struct AutosaveRow {
    user_id: i64,
    card_id: Option<String>,
    prompt: String,
    solution: String,
    updated_at: i64,
}

#[derive(serde::Serialize, sqlx::FromRow)]
struct PasskeyRow {
    id: i64,
    user_id: i64,
    credential_id: String,
    name: String,
    data: String,
    created_at: i64,
    last_used_at: Option<i64>,
}

#[derive(serde::Serialize, sqlx::FromRow)]
struct SessionRow {
    token: String,
    user_id: i64,
    expires_at: i64,
}

/// Dumps every table, deterministically ordered.
async fn dump(store: &Store) -> Result<serde_json::Value, sqlx::Error> {
    let pool = store.pool();
    let users =
        sqlx::query_as::<_, UserRow>("SELECT id, username, created_at FROM users ORDER BY id")
            .fetch_all(pool)
            .await?;
    let cards = sqlx::query_as::<_, CardRow>(
        "SELECT id, user_id, prompt, solution, state, change_time, next_time \
         FROM cards ORDER BY user_id, id",
    )
    .fetch_all(pool)
    .await?;
    let labels = sqlx::query_as::<_, LabelRow>(
        "SELECT id, user_id, name FROM labels ORDER BY user_id, name",
    )
    .fetch_all(pool)
    .await?;
    let card_labels = sqlx::query_as::<_, CardLabelRow>(
        "SELECT cl.card_id, l.name FROM card_labels cl JOIN labels l ON l.id = cl.label_id \
         ORDER BY cl.card_id, l.name",
    )
    .fetch_all(pool)
    .await?;
    let autosaves = sqlx::query_as::<_, AutosaveRow>(
        "SELECT user_id, card_id, prompt, solution, updated_at FROM autosaves ORDER BY user_id",
    )
    .fetch_all(pool)
    .await?;
    let passkeys = sqlx::query_as::<_, PasskeyRow>(
        "SELECT id, user_id, credential_id, name, data, created_at, last_used_at \
         FROM passkeys ORDER BY id",
    )
    .fetch_all(pool)
    .await?;
    let sessions = sqlx::query_as::<_, SessionRow>(
        "SELECT token, user_id, expires_at FROM sessions ORDER BY token",
    )
    .fetch_all(pool)
    .await?;
    Ok(serde_json::json!({
        "users": users,
        "passkeys": passkeys,
        "sessions": sessions,
        "cards": cards,
        "labels": labels,
        "card_labels": card_labels,
        "autosaves": autosaves,
    }))
}

#[tokio::test]
async fn import_produces_expected_database() -> TestResult {
    let dir = tempfile::tempdir()?;
    let store = Store::connect(dir.path().join("flasher.sqlite")).await?;

    let report = import(Path::new(FIXTURE), &store, NOW).await?;
    assert!(report.is_ok());
    insta::assert_snapshot!(render_report(&report));

    insta::assert_json_snapshot!(dump(&store).await?);
    Ok(())
}

#[tokio::test]
async fn import_is_idempotent() -> TestResult {
    let dir = tempfile::tempdir()?;
    let store = Store::connect(dir.path().join("flasher.sqlite")).await?;

    import(Path::new(FIXTURE), &store, NOW).await?;
    let first = dump(&store).await?;

    // Even with a different import time the database must not change:
    // user created_at is kept on conflict, and an unchanged autosave
    // keeps its updated_at.
    import(Path::new(FIXTURE), &store, NOW + 60_000).await?;
    let second = dump(&store).await?;

    assert_eq!(first, second);
    Ok(())
}

#[test]
fn dry_run_reports_without_touching_the_db() -> TestResult {
    let dir = tempfile::tempdir()?;
    let db = dir.path().join("flasher.sqlite");

    // Library level.
    let report = dry_run(Path::new(FIXTURE), NOW)?;
    assert!(report.is_ok());
    assert!(report.dry_run);
    assert_eq!(report.users.len(), 3);

    // CLI level: exit code 0, report on stdout, no database created.
    let output = Command::new(env!("CARGO_BIN_EXE_flasher-migrate"))
        .arg("--from")
        .arg(FIXTURE)
        .arg("--db")
        .arg(&db)
        .arg("--dry-run")
        .output()?;
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout)?;
    assert!(stdout.contains("DRY-RUN: database untouched"));
    assert!(stdout.ends_with("OK\n"));
    assert!(!db.exists());
    Ok(())
}

#[test]
fn corrupt_json_fails_with_nonzero_exit() -> TestResult {
    let dir = tempfile::tempdir()?;
    let from = dir.path().join("filestore");
    let user = from.join("bad");
    std::fs::create_dir_all(&user)?;
    std::fs::write(
        user.join("profile.json"),
        "{\n  \"UserName\": \"bad\",\n  \"PasswordHash\": \"x\"\n}\n",
    )?;
    std::fs::write(user.join("cards.json"), "[ { \"Id\": \"oops\", ")?;
    let db = dir.path().join("flasher.sqlite");

    let output = Command::new(env!("CARGO_BIN_EXE_flasher-migrate"))
        .arg("--from")
        .arg(&from)
        .arg("--db")
        .arg(&db)
        .output()?;
    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr)?;
    assert!(stderr.contains("failed to parse"), "stderr was: {stderr}");
    Ok(())
}

#[test]
fn card_without_disabled_field_fails_loudly() -> TestResult {
    let dir = tempfile::tempdir()?;
    let from = dir.path().join("filestore");
    let user = from.join("bad");
    std::fs::create_dir_all(&user)?;
    std::fs::write(user.join("profile.json"), "{\n  \"UserName\": \"bad\"\n}\n")?;
    std::fs::write(
        user.join("cards.json"),
        "[ {\n  \"Id\": \"c1\",\n  \"Prompt\": \"p\",\n  \"Solution\": \"s\",\n  \
         \"State\": \"New\",\n  \"ChangeTime\": \"2024-05-01T08:00:00+02:00\",\n  \
         \"NextTime\": \"2024-05-01T08:05:00+02:00\"\n} ]\n",
    )?;
    let db = dir.path().join("flasher.sqlite");

    let output = Command::new(env!("CARGO_BIN_EXE_flasher-migrate"))
        .arg("--from")
        .arg(&from)
        .arg("--db")
        .arg(&db)
        .output()?;
    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr)?;
    assert!(
        stderr.contains("missing field `Disabled`"),
        "stderr was: {stderr}"
    );
    Ok(())
}

/// Imports the fixture, then simulates SRS progress made in the new app
/// by rating one card. Returns the store, alice's id, and the card as
/// the fixture imported it.
async fn import_then_tamper(
    dir: &Path,
) -> Result<(Store, i64, flasher_store::Card), Box<dyn std::error::Error>> {
    let store = Store::connect(dir.join("flasher.sqlite")).await?;
    import(Path::new(FIXTURE), &store, NOW).await?;
    let alice = store
        .get_user_by_name("alice")
        .await?
        .ok_or("alice missing")?;
    let original = store
        .get_card(alice.id, ALICE_CARD)
        .await?
        .ok_or("card missing")?;
    store
        .set_card_state(alice.id, ALICE_CARD, CardState::Ok, 5, 6)
        .await?;
    Ok((store, alice.id, original))
}

#[tokio::test]
async fn conflict_without_overwrite_aborts_before_writing() -> TestResult {
    let dir = tempfile::tempdir()?;
    let (store, alice_id, _original) = import_then_tamper(dir.path()).await?;

    // The snapshot no longer matches the database: re-importing must
    // refuse (non-zero exit at the CLI) and write nothing.
    let result = import(Path::new(FIXTURE), &store, NOW).await;
    let Err(error) = result else {
        return Err("expected a conflict error, import succeeded".into());
    };
    assert!(
        error.to_string().contains("1 conflicting card"),
        "error was: {error}"
    );

    // The diverging card is untouched.
    let card = store
        .get_card(alice_id, ALICE_CARD)
        .await?
        .ok_or("card missing")?;
    assert_eq!(card.state, CardState::Ok);
    assert_eq!(card.change_time, 5);
    assert_eq!(card.next_time, 6);
    Ok(())
}

#[tokio::test]
async fn conflict_with_overwrite_restores_the_snapshot() -> TestResult {
    let dir = tempfile::tempdir()?;
    let (store, alice_id, original) = import_then_tamper(dir.path()).await?;

    let report = import_with_overwrite(Path::new(FIXTURE), &store, NOW).await?;
    assert!(report.is_ok());
    let alice = report
        .users
        .iter()
        .find(|u| u.username == "alice")
        .ok_or("alice report missing")?;
    assert_eq!(alice.cards_overwritten, 1);
    let rendered = render_report(&report);
    assert!(
        rendered.contains("1 overwritten from snapshot"),
        "report was: {rendered}"
    );

    // The card is back to the snapshot values.
    let card = store
        .get_card(alice_id, ALICE_CARD)
        .await?
        .ok_or("card missing")?;
    assert_eq!(card, original);
    Ok(())
}

#[tokio::test]
async fn verify_catches_a_tampered_db_state_count() -> TestResult {
    let dir = tempfile::tempdir()?;
    let store = Store::connect(dir.path().join("flasher.sqlite")).await?;
    import(Path::new(FIXTURE), &store, NOW).await?;
    let alice = store
        .get_user_by_name("alice")
        .await?
        .ok_or("alice missing")?;

    // Tamper: an extra `new` card the snapshot does not know about. The
    // re-import itself is conflict-free (no incoming id differs), so the
    // post-import verification is what must flag it.
    store
        .insert_card(&NewCard {
            user_id: alice.id,
            id: "tampered".to_owned(),
            prompt: "p".to_owned(),
            solution: "s".to_owned(),
            state: CardState::New,
            change_time: 1,
            next_time: 2,
            labels: vec!["Enabled".to_owned()],
        })
        .await?;

    let report = import(Path::new(FIXTURE), &store, NOW).await?;
    assert!(!report.is_ok());
    let rendered = render_report(&report);
    assert!(
        rendered.contains("verify: MISMATCH"),
        "report was: {rendered}"
    );
    Ok(())
}
