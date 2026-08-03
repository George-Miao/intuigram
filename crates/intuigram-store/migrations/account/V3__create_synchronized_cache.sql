CREATE TABLE sync_state (
    scope TEXT PRIMARY KEY,
    pts INTEGER NOT NULL,
    qts INTEGER NOT NULL,
    date INTEGER NOT NULL,
    seq INTEGER NOT NULL
);

CREATE TABLE chats (
    chat_id INTEGER PRIMARY KEY,
    kind TEXT NOT NULL,
    title TEXT NOT NULL,
    preview TEXT NOT NULL,
    unread_count INTEGER NOT NULL CHECK (unread_count >= 0),
    pinned INTEGER NOT NULL CHECK (pinned IN (0, 1))
);

CREATE TABLE chat_folders (
    chat_id INTEGER NOT NULL REFERENCES chats(chat_id) ON DELETE CASCADE,
    folder_id INTEGER NOT NULL,
    position INTEGER NOT NULL CHECK (position >= 0),
    PRIMARY KEY (chat_id, folder_id)
);

CREATE TABLE folders (
    folder_id INTEGER PRIMARY KEY,
    title TEXT NOT NULL,
    unread_count INTEGER NOT NULL CHECK (unread_count >= 0),
    position INTEGER NOT NULL UNIQUE CHECK (position >= 0)
);

CREATE TABLE messages (
    chat_id INTEGER NOT NULL REFERENCES chats(chat_id) ON DELETE CASCADE,
    message_id INTEGER NOT NULL,
    sender TEXT NOT NULL,
    body TEXT NOT NULL,
    timestamp TEXT NOT NULL,
    direction TEXT NOT NULL,
    delivery TEXT NOT NULL,
    reply_to_message_id INTEGER,
    thread_root_message_id INTEGER,
    content_kind TEXT NOT NULL,
    metadata TEXT NOT NULL,
    PRIMARY KEY (chat_id, message_id)
);

CREATE INDEX messages_chronological ON messages(chat_id, message_id);

CREATE VIRTUAL TABLE message_search USING fts5(
    body,
    content='messages',
    content_rowid='rowid'
);

CREATE TRIGGER message_search_insert AFTER INSERT ON messages BEGIN
    INSERT INTO message_search(rowid, body) VALUES (new.rowid, new.body);
END;

CREATE TRIGGER message_search_delete AFTER DELETE ON messages BEGIN
    INSERT INTO message_search(message_search, rowid, body)
    VALUES ('delete', old.rowid, old.body);
END;

CREATE TRIGGER message_search_update AFTER UPDATE OF body ON messages BEGIN
    INSERT INTO message_search(message_search, rowid, body)
    VALUES ('delete', old.rowid, old.body);
    INSERT INTO message_search(rowid, body) VALUES (new.rowid, new.body);
END;

CREATE TABLE drafts (
    chat_id INTEGER NOT NULL,
    thread_root_message_id INTEGER NOT NULL DEFAULT 0,
    text TEXT NOT NULL,
    reply_to_message_id INTEGER,
    modified_at INTEGER NOT NULL,
    PRIMARY KEY (chat_id, thread_root_message_id)
);

CREATE TABLE draft_history (
    version_id INTEGER PRIMARY KEY AUTOINCREMENT,
    chat_id INTEGER NOT NULL,
    thread_root_message_id INTEGER NOT NULL DEFAULT 0,
    text TEXT NOT NULL,
    reply_to_message_id INTEGER,
    displaced_at INTEGER NOT NULL
);

CREATE INDEX draft_history_by_chat ON draft_history(
    chat_id,
    thread_root_message_id,
    displaced_at DESC
);

CREATE TABLE media_metadata (
    media_id TEXT PRIMARY KEY,
    chat_id INTEGER NOT NULL,
    message_id INTEGER NOT NULL,
    kind TEXT NOT NULL,
    file_name TEXT,
    mime_type TEXT,
    byte_size INTEGER,
    remote_reference BLOB NOT NULL,
    UNIQUE (chat_id, message_id, media_id)
);
