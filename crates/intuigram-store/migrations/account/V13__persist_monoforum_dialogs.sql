ALTER TABLE chats ADD COLUMN has_direct_messages INTEGER NOT NULL DEFAULT 0
    CHECK (has_direct_messages IN (0, 1));

ALTER TABLE saved_dialogs ADD COLUMN unread_count INTEGER NOT NULL DEFAULT 0
    CHECK (unread_count >= 0);
ALTER TABLE saved_dialogs ADD COLUMN unread_mark INTEGER NOT NULL DEFAULT 0
    CHECK (unread_mark IN (0, 1));
ALTER TABLE saved_dialogs ADD COLUMN draft_text TEXT;
ALTER TABLE saved_dialogs ADD COLUMN draft_reply_to_message_id INTEGER;

CREATE TABLE drafts_v13 (
    chat_id INTEGER NOT NULL,
    thread_root_message_id INTEGER NOT NULL DEFAULT 0,
    saved_peer_id INTEGER NOT NULL DEFAULT 0,
    text TEXT NOT NULL,
    reply_to_message_id INTEGER,
    modified_at INTEGER NOT NULL,
    PRIMARY KEY (chat_id, thread_root_message_id, saved_peer_id)
);

INSERT INTO drafts_v13(
    chat_id,
    thread_root_message_id,
    saved_peer_id,
    text,
    reply_to_message_id,
    modified_at
)
SELECT chat_id, thread_root_message_id, 0, text, reply_to_message_id, modified_at
FROM drafts;

DROP TABLE drafts;
ALTER TABLE drafts_v13 RENAME TO drafts;

CREATE TABLE draft_history_v13 (
    version_id INTEGER PRIMARY KEY AUTOINCREMENT,
    chat_id INTEGER NOT NULL,
    thread_root_message_id INTEGER NOT NULL DEFAULT 0,
    saved_peer_id INTEGER NOT NULL DEFAULT 0,
    text TEXT NOT NULL,
    reply_to_message_id INTEGER,
    displaced_at INTEGER NOT NULL
);

INSERT INTO draft_history_v13(
    version_id,
    chat_id,
    thread_root_message_id,
    saved_peer_id,
    text,
    reply_to_message_id,
    displaced_at
)
SELECT version_id, chat_id, thread_root_message_id, 0, text, reply_to_message_id, displaced_at
FROM draft_history;

DROP TABLE draft_history;
ALTER TABLE draft_history_v13 RENAME TO draft_history;

CREATE INDEX draft_history_by_chat_v13 ON draft_history(
    chat_id,
    thread_root_message_id,
    saved_peer_id,
    displaced_at DESC
);
