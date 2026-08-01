//! `SQLite` persistence for Flasher.
//!
//! Uses sqlx with an embedded current-schema baseline and runtime-checked
//! queries (no `query!` macros, no `DATABASE_URL` needed at compile time).
//! All timestamps are unix epoch millis (`i64`).

mod types;

use std::collections::HashMap;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use sqlx::SqlitePool;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};
use time::OffsetDateTime;

pub use types::{AutoSave, Card, CardState, Label, NewCard, PasskeyRow, User};

/// Columns selected for every `Card` read, in `FromRow` order (labels are
/// loaded separately, from `card_labels` joined with `labels`).
const CARD_COLUMNS: &str = "id, prompt, solution, state, change_time, next_time";

/// Columns selected for every `PasskeyRow` read, in `FromRow` order.
const PASSKEY_COLUMNS: &str = "id, user_id, credential_id, name, data, created_at, last_used_at";

/// Errors of the store.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// A database operation failed.
    #[error("database error: {0}")]
    Sqlx(#[from] sqlx::Error),
    /// Running the embedded migrations failed.
    #[error("migration failed: {0}")]
    Migrate(#[from] sqlx::migrate::MigrateError),
    /// Creating the database file's parent directories failed.
    #[error("i/o error: {0}")]
    Io(#[from] std::io::Error),
    /// A database has a migration history older than the supported baseline.
    #[error(
        "database migration history is not current; refusing to squash anything older than migrations 0001-0004"
    )]
    MigrationHistoryNotCurrent,
    /// A label set referenced a label the user does not have (the API
    /// maps this to 422).
    #[error("unknown label: {0}")]
    UnknownLabel(String),
    /// A label set replacement was empty (the API maps this to 422); a
    /// card with no labels would be invisible to every union filter.
    #[error("label set must not be empty")]
    EmptyLabelSet,
}

impl Error {
    /// Whether this is a UNIQUE constraint violation (e.g. a duplicate
    /// passkey `credential_id`), as opposed to a generic database failure.
    /// Lets callers map constraint races to client errors instead of 500.
    #[must_use]
    pub fn is_unique_violation(&self) -> bool {
        match self {
            Self::Sqlx(sqlx::Error::Database(err)) => err.is_unique_violation(),
            _ => false,
        }
    }
}

/// Outcome of [`Store::set_card_state_if_unchanged`].
#[derive(Debug, Clone, PartialEq)]
pub enum SetCardState {
    /// The conditional update matched and was applied.
    Applied(Card),
    /// The card exists, but its stored `change_time` no longer equals
    /// the expected one — a concurrent rating already moved it.
    Stale(Card),
    /// No card with this id exists for the user.
    NotFound,
}

/// A connection pool to the Flasher `SQLite` database.
#[derive(Debug, Clone)]
pub struct Store {
    pool: SqlitePool,
}

#[derive(Debug, sqlx::FromRow)]
struct AppliedMigration {
    version: i64,
    description: String,
    success: bool,
    checksum: Vec<u8>,
}

/// The checksums/descriptions of the only legacy history that may be
/// squashed. They are the `SQLx` checksums of the deleted 0001–0004 files.
const LEGACY_MIGRATION_METADATA: [(i64, &str, &[u8]); 4] = [
    (
        1,
        "initial",
        &[
            0xE3, 0xEC, 0x56, 0x65, 0x2B, 0xCB, 0x90, 0x2F, 0x75, 0xF3, 0xC0, 0xB7, 0xCB, 0x82,
            0x8A, 0x20, 0x7C, 0xAA, 0x37, 0x7D, 0xCE, 0x44, 0x0D, 0x11, 0x94, 0xD4, 0x6A, 0x93,
            0xF4, 0x60, 0x65, 0x06, 0x34, 0x0E, 0x79, 0x9F, 0x38, 0xF2, 0x3F, 0x9D, 0xEE, 0xEA,
            0xBB, 0x82, 0xA3, 0x96, 0xB5, 0x52,
        ],
    ),
    (
        2,
        "autosave card id",
        &[
            0xCB, 0xC1, 0x3B, 0xB5, 0x96, 0x73, 0xDB, 0x17, 0xE9, 0x7E, 0xE6, 0x57, 0xCD, 0xB8,
            0x62, 0x25, 0xC2, 0xFE, 0x1B, 0x48, 0x41, 0x1F, 0x39, 0x91, 0x19, 0x6A, 0x0B, 0x1D,
            0xD6, 0xFC, 0x3A, 0xC6, 0xFA, 0x02, 0xCE, 0xB0, 0xB9, 0xCE, 0x6B, 0x04, 0xE8, 0x07,
            0x7B, 0x4F, 0x59, 0x90, 0x79, 0xD8,
        ],
    ),
    (
        3,
        "sessions verified at",
        &[
            0x01, 0x5B, 0xB9, 0x75, 0x54, 0x81, 0xEC, 0x7A, 0x4D, 0x22, 0x44, 0x85, 0xDC, 0xB2,
            0xF7, 0xEB, 0x77, 0x80, 0x61, 0xC3, 0x8C, 0x5F, 0x3A, 0xEC, 0x4B, 0xEC, 0xFD, 0xEC,
            0x3C, 0xBE, 0x9C, 0xB9, 0x88, 0x39, 0x78, 0x82, 0xF1, 0xA3, 0xD1, 0xD6, 0xA8, 0x56,
            0xD4, 0xCF, 0x6B, 0x70, 0x50, 0x8A,
        ],
    ),
    (
        4,
        "labels",
        &[
            0x66, 0x96, 0x39, 0xE7, 0x1C, 0x74, 0xF3, 0x33, 0x18, 0xFE, 0x8C, 0x1B, 0x12, 0xDF,
            0x68, 0x99, 0x4D, 0x72, 0x90, 0x7B, 0x4A, 0x23, 0x46, 0xD8, 0x06, 0x13, 0xA9, 0x93,
            0x7F, 0x7F, 0x47, 0x2E, 0xED, 0xBE, 0x61, 0xA3, 0x3F, 0x69, 0x3B, 0x55, 0x56, 0x05,
            0xC0, 0xC8, 0xCC, 0xDF, 0x8C, 0xA4,
        ],
    ),
];

