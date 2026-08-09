ALTER TABLE messages ADD COLUMN saved_peer_id INTEGER;

CREATE INDEX messages_by_saved_peer
ON messages(chat_id, saved_peer_id, message_id);

CREATE TABLE saved_dialogs (
    chat_id INTEGER NOT NULL REFERENCES chats(chat_id) ON DELETE CASCADE,
    saved_peer_id INTEGER NOT NULL,
    title TEXT NOT NULL,
    preview TEXT NOT NULL,
    timestamp TEXT NOT NULL,
    pinned INTEGER NOT NULL CHECK (pinned IN (0, 1)),
    top_message_id INTEGER NOT NULL,
    position INTEGER NOT NULL CHECK (position >= 0),
    PRIMARY KEY (chat_id, saved_peer_id),
    UNIQUE (chat_id, position)
);

CREATE TABLE transcript_anchors_v12 (
    chat_id INTEGER NOT NULL,
    thread_root_message_id INTEGER NOT NULL DEFAULT 0,
    saved_peer_id INTEGER NOT NULL DEFAULT 0,
    anchor_message_id INTEGER NOT NULL,
    PRIMARY KEY (chat_id, thread_root_message_id, saved_peer_id)
);

INSERT INTO transcript_anchors_v12(
    chat_id,
    thread_root_message_id,
    saved_peer_id,
    anchor_message_id
)
SELECT chat_id, thread_root_message_id, 0, anchor_message_id
FROM transcript_anchors;

DROP TABLE transcript_anchors;
ALTER TABLE transcript_anchors_v12 RENAME TO transcript_anchors;
