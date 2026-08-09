use std::path::PathBuf;

use clap::{ArgAction, Parser};

#[derive(Debug, Parser)]
#[command(
    name = "intuigram",
    version,
    about = "A fluent Telegram terminal client",
    disable_help_flag = true
)]
pub(super) struct Cli {
    /// Override the platform configuration directory.
    #[arg(long, value_name = "PATH")]
    pub(super) config_dir: Option<PathBuf>,

    /// Override the platform data directory.
    #[arg(long, value_name = "PATH")]
    pub(super) data_dir: Option<PathBuf>,

    /// Override the platform cache directory.
    #[arg(long, value_name = "PATH")]
    pub(super) cache_dir: Option<PathBuf>,

    /// Override the platform Downloads directory.
    #[arg(long, value_name = "PATH")]
    pub(super) downloads_dir: Option<PathBuf>,

    /// Switch to a registered Telegram Account.
    #[arg(long, value_name = "ID", conflicts_with = "add_account")]
    pub(super) account: Option<String>,

    /// Authorize and add another Telegram Account.
    #[arg(long, conflicts_with = "account")]
    pub(super) add_account: bool,

    /// List registered Accounts without opening the TUI.
    #[arg(long)]
    pub(super) list_accounts: bool,

    /// Test configured proxy order and direct fallback.
    #[arg(long, conflicts_with = "maintenance")]
    pub(super) test_connection: bool,

    /// Print help.
    #[arg(short = 'h', long, action = ArgAction::SetTrue)]
    pub(super) help: bool,

    /// Show one Account's cache usage and configured limit.
    #[arg(long, value_name = "ID", group = "maintenance")]
    pub(super) media_cache_usage: Option<String>,

    /// Clear only redownloadable media for one Account.
    #[arg(long, value_name = "ID", group = "maintenance")]
    pub(super) clear_media_cache: Option<String>,

    /// Clear local records, authorization, and media after confirmation.
    #[arg(long, value_name = "ID", group = "maintenance")]
    pub(super) clear_account_data: Option<String>,

    /// Remove local data; server authorization may remain active.
    #[arg(long, value_name = "ID", group = "maintenance")]
    pub(super) remove_account: Option<String>,

    /// Revoke Telegram authorization, then remove local Account data.
    #[arg(long, value_name = "ID", group = "maintenance")]
    pub(super) logout: Option<String>,

    /// Create a Folder from a comma-separated rule list.
    #[arg(long, num_args = 3, value_names = ["ID", "TITLE", "RULES"], group = "maintenance")]
    pub(super) folder_create: Option<Vec<String>>,

    /// Rename a custom Folder.
    #[arg(long, num_args = 3, value_names = ["ID", "FOLDER", "TITLE"], group = "maintenance")]
    pub(super) folder_rename: Option<Vec<String>>,

    /// Move a custom Folder to a zero-based position.
    #[arg(long, num_args = 3, value_names = ["ID", "FOLDER", "POSITION"], group = "maintenance")]
    pub(super) folder_reorder: Option<Vec<String>>,

    /// Export a Folder share link.
    #[arg(long, num_args = 2, value_names = ["ID", "FOLDER"], group = "maintenance")]
    pub(super) folder_share: Option<Vec<String>>,

    /// Delete a Folder without deleting its Chats.
    #[arg(long, num_args = 2, value_names = ["ID", "FOLDER"], group = "maintenance")]
    pub(super) folder_delete: Option<Vec<String>>,

    /// Replace Folder inclusion and exclusion rules.
    #[arg(long, num_args = 3, value_names = ["ID", "FOLDER", "RULES"], group = "maintenance")]
    pub(super) folder_rules: Option<Vec<String>>,

    /// Browse stickers, GIFs, or custom emoji.
    #[arg(long, num_args = 3, value_names = ["ID", "KIND", "QUERY"], allow_hyphen_values = true, group = "maintenance")]
    pub(super) media_browse: Option<Vec<String>>,

    /// Send an item from a media-library query.
    #[arg(long, num_args = 5, value_names = ["ID", "CHAT", "KIND", "INDEX", "QUERY"], allow_hyphen_values = true, group = "maintenance")]
    pub(super) media_send: Option<Vec<String>>,

    /// Send voice, video-note, Sticker, GIF, or custom-emoji media.
    #[arg(long, num_args = 4, value_names = ["ID", "CHAT", "KIND", "PATH"], allow_hyphen_values = true, group = "maintenance")]
    pub(super) media_file: Option<Vec<String>>,

    /// Record voice or a video note with ffmpeg, then send it.
    #[arg(long, num_args = 5, value_names = ["ID", "CHAT", "KIND", "SECONDS", "DEVICE"], allow_hyphen_values = true, group = "maintenance")]
    pub(super) record_media: Option<Vec<String>>,

    /// Share a Telegram contact card.
    #[arg(long, num_args = 5, value_names = ["ID", "CHAT", "PHONE", "FIRST", "LAST"], allow_hyphen_values = true, group = "maintenance")]
    pub(super) send_contact: Option<Vec<String>>,

    /// Schedule text at an offset time or when the recipient is online.
    #[arg(long, num_args = 4, value_names = ["ID", "CHAT", "DELIVERY", "TEXT"], allow_hyphen_values = true, group = "maintenance")]
    pub(super) schedule_message: Option<Vec<String>>,

    /// List Telegram-owned Scheduled Messages for a Chat.
    #[arg(long, num_args = 2, value_names = ["ID", "CHAT"], allow_hyphen_values = true, group = "maintenance")]
    pub(super) scheduled_list: Option<Vec<String>>,

    /// Replace a Scheduled Message's text.
    #[arg(long, num_args = 4, value_names = ["ID", "CHAT", "MESSAGE", "TEXT"], allow_hyphen_values = true, group = "maintenance")]
    pub(super) scheduled_edit: Option<Vec<String>>,

    /// Change a Scheduled Message's delivery time.
    #[arg(long, num_args = 4, value_names = ["ID", "CHAT", "MESSAGE", "DELIVERY"], allow_hyphen_values = true, group = "maintenance")]
    pub(super) scheduled_reschedule: Option<Vec<String>>,

    /// Delete a Scheduled Message after confirmation.
    #[arg(long, num_args = 3, value_names = ["ID", "CHAT", "MESSAGE"], allow_hyphen_values = true, group = "maintenance")]
    pub(super) scheduled_delete: Option<Vec<String>>,

    /// Ask Telegram to send a Scheduled Message immediately.
    #[arg(long, num_args = 3, value_names = ["ID", "CHAT", "MESSAGE"], allow_hyphen_values = true, group = "maintenance")]
    pub(super) scheduled_send_now: Option<Vec<String>>,
}
