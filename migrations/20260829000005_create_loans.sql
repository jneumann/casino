-- One row per house marker. A player may have at most one outstanding loan
-- (repaid_at IS NULL); the unique index is what two concurrent borrows race
-- against, the same way settle_spin guards the debit inside the UPDATE.
CREATE TABLE IF NOT EXISTS loans (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id    INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    principal  INTEGER NOT NULL,
    interest   INTEGER NOT NULL,
    created_at TEXT    NOT NULL,
    repaid_at  TEXT
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_loans_one_outstanding
    ON loans (user_id)
    WHERE repaid_at IS NULL;

CREATE INDEX IF NOT EXISTS idx_loans_user_created
    ON loans (user_id, created_at DESC);
