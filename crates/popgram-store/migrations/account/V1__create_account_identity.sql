CREATE TABLE account_identity (
    singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
    telegram_user_id INTEGER NOT NULL UNIQUE CHECK (telegram_user_id > 0)
);
