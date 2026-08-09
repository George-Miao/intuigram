use std::collections::{BTreeMap, HashMap, VecDeque};
use std::env;
use std::future::{Future, poll_fn};
use std::io::{self, Write};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::num::NonZeroUsize;
use std::path::PathBuf;
use std::pin::Pin;
use std::task::Poll;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use futures_util::Stream;
#[cfg(test)]
use intuigram::Application;
use intuigram::{
    UpdateCommit, UpdateCommitter, bootstrap_sync_batch, decode_stored_message,
    encode_stored_message, store_cursor,
};
use intuigram_app::{
    AccountKey, AccountLifecycle, AccountView, AdapterEvent, App, AttachmentId, AttachmentKind,
    AttachmentView, Bootstrap, ChatId, ChatKind, ChatView, ConnectionState, DeliveryState,
    DownloadId, DownloadView, DraftView, Effect, FolderId, FolderOperation, FolderOperationResult,
    FolderView, HistoryView, InlineImage, Input, Intent, MediaCard, MediaKind, MediaPreviewView,
    MessageDetails, MessageDirection, MessageId, MessageView, PollOptionView, PollView,
    RichMediaItemId, RichMediaItemView, RichMediaLibraryKind, RichMediaUploadKind,
    ScheduledDeliveryView, ScheduledMessageId, ScheduledMessageView, ScheduledRequest,
    SelectionView, TextEntity, TranscriptAnchorView, Update,
};
use intuigram_config::{
    Config, ConfigLoader, Overrides, PlatformDefaults, ViewMode as ConfigViewMode,
};
use intuigram_store::{
    AccountCipher, AccountDatabase, AccountId, AccountOpen, AccountRecord, AccountStore,
    CachedAccount, GlobalDatabase, SessionMaterial, StoreLayout,
};
use intuigram_telegram::{
    ApplicationCredentials, AuthorizedUser, Client, CodeRequest, CodeSignIn, FolderRules,
    LiveUpdates, LoginCodeDelivery, LoginCodeDeliveryMethod, LoginCodeToken, MediaLibraryEntry,
    MediaLibraryKind, QrLogin, ScheduledDelivery, Session, UploadKind,
};
use intuigram_tui::{
    QrLoginAction, QrLoginUi, TerminalEvents, TerminalUi, UiEvent, ViewMode as TuiViewMode,
    ViewOptions as TuiViewOptions,
};
use snafu::{OptionExt, ResultExt, Snafu};

mod actor_session;
mod authorization;
mod backend;
mod cache;
mod cached_session;
mod cli;
mod configuration;
#[cfg(test)]
mod configuration_tests;
mod fixtures;
mod folder_arguments;
mod local_lock;
mod login;
mod maintenance;
mod media_arguments;
mod proxy;
mod runtime;
mod schedule_arguments;
mod startup;
mod types;
mod ui;

use actor_session::{ActorConnection, ActorSession, ConnectedActorSession};
use authorization::{authorize_new_account, resume_account};
use cache::cached_bootstrap;
use cached_session::{CachedSession, run_cached_account};
#[cfg(test)]
use cli::help_text;
use cli::{parse_arguments, print_help};
use configuration::{
    derived_random_id, mime_type_for_path, platform_defaults, prompt, resolve_telegram_credentials,
    store_session, telegram_session,
};
#[cfg(test)]
use fixtures::application_fixture;
use folder_arguments::{next_argument, parse_folder_maintenance};
use local_lock::{delete_local_lock_key, unlock_local_lock};
#[cfg(test)]
use login::{login_code_delivery_message, login_code_delivery_method_name, seconds_until_at};
use login::{
    request_code_with_migration, seconds_until, sign_in_with_delivered_code, sign_in_with_password,
    unix_timestamp,
};
use maintenance::{
    record_media, remove_local_account, run_folder_maintenance, run_logout, run_maintenance,
    run_rich_media_maintenance, run_scheduled_maintenance,
};
use media_arguments::parse_media_maintenance;
use proxy::telegram_route;
use runtime::{
    AccountSessionExit, AdapterBatch, AdapterEffect, ApplicationAdapterEvents, ApplicationEvents,
    ApplicationExit, ApplicationState, BackendOutput, DisconnectedApplication, enqueue_effect,
    run_application_state,
};
#[cfg(test)]
use runtime::{ApplicationBackend, run_application};
use schedule_arguments::parse_scheduled_maintenance;
use startup::run_async;
use types::*;
pub(super) use ui::main;
use ui::{ApplicationUi, error_lines};

