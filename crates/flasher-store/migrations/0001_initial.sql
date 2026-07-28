-- Initial schema for the SQLite store.
-- Timestamps are unix epoch MILLIS (i64) to avoid any time-type dependencies.

CREATE TABLE users (
    id INTEGER PRIMARY KEY,
    username TEXT UNIQUE NOT NULL COLLATE NOCASE,
    created_at INTEGER NOT NULL
);

CREATE TABLE passkeys (
    id INTEGER PRIMARY KEY,
    user_id INTEGER NOT NULL REFERENCES users (id),
    credential_id TEXT UNIQUE NOT NULL,
    name TEXT NOT NULL,
    -- Serialized passkey; format decided in Phase 5.
    data TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    last_used_at INTEGER
);

CREATE TABLE sessions (
    token TEXT PRIMARY KEY,
    user_id INTEGER NOT NULL REFERENCES users (id),
    expires_at INTEGER NOT NULL
);

CREATE TABLE cards (
    -- uuid string, preserved from the old file store
    id TEXT PRIMARY KEY,
    user_id INTEGER NOT NULL REFERENCES users (id),
    prompt TEXT NOT NULL,
    solution TEXT NOT NULL,
    state TEXT NOT NULL CHECK (state IN ('new', 'ok', 'failed')),
    change_time INTEGER NOT NULL,
    next_time INTEGER NOT NULL,
    disabled INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX idx_cards_user_next_time ON cards (user_id, next_time);
CREATE INDEX idx_cards_user_disabled ON cards (user_id, disabled);

CREATE TABLE autosaves (
    user_id INTEGER PRIMARY KEY REFERENCES users (id),
    prompt TEXT NOT NULL,
    solution TEXT NOT NULL,
    updated_at INTEGER NOT NULL
);
