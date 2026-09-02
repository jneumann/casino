-- One row per video-poker hand. An open row (settled_at IS NULL) is the deal
-- waiting on a draw; the remaining stock lives here and is never sent out.
CREATE TABLE IF NOT EXISTS video_poker_hands (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id       INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    bet           INTEGER NOT NULL,
    dealt         TEXT NOT NULL,
    remaining     TEXT NOT NULL,
    held          TEXT,
    drawn         TEXT,
    outcome       TEXT,
    payout        INTEGER,
    balance_after INTEGER NOT NULL,
    created_at    TEXT NOT NULL,
    settled_at    TEXT
);

-- One hand in play at a time, the same way the cashier writes one marker.
CREATE UNIQUE INDEX IF NOT EXISTS idx_video_poker_open
    ON video_poker_hands (user_id)
    WHERE settled_at IS NULL;

CREATE INDEX IF NOT EXISTS idx_video_poker_user_created
    ON video_poker_hands (user_id, created_at DESC);
