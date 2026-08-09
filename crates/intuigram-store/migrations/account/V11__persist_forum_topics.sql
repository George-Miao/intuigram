ALTER TABLE chats ADD COLUMN has_topics INTEGER NOT NULL DEFAULT 0
    CHECK (has_topics IN (0, 1));

CREATE TABLE topics (
    chat_id INTEGER NOT NULL REFERENCES chats(chat_id) ON DELETE CASCADE,
    topic_id INTEGER NOT NULL,
    title TEXT NOT NULL,
    preview TEXT NOT NULL,
    timestamp TEXT NOT NULL,
    unread_count INTEGER NOT NULL CHECK (unread_count >= 0),
    pinned INTEGER NOT NULL CHECK (pinned IN (0, 1)),
    closed INTEGER NOT NULL CHECK (closed IN (0, 1)),
    hidden INTEGER NOT NULL CHECK (hidden IN (0, 1)),
    icon_color INTEGER NOT NULL CHECK (icon_color >= 0),
    icon_emoji_id INTEGER,
    top_message_id INTEGER,
    draft_text TEXT,
    draft_reply_to_message_id INTEGER,
    position INTEGER NOT NULL CHECK (position >= 0),
    PRIMARY KEY (chat_id, topic_id),
    UNIQUE (chat_id, position)
);
