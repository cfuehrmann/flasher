//! Imports an old .NET `FileStore` directory into the `SQLite` store.
//!
//! Old on-disk layout (`{FileStore:Directory}/{userName}/...`):
//!
//! - `profile.json` — `{ "UserName": ..., "PasswordHash": ... }`. The
//!   password hash is ignored (auth becomes passkeys); the user name is
//!   taken from the directory name, which is what the old store used as
//!   the user key.
//! - `cards.json` — array of
//!   `{ "Id", "Prompt", "Solution", "State", "ChangeTime", "NextTime", "Disabled" }`.
//! - `autoSave.json` — optional `{ "Id", "Prompt", "Solution" }`.
//!
//! Serialization details of the old store (`FileStoreJsonContextProvider`):
//! `PascalCase` property names (no naming policy configured), `State` as a
//! `PascalCase` string (`New`/`Ok`/`Failed` via `JsonStringEnumConverter`),
//! and `DateTime` in System.Text.Json's ISO 8601 round-trip form, e.g.
//! `2024-05-01T08:00:00.1234567+02:00` (local time with offset, fractional
//! seconds only when non-zero).

use std::path::{Path, PathBuf};

use flasher_store::{AutoSave, Card, CardState, DISABLED_LABEL, ENABLED_LABEL, Store};
use serde::Deserialize;
use time::format_description::FormatItem;
use time::format_description::well_known::Rfc3339;
use time::macros::format_description;
use time::{OffsetDateTime, PrimitiveDateTime};

/// Fallback format for datetimes without an offset (`DateTimeKind::
/// Unspecified`); such values are interpreted as UTC.
const NO_OFFSET_FORMAT: &[FormatItem<'_>] = format_description!(
    "[year]-[month]-[day]T[hour]:[minute]:[second][optional [.[subsecond digits:1+]]]"
);

/// Errors of the importer.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Reading a file or directory failed.
    #[error("failed to read {path}: {source}")]
    Read {
        path: PathBuf,
        source: std::io::Error,
    },
    /// A JSON file did not match the old format.
    #[error("failed to parse {path}: {source}")]
    Json {
        path: PathBuf,
        source: serde_json::Error,
    },
    /// A datetime string was not in the expected ISO 8601 form.
    #[error("invalid datetime {value:?} in {path}")]
    DateTime { path: PathBuf, value: String },
    /// Re-running the import would overwrite database cards whose fields
    /// differ from the legacy snapshot (typically SRS progress made in
    /// the new app), and `--overwrite` was not given.
    #[error(
        "{count} conflicting card(s): the database no longer matches the old snapshot. \
         Nothing was written; re-run with --overwrite to restore the snapshot, \
         discarding the diverging changes."
    )]
    Conflicts {
        /// Number of conflicting cards.
        count: usize,
    },
    /// A store operation failed.
    #[error(transparent)]
    Store(#[from] flasher_store::Error),
}

/// Per-user import statistics and notes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserReport {
    pub username: String,
    pub cards_imported: usize,
    pub state_new: usize,
    pub state_ok: usize,
    pub state_failed: usize,
    pub disabled: usize,
    /// Cards whose diverging database rows were overwritten from the
    /// snapshot (`--overwrite` only; always 0 otherwise).
    pub cards_overwritten: usize,
    pub autosave: bool,
    pub notes: Vec<String>,
    /// `Some(true)` if the post-import verification matched, `Some(false)`
    /// on mismatch, `None` for a dry run (no database touched).
    pub verified: Option<bool>,
}

/// The result of an import (or dry run).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
    pub users: Vec<UserReport>,
    pub dry_run: bool,
}

impl Report {
    /// Whether everything is fine: no verification mismatched.
    #[must_use]
    pub fn is_ok(&self) -> bool {
        self.users.iter().all(|u| u.verified != Some(false))
    }
}