const PRIMARY_DC_ID: i32 = 2;
const PRIMARY_DC_ENDPOINT: SocketAddr =
    SocketAddr::new(IpAddr::V4(Ipv4Addr::new(149, 154, 167, 41)), 443);
const EFFECT_CAPACITY: usize = 64;

#[derive(Debug, Snafu)]
enum Error {
    #[snafu(display("platform did not provide a {kind} directory"))]
    MissingPlatformDirectory { kind: &'static str },

    #[snafu(display("missing value after {argument}"))]
    MissingArgumentValue { argument: String },

    #[snafu(display("unknown argument {argument}"))]
    UnknownArgument { argument: String },

    #[snafu(display("command-line arguments are invalid"))]
    ParseArguments { source: clap::Error },

    #[snafu(display("{argument} requires a positive decimal Telegram user ID, got {value:?}"))]
    InvalidArgumentValue { argument: String, value: String },

    #[snafu(display("only one storage maintenance action may be requested"))]
    ConflictingMaintenance,

    #[snafu(display("--account and --add-account cannot be used together"))]
    ConflictingAccountSelection,

    #[snafu(display("Telegram Account {account} is not registered; use --add-account first"))]
    UnknownAccount { account: i64 },

    #[snafu(display("failed to load Intuigram configuration"))]
    LoadConfiguration { source: intuigram_config::Error },

    #[snafu(display("Telegram proxy configuration is invalid"))]
    ProxyConfiguration { source: compio_mtproto::ProxyError },

    #[snafu(display("Telegram connection test failed"))]
    ProxyConnectionTest { source: intuigram_telegram::Error },

    #[snafu(display("Telegram application ID must be a positive decimal integer"))]
    InvalidApplicationId,

    #[snafu(display("failed to read the hidden Telegram application hash"))]
    PromptApplicationHash { source: io::Error },

    #[snafu(display("failed to save first-run Telegram application credentials"))]
    SaveApplicationCredentials {
        source: intuigram_config::CredentialError,
    },

    #[snafu(display("failed to open Intuigram Account registry"))]
    OpenAccountRegistry {
        source: intuigram_store::GlobalError,
    },

    #[snafu(display("failed to read Intuigram Account registry"))]
    ReadAccountRegistry {
        source: intuigram_store::GlobalError,
    },

    #[snafu(display("failed to update Intuigram Account registry"))]
    UpdateAccountRegistry {
        source: intuigram_store::GlobalError,
    },

    #[snafu(display("failed to clear durable Account data"))]
    ClearAccountData {
        source: intuigram_store::LifecycleError,
    },

    #[snafu(display("failed to unlock Local Lock"))]
    LocalLock { source: local_lock::Error },

    #[snafu(display("failed to encrypt existing Account data for Local Lock"))]
    EnableLocalLock {
        source: intuigram_store::SecurityError,
    },

    #[snafu(display("failed to access Intuigram Account database"))]
    AccountDatabase { source: intuigram_store::Error },

    #[snafu(display("Account database requires recovery"))]
    AccountRecovery { source: crate::recovery::Error },

    #[snafu(display("failed to durably apply a Telegram update"))]
    CommitTelegramUpdate { source: intuigram::SyncError },

    #[snafu(display("Account {} has no saved MTProto session", account.get()))]
    MissingSession { account: AccountId },

    #[snafu(display("stored Telegram endpoint {endpoint:?} is invalid"))]
    InvalidEndpoint {
        endpoint: String,
        source: std::net::AddrParseError,
    },

    #[snafu(display("Telegram returned invalid user ID {value}"))]
    InvalidAccountId { value: i64 },

    #[snafu(display("Telegram did not advertise a direct endpoint for data center {dc_id}"))]
    MissingDataCenter { dc_id: i32 },

    #[snafu(display("failed to initialize the Compio runtime"))]
    Runtime { source: io::Error },

    #[snafu(display("failed to start the Telegram actor cluster"))]
    StartActorCluster { source: io::Error },

    #[snafu(display("failed to join the Telegram actor cluster"))]
    JoinActorCluster { source: io::Error },

    #[snafu(display("Telegram actor cluster is unavailable"))]
    TelegramActorUnavailable,

    #[snafu(display("Telegram actor worker stopped during startup"))]
    TelegramActorWorkerStopped,

    #[snafu(display("Telegram actor supervisor stopped during startup"))]
    TelegramActorSupervisorStopped,

    #[snafu(display("Telegram actor name {name:?} is already registered"))]
    TelegramActorNameTaken { name: String },

    #[snafu(display("Telegram actor startup result channel closed"))]
    TelegramActorStartupClosed,

    #[snafu(display("Telegram actor mailbox is full"))]
    TelegramActorMailboxFull,

    #[snafu(display("Telegram actor mailbox is closed"))]
    TelegramActorMailboxClosed,

    #[snafu(display("Telegram actor did not reply"))]
    TelegramActorNoReply,

    #[snafu(display("Telegram actor worker stopped before reporting its exit"))]
    TelegramActorExitClosed,

    #[snafu(display("Telegram event driver was cancelled"))]
    TelegramActorDriverCancelled,

    #[snafu(display("Telegram actor operation was cancelled for shutdown"))]
    TelegramActorCancelled,

    #[snafu(display("Telegram actor stopped before committing its returned update"))]
    TelegramActorCommitClosed,

    #[snafu(display("Telegram operation failed"))]
    Telegram { source: intuigram_telegram::Error },

    #[snafu(display("Telegram update stream stopped"))]
    TelegramUpdatesClosed,

    #[snafu(display("Message pin effect bypassed the typed Telegram update path"))]
    MisroutedPinEffect,

    #[snafu(display("native Clipboard Paste failed"))]
    Clipboard { source: rich_clipboard::Error },

    #[snafu(display("notification fallback failed"))]
    Notification { source: io::Error },

    #[snafu(display("failed to read attachment {}", path.display()))]
    ReadAttachment { path: PathBuf, source: io::Error },

    #[snafu(display("attachment {} reached Telegram before local preparation", path.display()))]
    UnpreparedAttachment { path: PathBuf },

    #[snafu(display("prepared attachment {attachment:?} disappeared before Telegram send"))]
    MissingPreparedAttachment { attachment: AttachmentId },

    #[snafu(display("rich media reached Telegram before local preparation"))]
    UnpreparedRichMedia,

    #[snafu(display("failed to run ffmpeg for {kind} recording"))]
    RecordMedia {
        kind: &'static str,
        source: io::Error,
    },

    #[snafu(display("ffmpeg {kind} recording exited with {status}"))]
    RecorderFailed {
        kind: &'static str,
        status: std::process::ExitStatus,
    },

    #[snafu(display("media library item {index} is unavailable"))]
    MediaIndexUnavailable { index: usize },

    #[snafu(display("selected media library item {item} is no longer available"))]
    MediaLibraryItemUnavailable { item: u64 },

    #[snafu(display("failed to save Telegram media to the download directory"))]
    SaveDownload {
        source: intuigram_media::DownloadError,
    },

    #[snafu(display("failed to access the redownloadable Media Cache"))]
    MediaCache { source: intuigram_media::CacheError },

    #[snafu(display("completed download {download_id} is no longer available"))]
    DownloadUnavailable { download_id: u64 },

    #[snafu(display("failed to read {field} from the terminal"))]
    Prompt {
        field: &'static str,
        source: io::Error,
    },

    #[snafu(display("{field} must not be empty"))]
    EmptyPrompt { field: &'static str },

    #[snafu(display("Telegram login was cancelled"))]
    LoginCancelled,

    #[snafu(display("pending adapter effect limit ({capacity}) was reached"))]
    EffectsFull { capacity: usize },

    #[snafu(display("failed to create a Telegram operation idempotency token"))]
    OperationId { source: getrandom::Error },

    #[snafu(display("terminal UI failed"))]
    Terminal { source: intuigram_tui::Error },
}

#[cfg(test)]
mod tests;

type Result<T, E = Error> = std::result::Result<T, E>;
