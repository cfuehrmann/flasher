//! Domain types for the Flasher `SQLite` store.
//!
//! [`CardState`] itself lives in `flasher-types` (single contract
//! authority shared with the frontend); `flasher-store` enables its
//! `sqlx` feature so the enum is stored as TEXT in `SQLite`.

use serde::{Deserialize, Serialize};

pub use flasher_types::CardState;

/// A user account.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, sqlx::FromRow)]
pub struct User {
    pub id: i64,
    pub username: String,
    /// Unix epoch millis.
    pub created_at: i64,
}

/// A flash card as persisted in the `cards` table.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, sqlx::FromRow)]
pub struct Card {
    pub id: String,
    pub prompt: String,
    pub solution: String,
    pub state: CardState,
    /// Unix epoch millis.
    pub change_time: i64,
    /// Unix epoch millis.
    pub next_time: i64,
    pub disabled: bool,
}

/// Data needed to insert a new card.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewCard {
    pub user_id: i64,
    pub id: String,
    pub prompt: String,
    pub solution: String,
    pub state: CardState,
    /// Unix epoch millis.
    pub change_time: i64,
    /// Unix epoch millis.
    pub next_time: i64,
    pub disabled: bool,
}

/// The in-progress edit a client autosaves during a card edit session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, sqlx::FromRow)]
pub struct AutoSave {
    /// The card being edited (the old `AutoSave.Id`); `None` for a draft
    /// of a brand-new card.
    pub card_id: Option<String>,
    pub prompt: String,
    pub solution: String,
    /// Unix epoch millis.
    pub updated_at: i64,
}

/// A `WebAuthn` passkey as persisted in the `passkeys` table.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, sqlx::FromRow)]
pub struct PasskeyRow {
    pub id: i64,
    pub user_id: i64,
    /// Base64url (no padding) encoding of the raw credential id.
    pub credential_id: String,
    pub name: String,
    /// Serialized webauthn-rs `Passkey` (serde JSON); `flasher-store`
    /// treats it as an opaque blob.
    pub data: String,
    /// Unix epoch millis.
    pub created_at: i64,
    /// Unix epoch millis; `None` if never used to log in.
    pub last_used_at: Option<i64>,
}