/// Renders the report in the CLI's text format.
#[must_use]
pub fn render_report(report: &Report) -> String {
    let mut lines: Vec<String> = Vec::new();
    for user in &report.users {
        let mut line = format!(
            "user {:?}: {} cards (new={}, ok={}, failed={}), {} disabled, autosave: {}",
            user.username,
            user.cards_imported,
            user.state_new,
            user.state_ok,
            user.state_failed,
            user.disabled,
            if user.autosave { "yes" } else { "no" },
        );
        if user.cards_overwritten > 0 {
            use std::fmt::Write as _;
            let _ = write!(
                line,
                ", {} overwritten from snapshot",
                user.cards_overwritten
            );
        }
        match user.verified {
            Some(true) => line.push_str(" — verify: OK"),
            Some(false) => line.push_str(" — verify: MISMATCH"),
            None => {}
        }
        lines.push(line);
        for note in &user.notes {
            lines.push(format!("  note: {note}"));
        }
    }
    if report.dry_run {
        lines.push("DRY-RUN: database untouched".to_owned());
    }
    lines.push(if report.is_ok() {
        "OK".to_owned()
    } else {
        "MISMATCH".to_owned()
    });
    let mut out = lines.join("\n");
    out.push('\n');
    out
}

/// A user's parsed legacy data.
#[derive(Debug)]
struct LegacyUser {
    username: String,
    cards: Vec<Card>,
    autosave: Option<AutoSave>,
    notes: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct LegacyProfile {
    #[serde(rename = "UserName")]
    user_name: Option<String>,
    #[serde(rename = "PasswordHash")]
    password_hash: Option<String>,
}

#[derive(Debug, Deserialize)]
struct LegacyCard {
    #[serde(rename = "Id")]
    id: String,
    #[serde(rename = "Prompt")]
    prompt: String,
    #[serde(rename = "Solution")]
    solution: String,
    #[serde(rename = "State")]
    state: LegacyState,
    #[serde(rename = "ChangeTime")]
    change_time: String,
    #[serde(rename = "NextTime")]
    next_time: String,
    // No `default`: a legacy card without `Disabled` must fail loudly
    // instead of silently becoming enabled.
    #[serde(rename = "Disabled")]
    disabled: bool,
}

/// The old `State` enum, serialized as a `PascalCase` string via
/// `JsonStringEnumConverter`.
#[derive(Debug, Clone, Copy, Deserialize)]
enum LegacyState {
    New,
    Ok,
    Failed,
}

impl From<LegacyState> for CardState {
    fn from(state: LegacyState) -> Self {
        match state {
            LegacyState::New => Self::New,
            LegacyState::Ok => Self::Ok,
            LegacyState::Failed => Self::Failed,
        }
    }
}

#[derive(Debug, Deserialize)]
struct LegacyAutoSave {
    // The old `Id` is the id of the card being edited (the autosave
    // belongs to a card edit session), so it maps to `card_id`.
    #[serde(rename = "Id")]
    id: Option<String>,
    #[serde(rename = "Prompt")]
    prompt: Option<String>,
    #[serde(rename = "Solution")]
    solution: Option<String>,
}

/// Parses the whole `FileStore` directory. Each subdirectory is a user.
fn parse_filestore(from: &Path, now: i64) -> Result<Vec<LegacyUser>, Error> {
    let mut entries = std::fs::read_dir(from)
        .map_err(|source| Error::Read {
            path: from.to_path_buf(),
            source,
        })?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|source| Error::Read {
            path: from.to_path_buf(),
            source,
        })?;
    entries.sort_by_key(std::fs::DirEntry::file_name);

    let mut users = Vec::new();
    for entry in entries {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let Some(username) = entry.file_name().to_str().map(str::to_owned) else {
            // Non-UTF-8 directory names cannot be usernames; skip them.
            continue;
        };
        if let Some(user) = parse_user(&path, &username, now)? {
            users.push(user);
        }
    }
    Ok(users)
}

