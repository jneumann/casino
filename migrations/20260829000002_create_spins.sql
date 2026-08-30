-- One row per settled spin: an audit trail for reconciling balances and for
-- checking the machine pays out as designed.
CREATE TABLE IF NOT EXISTS spins (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id       INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    bet           INTEGER NOT NULL,
    payout        INTEGER NOT NULL,
    reels         TEXT NOT NULL,
    balance_after INTEGER NOT NULL,
    created_at    TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_spins_user_created ON spins (user_id, created_at DESC);
