-- Target-scoped drafts (issue #137).
-- The released autosaves table conflated a new-card draft and every card
-- edit. Split those workflows and retain any existing content while doing so.

ALTER TABLE cards ADD COLUMN revision INTEGER NOT NULL DEFAULT 0;

CREATE TABLE new_card_drafts (
    user_id INTEGER PRIMARY KEY REFERENCES users (id) ON DELETE CASCADE,
    prompt TEXT NOT NULL,
    solution TEXT NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE TABLE card_edit_drafts (
    user_id INTEGER NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    card_id TEXT NOT NULL REFERENCES cards (id) ON DELETE CASCADE,
    prompt TEXT NOT NULL,
    solution TEXT NOT NULL,
    labels TEXT NOT NULL,
    base_revision INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    PRIMARY KEY (user_id, card_id)
);

INSERT INTO new_card_drafts (user_id, prompt, solution, updated_at)
SELECT user_id, prompt, solution, updated_at
FROM autosaves
WHERE card_id IS NULL;

INSERT INTO card_edit_drafts (
    user_id, card_id, prompt, solution, labels, base_revision, updated_at
)
SELECT a.user_id, a.card_id, a.prompt, a.solution,
       COALESCE(
           (SELECT json_group_array(name)
            FROM (
                SELECT l.name
                FROM card_labels cl
                JOIN labels l ON l.id = cl.label_id
                WHERE cl.card_id = a.card_id
                ORDER BY l.name
            )),
           '[]'
       ),
       c.revision,
       a.updated_at
FROM autosaves a
JOIN cards c ON c.id = a.card_id AND c.user_id = a.user_id
WHERE a.card_id IS NOT NULL;

DROP TABLE autosaves;