/// Parses one user directory. Returns `None` (with a note logged by the
/// caller's report) if there is no `profile.json` — the old store treats
/// directories as users, but a directory without a profile was never a
/// real account.
fn parse_user(dir: &Path, username: &str, now: i64) -> Result<Option<LegacyUser>, Error> {
    let profile_path = dir.join("profile.json");
    if !profile_path.exists() {
        return Ok(None);
    }
    let mut notes = Vec::new();

    let profile: LegacyProfile = read_json(&profile_path)?;
    if profile.password_hash.is_some() {
        notes.push(format!(
            "ignoring PasswordHash for user {username:?} (auth becomes passkeys)"
        ));
    }
    if let Some(profile_name) = profile.user_name
        && profile_name != username
    {
        notes.push(format!(
            "profile UserName {profile_name:?} differs from directory name; using {username:?}"
        ));
    }

    let cards_path = dir.join("cards.json");
    let mut cards = Vec::new();
    if cards_path.exists() {
        let legacy_cards: Vec<LegacyCard> = read_json(&cards_path)?;
        for legacy in legacy_cards {
            cards.push(Card {
                id: legacy.id,
                prompt: legacy.prompt,
                solution: legacy.solution,
                state: CardState::from(legacy.state),
                change_time: parse_datetime(&cards_path, &legacy.change_time)?,
                next_time: parse_datetime(&cards_path, &legacy.next_time)?,
                // The flag dissolves into its label (owner decision
                // 2026-08-01).
                labels: vec![if legacy.disabled {
                    DISABLED_LABEL.to_owned()
                } else {
                    ENABLED_LABEL.to_owned()
                }],
            });
        }
    } else {
        notes.push("no cards.json; importing zero cards".to_owned());
    }

    let autosave_path = dir.join("autoSave.json");
    let mut autosave = None;
    if autosave_path.exists() {
        let legacy: LegacyAutoSave = read_json(&autosave_path)?;
        let (Some(prompt), Some(solution)) = (legacy.prompt, legacy.solution) else {
            return Err(Error::Json {
                path: autosave_path,
                source: serde::de::Error::missing_field("Prompt/Solution"),
            });
        };
        autosave = Some(AutoSave {
            card_id: legacy.id,
            prompt,
            solution,
            // The old format has no timestamp; use the import time.
            updated_at: now,
        });
    }

    Ok(Some(LegacyUser {
        username: username.to_owned(),
        cards,
        autosave,
        notes,
    }))
}

fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T, Error> {
    let text = std::fs::read_to_string(path).map_err(|source| Error::Read {
        path: path.to_path_buf(),
        source,
    })?;
    serde_json::from_str(&text).map_err(|source| Error::Json {
        path: path.to_path_buf(),
        source,
    })
}

/// Parses a System.Text.Json `DateTime` string into unix epoch millis.
///
/// The old host used `DateTime.Now`, i.e. local time with an offset, in
/// ISO 8601 round-trip form with up to 7 fractional digits (`Rfc3339`
/// accepts 0-9). Values without an offset are assumed to be UTC.
fn parse_datetime(path: &Path, value: &str) -> Result<i64, Error> {
    let parsed = OffsetDateTime::parse(value, &Rfc3339)
        .or_else(|_| {
            PrimitiveDateTime::parse(value, NO_OFFSET_FORMAT).map(PrimitiveDateTime::assume_utc)
        })
        .map_err(|_| Error::DateTime {
            path: path.to_path_buf(),
            value: value.to_owned(),
        })?;
    Ok(parsed.unix_timestamp() * 1_000 + i64::from(parsed.millisecond()))
}

/// Parses the `FileStore` directory and reports what would be imported,
/// without touching any database.
///
/// `now` is only used for the (unsaved) autosave timestamps.
///
/// # Errors
/// Returns an error if a file cannot be read or parsed.
pub fn dry_run(from: &Path, now: i64) -> Result<Report, Error> {
    let users = parse_filestore(from, now)?;
    Ok(Report {
        users: users.iter().map(report_of).collect(),
        dry_run: true,
    })
}

/// Imports the `FileStore` directory into the database, refusing to
/// overwrite diverging database state. Users are upserted by name and
/// cards by id, so re-running the import over an *unchanged* database is
/// a no-op (an unchanged autosave also keeps its `updated_at`).
///
/// Re-running is **not** unconditionally safe: if a database card's
/// fields differ from the legacy snapshot — typically because the user
/// rated cards in the new app — that is a conflict. Without `--overwrite`
/// (this function) the import aborts with [`Error::Conflicts`] before
/// writing anything; use [`import_with_overwrite`] to restore the
/// snapshot, discarding the diverging changes.
///
/// `now` is used for `created_at` of newly created users and for autosave
/// timestamps; pass a fixed value for deterministic output.
///
/// After the import, each user's data is verified against the database
/// (per-state card counts and autosave presence) and the result is
/// recorded in the report.
///
/// # Errors
/// Returns an error if a file cannot be read or parsed, a store
/// operation fails, or conflicting cards are found.
pub async fn import(from: &Path, store: &Store, now: i64) -> Result<Report, Error> {
    import_impl(from, store, now, false).await
}

