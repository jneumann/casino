-- Staff who can reach the admin dashboard. Kept separate from `users` so a
-- self-registered player can never hold backend access.
CREATE TABLE IF NOT EXISTS operators (
    id                  INTEGER PRIMARY KEY AUTOINCREMENT,
    username            TEXT NOT NULL COLLATE NOCASE UNIQUE,
    password_hash       TEXT NOT NULL,
    role                TEXT NOT NULL DEFAULT 'viewer',
    is_active           INTEGER NOT NULL DEFAULT 1,
    created_at          TEXT NOT NULL,
    last_login_at       TEXT,
    -- Tokens issued before this instant are refused, so changing a password
    -- signs that operator's other sessions out.
    password_changed_at TEXT NOT NULL
);
