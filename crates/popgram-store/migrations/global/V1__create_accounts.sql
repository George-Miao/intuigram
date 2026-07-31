CREATE TABLE accounts (
    telegram_user_id INTEGER PRIMARY KEY CHECK (telegram_user_id > 0),
    display_name TEXT NOT NULL,
    active INTEGER NOT NULL DEFAULT 0 CHECK (active IN (0, 1)),
    last_used_at INTEGER NOT NULL DEFAULT 0
);

CREATE UNIQUE INDEX one_active_account ON accounts(active) WHERE active = 1;
