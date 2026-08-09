CREATE TABLE outbox (
    outbox_id INTEGER PRIMARY KEY AUTOINCREMENT,
    operation TEXT NOT NULL CHECK (operation IN ('create', 'send', 'mutation')),
    state TEXT NOT NULL CHECK (
        state IN ('ready', 'in_flight', 'deferred', 'failed', 'conflict', 'expired', 'cancelled')
    ),
    payload BLOB NOT NULL,
    admitted_at INTEGER NOT NULL,
    available_at INTEGER,
    expires_at INTEGER,
    attempts INTEGER NOT NULL DEFAULT 0 CHECK (attempts >= 0),
    last_error TEXT
);

CREATE INDEX outbox_fifo ON outbox(state, available_at, admitted_at, outbox_id);

CREATE TABLE outbox_media (
    outbox_id INTEGER NOT NULL REFERENCES outbox(outbox_id) ON DELETE CASCADE,
    position INTEGER NOT NULL CHECK (position >= 0),
    file_name TEXT NOT NULL,
    mime_type TEXT NOT NULL,
    bytes BLOB NOT NULL,
    sha256 BLOB NOT NULL CHECK (length(sha256) = 32),
    PRIMARY KEY (outbox_id, position)
);
