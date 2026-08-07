ALTER TABLE chats ADD COLUMN can_pin_messages INTEGER NOT NULL DEFAULT 0
    CHECK (can_pin_messages IN (0, 1));

UPDATE chats SET can_pin_messages = 1
WHERE kind IN ('saved_messages', 'private', 'bot');

CREATE TABLE pinned_message_projection (
    chat_id INTEGER NOT NULL,
    message_id INTEGER NOT NULL,
    PRIMARY KEY (chat_id, message_id),
    FOREIGN KEY (chat_id, message_id)
        REFERENCES messages(chat_id, message_id) ON DELETE CASCADE
);

CREATE TABLE message_projections (
    chat_id INTEGER NOT NULL,
    message_id INTEGER NOT NULL,
    PRIMARY KEY (chat_id, message_id),
    FOREIGN KEY (chat_id, message_id)
        REFERENCES messages(chat_id, message_id) ON DELETE CASCADE
);
