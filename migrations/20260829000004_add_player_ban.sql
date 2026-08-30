-- Banned players keep their row and history but cannot sign in or play.
ALTER TABLE users ADD COLUMN is_banned INTEGER NOT NULL DEFAULT 0;