impl Store {
    /// Opens (creating if necessary) the database at `path`, creating
    /// missing parent directories, and runs the embedded migrations.
    ///
    /// If the database file already exists and is non-empty, it is first
    /// copied to `<db-parent>/backups/<db-name>-<yyyymmdd-hhmmss>.db`
    /// (UTC) as a safety net in case a migration goes wrong. The copy is
    /// a plain file copy; with an active WAL writer it is not a
    /// point-in-time-consistent snapshot. Only the 10 newest backups of
    /// the database are kept; older ones are deleted.
    ///
    /// # Errors
    /// Returns an error if the directories cannot be created, the backup
    /// copy fails, the database cannot be opened, or a migration fails.
    pub async fn connect(path: impl AsRef<Path>) -> Result<Self, Error> {
        let path = path.as_ref();
        if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
            std::fs::create_dir_all(parent)?;
        }
        backup_before_migration(path)?;
        let options = Self::connect_options().filename(path);
        Self::finish_connect(options).await
    }

    /// Opens an in-memory database and runs the embedded migrations.
    ///
    /// The pool is limited to a single connection so all operations share
    /// the same in-memory database.
    ///
    /// # Errors
    /// Returns an error if the database cannot be opened or a migration
    /// fails.
    pub async fn connect_in_memory() -> Result<Self, Error> {
        let options = Self::connect_options().filename(":memory:");
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await?;
        run_migrations(&pool).await?;
        Ok(Self { pool })
    }

    fn connect_options() -> SqliteConnectOptions {
        SqliteConnectOptions::new()
            .create_if_missing(true)
            .journal_mode(SqliteJournalMode::Wal)
            .foreign_keys(true)
    }

    async fn finish_connect(options: SqliteConnectOptions) -> Result<Self, Error> {
        let pool = SqlitePoolOptions::new().connect_with(options).await?;
        run_migrations(&pool).await?;
        Ok(Self { pool })
    }

    /// The underlying connection pool, e.g. for verification dumps.
    #[must_use]
    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    // ---------------------------------------------------------------- users

    /// Creates a user with `created_at` set to the current time.
    ///
    /// # Errors
    /// Returns an error on database failure, including a uniqueness
    /// violation if the username already exists (case-insensitively).
    pub async fn create_user(&self, username: &str) -> Result<User, Error> {
        self.create_user_at(username, now_millis()).await
    }

    /// Creates a user with an explicit `created_at` timestamp.
    ///
    /// # Errors
    /// Returns an error on database failure, including a uniqueness
    /// violation if the username already exists (case-insensitively).
    pub async fn create_user_at(&self, username: &str, created_at: i64) -> Result<User, Error> {
        let user = sqlx::query_as::<_, User>(
            "INSERT INTO users (username, created_at) VALUES (?, ?) \
             RETURNING id, username, created_at",
        )
        .bind(username)
        .bind(created_at)
        .fetch_one(&self.pool)
        .await?;
        Ok(user)
    }

    /// Returns the user with this (case-insensitive) name, if any.
    ///
    /// # Errors
    /// Returns an error on database failure.
    pub async fn get_user_by_name(&self, username: &str) -> Result<Option<User>, Error> {
        let user = sqlx::query_as::<_, User>(
            "SELECT id, username, created_at FROM users WHERE username = ?",
        )
        .bind(username)
        .fetch_optional(&self.pool)
        .await?;
        Ok(user)
    }

    /// Returns the user with this id, if any.
    ///
    /// # Errors
    /// Returns an error on database failure.
    pub async fn get_user_by_id(&self, id: i64) -> Result<Option<User>, Error> {
        let user =
            sqlx::query_as::<_, User>("SELECT id, username, created_at FROM users WHERE id = ?")
                .bind(id)
                .fetch_optional(&self.pool)
                .await?;
        Ok(user)
    }

    /// Returns the user with this name, creating it if necessary.
    ///
    /// Used by the importer: re-running an import neither duplicates the
    /// user nor overwrites the original `created_at`.
    ///
    /// # Errors
    /// Returns an error on database failure.
    pub async fn upsert_user(&self, username: &str) -> Result<User, Error> {
        self.upsert_user_at(username, now_millis()).await
    }

    /// Like [`Store::upsert_user`], but with an explicit `created_at`
    /// used only when the user is newly inserted. Exists so the importer
    /// can produce deterministic output in tests.
    ///
    /// # Errors
    /// Returns an error on database failure.
    pub async fn upsert_user_at(&self, username: &str, created_at: i64) -> Result<User, Error> {
        let inserted = sqlx::query_as::<_, User>(
            "INSERT INTO users (username, created_at) VALUES (?, ?) \
             ON CONFLICT (username) DO NOTHING \
             RETURNING id, username, created_at",
        )
        .bind(username)
        .bind(created_at)
        .fetch_optional(&self.pool)
        .await?;
        let user = match inserted {
            Some(user) => user,
            // The insert hit the uniqueness conflict: the user already
            // exists. RowNotFound can only occur if the row is deleted
            // concurrently.
            None => self
                .get_user_by_name(username)
                .await?
                .ok_or(Error::Sqlx(sqlx::Error::RowNotFound))?,
        };
        Ok(user)
    }

    /// The number of users.
    ///
    /// # Errors
    /// Returns an error on database failure.
    pub async fn count_users(&self) -> Result<i64, Error> {
        let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM users")
            .fetch_one(&self.pool)
            .await?;
        Ok(count)
    }

    /// All users, ordered by id.
    ///
    /// # Errors
    /// Returns an error on database failure.
    pub async fn list_users(&self) -> Result<Vec<User>, Error> {
        let users =
            sqlx::query_as::<_, User>("SELECT id, username, created_at FROM users ORDER BY id")
                .fetch_all(&self.pool)
                .await?;
        Ok(users)
    }

    // ---------------------------------------------------------------- labels

    /// All labels of the user, ordered by name.
    ///
    /// # Errors
    /// Returns an error on database failure.
    pub async fn labels(&self, user_id: i64) -> Result<Vec<Label>, Error> {
        let labels = sqlx::query_as::<_, Label>(
            "SELECT id, name FROM labels WHERE user_id = ? ORDER BY name",
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(labels)
    }

    /// Returns the id of the user's label `name`, creating it if needed.
    ///
    /// Used by insert/upsert paths and the importer; the API's label-set
    /// replacement deliberately does NOT go through here (unknown labels
    /// are a client error there, not an auto-create).
    ///
    /// # Errors
    /// Returns an error on database failure.
    pub async fn ensure_label(&self, user_id: i64, name: &str) -> Result<i64, Error> {
        let inserted = sqlx::query_scalar::<_, i64>(
            "INSERT INTO labels (user_id, name) VALUES (?, ?) \
             ON CONFLICT (user_id, name) DO NOTHING \
             RETURNING id",
        )
        .bind(user_id)
        .bind(name)
        .fetch_optional(&self.pool)
        .await?;
        if let Some(id) = inserted {
            return Ok(id);
        }
        let id =
            sqlx::query_scalar::<_, i64>("SELECT id FROM labels WHERE user_id = ? AND name = ?")
                .bind(user_id)
                .bind(name)
                .fetch_one(&self.pool)
                .await?;
        Ok(id)
    }

    /// The label names attached to one card, ordered by name.
    async fn labels_of(&self, card_id: &str) -> Result<Vec<String>, Error> {
        let names = sqlx::query_scalar::<_, String>(
            "SELECT l.name FROM card_labels cl JOIN labels l ON l.id = cl.label_id \
             WHERE cl.card_id = ? ORDER BY l.name",
        )
        .bind(card_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(names)
    }

    /// `card_id -> label names` for every labeled card of the user (one
    /// query for the all-in-memory `search_cards`).
    async fn card_labels_map(&self, user_id: i64) -> Result<HashMap<String, Vec<String>>, Error> {
        let rows = sqlx::query_as::<_, (String, String)>(
            "SELECT cl.card_id, l.name FROM card_labels cl \
             JOIN labels l ON l.id = cl.label_id \
             JOIN cards c ON c.id = cl.card_id \
             WHERE c.user_id = ? ORDER BY l.name",
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await?;
        let mut map: HashMap<String, Vec<String>> = HashMap::new();
        for (card_id, name) in rows {
            map.entry(card_id).or_default().push(name);
        }
        Ok(map)
    }

    /// Attaches `card`'s labels (via [`Store::ensure_label`], replacing
    /// any current set). Shared by insert and upsert.
    async fn attach_labels(&self, user_id: i64, card: &NewCard) -> Result<(), Error> {
        sqlx::query("DELETE FROM card_labels WHERE card_id = ?")
            .bind(&card.id)
            .execute(&self.pool)
            .await?;
        for name in &card.labels {
            let label_id = self.ensure_label(user_id, name).await?;
            sqlx::query("INSERT OR IGNORE INTO card_labels (card_id, label_id) VALUES (?, ?)")
                .bind(&card.id)
                .bind(label_id)
                .execute(&self.pool)
                .await?;
        }
        Ok(())
    }

    // ---------------------------------------------------------------- cards

    /// Inserts a new card, attaching its labels.
    ///
    /// # Errors
    /// Returns an error on database failure, including a uniqueness
    /// violation if the card id already exists.
    pub async fn insert_card(&self, card: &NewCard) -> Result<(), Error> {
        sqlx::query(
            "INSERT INTO cards (id, user_id, prompt, solution, state, change_time, next_time) \
             VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&card.id)
        .bind(card.user_id)
        .bind(&card.prompt)
        .bind(&card.solution)
        .bind(card.state)
        .bind(card.change_time)
        .bind(card.next_time)
        .execute(&self.pool)
        .await?;
        self.attach_labels(card.user_id, card).await
    }

    /// Inserts the card, or replaces all of its fields and labels if the
    /// id already exists. Used by the importer for idempotence.
    ///
    /// # Errors
    /// Returns an error on database failure.
    pub async fn upsert_card(&self, user_id: i64, card: &Card) -> Result<(), Error> {
        sqlx::query(
            "INSERT INTO cards (id, user_id, prompt, solution, state, change_time, next_time) \
             VALUES (?, ?, ?, ?, ?, ?, ?) \
             ON CONFLICT (id) DO UPDATE SET \
             user_id = excluded.user_id, \
             prompt = excluded.prompt, \
             solution = excluded.solution, \
             state = excluded.state, \
             change_time = excluded.change_time, \
             next_time = excluded.next_time",
        )
        .bind(&card.id)
        .bind(user_id)
        .bind(&card.prompt)
        .bind(&card.solution)
        .bind(card.state)
        .bind(card.change_time)
        .bind(card.next_time)
        .execute(&self.pool)
        .await?;
        let as_new = NewCard {
            user_id,
            id: card.id.clone(),
            prompt: card.prompt.clone(),
            solution: card.solution.clone(),
            state: card.state,
            change_time: card.change_time,
            next_time: card.next_time,
            labels: card.labels.clone(),
        };
        self.attach_labels(user_id, &as_new).await
    }

    /// Returns the card with this id owned by this user, if any.
    ///
    /// # Errors
    /// Returns an error on database failure.
    pub async fn get_card(&self, user_id: i64, id: &str) -> Result<Option<Card>, Error> {
        let card = sqlx::query_as::<_, Card>(&format!(
            "SELECT {CARD_COLUMNS} FROM cards WHERE user_id = ? AND id = ?"
        ))
        .bind(user_id)
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;
        match card {
            Some(mut card) => {
                card.labels = self.labels_of(&card.id).await?;
                Ok(Some(card))
            }
            None => Ok(None),
        }
    }

    /// Applies a partial update of the content fields; `None` leaves the
    /// field unchanged. Returns the updated card, or `None` if no card
    /// with this id exists for the user. (Labels are replaced separately,
    /// via [`Store::set_card_labels`].)
    ///
    /// # Errors
    /// Returns an error on database failure.
    pub async fn update_card_fields(
        &self,
        user_id: i64,
        id: &str,
        prompt: Option<&str>,
        solution: Option<&str>,
    ) -> Result<Option<Card>, Error> {
        let card = sqlx::query_as::<_, Card>(&format!(
            "UPDATE cards SET \
             prompt = COALESCE(?, prompt), \
             solution = COALESCE(?, solution) \
             WHERE user_id = ? AND id = ? \
             RETURNING {CARD_COLUMNS}"
        ))
        .bind(prompt)
        .bind(solution)
        .bind(user_id)
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;
        match card {
            Some(mut card) => {
                card.labels = self.labels_of(&card.id).await?;
                Ok(Some(card))
            }
            None => Ok(None),
        }
    }

    /// Validates a label set for [`Store::set_card_labels`] and card
    /// creation: the set must not be empty (a label-less card would be
    /// invisible to every union filter) and every name must be an
    /// existing label of the user (no backdoor creation).
    ///
    /// # Errors
    /// [`Error::UnknownLabel`] for a label the user does not have,
    /// [`Error::EmptyLabelSet`] for an empty set, or a database error.
    pub async fn validate_labels(&self, user_id: i64, labels: &[String]) -> Result<(), Error> {
        if labels.is_empty() {
            return Err(Error::EmptyLabelSet);
        }
        let known = self.labels(user_id).await?;
        for name in labels {
            if !known.iter().any(|label| &label.name == name) {
                return Err(Error::UnknownLabel(name.clone()));
            }
        }
        Ok(())
    }

    /// Replaces the card's whole label set. Every name must be an
    /// existing label of the user (no backdoor creation) and the set
    /// must not be empty (a label-less card would be invisible to every
    /// union filter). Returns `None` if no card with this id exists for
    /// the user — checked FIRST, so an unknown card is a clean `None`
    /// regardless of label validity (404 beats 422 at the API).
    ///
    /// # Errors
    /// [`Error::UnknownLabel`] for a label the user does not have,
    /// [`Error::EmptyLabelSet`] for an empty set, or a database error.
    pub async fn set_card_labels(
        &self,
        user_id: i64,
        id: &str,
        labels: &[String],
    ) -> Result<Option<Card>, Error> {
        // The card must exist and belong to the user.
        let Some(_card) = self.get_card(user_id, id).await? else {
            return Ok(None);
        };
        self.validate_labels(user_id, labels).await?;
        let mut label_ids = Vec::with_capacity(labels.len());
        let known = self.labels(user_id).await?;
        for name in labels {
            let label = known
                .iter()
                .find(|label| &label.name == name)
                .ok_or_else(|| Error::UnknownLabel(name.clone()))?;
            label_ids.push(label.id);
        }
        sqlx::query("DELETE FROM card_labels WHERE card_id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;
        for label_id in label_ids {
            sqlx::query("INSERT OR IGNORE INTO card_labels (card_id, label_id) VALUES (?, ?)")
                .bind(id)
                .bind(label_id)
                .execute(&self.pool)
                .await?;
        }
        self.get_card(user_id, id).await
    }

    /// Sets the SRS state and scheduling times of a card owned by this
    /// user. Returns the updated card, or `None` if no card with this id
    /// exists for the user.
    ///
    /// # Errors
    /// Returns an error on database failure.
    pub async fn set_card_state(
        &self,
        user_id: i64,
        id: &str,
        state: CardState,
        change_time: i64,
        next_time: i64,
    ) -> Result<Option<Card>, Error> {
        let card = sqlx::query_as::<_, Card>(&format!(
            "UPDATE cards SET state = ?, change_time = ?, next_time = ? \
             WHERE user_id = ? AND id = ? \
             RETURNING {CARD_COLUMNS}"
        ))
        .bind(state)
        .bind(change_time)
        .bind(next_time)
        .bind(user_id)
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;
        match card {
            Some(mut card) => {
                card.labels = self.labels_of(&card.id).await?;
                Ok(Some(card))
            }
            None => Ok(None),
        }
    }

    /// Conditional variant of [`Store::set_card_state`]: the update is
    /// applied in a single statement only while the stored `change_time`
    /// still equals `expected_change_time` (compare-and-set on the value
    /// the client based its rating on). This closes the double-rating
    /// race (issue #124): a second, concurrent rating observes the first
    /// one's just-written `change_time`, would compute a ~0 interval off
    /// it, and is rejected instead of collapsing the schedule.
    ///
    /// # Errors
    /// Returns an error on database failure.
    pub async fn set_card_state_if_unchanged(
        &self,
        user_id: i64,
        id: &str,
        state: CardState,
        change_time: i64,
        next_time: i64,
        expected_change_time: i64,
    ) -> Result<SetCardState, Error> {
        let updated = sqlx::query_as::<_, Card>(&format!(
            "UPDATE cards SET state = ?, change_time = ?, next_time = ? \
             WHERE user_id = ? AND id = ? AND change_time = ? \
             RETURNING {CARD_COLUMNS}"
        ))
        .bind(state)
        .bind(change_time)
        .bind(next_time)
        .bind(user_id)
        .bind(id)
        .bind(expected_change_time)
        .fetch_optional(&self.pool)
        .await?;
        if let Some(mut card) = updated {
            card.labels = self.labels_of(&card.id).await?;
            return Ok(SetCardState::Applied(card));
        }
        // The conditional update missed: either the card does not exist
        // for this user at all, or its `change_time` moved under us.
        Ok(match self.get_card(user_id, id).await? {
            Some(current) => SetCardState::Stale(current),
            None => SetCardState::NotFound,
        })
    }

    /// Deletes the card with this id owned by this user. Returns whether
    /// a card was deleted.
    ///
    /// # Errors
    /// Returns an error on database failure.
    pub async fn delete_card(&self, user_id: i64, id: &str) -> Result<bool, Error> {
        let result = sqlx::query("DELETE FROM cards WHERE user_id = ? AND id = ?")
            .bind(user_id)
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    /// Port of the old `CardStore.Find`: full-Unicode case-insensitive
    /// substring match over prompt and solution
    /// (`Contains(searchText, OrdinalIgnoreCase)`), ordered by
    /// `next_time` ascending (most due first), ties broken by `id`,
    /// then skip/take paging. `None` or an empty search matches all
    /// cards of the user. `labels` restricts the hits by label
    /// (labels replace the `disabled` flag, owner decision 2026-08-01):
    /// `None` disables filtering, `Some(set)` keeps cards carrying ANY
    /// label of the set (union semantics — `Some([])` matches nothing).
    /// Returns the page and the total number of matching cards.
    ///
    /// Deliberate deviation from the old Find (owner decision
    /// 2026-07-31): labels are NOT a sort key — the old "enabled first,
    /// disabled last" order made the groom list re-sort on every
    /// enable/disable toggle.
    ///
    /// Filtering and sorting happen in Rust, not SQL: `SQLite`'s `LIKE`
    /// folds ASCII only, so it would miss e.g. `Äpfel` ~ `äpfel`. The old
    /// app also cached all cards in memory, so this personal-app scale is
    /// fine.
    ///
    /// # Errors
    /// Returns an error on database failure.
    pub async fn search_cards(
        &self,
        user_id: i64,
        search: Option<&str>,
        labels: Option<&[String]>,
        skip: u32,
        limit: u32,
    ) -> Result<(Vec<Card>, i64), Error> {
        let cards = sqlx::query_as::<_, Card>(&format!(
            "SELECT {CARD_COLUMNS} FROM cards WHERE user_id = ?"
        ))
        .bind(user_id)
        .fetch_all(&self.pool)
        .await?;
        let mut label_map = self.card_labels_map(user_id).await?;
        let needle = search.unwrap_or("").to_lowercase();
        let mut hits: Vec<Card> = cards
            .into_iter()
            .map(|mut card| {
                card.labels = label_map.remove(&card.id).unwrap_or_default();
                card
            })
            .filter(|card| {
                let labels_match = match labels {
                    None => true,
                    Some(set) => card.labels.iter().any(|name| set.contains(name)),
                };
                labels_match
                    && (needle.is_empty()
                        || card.prompt.to_lowercase().contains(&needle)
                        || card.solution.to_lowercase().contains(&needle))
            })
            .collect();
        // `next_time` (most due first), then `id` to break ties
        // deterministically (the old dictionary order was arbitrary).
        hits.sort_by(|a, b| a.next_time.cmp(&b.next_time).then_with(|| a.id.cmp(&b.id)));
        let count = i64::try_from(hits.len()).unwrap_or(i64::MAX);
        let page = hits
            .into_iter()
            .skip(usize::try_from(skip).unwrap_or(usize::MAX))
            .take(usize::try_from(limit).unwrap_or(usize::MAX))
            .collect();
        Ok((page, count))
    }

    /// The next card to review: `next_time <= now`, earliest `next_time`
    /// first. `labels` is the quiz's label filter: `None` disables
    /// filtering, `Some(set)` keeps cards carrying ANY of the set (union
    /// semantics — `Some([])` yields no card). Matches the rest of the
    /// semantics of the old `CardStore.FindNext`; there is deliberately
    /// no special handling of `state = 'new'` at the store level.
    ///
    /// # Errors
    /// Returns an error on database failure.
    pub async fn next_card(
        &self,
        user_id: i64,
        now: i64,
        labels: Option<&[String]>,
    ) -> Result<Option<Card>, Error> {
        let label_clause = match labels {
            None => String::new(),
            Some(&[]) => return Ok(None),
            // One bound parameter per label name (union via IN).
            Some(set) => format!(
                "AND EXISTS ( \
                     SELECT 1 FROM card_labels cl JOIN labels l ON l.id = cl.label_id \
                     WHERE cl.card_id = cards.id AND l.name IN ({}) \
                 )",
                set.iter().map(|_| "?").collect::<Vec<_>>().join(", ")
            ),
        };
        let statement = format!(
            "SELECT {CARD_COLUMNS} FROM cards \
             WHERE user_id = ? AND next_time <= ? {label_clause} \
             ORDER BY next_time ASC, id \
             LIMIT 1"
        );
        let mut query = sqlx::query_as::<_, Card>(&statement)
            .bind(user_id)
            .bind(now);
        if let Some(set) = labels {
            for name in set {
                query = query.bind(name);
            }
        }
        let card = query.fetch_optional(&self.pool).await?;
        match card {
            Some(mut card) => {
                card.labels = self.labels_of(&card.id).await?;
                Ok(Some(card))
            }
            None => Ok(None),
        }
    }

    // ------------------------------------------------------------- autosave

    /// Stores the autosave for a user (one per user). If an autosave
    /// already exists with identical content (`card_id`, `prompt`,
    /// `solution`), `updated_at` is kept, so re-applying the same
    /// autosave is a no-op.
    ///
    /// # Errors
    /// Returns an error on database failure.
    pub async fn put_autosave(
        &self,
        user_id: i64,
        card_id: Option<&str>,
        prompt: &str,
        solution: &str,
        now: i64,
    ) -> Result<(), Error> {
        sqlx::query(
            "INSERT INTO autosaves (user_id, card_id, prompt, solution, updated_at) \
             VALUES (?, ?, ?, ?, ?) \
             ON CONFLICT (user_id) DO UPDATE SET \
             card_id = excluded.card_id, \
             prompt = excluded.prompt, \
             solution = excluded.solution, \
             updated_at = CASE \
             WHEN autosaves.card_id IS excluded.card_id \
             AND autosaves.prompt = excluded.prompt \
             AND autosaves.solution = excluded.solution \
             THEN autosaves.updated_at ELSE excluded.updated_at END",
        )
        .bind(user_id)
        .bind(card_id)
        .bind(prompt)
        .bind(solution)
        .bind(now)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// The user's autosave, if any.
    ///
    /// # Errors
    /// Returns an error on database failure.
    pub async fn get_autosave(&self, user_id: i64) -> Result<Option<AutoSave>, Error> {
        let autosave = sqlx::query_as::<_, AutoSave>(
            "SELECT card_id, prompt, solution, updated_at FROM autosaves WHERE user_id = ?",
        )
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(autosave)
    }

    /// Deletes the user's autosave. Returns whether one existed.
    ///
    /// # Errors
    /// Returns an error on database failure.
    pub async fn delete_autosave(&self, user_id: i64) -> Result<bool, Error> {
        let result = sqlx::query("DELETE FROM autosaves WHERE user_id = ?")
            .bind(user_id)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    // ------------------------------------------------------------ passkeys

    /// Inserts a passkey for a user. Returns the row id.
    ///
    /// # Errors
    /// Returns an error on database failure, including a uniqueness
    /// violation if the credential id is already registered (to any user).
    pub async fn insert_passkey(
        &self,
        user_id: i64,
        credential_id: &str,
        name: &str,
        data: &str,
        created_at: i64,
    ) -> Result<i64, Error> {
        let row: (i64,) = sqlx::query_as(
            "INSERT INTO passkeys (user_id, credential_id, name, data, created_at) \
             VALUES (?, ?, ?, ?, ?) RETURNING id",
        )
        .bind(user_id)
        .bind(credential_id)
        .bind(name)
        .bind(data)
        .bind(created_at)
        .fetch_one(&self.pool)
        .await?;
        Ok(row.0)
    }

    /// All passkeys of a user, ordered by id.
    ///
    /// # Errors
    /// Returns an error on database failure.
    pub async fn get_passkeys_for_user(&self, user_id: i64) -> Result<Vec<PasskeyRow>, Error> {
        let rows = sqlx::query_as::<_, PasskeyRow>(&format!(
            "SELECT {PASSKEY_COLUMNS} FROM passkeys WHERE user_id = ? ORDER BY id"
        ))
        .bind(user_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// The passkey with this (base64url) credential id, if any — the login
    /// lookup: the credential id identifies the user during username-less
    /// authentication.
    ///
    /// # Errors
    /// Returns an error on database failure.
    pub async fn get_passkey_by_credential_id(
        &self,
        credential_id: &str,
    ) -> Result<Option<PasskeyRow>, Error> {
        let row = sqlx::query_as::<_, PasskeyRow>(&format!(
            "SELECT {PASSKEY_COLUMNS} FROM passkeys WHERE credential_id = ?"
        ))
        .bind(credential_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row)
    }

    /// Renames a passkey owned by this user. Returns whether a row was
    /// updated (`false` for unknown/other-user ids).
    ///
    /// # Errors
    /// Returns an error on database failure.
    pub async fn rename_passkey(&self, user_id: i64, id: i64, name: &str) -> Result<bool, Error> {
        let result = sqlx::query("UPDATE passkeys SET name = ? WHERE user_id = ? AND id = ?")
            .bind(name)
            .bind(user_id)
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    /// Deletes a passkey owned by this user, refusing the user's LAST
    /// passkey (deleting it would lock the user out). The guard is part
    /// of the statement itself, so a concurrent delete cannot race it
    /// (check-then-act would). Returns whether a row was deleted (`false`
    /// for unknown/other-user ids AND for the last-passkey refusal; the
    /// caller distinguishes the two by checking whether the row exists).
    ///
    /// # Errors
    /// Returns an error on database failure.
    pub async fn delete_passkey(&self, user_id: i64, id: i64) -> Result<bool, Error> {
        let result = sqlx::query(
            "DELETE FROM passkeys WHERE user_id = ? AND id = ? \
             AND (SELECT COUNT(*) FROM passkeys WHERE user_id = ?) > 1",
        )
        .bind(user_id)
        .bind(id)
        .bind(user_id)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    /// The number of passkeys in the whole system. Zero means the
    /// bootstrap registration is open (no session required).
    ///
    /// # Errors
    /// Returns an error on database failure.
    pub async fn count_passkeys(&self) -> Result<i64, Error> {
        let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM passkeys")
            .fetch_one(&self.pool)
            .await?;
        Ok(count)
    }

    /// The number of passkeys of one user (used to refuse deleting the
    /// user's last passkey).
    ///
    /// # Errors
    /// Returns an error on database failure.
    pub async fn count_passkeys_for_user(&self, user_id: i64) -> Result<i64, Error> {
        let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM passkeys WHERE user_id = ?")
            .bind(user_id)
            .fetch_one(&self.pool)
            .await?;
        Ok(count)
    }

    /// Replaces the serialized passkey blob and stamps `last_used_at`
    /// after a successful authentication (webauthn-rs updates the
    /// credential counter and backup-state flags inside the blob).
    /// `id` is the passkey row id; scoped to `user_id` for consistency
    /// with the other passkey mutations. Returns whether a row was
    /// updated.
    ///
    /// # Errors
    /// Returns an error on database failure.
    pub async fn update_passkey_after_auth(
        &self,
        user_id: i64,
        id: i64,
        data: &str,
        last_used_at: i64,
    ) -> Result<bool, Error> {
        let result = sqlx::query(
            "UPDATE passkeys SET data = ?, last_used_at = ? WHERE user_id = ? AND id = ?",
        )
        .bind(data)
        .bind(last_used_at)
        .bind(user_id)
        .bind(id)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    // ------------------------------------------------------------ sessions

    /// Creates a session row: opaque `token` for `user_id`, valid until
    /// `expires_at` (unix epoch millis). `verified_at` stamps the last
    /// passkey proof (a fresh login counts); sensitive operations require
    /// a recent one (step-up, "sudo mode").
    ///
    /// # Errors
    /// Returns an error on database failure, including a uniqueness
    /// violation if the token already exists (a 244-bit random token
    /// collision — the caller may retry).
    pub async fn create_session(
        &self,
        token: &str,
        user_id: i64,
        expires_at: i64,
        verified_at: i64,
    ) -> Result<(), Error> {
        sqlx::query(
            "INSERT INTO sessions (token, user_id, expires_at, verified_at) VALUES (?, ?, ?, ?)",
        )
        .bind(token)
        .bind(user_id)
        .bind(expires_at)
        .bind(verified_at)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// The `verified_at` stamp of a session token (see
    /// [`Store::create_session`]), or `None` for an unknown token.
    ///
    /// # Errors
    /// Returns an error on database failure.
    pub async fn get_session_verified_at(&self, token: &str) -> Result<Option<i64>, Error> {
        let row: Option<(i64,)> =
            sqlx::query_as("SELECT verified_at FROM sessions WHERE token = ?")
                .bind(token)
                .fetch_optional(&self.pool)
                .await?;
        Ok(row.map(|(verified_at,)| verified_at))
    }

    /// Re-stamps a session's `verified_at` after a successful step-up
    /// ceremony. Returns whether the session existed.
    ///
    /// # Errors
    /// Returns an error on database failure.
    pub async fn touch_session_verified(&self, token: &str, now: i64) -> Result<bool, Error> {
        let result = sqlx::query("UPDATE sessions SET verified_at = ? WHERE token = ?")
            .bind(now)
            .bind(token)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    /// The user behind a session token, or `None` if the token is unknown
    /// or expired (checked against `now`, unix epoch millis). Expired rows
    /// found this way are deleted eagerly.
    ///
    /// # Errors
    /// Returns an error on database failure.
    pub async fn get_session_user(&self, token: &str, now: i64) -> Result<Option<User>, Error> {
        let row: Option<(i64, i64)> =
            sqlx::query_as("SELECT user_id, expires_at FROM sessions WHERE token = ?")
                .bind(token)
                .fetch_optional(&self.pool)
                .await?;
        let Some((user_id, expires_at)) = row else {
            return Ok(None);
        };
        if expires_at <= now {
            sqlx::query("DELETE FROM sessions WHERE token = ?")
                .bind(token)
                .execute(&self.pool)
                .await?;
            return Ok(None);
        }
        let user =
            sqlx::query_as::<_, User>("SELECT id, username, created_at FROM users WHERE id = ?")
                .bind(user_id)
                .fetch_optional(&self.pool)
                .await?;
        Ok(user)
    }

    /// Deletes a session (logout). Returns whether one existed.
    ///
    /// # Errors
    /// Returns an error on database failure.
    pub async fn delete_session(&self, token: &str) -> Result<bool, Error> {
        let result = sqlx::query("DELETE FROM sessions WHERE token = ?")
            .bind(token)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    /// Deletes every session of a user EXCEPT `keep_token` (the session
    /// performing the action, so the user is not logged out mid-action).
    /// Called when a passkey is deleted: a lost device's passkey being
    /// removed must also kill that device's session. Returns the number
    /// of sessions deleted.
    ///
    /// # Errors
    /// Returns an error on database failure.
    pub async fn delete_other_sessions(
        &self,
        user_id: i64,
        keep_token: &str,
    ) -> Result<u64, Error> {
        let result = sqlx::query("DELETE FROM sessions WHERE user_id = ? AND token != ?")
            .bind(user_id)
            .bind(keep_token)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected())
    }

    /// Deletes all sessions expired at `now` (unix epoch millis); called
    /// on server startup. Returns the number of rows deleted.
    ///
    /// # Errors
    /// Returns an error on database failure.
    pub async fn delete_expired_sessions(&self, now: i64) -> Result<u64, Error> {
        let result = sqlx::query("DELETE FROM sessions WHERE expires_at <= ?")
            .bind(now)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected())
    }
}

/// Runs the embedded migration baseline, handling the one-time history
/// squash from the original 0001–0004 chain.
///
/// `SQLx` normally rejects a database when an applied migration is no longer
/// present in the embedded directory. That is exactly what we want after
/// the cut-over, but it would prevent the first baseline release from
/// opening a current database. The compatibility branch is deliberately
/// narrow: only the complete old history is accepted, and the schema shape
/// is checked before missing-history validation is bypassed. The baseline
/// migration then removes the old history rows and records version 0005.
async fn run_migrations(pool: &SqlitePool) -> Result<(), Error> {
    let mut migrator = sqlx::migrate!("./migrations");
    let applied_migrations = applied_migrations(pool).await?;
    let applied_versions: Vec<i64> = applied_migrations
        .iter()
        .map(|migration| migration.version)
        .collect();
    let missing_version = applied_versions
        .iter()
        .find(|version| !migrator.version_exists(**version))
        .copied();

    if missing_version.is_some() {
        if applied_versions != [1, 2, 3, 4]
            || !legacy_metadata_is_authentic(&applied_migrations)
            || !current_schema_is_present(pool).await?
        {
            return Err(Error::MigrationHistoryNotCurrent);
        }
        migrator.set_ignore_missing(true);
    }

    migrator.run(pool).await?;
    Ok(())
}

/// Returns applied migrations, or an empty list for a new database.
async fn applied_migrations(pool: &SqlitePool) -> Result<Vec<AppliedMigration>, Error> {
    let has_history: i64 = sqlx::query_scalar(
        "SELECT EXISTS(
             SELECT 1 FROM sqlite_master
             WHERE type = 'table' AND name = '_sqlx_migrations'
         )",
    )
    .fetch_one(pool)
    .await?;
    if has_history == 0 {
        let has_application_tables: i64 = sqlx::query_scalar(
            "SELECT EXISTS(
                 SELECT 1 FROM sqlite_master
                 WHERE type = 'table'
                   AND name NOT LIKE 'sqlite_%'
                   AND name != '_sqlx_migrations'
             )",
        )
        .fetch_one(pool)
        .await?;
        if has_application_tables != 0 {
            return Err(Error::MigrationHistoryNotCurrent);
        }
        return Ok(Vec::new());
    }
    Ok(sqlx::query_as::<_, AppliedMigration>(
        "SELECT version, description, success, checksum
         FROM _sqlx_migrations ORDER BY version",
    )
    .fetch_all(pool)
    .await?)
}

/// Confirms that the legacy rows came from the deleted migration files, not
/// merely that their version numbers happen to be 1 through 4.
fn legacy_metadata_is_authentic(applied: &[AppliedMigration]) -> bool {
    applied.len() == LEGACY_MIGRATION_METADATA.len()
        && applied.iter().zip(LEGACY_MIGRATION_METADATA).all(
            |(actual, (version, description, checksum))| {
                actual.version == version
                    && actual.description == description
                    && actual.success
                    && actual.checksum.as_slice() == checksum
            },
        )
}

/// Checks the schema that migration 0004 produced before its history is
/// discarded. An older database must fail loudly instead of being mistaken
/// for the current baseline.
async fn current_schema_is_present(pool: &SqlitePool) -> Result<bool, Error> {
    const TABLES_AND_COLUMNS: [(&str, &[&str]); 8] = [
        ("users", &["id", "username", "created_at"]),
        (
            "passkeys",
            &[
                "id",
                "user_id",
                "credential_id",
                "name",
                "data",
                "created_at",
                "last_used_at",
            ],
        ),
        (
            "sessions",
            &["token", "user_id", "expires_at", "verified_at"],
        ),
        (
            "cards",
            &[
                "id",
                "user_id",
                "prompt",
                "solution",
                "state",
                "change_time",
                "next_time",
            ],
        ),
        (
            "autosaves",
            &["user_id", "prompt", "solution", "updated_at", "card_id"],
        ),
        ("labels", &["id", "user_id", "name"]),
        ("card_labels", &["card_id", "label_id"]),
        (
            "_sqlx_migrations",
            &["version", "description", "installed_on"],
        ),
    ];
    const FOREIGN_KEYS: [(&str, &str, &str, &str); 7] = [
        ("passkeys", "user_id", "users", "NO ACTION"),
        ("sessions", "user_id", "users", "NO ACTION"),
        ("cards", "user_id", "users", "NO ACTION"),
        ("autosaves", "user_id", "users", "NO ACTION"),
        ("labels", "user_id", "users", "NO ACTION"),
        ("card_labels", "card_id", "cards", "CASCADE"),
        ("card_labels", "label_id", "labels", "CASCADE"),
    ];

    for (table, expected_columns) in TABLES_AND_COLUMNS {
        let columns: Vec<String> = sqlx::query_scalar("SELECT name FROM pragma_table_info(?)")
            .bind(table)
            .fetch_all(pool)
            .await?;
        if !expected_columns
            .iter()
            .all(|expected| columns.iter().any(|column| column == expected))
        {
            return Ok(false);
        }
    }

    if !has_index_with_columns(pool, "labels", &["user_id", "name"], true).await?
        || !has_index_with_columns(pool, "cards", &["user_id", "next_time"], false).await?
    {
        return Ok(false);
    }

    for (table, from, referenced_table, on_delete) in FOREIGN_KEYS {
        if !has_foreign_key(pool, table, from, referenced_table, on_delete).await? {
            return Ok(false);
        }
    }

    let cards_columns: Vec<String> = sqlx::query_scalar("SELECT name FROM pragma_table_info(?)")
        .bind("cards")
        .fetch_all(pool)
        .await?;
    if cards_columns.iter().any(|column| column == "disabled") {
        return Ok(false);
    }
    let unlabeled_cards: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM cards c
         WHERE NOT EXISTS (
             SELECT 1 FROM card_labels cl WHERE cl.card_id = c.id
         )",
    )
    .fetch_one(pool)
    .await?;
    Ok(unlabeled_cards == 0)
}

/// Checks an index's ordered columns, optionally requiring uniqueness.
async fn has_index_with_columns(
    pool: &SqlitePool,
    table: &str,
    expected_columns: &[&str],
    unique: bool,
) -> Result<bool, Error> {
    let indexes: Vec<(String, i64)> =
        sqlx::query_as("SELECT name, \"unique\" FROM pragma_index_list(?)")
            .bind(table)
            .fetch_all(pool)
            .await?;
    for (name, is_unique) in indexes {
        if unique && is_unique == 0 {
            continue;
        }
        let columns: Vec<String> = sqlx::query_scalar("SELECT name FROM pragma_index_info(?)")
            .bind(name)
            .fetch_all(pool)
            .await?;
        if columns.len() == expected_columns.len()
            && columns
                .iter()
                .zip(expected_columns)
                .all(|(actual, expected)| actual == expected)
        {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Checks one foreign-key relationship and its delete action.
async fn has_foreign_key(
    pool: &SqlitePool,
    table: &str,
    from: &str,
    referenced_table: &str,
    on_delete: &str,
) -> Result<bool, Error> {
    let present: i64 = sqlx::query_scalar(
        "SELECT EXISTS(
             SELECT 1 FROM pragma_foreign_key_list(?)
             WHERE \"table\" = ? AND \"from\" = ? AND \"on_delete\" = ?
         )",
    )
    .bind(table)
    .bind(referenced_table)
    .bind(from)
    .bind(on_delete)
    .fetch_one(pool)
    .await?;
    Ok(present != 0)
}

/// Current time as unix epoch millis, falling back to 0 if the system
/// clock is before the epoch.
fn now_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_millis()).unwrap_or(i64::MAX))
}

/// Copies an existing non-empty database file to
/// `<db-parent>/backups/<db-name>-<yyyymmdd-hhmmss>.db` (UTC) before
/// migrations run. No-op for a missing or empty database (nothing worth
/// backing up yet). After writing a backup, only the newest
/// [`BACKUPS_TO_KEEP`] backups of this database are kept; older ones are
/// deleted (the timestamped names sort chronologically).
fn backup_before_migration(path: &Path) -> Result<(), Error> {
    let Ok(metadata) = std::fs::metadata(path) else {
        // Not existing yet (or unreadable metadata): nothing to back up.
        return Ok(());
    };
    if metadata.len() == 0 {
        return Ok(());
    }
    let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) else {
        return Ok(());
    };
    let Some(name) = path.file_name() else {
        return Ok(());
    };
    let name = name.to_string_lossy();
    let backups = parent.join("backups");
    std::fs::create_dir_all(&backups)?;
    let stamp = OffsetDateTime::now_utc()
        .format(BACKUP_TIMESTAMP_FORMAT)
        .map_err(|err| Error::Io(std::io::Error::other(err)))?;
    let target = backups.join(format!("{name}-{stamp}.db"));
    std::fs::copy(path, target)?;
    prune_backups(&backups, &name)
}

/// How many pre-migration backups of a database are kept.
const BACKUPS_TO_KEEP: usize = 10;

/// Deletes all but the newest [`BACKUPS_TO_KEEP`] backups of `name` in
/// `backups`. Only files named `<name>-<stamp>.db` (as written by
/// [`backup_before_migration`]) are touched; the timestamped names sort
/// chronologically, so a plain name sort finds the oldest.
fn prune_backups(backups: &Path, name: &str) -> Result<(), Error> {
    let prefix = format!("{name}-");
    let mut files: Vec<std::path::PathBuf> = std::fs::read_dir(backups)?
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name().and_then(|n| n.to_str()).is_some_and(|n| {
                n.starts_with(&prefix)
                    && Path::new(n)
                        .extension()
                        .is_some_and(|ext| ext.eq_ignore_ascii_case("db"))
            })
        })
        .collect();
    files.sort();
    for old in files
        .iter()
        .take(files.len().saturating_sub(BACKUPS_TO_KEEP))
    {
        std::fs::remove_file(old)?;
    }
    Ok(())
}

/// `yyyymmdd-hhmmss` for backup file names (UTC).
const BACKUP_TIMESTAMP_FORMAT: &[time::format_description::FormatItem<'_>] =
    time::macros::format_description!("[year][month][day]-[hour][minute][second]");

#[cfg(test)]
mod tests;
