ALTER TABLE chats ADD COLUMN position INTEGER CHECK (position >= 0);
