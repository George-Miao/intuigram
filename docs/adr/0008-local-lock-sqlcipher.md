# Local Lock uses full Account-database encryption

Optional Local Lock encrypts each complete Account SQLite database with
SQLCipher before schema inspection, migration, or ordinary access. This covers
Telegram authorization material, synchronization cursors, Chat metadata,
Message text, Drafts, and other Local Records through the existing Account
isolation seam; `global.db` retains only the non-secret Account directory, and
redownloadable Media Cache bytes remain governed by cache lifecycle policy.
Existing plaintext Account databases and their retained migration/recovery
backups are converted through `sqlcipher_export`, validated under the selected
key, atomically installed, and only then have their plaintext migration
workspaces removed.

The unlock key is derived with PBKDF2-SHA-256 from a hidden passphrase or is a
random 256-bit value stored through the native OS credential vault. Initial
passphrases require confirmation, secrets and raw keys use zeroizing memory,
and diagnostics redact database keys. Keyring calls happen during startup
before Account storage or terminal interaction begins. This increases binary
size and migration complexity, but avoids partial field encryption that would
leak new columns or leave authorization material outside the protected
boundary.
