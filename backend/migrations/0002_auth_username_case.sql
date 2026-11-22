-- Chess.com usernames are case-insensitive. Enforce that at the database level
-- so a login with different capitalisation cannot create a duplicate account.
CREATE UNIQUE INDEX IF NOT EXISTS idx_users_username_lower
    ON users (lower(chesscom_username));
