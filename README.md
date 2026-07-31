# Popgram

Popgram is becoming a dense, configurable Telegram terminal client designed for
daily use. The repository is currently in the foundational implementation
stage; it is not yet a usable Telegram client.

The implemented workspace seams are:

- `popgram-app`: a deterministic, asynchronous single-owner state loop;
- `popgram-config`: layered TOML, YAML, JSON, environment, and explicit
  command-line configuration through Figment;
- `popgram-store`: Refinery-migrated account databases operated by dedicated
  SQLite worker threads;
- `popgram-tui`: context-sensitive input and Action Bar hints generated from one
  effective keymap.

The accepted product scope and priorities live in [TODO.md](TODO.md). Domain
language and architectural decisions live in [CONTEXT.md](CONTEXT.md) and
[`docs/adr`](docs/adr), respectively.

## Filesystem layout

The executable will supply platform-native configuration, data, cache, and
download directories. Within them, Popgram uses:

```text
<config>/popgram/config.toml

<data>/popgram/global.db
<data>/popgram/.pending.db
<data>/popgram/<telegram-user-id>.db

<cache>/popgram/<telegram-user-id>/media/
<cache>/popgram/<telegram-user-id>/thumbnails/
```

Configuration may also come from `config.yaml`, `config.yml`, `config.json`,
`POPGRAM_`-prefixed environment variables using `__` between nested keys, and
command-line overrides. Later sources take precedence. The media-cache default
is 2 GiB.

## Development

Run the repository checks from the workspace root:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo nextest run --workspace --all-features
cargo test --workspace --doc --all-features
```
