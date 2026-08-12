# Intuigram

Telegram, at terminal speed.

Intuigram is a local-first Telegram client for people who want routine messaging to feel fast, focused, and native to the keyboard. It keeps Chats, Messages, Drafts, and navigation state close at hand, connects directly to Telegram, and presents the parts that matter in a dense terminal interface rather than a web wrapper.

![Intuigram showing a generated group Chat with realistic demo Messages and text avatars](docs/assets/intuigram.webp)

The screenshot comes from Intuigram's production Ratatui renderer. It uses generated demo conversations and text-avatar fallbacks; it contains no real Telegram data.

## Why Intuigram

- Move through Folders and Chats without reaching for a mouse.
- Keep the Composer ready while reading and replying to Messages.
- Resume from locally synchronized Chats, Drafts, and Transcript positions.
- See optimistic sends, Telegram acknowledgements, downloads, and reconnect state instead of waiting behind an opaque spinner.
- Use inline terminal graphics where supported while retaining useful text fallbacks everywhere.
- Keep multiple Accounts in isolated local databases.
- Protect authorization and synchronized Message text with optional SQLCipher Local Lock.
- Connect directly or through ordered SOCKS5, HTTP CONNECT, and MTProxy routes.
- Discover every important key from the context-sensitive Action Bar and `?` Help.

Intuigram talks to Telegram through a Compio-native MTProto stack. It does not embed TDLib.

## A keyboard-first hierarchy

Intuigram follows one predictable path:

```text
Chat list → Active Chat → Active Message
```

Moving in the Chat list changes the adjacent Transcript immediately. `Enter` opens the Active Chat with the Composer ready. `Alt+Up` moves from the Composer into Messages, `Alt+Down` returns toward the Composer, and `Esc` climbs back one level at a time. `Left` and `Right` switch Folders while the Chat list is active.

Useful defaults include:

| Key                   | Action                                         |
| --------------------- | ---------------------------------------------- |
| `Enter`               | Open a Chat or send the current Draft          |
| `Shift+Enter`         | Insert a line break                            |
| `Alt+Up` / `Alt+Down` | Move between the Composer and Messages         |
| `Ctrl+F`              | Search the Active Chat or the active Account   |
| `Alt+A`               | Open context actions                           |
| `?`                   | Show all keys available in the current context |
| `Ctrl+C`              | Quit cleanly                                   |

The Action Bar always reflects the current context, so a shortcut is shown only when its action is available.

## Current status

Intuigram is under active development. The current proof of concept can authorize and resume Telegram Accounts, synchronize Folders and Chats, load Message History, send text Messages and replies, manage Drafts, display common rich content with fallbacks, operate a durable Outbox, and perform Account, Folder, media, and Scheduled Message tasks through the CLI.

It is not yet a complete replacement for every Telegram client. Many features are outside the current product promise, and several performance and media-workflow items remain open. The live scope and priorities are tracked in [TODO.md](TODO.md).

## Install and start

### Release binary

Tagged releases provide archives for Linux x86-64, macOS Apple Silicon, and Windows x86-64.

### Compile from source

From a source checkout:

```sh
cargo install --locked --path crates/intuigram
intuigram
```

Running `intuigram` without a subcommand starts the TUI. `intuigram start` is the explicit equivalent.

