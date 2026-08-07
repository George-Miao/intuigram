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

## Running the PoC

Create an application at `my.telegram.org`, copy
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

The current PoC protects Account files with owner-only filesystem permissions,
but its Telegram authorization material and synchronized data are not encrypted
at rest. Local Lock encryption remains a future p-high feature.

## Development

Run the repository checks from the workspace root:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo nextest run --workspace --all-features
cargo test --workspace --doc --all-features
```
