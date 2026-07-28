-- Step-up authentication ("sudo mode"): `verified_at` stamps when the
-- session's user last proved possession of a passkey (login or step-up
-- ceremony). Sensitive operations (adding/removing passkeys) require a
-- recent `verified_at`. Existing rows get 0 = "long ago" and must re-verify.

ALTER TABLE sessions ADD COLUMN verified_at INTEGER NOT NULL DEFAULT 0;