/// Like [`import`], but conflicting cards are overwritten from the
/// legacy snapshot instead of aborting the import. The number of
/// overwritten cards is reported per user
/// ([`UserReport::cards_overwritten`]).
///
/// # Errors
/// Returns an error if a file cannot be read or parsed or a store
/// operation fails.
pub async fn import_with_overwrite(from: &Path, store: &Store, now: i64) -> Result<Report, Error> {
    import_impl(from, store, now, true).await
}

async fn import_impl(
    from: &Path,
    store: &Store,
    now: i64,
    overwrite: bool,
) -> Result<Report, Error> {
    let users = parse_filestore(from, now)?;

    // Phase 1: conflict detection, strictly before writing anything. A
    // conflict is an incoming card id that already exists for the user
    // with at least one differing field.
    let mut conflicts_per_user = Vec::with_capacity(users.len());
    let mut total_conflicts = 0;
    for user in &users {
        let existing = existing_cards(store, &user.username).await?;
        let conflicts = user
            .cards
            .iter()
            .filter(|card| existing.get(&card.id).is_some_and(|db| db != *card))
            .count();
        total_conflicts += conflicts;
        conflicts_per_user.push(conflicts);
    }
    if total_conflicts > 0 && !overwrite {
        return Err(Error::Conflicts {
            count: total_conflicts,
        });
    }

    // Phase 2: write.
    let mut reports = Vec::new();
    for (user, &conflicts) in users.iter().zip(&conflicts_per_user) {
        let db_user = store.upsert_user_at(&user.username, now).await?;
        for card in &user.cards {
            store.upsert_card(db_user.id, card).await?;
        }
        if let Some(autosave) = &user.autosave {
            store
                .put_autosave(
                    db_user.id,
                    autosave.card_id.as_deref(),
                    &autosave.prompt,
                    &autosave.solution,
                    autosave.updated_at,
                )
                .await?;
        }

        let verified = verify(store, db_user.id, user).await?;
        let mut report = report_of(user);
        report.cards_overwritten = if overwrite { conflicts } else { 0 };
        report.verified = Some(verified);
        reports.push(report);
    }
    Ok(Report {
        users: reports,
        dry_run: false,
    })
}

/// The user's current database cards by id; empty if the user does not
/// exist yet.
async fn existing_cards(
    store: &Store,
    username: &str,
) -> Result<std::collections::HashMap<String, Card>, Error> {
    let Some(user) = store.get_user_by_name(username).await? else {
        return Ok(std::collections::HashMap::new());
    };
    let (cards, _) = store.search_cards(user.id, None, None, 0, u32::MAX).await?;
    Ok(cards
        .into_iter()
        .map(|card| (card.id.clone(), card))
        .collect())
}

fn report_of(user: &LegacyUser) -> UserReport {
    let count = |state: CardState| user.cards.iter().filter(|c| c.state == state).count();
    UserReport {
        username: user.username.clone(),
        cards_imported: user.cards.len(),
        state_new: count(CardState::New),
        state_ok: count(CardState::Ok),
        state_failed: count(CardState::Failed),
        disabled: user
            .cards
            .iter()
            .filter(|c| c.labels.iter().any(|name| name == DISABLED_LABEL))
            .count(),
        cards_overwritten: 0,
        autosave: user.autosave.is_some(),
        notes: user.notes.clone(),
        verified: None,
    }
}

/// Compares the database content for a user against the parsed files:
/// total card count, the per-state histogram, and autosave presence, all
/// read back from the database. This catches lost/duplicated cards, a
/// wrong state mapping and a missing or spurious autosave — but NOT a
/// wrong field-level mapping (prompt/solution/timestamps/disabled swapped
/// or mis-converted would still report OK); that is pinned by the insta
/// golden test instead.
async fn verify(store: &Store, user_id: i64, user: &LegacyUser) -> Result<bool, Error> {
    let (db_cards, _) = store.search_cards(user_id, None, None, 0, u32::MAX).await?;
    if db_cards.len() != user.cards.len() {
        return Ok(false);
    }
    for state in [CardState::New, CardState::Ok, CardState::Failed] {
        let db_count = db_cards.iter().filter(|c| c.state == state).count();
        let source_count = user.cards.iter().filter(|c| c.state == state).count();
        if db_count != source_count {
            return Ok(false);
        }
    }
    let has_autosave = store.get_autosave(user_id).await?.is_some();
    Ok(has_autosave == user.autosave.is_some())
}
