-- One row per blackjack hand. An open row is the player's turn; the hole card
-- and remaining stock live here and are never sent out until settlement.
CREATE TABLE IF NOT EXISTS blackjack_hands (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id       INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    bet           INTEGER NOT NULL,
    doubled       INTEGER NOT NULL DEFAULT 0,
    player        TEXT NOT NULL,
    dealer        TEXT NOT NULL,
    remaining     TEXT NOT NULL,
    outcome       TEXT,
    payout        INTEGER,
    balance_after INTEGER NOT NULL,
    version       INTEGER NOT NULL DEFAULT 0,
    created_at    TEXT NOT NULL,
    settled_at    TEXT
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_blackjack_open
    ON blackjack_hands (user_id)
    WHERE settled_at IS NULL;

CREATE INDEX IF NOT EXISTS idx_blackjack_user_created
    ON blackjack_hands (user_id, created_at DESC);
