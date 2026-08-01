//! `SQLite` persistence for Flasher.
//!
//! Uses sqlx with embedded migrations and runtime-checked queries (no
//! `query!` macros, no `DATABASE_URL` needed at compile time). All
//! timestamps are unix epoch millis (`i64`).

mod types;

use std::collections::HashMap;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use sqlx::SqlitePool;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};
use time::OffsetDateTime;

pub use flasher_types::{DISABLED_LABEL, ENABLED_LABEL};
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
        sqlx::migrate!("./migrations").run(&pool).await?;
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
        sqlx::migrate!("./migrations").run(&pool).await?;
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
        self.ensure_seed_labels(user.id).await?;
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
        self.ensure_seed_labels(user.id).await?;
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

    /// Makes sure the user carries the two seed labels (the dissolved
    /// `disabled` flag). Called from the user create/upsert paths: users
    /// created after the labels migration get their seeds here, not from
    /// the migration.
    async fn ensure_seed_labels(&self, user_id: i64) -> Result<(), Error> {
        self.ensure_label(user_id, ENABLED_LABEL).await?;
        self.ensure_label(user_id, DISABLED_LABEL).await?;
        Ok(())
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

    /// Replaces the card's whole label set. Every name must be an
    /// existing label of the user (no backdoor creation) and the set
    /// must not be empty (a label-less card would be invisible to every
    /// union filter). Returns the updated card, `None` if no card with
    /// this id exists for the user.
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
        if labels.is_empty() {
            return Err(Error::EmptyLabelSet);
        }
        let mut label_ids = Vec::with_capacity(labels.len());
        let known = self.labels(user_id).await?;
        for name in labels {
            let label = known
                .iter()
                .find(|label| &label.name == name)
                .ok_or_else(|| Error::UnknownLabel(name.clone()))?;
            label_ids.push(label.id);
        }
        // The card must exist and belong to the user.
        let Some(_card) = self.get_card(user_id, id).await? else {
            return Ok(None);
        };
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
    /// first, carrying ANY of `labels` (union semantics — the quiz's
    /// label filter; an empty slice yields no card). Matches the rest of
    /// the semantics of the old `CardStore.FindNext`; there is
    /// deliberately no special handling of `state = 'new'` at the store
    /// level.
    ///
    /// # Errors
    /// Returns an error on database failure.
    pub async fn next_card(
        &self,
        user_id: i64,
        now: i64,
        labels: &[String],
    ) -> Result<Option<Card>, Error> {
        if labels.is_empty() {
            return Ok(None);
        }
        // One bound parameter per label name (union via IN).
        let placeholders = labels.iter().map(|_| "?").collect::<Vec<_>>().join(", ");
        let statement = format!(
            "SELECT {CARD_COLUMNS} FROM cards \
             WHERE user_id = ? AND next_time <= ? \
             AND EXISTS ( \
                 SELECT 1 FROM card_labels cl JOIN labels l ON l.id = cl.label_id \
                 WHERE cl.card_id = cards.id AND l.name IN ({placeholders}) \
             ) \
             ORDER BY next_time ASC, id \
             LIMIT 1"
        );
        let mut query = sqlx::query_as::<_, Card>(&statement)
            .bind(user_id)
            .bind(now);
        for name in labels {
            query = query.bind(name);
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
