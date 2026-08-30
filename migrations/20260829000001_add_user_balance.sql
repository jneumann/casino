-- Every account holds a coin balance. Existing rows inherit the same starting
-- stake as new signups.
ALTER TABLE users ADD COLUMN balance INTEGER NOT NULL DEFAULT 1000;
