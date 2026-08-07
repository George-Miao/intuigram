CREATE TABLE transcript_anchors (
    chat_id INTEGER NOT NULL,
    thread_root_message_id INTEGER NOT NULL DEFAULT 0,
    anchor_message_id INTEGER NOT NULL,
    PRIMARY KEY (chat_id, thread_root_message_id)
);

INSERT INTO transcript_anchors(chat_id, thread_root_message_id, anchor_message_id)
SELECT chat_id, 0, anchor_message_id
FROM ui_selection
WHERE chat_id IS NOT NULL AND anchor_message_id IS NOT NULL;
