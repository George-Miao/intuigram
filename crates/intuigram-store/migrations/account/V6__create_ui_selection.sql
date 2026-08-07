CREATE TABLE ui_selection (
    singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
    folder_id INTEGER NOT NULL,
    chat_id INTEGER
);
