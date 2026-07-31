CREATE TABLE mtproto_session (
    singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
    dc_id INTEGER NOT NULL CHECK (dc_id > 0),
    endpoint TEXT NOT NULL,
    auth_key BLOB NOT NULL CHECK (length(auth_key) = 256),
    time_offset INTEGER NOT NULL,
    first_salt INTEGER NOT NULL
);
