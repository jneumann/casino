-- Admin-issued credits and debits. Each row is an audit record of who
-- changed a player's balance, by how much, and why. Operator username is
-- snapshotted so the log stays readable after that account is deleted.
CREATE TABLE IF NOT EXISTS balance_adjustments (
    id                 INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id            INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    player_username    TEXT    NOT NULL,
    operator_id        INTEGER REFERENCES operators(id) ON DELETE SET NULL,
    operator_username  TEXT    NOT NULL,
    delta              INTEGER NOT NULL,
    balance_before     INTEGER NOT NULL,
    balance_after      INTEGER NOT NULL,
    reason             TEXT    NOT NULL,
    created_at         TEXT    NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_balance_adjustments_created
    ON balance_adjustments (created_at DESC, id DESC);

CREATE INDEX IF NOT EXISTS idx_balance_adjustments_user
    ON balance_adjustments (user_id, created_at DESC);