Official Intuigram builds contain no shared Telegram application credentials. Create an application at [my.telegram.org](https://my.telegram.org), then start Intuigram. The first-run flow explains the policy, prompts for the application ID and hidden hash, and saves them separately in an owner-protected `credentials.toml`.

Intuigram then shows a QR code. In another Telegram client, open `Settings → Devices → Link Desktop Device` and scan it. Press `P` on the login screen to use phone-number login instead. Intuigram prompts for the delivered code and, when enabled, a hidden 2FA password.

## Configuration

Copy [`config.example.toml`](config.example.toml) to the platform configuration directory as `config.toml`, then replace the example credentials:

```toml
[telegram]
api_id = 123456
api_hash = "your-api-hash"
# phone_number = "+12025550123"

[view]
mode = "default"
message_max_width = 96
```

Intuigram loads `config.toml`, `config.yaml`, `config.yml`, and `config.json`, plus `INTUIGRAM_`-prefixed environment variables and command-line overrides. Later sources take precedence. Environment variables use `__` between nested keys:

```sh
INTUIGRAM_TELEGRAM__API_ID=123456 \
INTUIGRAM_TELEGRAM__API_HASH=your-api-hash \
intuigram
```

The example configuration documents connection routes, file logging, media-cache limits, view density, external path pickers, Local Lock, and platform-directory overrides. Intuigram appends diagnostics to `<data>/intuigram/intuigram.log` by default. Override the exact destination when needed:

```toml
[logging]
path = "/path/to/intuigram.log"
```

## Connections and proxies

Intuigram can try an ordered mix of SOCKS5, HTTP CONNECT, and MTProxy routes before an optional direct connection. SOCKS5 supports local or proxy-side target DNS and RFC 1929 authentication. HTTP CONNECT supports Basic authentication. MTProxy accepts bare abridged and `dd` padded-intermediate secrets. Intuigram redacts passwords and secrets from diagnostics.

Run a complete connection check without opening an Account:

```sh
intuigram --test-connection
```

Conventional proxy variables are supported in this order: `all_proxy`, `ALL_PROXY`, `https_proxy`, `HTTPS_PROXY`, `http_proxy`, then `HTTP_PROXY`. Use `socks5://` for local target DNS, `socks5h://` for proxy-side target DNS, and `http://` for HTTP CONNECT.

## Accounts and local data

Add, inspect, or open isolated Accounts:

```sh
intuigram account add
intuigram account list
intuigram --account TELEGRAM_USER_ID
```

Intuigram remembers the active Account. Each Account reopens its own Folder, Active Chat, Transcript position, Drafts, synchronized records, Media Cache, Local Lock key, and displayed identity.

Inspect or explicitly clear local data without starting the TUI:

```sh
intuigram --account TELEGRAM_USER_ID cache usage
intuigram --account TELEGRAM_USER_ID cache clear
intuigram --account TELEGRAM_USER_ID account clear-data
intuigram --account TELEGRAM_USER_ID account remove
intuigram --account TELEGRAM_USER_ID account logout
```

Clearing media never deletes Chat metadata or Message text. Destructive Account commands name the exact data they remove and require an Account-specific confirmation. `account logout` deletes local data only after Telegram acknowledges server-side revocation. `account remove` deletes locally and warns that the authorization may still need termination from another Telegram client.

## Local Lock

Account files always use owner-only filesystem permissions. Optional Local Lock encrypts the complete Account database, including Telegram authorization and synchronized Message text:

```toml
[local_lock]
enabled = true
unlock = "keyring" # or "passphrase"
```

Enabling Local Lock converts the Account database and retained backups before the TUI opens. Redownloadable Media Cache bytes remain outside Local Lock and can be cleared independently.

## Filesystem layout

Intuigram uses platform-native configuration, data, cache, and download directories:

```text
<config>/intuigram/config.toml
<config>/intuigram/credentials.toml

<data>/intuigram/global.db
<data>/intuigram/.pending.db
<data>/intuigram/<telegram-user-id>.db
<data>/intuigram/intuigram.log

<cache>/intuigram/<telegram-user-id>/media/
<cache>/intuigram/<telegram-user-id>/thumbnails/
```

Database migrations are versioned and transactional. Intuigram creates a backup and runs integrity checks before normal startup instead of silently replacing damaged Account data.

## More commands

Folder management is available through `intuigram folder --help`, including create, rename, reorder, share, delete, and rule operations. The TUI also provides explicit per-Chat Folder membership overrides.

Rich-media commands are listed under `intuigram media --help`. They cover recent stickers, saved GIFs, custom emoji, local files, contacts, and asynchronous voice or circular-video capture through `ffmpeg`.

Scheduled Messages remain owned by Telegram and survive Intuigram exiting. Use `intuigram scheduled --help` to create, list, edit, reschedule, delete, or send them immediately.

## Development

The product vocabulary lives in [CONTEXT.md](CONTEXT.md), architectural decisions live in [`docs/adr`](docs/adr), and implementation priorities live in [TODO.md](TODO.md).

Run the repository checks from the workspace root:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo nextest run --workspace --all-features
cargo test --workspace --doc --all-features
```
