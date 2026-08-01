-- Current-schema baseline (squashed 2026-08-01; issue #130).
--
-- A database that already has migrations 0001 through 0004 reaches this
-- migration through the compatibility path in Store::run_migrations. The
-- IF NOT EXISTS clauses make the same file usable as the only migration on
-- a fresh database. The history cleanup below leaves one stable baseline
-- row, so future migrations can be appended normally from version 0005.

CREATE TABLE IF NOT EXISTS users (
    id INTEGER PRIMARY KEY,
    username TEXT UNIQUE NOT NULL COLLATE NOCASE,
    created_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS passkeys (
    id INTEGER PRIMARY KEY,
    user_id INTEGER NOT NULL REFERENCES users (id),
    credential_id TEXT UNIQUE NOT NULL,
    name TEXT NOT NULL,
    -- Serialized passkey; format decided in Phase 5.
    data TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    last_used_at INTEGER
);

CREATE TABLE IF NOT EXISTS sessions (
    token TEXT PRIMARY KEY,
    user_id INTEGER NOT NULL REFERENCES users (id),
    expires_at INTEGER NOT NULL,
    verified_at INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS cards (
    -- uuid string, preserved from the old file store
    id TEXT PRIMARY KEY,
    user_id INTEGER NOT NULL REFERENCES users (id),
    prompt TEXT NOT NULL,
    solution TEXT NOT NULL,
    state TEXT NOT NULL CHECK (state IN ('new', 'ok', 'failed')),
    change_time INTEGER NOT NULL,
    next_time INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_cards_user_next_time
    ON cards (user_id, next_time);

CREATE TABLE IF NOT EXISTS autosaves (
    user_id INTEGER PRIMARY KEY REFERENCES users (id),
    prompt TEXT NOT NULL,
    solution TEXT NOT NULL,
    updated_at INTEGER NOT NULL,
    card_id TEXT
);

CREATE TABLE IF NOT EXISTS labels (
    id INTEGER PRIMARY KEY,
    user_id INTEGER NOT NULL REFERENCES users (id),
    name TEXT NOT NULL,
    UNIQUE (user_id, name)
);

CREATE TABLE IF NOT EXISTS card_labels (
    card_id TEXT NOT NULL REFERENCES cards (id) ON DELETE CASCADE,
    label_id INTEGER NOT NULL REFERENCES labels (id) ON DELETE CASCADE,
    PRIMARY KEY (card_id, label_id)
);

-- The old rows describe the same schema. Keep only this baseline so the
-- deleted historical files are not required on future startups.
DELETE FROM _sqlx_migrations WHERE version < 5;
