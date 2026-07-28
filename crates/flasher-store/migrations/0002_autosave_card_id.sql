-- The autosave belongs to a card edit session: `card_id` is the id of
-- the card being edited (the old `AutoSave.Id`), NULL for a draft of a
-- brand-new card. Deliberately no foreign key: the draft must survive
-- deletion of the card it refers to.
ALTER TABLE autosaves ADD COLUMN card_id TEXT;
