-- Labels on cards (owner decision 2026-08-01): the `disabled` flag
-- dissolves into the two per-user seed labels `Enabled`/`Disabled`.
-- Users created AFTER this migration get their seed labels from the
-- store's create/upsert-user path instead.

CREATE TABLE labels (
    id INTEGER PRIMARY KEY,
    user_id INTEGER NOT NULL REFERENCES users (id),
    name TEXT NOT NULL,
    UNIQUE (user_id, name)
);

CREATE TABLE card_labels (
    card_id TEXT NOT NULL REFERENCES cards (id) ON DELETE CASCADE,
    label_id INTEGER NOT NULL REFERENCES labels (id) ON DELETE CASCADE,
    PRIMARY KEY (card_id, label_id)
);

-- Seed the two labels for every existing user.
INSERT INTO labels (user_id, name)
SELECT id, 'Enabled' FROM users
UNION ALL
SELECT id, 'Disabled' FROM users;

-- Convert the flag: disabled = 1 -> 'Disabled', otherwise 'Enabled'.
INSERT INTO card_labels (card_id, label_id)
SELECT c.id, l.id
FROM cards c
JOIN labels l
  ON l.user_id = c.user_id
 AND l.name = CASE c.disabled WHEN 1 THEN 'Disabled' ELSE 'Enabled' END;

DROP INDEX idx_cards_user_disabled;
ALTER TABLE cards DROP COLUMN disabled;
