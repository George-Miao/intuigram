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
use intuigram::{
    Application, UpdateCommit, UpdateCommitter, bootstrap_sync_batch, decode_stored_message,
    encode_stored_message, store_cursor,
};
use intuigram_app::{
    AdapterEvent, App, AttachmentId, AttachmentKind, AttachmentView, Bootstrap, ChatId, ChatKind,
    ChatView, ConnectionState, DeliveryState, DownloadId, DownloadView, DraftView, Effect,
    FolderView, HistoryView, InlineImage, Input, Intent, MediaCard, MediaKind, MediaPreviewView,
    MessageDetails, MessageDirection, MessageId, MessageView, PollOptionView, PollView, TextEntity,
    Update,
};
use intuigram_config::{
    Config, ConfigLoader, Overrides, PlatformDefaults, ViewMode as ConfigViewMode,
};
use intuigram_store::{
    AccountDatabase, AccountId, AccountOpen, AccountRecord, AccountStore, CachedAccount,
    GlobalDatabase, SessionMaterial, StoreLayout,
};
use intuigram_telegram::{
    ApplicationCredentials, AuthorizedUser, Client, CodeRequest, CodeSignIn, LiveUpdates,
    LoginCodeDelivery, LoginCodeDeliveryMethod, LoginCodeToken, QrLogin, Session,
};
use intuigram_tui::{
    QrLoginAction, QrLoginUi, TerminalEvents, TerminalUi, UiEvent, ViewMode as TuiViewMode,
};
use snafu::{OptionExt, ResultExt, Snafu};

mod authorization;
mod backend;
mod backend_download;
mod backend_effects;
mod backend_pins;
mod cache;
mod configuration;
mod fixtures;
mod history_failure;
mod login;
mod poll;
mod runtime_adapters;
mod runtime_loop;
mod runtime_types;
mod startup;
mod ui;

use authorization::{authorize_new_account, resume_account};
use backend::{MessageSend, OutgoingRecord};
use cache::cached_bootstrap;
use configuration::{
    derived_random_id, mime_type_for_path, parse_arguments, platform_defaults, print_help, prompt,
    store_session, telegram_credentials, telegram_session,
};
#[cfg(test)]
use fixtures::application_fixture;
use history_failure::history_failure_event;
#[cfg(test)]
use login::{login_code_delivery_message, login_code_delivery_method_name, seconds_until_at};
use login::{
    request_code_with_migration, seconds_until, sign_in_with_delivered_code, sign_in_with_password,
    unix_timestamp,
};
use poll::PollPersistence;
use runtime_adapters::{
    AdapterBatch, ApplicationAdapterEvents, ApplicationBackend, ApplicationEvents, BackendOutput,
};
use runtime_loop::{run_application, run_application_state};
use runtime_types::{
    AdapterEffect, ApplicationExit, ApplicationState, ApplicationWake, DisconnectedApplication,
    PendingEffect, connection_failure_reason, enqueue_effect, start_effect,
};
use startup::run_async;
pub(super) use ui::main;
use ui::{ApplicationUi, error_lines};

const PRIMARY_DC_ID: i32 = 2;
const PRIMARY_DC_ENDPOINT: SocketAddr =
    SocketAddr::new(IpAddr::V4(Ipv4Addr::new(149, 154, 167, 41)), 443);
const EFFECT_CAPACITY: usize = 64;

#[derive(Default)]
struct Arguments {
    config: Option<PathBuf>,
    data: Option<PathBuf>,
    cache: Option<PathBuf>,
    downloads: Option<PathBuf>,
    help: bool,
}

struct Backend {
    client: Box<Client>,
    _database: AccountDatabase,
    store: AccountStore,
    next_local_message_id: i64,
    attachments: AttachmentStore,
    downloads: intuigram_media::DownloadDirectory,
    downloaded: DownloadStore,
}

#[derive(Default)]
struct AttachmentStore {
    next_id: u64,
    payloads: HashMap<AttachmentId, AttachmentPayload>,
}

#[derive(Clone)]
enum AttachmentPayload {
    Image { mime_type: String, bytes: Vec<u8> },
    File { path: PathBuf, kind: AttachmentKind },
}

#[derive(Default)]
struct DownloadStore {
    next_id: u64,
    paths: HashMap<DownloadId, PathBuf>,
}

impl DownloadStore {
    fn register(&mut self, path: PathBuf) -> DownloadId {
        self.next_id = self.next_id.saturating_add(1);
        let id = DownloadId(self.next_id);
        self.paths.insert(id, path);
        id
    }

    fn merge(&mut self, mut other: Self) {
        self.next_id = self.next_id.max(other.next_id);
        self.paths.extend(other.paths.drain());
    }
}

impl AttachmentStore {
    fn register(&mut self, payload: AttachmentPayload) -> AttachmentId {
        self.next_id = self.next_id.saturating_add(1);
        let id = AttachmentId(self.next_id);
        self.payloads.insert(id, payload);
        id
    }

    fn merge(&mut self, mut other: Self) {
        self.next_id = self.next_id.max(other.next_id);
        self.payloads.extend(other.payloads.drain());
    }
}

struct BackendEvents {
    updates: LiveUpdates,
    committer: UpdateCommitter,
    pending: Option<UpdateCommit>,
    pending_events: VecDeque<AdapterEvent>,
    submitted_updates: VecDeque<intuigram_telegram::LiveEvent>,
}

enum QrAuthorization {
    Authorized(Box<(Client, Session, AuthorizedUser)>),
    PhoneLogin(Box<(Client, Session)>),
}

#[derive(Debug, Snafu)]
enum Error {
    #[snafu(display("platform did not provide a {kind} directory"))]
    MissingPlatformDirectory { kind: &'static str },

    #[snafu(display("missing value after {argument}"))]
    MissingArgumentValue { argument: String },

    #[snafu(display("unknown argument {argument}"))]
    UnknownArgument { argument: String },

    #[snafu(display("failed to load Intuigram configuration"))]
    LoadConfiguration { source: intuigram_config::Error },

    #[snafu(display("Telegram setting {setting} is required"))]
    MissingTelegramSetting { setting: &'static str },

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

    #[snafu(display("Telegram operation failed"))]
    Telegram { source: intuigram_telegram::Error },

    #[snafu(display("Telegram update stream stopped"))]
    TelegramUpdatesClosed,

    #[snafu(display("Message pin effect bypassed the typed Telegram update path"))]
    MisroutedPinEffect,

    #[snafu(display("native Clipboard Paste failed"))]
    Clipboard { source: rich_clipboard::Error },

    #[snafu(display("failed to read attachment {}", path.display()))]
    ReadAttachment { path: PathBuf, source: io::Error },

    #[snafu(display("failed to save Telegram media to the download directory"))]
    SaveDownload {
        source: intuigram_media::DownloadError,
    },

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
