use std::path::PathBuf;

use clap::builder::Styles;
use clap::builder::styling::{AnsiColor, Effects};
use clap::{ColorChoice, Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(
    name = "intuigram",
    version,
    about = "A fluent Telegram terminal client",
    color = ColorChoice::Auto,
    styles = styles()
)]
pub(super) struct Cli {
    /// Override the platform configuration directory.
    #[arg(long, global = true, value_name = "PATH")]
    pub(super) config_dir: Option<PathBuf>,

    /// Override the platform data directory.
    #[arg(long, global = true, value_name = "PATH")]
    pub(super) data_dir: Option<PathBuf>,

    /// Override the platform cache directory.
    #[arg(long, global = true, value_name = "PATH")]
    pub(super) cache_dir: Option<PathBuf>,

    /// Override the platform Downloads directory.
    #[arg(long, global = true, value_name = "PATH")]
    pub(super) downloads_dir: Option<PathBuf>,

    /// Select a registered Telegram Account.
    #[arg(long, global = true, value_name = "ID")]
    pub(super) account: Option<String>,

    /// Test configured proxy order and direct fallback, then exit.
    #[arg(long, global = true)]
    pub(super) test_connection: bool,

    #[command(subcommand)]
    pub(super) command: Option<Command>,
}

#[derive(Debug, Subcommand)]
pub(super) enum Command {
    /// Start the terminal interface. This is the default command.
    Start,

    /// Manage registered Telegram Accounts.
    Account {
        #[command(subcommand)]
        command: AccountCommand,
    },

    /// Inspect or clear redownloadable media.
    Cache {
        #[command(subcommand)]
        command: CacheCommand,
    },

    /// Manage Telegram Chat Folders.
    Folder {
        #[command(subcommand)]
        command: FolderCommand,
    },

    /// Browse, record, or send rich media.
    Media {
        #[command(subcommand)]
        command: MediaCommand,
    },

    /// Manage Telegram-owned Scheduled Messages.
    Scheduled {
        #[command(subcommand)]
        command: ScheduledCommand,
    },
}

#[derive(Debug, Subcommand)]
pub(super) enum AccountCommand {
    /// Authorize and register another Telegram Account.
    Add,

    /// List registered Accounts without opening the TUI.
    List,

    /// Clear local records, authorization, and media after confirmation.
    ClearData,

    /// Remove local data; server authorization may remain active.
    Remove,

    /// Revoke Telegram authorization, then remove local Account data.
    Logout,
}

#[derive(Debug, Subcommand)]
pub(super) enum CacheCommand {
    /// Show cache usage and the configured limit.
    Usage,

    /// Clear only redownloadable media.
    Clear,
}

#[derive(Debug, Subcommand)]
pub(super) enum FolderCommand {
    /// Create a Folder from a comma-separated rule list.
    Create {
        #[arg(value_name = "TITLE")]
        title: String,

        #[arg(value_name = "RULES")]
        rules: String,
    },

    /// Rename a custom Folder.
    Rename {
        #[arg(value_name = "FOLDER")]
        folder: String,

        #[arg(value_name = "TITLE")]
        title: String,
    },

    /// Move a custom Folder to a zero-based position.
    Reorder {
        #[arg(value_name = "FOLDER")]
        folder: String,

        #[arg(value_name = "POSITION")]
        position: String,
    },

    /// Export a Folder share link.
    Share {
        #[arg(value_name = "FOLDER")]
        folder: String,
    },

    /// Delete a Folder without deleting its Chats.
    Delete {
        #[arg(value_name = "FOLDER")]
        folder: String,
    },

    /// Replace Folder inclusion and exclusion rules.
    Rules {
        #[arg(value_name = "FOLDER")]
        folder: String,

        #[arg(value_name = "RULES")]
        rules: String,
    },
}

