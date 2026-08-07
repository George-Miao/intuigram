# Intuigram

Intuigram is becoming a dense, configurable Telegram terminal client designed for
daily use. The current core PoC can authorize an Account, resume its stored
session, list Chats, load recent text history, and send text Messages and
replies through a Compio-native MTProto transport. Many daily-driver features
in the roadmap are not implemented yet.

The implemented workspace seams are:

- `intuigram-app`: a deterministic, asynchronous single-owner state loop;
- `intuigram-config`: layered TOML, YAML, JSON, environment, and explicit
  command-line configuration through Figment;
- `intuigram-store`: Refinery-migrated account databases operated by dedicated
  SQLite worker threads;
- `intuigram-tui`: context-sensitive input and Action Bar hints generated from one
  effective keymap;
- `compio-mtproto`: Compio `Framed` abridged transport, authorization-key
  exchange, and sequential encrypted RPC invocation;
- `compio-term`: an experimental terminal-event stream with Unix TTY and
  `SIGWINCH` readiness behind a platform-specific backend while Crossterm
  still supplies event decoding;
- `intuigram-telegram`: login, dialog/history requests, sending, and normalization
  from Telegram constructors to Intuigram-owned view data;
- `intuigram`: platform paths, onboarding, adapter composition, and the executable.

The accepted product scope and priorities live in [TODO.md](TODO.md). Domain
language and architectural decisions live in [CONTEXT.md](CONTEXT.md) and
[`docs/adr`](docs/adr), respectively.

## Filesystem layout

The executable will supply platform-native configuration, data, cache, and
download directories. Within them, Intuigram uses:

```text
<config>/intuigram/config.toml

<data>/intuigram/global.db
<data>/intuigram/.pending.db
<data>/intuigram/<telegram-user-id>.db

<cache>/intuigram/<telegram-user-id>/media/
<cache>/intuigram/<telegram-user-id>/thumbnails/
```

Configuration may also come from `config.yaml`, `config.yml`, `config.json`,
`INTUIGRAM_`-prefixed environment variables using `__` between nested keys, and
command-line overrides. Later sources take precedence. The media-cache default
is 2 GiB.

Inspect or explicitly clear one Account's local storage without starting the
TUI:

```sh
intuigram --media-cache-usage TELEGRAM_USER_ID
intuigram --clear-media-cache TELEGRAM_USER_ID
intuigram --clear-account-data TELEGRAM_USER_ID
intuigram --remove-account TELEGRAM_USER_ID
intuigram --logout TELEGRAM_USER_ID
```

Clearing media never deletes Chat metadata or Message text. Clearing Account
data requires typing the displayed Account-specific confirmation and names the
authorization, records, backups, and Media Cache that it removes.
`--logout` additionally requires a live Telegram acknowledgement before any
local deletion; an offline or rejected request leaves the Account intact.
`--remove-account` gives the exact local scope and warns that its server-side
authorization may still need termination from another Telegram client.

## Install and first run

Tagged releases provide archives for Linux x86-64, macOS Apple Silicon, and
Windows x86-64. From a source checkout, install with:

```sh
cargo install --locked --path crates/intuigram
```

Official Intuigram builds deliberately contain no shared Telegram application
credentials. Each user creates an application at `my.telegram.org`; on first
run Intuigram explains this policy, prompts for the ID and a hidden hash, and
saves them separately in an owner-protected `credentials.toml`. Existing
configuration files and Account databases are migrated in place; database
migrations create a backup and run integrity checks before normal startup.

For non-interactive setup, copy
[`config.example.toml`](config.example.toml) to the platform config directory as
`config.toml`, then replace its example credentials:

```toml
[telegram]
api_id = 123456
api_hash = "your-api-hash"
# phone_number = "+12025550123" # optional phone-login fallback

[view]
mode = "default" # use "compact" for the original dense layout
```

Environment variables are also accepted:

```sh
INTUIGRAM_TELEGRAM__API_ID=123456 \
INTUIGRAM_TELEGRAM__API_HASH=your-api-hash \
cargo run -p intuigram
```

On a new Account, Intuigram displays a QR code that can be scanned from Telegram
under **Settings → Devices → Link Desktop Device**. The code refreshes
automatically. Press `P` on that screen to use phone-number login instead;
Intuigram then prompts for the delivered code and, when enabled, a hidden 2FA
password.
Use `Ctrl+C` to quit and `?` for the context-sensitive key reference.

Add and switch among isolated Accounts from the launcher:

```sh
intuigram --add-account
intuigram --list-accounts
intuigram --account TELEGRAM_USER_ID
```

The active Account is remembered. Each Account reopens its own Folder, Chat,
Transcript position, Drafts, synchronized records, Media Cache, Local Lock key,
and displayed identity.

Custom Telegram Folders can be administered without opening the TUI. For
example, `intuigram --folder-create 123456 "Work" groups,contacts,exclude-muted`
creates a rule-based Folder for Account `123456`. The related
`--folder-rename`, `--folder-reorder`, `--folder-share`, `--folder-delete`, and
`--folder-rules` commands are listed by `intuigram --help`; the in-TUI Folder
membership picker provides explicit per-Chat inclusion and exclusion overrides.

Account files always use owner-only filesystem permissions. Optional Local Lock
also encrypts the complete Account database, including Telegram authorization
and synchronized Message text. Configure either a hidden passphrase prompt or
the native OS credential vault:

```toml
[local_lock]
enabled = true
unlock = "keyring" # or "passphrase"
```

Enabling it converts an existing Account and its retained backups before the
TUI opens. Redownloadable Media Cache bytes are outside Local Lock and can be
cleared independently.

## Development

Run the repository checks from the workspace root:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo nextest run --workspace --all-features
cargo test --workspace --doc --all-features
```
