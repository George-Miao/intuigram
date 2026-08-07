ALTER TABLE accounts ADD COLUMN notification_identity TEXT NOT NULL DEFAULT '';

UPDATE accounts
SET notification_identity = 'telegram:' || telegram_user_id
WHERE notification_identity = '';