#[derive(Debug, Subcommand)]
pub(super) enum MediaCommand {
    /// Browse stickers, GIFs, or custom emoji.
    Browse {
        #[arg(value_name = "KIND")]
        kind: String,

        #[arg(value_name = "QUERY", allow_hyphen_values = true)]
        query: String,
    },

    /// Send an item from a media-library query.
    Send {
        #[arg(value_name = "CHAT", allow_hyphen_values = true)]
        chat: String,

        #[arg(value_name = "KIND")]
        kind: String,

        #[arg(value_name = "INDEX")]
        index: String,

        #[arg(value_name = "QUERY", allow_hyphen_values = true)]
        query: String,
    },

    /// Send voice, video-note, Sticker, GIF, or custom-emoji media.
    File {
        #[arg(value_name = "CHAT", allow_hyphen_values = true)]
        chat: String,

        #[arg(value_name = "KIND")]
        kind: String,

        #[arg(value_name = "PATH", allow_hyphen_values = true)]
        path: String,
    },

    /// Record voice or a video note with ffmpeg, then send it.
    Record {
        #[arg(value_name = "CHAT", allow_hyphen_values = true)]
        chat: String,

        #[arg(value_name = "KIND")]
        kind: String,

        #[arg(value_name = "SECONDS")]
        seconds: String,

        #[arg(value_name = "DEVICE", allow_hyphen_values = true)]
        device: String,
    },

    /// Share a Telegram contact card.
    Contact {
        #[arg(value_name = "CHAT", allow_hyphen_values = true)]
        chat: String,

        #[arg(value_name = "PHONE")]
        phone: String,

        #[arg(value_name = "FIRST")]
        first_name: String,

        #[arg(value_name = "LAST", allow_hyphen_values = true)]
        last_name: String,
    },
}

#[derive(Debug, Subcommand)]
pub(super) enum ScheduledCommand {
    /// Schedule text at an offset time or when the recipient is online.
    Create {
        #[arg(value_name = "CHAT", allow_hyphen_values = true)]
        chat: String,

        #[arg(value_name = "DELIVERY")]
        delivery: String,

        #[arg(value_name = "TEXT", allow_hyphen_values = true)]
        text: String,
    },

    /// List Telegram-owned Scheduled Messages for a Chat.
    List {
        #[arg(value_name = "CHAT", allow_hyphen_values = true)]
        chat: String,
    },

    /// Replace a Scheduled Message's text.
    Edit {
        #[arg(value_name = "CHAT", allow_hyphen_values = true)]
        chat: String,

        #[arg(value_name = "MESSAGE")]
        message: String,

        #[arg(value_name = "TEXT", allow_hyphen_values = true)]
        text: String,
    },

    /// Change a Scheduled Message's delivery time.
    Reschedule {
        #[arg(value_name = "CHAT", allow_hyphen_values = true)]
        chat: String,

        #[arg(value_name = "MESSAGE")]
        message: String,

        #[arg(value_name = "DELIVERY")]
        delivery: String,
    },

    /// Delete a Scheduled Message after confirmation.
    Delete {
        #[arg(value_name = "CHAT", allow_hyphen_values = true)]
        chat: String,

        #[arg(value_name = "MESSAGE")]
        message: String,
    },

    /// Ask Telegram to send a Scheduled Message immediately.
    SendNow {
        #[arg(value_name = "CHAT", allow_hyphen_values = true)]
        chat: String,

        #[arg(value_name = "MESSAGE")]
        message: String,
    },
}

fn styles() -> Styles {
    Styles::styled()
        .header(AnsiColor::Cyan.on_default() | Effects::BOLD)
        .usage(AnsiColor::Green.on_default() | Effects::BOLD)
        .literal(AnsiColor::Cyan.on_default() | Effects::BOLD)
        .placeholder(AnsiColor::Yellow.on_default())
        .valid(AnsiColor::Green.on_default())
        .invalid(AnsiColor::Red.on_default() | Effects::BOLD)
        .error(AnsiColor::Red.on_default() | Effects::BOLD)
}
