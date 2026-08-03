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
use intuigram_app::{
    AdapterEvent, App, AttachmentId, AttachmentKind, AttachmentView, Bootstrap, ChatId, ChatKind,
    ChatView, ConnectionState, DeliveryState, DraftView, Effect, FolderView, HistoryView, Input,
    MediaCard, MediaKind, MessageDetails, MessageDirection, MessageId, MessageView, ReactionView,
    TextEntity, TextEntityKind, Update,
};
use intuigram_config::{Config, ConfigLoader, Overrides, PlatformDefaults};
use intuigram_store::{
    AccountDatabase, AccountId, AccountRecord, AccountStore, CachedAccount, DatabaseRequest,
    GlobalDatabase, SessionMaterial, StoreLayout, StoredChat, StoredFolder, StoredMessage,
    SyncBatch, SyncCursor as StoreSyncCursor,
};
use intuigram_telegram::{
    ApplicationCredentials, AuthorizedUser, Client, CodeRequest, CodeSignIn, LiveEvent,
    LiveUpdates, LoginCodeDelivery, LoginCodeDeliveryMethod, LoginCodeToken, QrLogin, Session,
    UpdateCursor as TelegramCursor,
};
use intuigram_tui::{QrLoginAction, QrLoginUi, TerminalEvents, TerminalUi, UiEvent};
use serde::{Deserialize, Serialize};
use snafu::{OptionExt, ResultExt, Snafu};

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

    #[snafu(display("native Clipboard Paste failed"))]
    Clipboard { source: rich_clipboard::Error },

    #[snafu(display("failed to read attachment {}", path.display()))]
    ReadAttachment { path: PathBuf, source: io::Error },

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

type Result<T, E = Error> = std::result::Result<T, E>;

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
}

#[derive(Default)]
struct AttachmentStore {
    next_id: u64,
    payloads: HashMap<AttachmentId, AttachmentPayload>,
}

#[derive(Clone)]
enum AttachmentPayload {
    Image { mime_type: String, bytes: Vec<u8> },
    File(PathBuf),
}

#[derive(Default, Deserialize, Serialize)]
#[serde(default)]
struct StoredMessageMetadata {
    edited: bool,
    pinned: bool,
    forwarded_from: Option<String>,
    views: Option<u32>,
    forwards: Option<u32>,
    replies: Option<u32>,
    service: Option<String>,
    media: Option<StoredMediaMetadata>,
    reactions: Vec<StoredReaction>,
    entities: Vec<StoredEntity>,
}

#[derive(Deserialize, Serialize)]
struct StoredMediaMetadata {
    title: String,
    description: String,
    remote_id: Option<String>,
}

#[derive(Deserialize, Serialize)]
struct StoredReaction {
    label: String,
    count: u32,
    chosen: bool,
}

#[derive(Deserialize, Serialize)]
struct StoredEntity {
    offset: usize,
    length: usize,
    kind: String,
    value: Option<String>,
    document_id: Option<i64>,
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
    store: AccountStore,
    cursor: StoreSyncCursor,
    pending: Option<Pin<Box<DatabaseRequest<()>>>>,
    pending_event: Option<AdapterEvent>,
}

enum QrAuthorization {
    Authorized(Box<(Client, Session, AuthorizedUser)>),
    PhoneLogin(Box<(Client, Session)>),
}

impl Backend {
    fn attachment_store(&mut self) -> &mut AttachmentStore {
        &mut self.attachments
    }

    async fn read_clipboard(
        &mut self,
        chat: ChatId,
        thread_root: Option<MessageId>,
    ) -> Result<AdapterEvent> {
        let content = rich_clipboard::read().await.context(ClipboardSnafu)?;
        let (text, attachments) = match content {
            rich_clipboard::ClipboardContent::Text(text) => (Some(text), Vec::new()),
            rich_clipboard::ClipboardContent::Image { mime_type, bytes } => {
                let id = self
                    .attachment_store()
                    .register(AttachmentPayload::Image { mime_type, bytes });
                (
                    None,
                    vec![AttachmentView {
                        id,
                        kind: AttachmentKind::Photo,
                        name: "clipboard.png".to_owned(),
                    }],
                )
            }
            rich_clipboard::ClipboardContent::Files(paths) => {
                let attachments = paths
                    .into_iter()
                    .map(|path| {
                        let name = path.file_name().map_or_else(
                            || "attachment".to_owned(),
                            |name| name.to_string_lossy().into_owned(),
                        );
                        let id = self
                            .attachment_store()
                            .register(AttachmentPayload::File(path));
                        AttachmentView {
                            id,
                            kind: AttachmentKind::File,
                            name,
                        }
                    })
                    .collect();
                (None, attachments)
            }
        };
        Ok(AdapterEvent::ClipboardReady {
            chat,
            thread_root,
            text,
            attachments,
        })
    }

    async fn save_draft(
        &mut self,
        chat: ChatId,
        thread_root: Option<MessageId>,
        text: String,
        reply_to: Option<MessageId>,
    ) -> Result<()> {
        self.store
            .save_draft(intuigram_store::StoredDraft {
                chat_id: chat.0,
                thread_root: thread_root.map(|message| message.0),
                text,
                reply_to: reply_to.map(|message| message.0),
                modified_at: unix_timestamp(),
            })
            .context(AccountDatabaseSnafu)?
            .await
            .context(AccountDatabaseSnafu)
    }

    async fn load_chat(&mut self, chat: ChatId) -> Result<Vec<MessageView>> {
        let messages = self
            .client
            .history(chat, 100)
            .await
            .context(TelegramSnafu)?;
        self.store
            .save_messages(
                messages
                    .iter()
                    .map(|message| stored_message(chat, message))
                    .collect(),
            )
            .context(AccountDatabaseSnafu)?
            .await
            .context(AccountDatabaseSnafu)?;
        Ok(messages)
    }

    async fn load_thread(&mut self, chat: ChatId, root: MessageId) -> Result<Vec<MessageView>> {
        let messages = self
            .client
            .thread_history(chat, root, 100)
            .await
            .context(TelegramSnafu)?;
        self.store
            .save_messages(
                messages
                    .iter()
                    .map(|message| stored_message(chat, message))
                    .collect(),
            )
            .context(AccountDatabaseSnafu)?
            .await
            .context(AccountDatabaseSnafu)?;
        Ok(messages)
    }

    async fn persist_outgoing(
        &mut self,
        chat: ChatId,
        local_id: MessageId,
        text: &str,
        reply_to: Option<MessageId>,
        thread_root: Option<MessageId>,
        delivery: DeliveryState,
    ) -> Result<()> {
        self.store
            .save_messages(vec![StoredMessage {
                chat_id: chat.0,
                id: local_id.0,
                sender: "You".to_owned(),
                body: text.to_owned(),
                timestamp: "now".to_owned(),
                direction: "outgoing".to_owned(),
                delivery: match delivery {
                    DeliveryState::Pending => "pending",
                    DeliveryState::Sent => "sent",
                    DeliveryState::Read => "read",
                    DeliveryState::Failed => "failed",
                }
                .to_owned(),
                reply_to: reply_to.map(|message| message.0),
                thread_root: thread_root.map(|message| message.0),
                content_kind: "text".to_owned(),
                metadata: String::new(),
            }])
            .context(AccountDatabaseSnafu)?
            .await
            .context(AccountDatabaseSnafu)
    }

    async fn send_message(
        &mut self,
        chat: ChatId,
        text: String,
        reply_to: Option<MessageId>,
        thread_root: Option<MessageId>,
        attachment_ids: Vec<AttachmentId>,
        random_id: i64,
    ) -> Result<MessageView> {
        let message_id = {
            let Self {
                client,
                next_local_message_id,
                attachments,
                ..
            } = self;
            if attachment_ids.is_empty() {
                client
                    .send_text(chat, text.clone(), reply_to, thread_root, random_id)
                    .await
                    .context(TelegramSnafu)?;
            } else {
                let payloads = attachment_ids
                    .iter()
                    .filter_map(|id| {
                        attachments
                            .payloads
                            .get(id)
                            .cloned()
                            .map(|payload| (*id, payload))
                    })
                    .collect::<Vec<_>>();
                for (index, (_, payload)) in payloads.iter().enumerate() {
                    let upload = match payload {
                        AttachmentPayload::Image { mime_type, bytes } => {
                            intuigram_telegram::Upload {
                                name: "clipboard.png".to_owned(),
                                mime_type: mime_type.clone(),
                                bytes: bytes.clone(),
                                photo: true,
                            }
                        }
                        AttachmentPayload::File(path) => intuigram_telegram::Upload {
                            name: path.file_name().map_or_else(
                                || "attachment".to_owned(),
                                |name| name.to_string_lossy().into_owned(),
                            ),
                            mime_type: mime_type_for_path(path),
                            bytes: compio::fs::read(path)
                                .await
                                .context(ReadAttachmentSnafu { path: path.clone() })?,
                            photo: false,
                        },
                    };
                    client
                        .send_upload(
                            chat,
                            upload,
                            if index == 0 {
                                text.clone()
                            } else {
                                String::new()
                            },
                            reply_to,
                            thread_root,
                            intuigram_telegram::UploadIds {
                                file: derived_random_id(random_id, index, 0x4649_4c45),
                                message: derived_random_id(random_id, index, 0x4d45_5353),
                            },
                        )
                        .await
                        .context(TelegramSnafu)?;
                }
                for id in &attachment_ids {
                    attachments.payloads.remove(id);
                }
            }
            *next_local_message_id -= 1;
            *next_local_message_id
        };
        Ok(MessageView {
            id: MessageId(message_id),
            sender: "You".to_owned(),
            body: text,
            timestamp: "now".to_owned(),
            direction: MessageDirection::Outgoing,
            delivery: DeliveryState::Sent,
            reply_to,
            details: MessageDetails {
                thread_root,
                ..MessageDetails::default()
            },
        })
    }

    async fn execute(&mut self, effect: AdapterEffect) -> Result<Option<AdapterEvent>> {
        let AdapterEffect { effect, random_id } = effect;
        match effect {
            Effect::Quit | Effect::Reconnect => Ok(None),
            Effect::SetChatFolder {
                chat,
                folder,
                included,
            } => Ok(Some(
                match self.client.set_chat_folder(chat, folder, included).await {
                    Ok(()) => AdapterEvent::FolderMembershipChanged {
                        chat,
                        folder,
                        included,
                    },
                    Err(source) if source.is_connection_failure() => {
                        return Err(Error::Telegram { source });
                    }
                    Err(error) => AdapterEvent::OperationFailed(error.to_string()),
                },
            )),
            Effect::LoadChat { chat } => {
                let messages = self.load_chat(chat).await?;
                Ok(Some(AdapterEvent::ChatLoaded { chat, messages }))
            }
            Effect::LoadThread { chat, root } => {
                let messages = self.load_thread(chat, root).await?;
                Ok(Some(AdapterEvent::ThreadLoaded {
                    chat,
                    root,
                    messages,
                }))
            }
            Effect::ReadClipboard { chat, thread_root } => {
                self.read_clipboard(chat, thread_root).await.map(Some)
            }
            Effect::SaveDraft {
                chat,
                thread_root,
                text,
                reply_to,
            } => {
                self.save_draft(chat, thread_root, text, reply_to).await?;
                Ok(None)
            }
            Effect::SendMessage {
                chat,
                text,
                reply_to,
                thread_root,
                attachments,
                local_id,
            } => {
                self.persist_outgoing(
                    chat,
                    local_id,
                    &text,
                    reply_to,
                    thread_root,
                    DeliveryState::Pending,
                )
                .await?;
                self.save_draft(chat, thread_root, String::new(), None)
                    .await?;
                let result = self
                    .send_message(
                        chat,
                        text.clone(),
                        reply_to,
                        thread_root,
                        attachments,
                        random_id.expect("every queued send has an idempotency token"),
                    )
                    .await;
                let result = match result {
                    Err(Error::Telegram { source }) if source.is_connection_failure() => {
                        return Err(Error::Telegram { source });
                    }
                    result => result,
                };
                self.persist_outgoing(
                    chat,
                    local_id,
                    &text,
                    reply_to,
                    thread_root,
                    if result.is_ok() {
                        DeliveryState::Sent
                    } else {
                        DeliveryState::Failed
                    },
                )
                .await?;
                Ok(Some(match result {
                    Ok(_) => AdapterEvent::MessageAcknowledged { chat, local_id },
                    Err(error) => AdapterEvent::MessageFailed {
                        chat,
                        local_id,
                        thread_root,
                        text,
                        reason: error.to_string(),
                    },
                }))
            }
        }
    }
}

trait ApplicationUi {
    fn draw(&mut self, view: &intuigram_app::View) -> intuigram_tui::Result<()>;

    fn resolve_event(
        &self,
        view: &intuigram_app::View,
        event: crossterm::event::Event,
    ) -> Option<UiEvent>;
}

impl ApplicationUi for TerminalUi {
    fn draw(&mut self, view: &intuigram_app::View) -> intuigram_tui::Result<()> {
        Self::draw(self, view)
    }

    fn resolve_event(
        &self,
        view: &intuigram_app::View,
        event: crossterm::event::Event,
    ) -> Option<UiEvent> {
        Self::resolve_event(self, view, event)
    }
}

fn main() {
    if let Err(error) = run() {
        print_error_chain(&error);
        std::process::exit(1);
    }
}

fn print_error_chain(error: &(dyn std::error::Error + 'static)) {
    for (depth, line) in error_lines(error).into_iter().enumerate() {
        if depth == 0 {
            eprintln!("intuigram: {line}");
        } else {
            eprintln!("  caused by: {line}");
        }
    }
}

fn error_lines(error: &(dyn std::error::Error + 'static)) -> Vec<String> {
    std::iter::successors(Some(error), |error| error.source())
        .map(|error| {
            error
                .to_string()
                .lines()
                .map(str::trim)
                .filter(|line| !line.is_empty())
                .collect::<Vec<_>>()
                .join(" ")
        })
        .collect()
}

fn run() -> Result<()> {
    let arguments = parse_arguments(env::args().skip(1))?;
    if arguments.help {
        print_help();
        return Ok(());
    }
    let runtime = compio::runtime::Runtime::new().context(RuntimeSnafu)?;
    runtime.block_on(run_async(arguments))
}

async fn run_async(arguments: Arguments) -> Result<()> {
    let defaults = platform_defaults(arguments.config.clone())?;
    let config = ConfigLoader::new(defaults)
        .with_overrides(Overrides {
            data: arguments.data,
            cache: arguments.cache,
            downloads: arguments.downloads,
            media_cache_bytes: None,
        })
        .load()
        .context(LoadConfigurationSnafu)?;
    let layout = StoreLayout::new(config.paths.data.clone());
    let global = GlobalDatabase::open(&layout).context(OpenAccountRegistrySnafu)?;
    let mut terminal = TerminalUi::enter().context(TerminalSnafu)?;
    let mut events = TerminalEvents::new().context(TerminalSnafu)?;
    let accounts = global.accounts().context(ReadAccountRegistrySnafu)?;
    if let Some(account) = accounts.into_iter().find(|account| account.active) {
        let database = AccountDatabase::open(&layout, account.id).context(AccountDatabaseSnafu)?;
        let cached = database.cached_account().context(AccountDatabaseSnafu)?;
        drop(database);
        return run_cached_account(
            &mut terminal,
            &mut events,
            telegram_credentials(&config)?,
            layout,
            account.clone(),
            cached_bootstrap(account.display_name, cached),
        )
        .await;
    }
    let (backend, mut backend_events, bootstrap) =
        authorize_new_account(&telegram_credentials(&config)?, &config, &layout, &global).await?;
    run_application(
        &mut terminal,
        &mut events,
        &mut backend_events,
        backend,
        bootstrap,
    )
    .await
}

async fn run_cached_account<U, E>(
    terminal: &mut U,
    events: &mut E,
    credentials: ApplicationCredentials,
    layout: StoreLayout,
    account: AccountRecord,
    bootstrap: Bootstrap,
) -> Result<()>
where
    U: ApplicationUi,
    E: ApplicationEvents,
{
    let mut app = App::new();
    let mut update = app.transition(Input::Adapter(AdapterEvent::Bootstrap(bootstrap)));
    let mut pending_effects = VecDeque::with_capacity(EFFECT_CAPACITY);
    let mut retained_attachments = AttachmentStore::default();
    let mut attempt = Some(Box::pin(resume_account(
        credentials.clone(),
        &layout,
        &account,
    )));

    loop {
        terminal.draw(&update.view).context(TerminalSnafu)?;
        if let Some(effect) = update.effect.take() {
            match effect {
                Effect::Quit => return Ok(()),
                Effect::Reconnect if attempt.is_none() => {
                    attempt = Some(Box::pin(resume_account(
                        credentials.clone(),
                        &layout,
                        &account,
                    )));
                }
                Effect::Reconnect => {}
                effect => {
                    enqueue_effect::<Backend>(&mut pending_effects, &None, Some(effect))?;
                }
            }
        }

        enum Wake<T> {
            Terminal(T),
            Connected(Box<Result<(Backend, BackendEvents, Bootstrap)>>),
        }
        let wake = poll_fn(|cx| {
            if let Poll::Ready(event) = events.poll_next_event(cx) {
                return Poll::Ready(Wake::Terminal(event));
            }
            if let Some(connection) = &mut attempt
                && let Poll::Ready(result) = connection.as_mut().poll(cx)
            {
                return Poll::Ready(Wake::Connected(Box::new(result)));
            }
            Poll::Pending
        })
        .await;

        match wake {
            Wake::Terminal(event) => {
                let event = event.context(TerminalSnafu)?;
                let Some(event) = terminal.resolve_event(&update.view, event) else {
                    continue;
                };
                match event {
                    UiEvent::Redraw => {}
                    UiEvent::Intent(intent) => update = app.transition(Input::Intent(intent)),
                }
            }
            Wake::Connected(result) if result.is_ok() => {
                let Ok((mut backend, mut adapter_events, bootstrap)) = *result else {
                    unreachable!("successful connection result was checked")
                };
                backend
                    .attachments
                    .merge(std::mem::take(&mut retained_attachments));
                update =
                    app.transition(Input::Adapter(AdapterEvent::ConnectionRestored(bootstrap)));
                match run_application_state(
                    terminal,
                    events,
                    &mut adapter_events,
                    backend,
                    app,
                    update,
                    pending_effects,
                )
                .await?
                {
                    ApplicationExit::Quit => return Ok(()),
                    ApplicationExit::Disconnected(state) => {
                        let DisconnectedApplication {
                            app: disconnected_app,
                            backend: disconnected_backend,
                            pending_effects: disconnected_effects,
                        } = *state;
                        retained_attachments.merge(disconnected_backend.attachments);
                        app = disconnected_app;
                        pending_effects = disconnected_effects;
                        update = app.transition(Input::Adapter(AdapterEvent::ConnectionChanged(
                            ConnectionState::Connecting,
                        )));
                        attempt = Some(Box::pin(resume_account(
                            credentials.clone(),
                            &layout,
                            &account,
                        )));
                    }
                }
            }
            Wake::Connected(result) => {
                let Err(error) = *result else {
                    unreachable!("failed connection result was checked")
                };
                attempt = None;
                let reason = error_lines(&error).join(": ");
                update = app.transition(Input::Adapter(AdapterEvent::ConnectionFailed(reason)));
            }
        }
    }
}

trait ApplicationBackend: Sized + 'static {
    async fn execute(&mut self, effect: AdapterEffect) -> Result<Option<AdapterEvent>>;
}

impl ApplicationBackend for Backend {
    async fn execute(&mut self, effect: AdapterEffect) -> Result<Option<AdapterEvent>> {
        Self::execute(self, effect).await
    }
}

trait ApplicationEvents {
    fn poll_next_event(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<intuigram_tui::Result<crossterm::event::Event>>;
}

impl ApplicationEvents for TerminalEvents {
    fn poll_next_event(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<intuigram_tui::Result<crossterm::event::Event>> {
        Self::poll_next_event(self, cx)
    }
}

trait ApplicationAdapterEvents {
    fn poll_adapter_event(&mut self, cx: &mut std::task::Context<'_>)
    -> Poll<Result<AdapterEvent>>;
}

impl ApplicationAdapterEvents for BackendEvents {
    fn poll_adapter_event(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<Result<AdapterEvent>> {
        loop {
            if let Some(request) = &mut self.pending {
                match request.as_mut().poll(cx) {
                    Poll::Ready(Ok(())) => {
                        self.pending = None;
                        return Poll::Ready(Ok(self
                            .pending_event
                            .take()
                            .expect("a durable event accompanies every database request")));
                    }
                    Poll::Ready(Err(source)) => {
                        self.pending = None;
                        return Poll::Ready(Err(Error::AccountDatabase { source }));
                    }
                    Poll::Pending => return Poll::Pending,
                }
            }
            match Pin::new(&mut self.updates).poll_next(cx) {
                Poll::Ready(Some(Ok(event))) => {
                    apply_cursor_delta(&mut self.cursor, event.cursor);
                    let batch = sync_batch_for_event(self.cursor.clone(), &event);
                    let request = match self.store.commit_sync(batch) {
                        Ok(request) => request,
                        Err(source) => {
                            return Poll::Ready(Err(Error::AccountDatabase { source }));
                        }
                    };
                    self.pending_event = Some(event.event);
                    self.pending = Some(Box::pin(request));
                }
                Poll::Ready(Some(Err(source))) => {
                    return Poll::Ready(Err(Error::Telegram { source }));
                }
                Poll::Ready(None) => {
                    return Poll::Ready(Err(Error::TelegramUpdatesClosed));
                }
                Poll::Pending => return Poll::Pending,
            }
        }
    }
}

struct BackendCompletion<B> {
    backend: B,
    effect: AdapterEffect,
    result: Result<Option<AdapterEvent>>,
}

type PendingEffect<B> = Pin<Box<dyn Future<Output = BackendCompletion<B>>>>;

enum ApplicationWake<B> {
    Terminal(intuigram_tui::Result<crossterm::event::Event>),
    Adapter(Result<AdapterEvent>),
    Backend(BackendCompletion<B>),
}

struct DisconnectedApplication<B> {
    app: App,
    backend: B,
    pending_effects: VecDeque<AdapterEffect>,
}

enum ApplicationExit<B> {
    Quit,
    Disconnected(Box<DisconnectedApplication<B>>),
}

fn connection_failure_reason(error: &Error) -> Option<String> {
    match error {
        Error::Telegram { source } if source.is_connection_failure() => {
            Some(error_lines(error).join(": "))
        }
        Error::TelegramUpdatesClosed => Some(error.to_string()),
        _ => None,
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct AdapterEffect {
    effect: Effect,
    random_id: Option<i64>,
}

impl AdapterEffect {
    fn new(effect: Effect) -> Result<Self> {
        let random_id = if matches!(effect, Effect::SendMessage { .. }) {
            let mut bytes = [0_u8; 8];
            getrandom::fill(&mut bytes).context(OperationIdSnafu)?;
            Some(i64::from_le_bytes(bytes))
        } else {
            None
        };
        Ok(Self { effect, random_id })
    }
}

fn start_effect<B: ApplicationBackend>(mut backend: B, effect: AdapterEffect) -> PendingEffect<B> {
    Box::pin(async move {
        let retained = effect.clone();
        let result = backend.execute(effect).await;
        BackendCompletion {
            backend,
            effect: retained,
            result,
        }
    })
}

fn enqueue_effect<B>(
    pending: &mut VecDeque<AdapterEffect>,
    active: &Option<PendingEffect<B>>,
    effect: Option<Effect>,
) -> Result<bool> {
    let Some(effect) = effect else {
        return Ok(false);
    };
    if effect == Effect::Quit {
        return Ok(true);
    }
    if let Effect::SaveDraft {
        chat, thread_root, ..
    } = &effect
    {
        pending.retain(|pending| {
            !matches!(
                &pending.effect,
                Effect::SaveDraft {
                    chat: pending_chat,
                    thread_root: pending_thread,
                    ..
                } if pending_chat == chat && pending_thread == thread_root
            )
        });
    }
    if pending.len() + usize::from(active.is_some()) >= EFFECT_CAPACITY {
        return EffectsFullSnafu {
            capacity: EFFECT_CAPACITY,
        }
        .fail();
    }
    pending.push_back(AdapterEffect::new(effect)?);
    Ok(false)
}

async fn run_application<U, E, A, B>(
    terminal: &mut U,
    events: &mut E,
    adapter_events: &mut A,
    backend: B,
    bootstrap: Bootstrap,
) -> Result<()>
where
    U: ApplicationUi,
    E: ApplicationEvents,
    A: ApplicationAdapterEvents,
    B: ApplicationBackend,
{
    let mut app = App::new();
    let update = app.transition(Input::Adapter(AdapterEvent::Bootstrap(bootstrap)));
    match run_application_state(
        terminal,
        events,
        adapter_events,
        backend,
        app,
        update,
        VecDeque::with_capacity(EFFECT_CAPACITY),
    )
    .await?
    {
        ApplicationExit::Quit => Ok(()),
        ApplicationExit::Disconnected(_) => TelegramUpdatesClosedSnafu.fail(),
    }
}

async fn run_application_state<U, E, A, B>(
    terminal: &mut U,
    events: &mut E,
    adapter_events: &mut A,
    backend: B,
    mut app: App,
    mut update: Update,
    mut pending_effects: VecDeque<AdapterEffect>,
) -> Result<ApplicationExit<B>>
where
    U: ApplicationUi,
    E: ApplicationEvents,
    A: ApplicationAdapterEvents,
    B: ApplicationBackend,
{
    let mut backend = Some(backend);
    let mut active_effect: Option<PendingEffect<B>> = None;
    let mut disconnected = false;

    loop {
        terminal.draw(&update.view).context(TerminalSnafu)?;
        if enqueue_effect(&mut pending_effects, &active_effect, update.effect.take())? {
            return Ok(ApplicationExit::Quit);
        }

        if disconnected && active_effect.is_none() {
            return Ok(ApplicationExit::Disconnected(Box::new(
                DisconnectedApplication {
                    app,
                    backend: backend
                        .take()
                        .expect("completed effects return the disconnected backend"),
                    pending_effects,
                },
            )));
        }

        if !disconnected
            && active_effect.is_none()
            && let Some(effect) = pending_effects.pop_front()
        {
            let available = backend
                .take()
                .expect("backend is available whenever no effect owns it");
            active_effect = Some(start_effect(available, effect));
        }

        let wake = poll_fn(|cx| {
            if !disconnected && let Poll::Ready(event) = adapter_events.poll_adapter_event(cx) {
                return Poll::Ready(ApplicationWake::Adapter(event));
            }
            if let Poll::Ready(event) = events.poll_next_event(cx) {
                return Poll::Ready(ApplicationWake::Terminal(event));
            }
            if let Some(effect) = &mut active_effect
                && let Poll::Ready(completion) = effect.as_mut().poll(cx)
            {
                return Poll::Ready(ApplicationWake::Backend(completion));
            }
            Poll::Pending
        })
        .await;

        match wake {
            ApplicationWake::Terminal(event) => {
                let event = event.context(TerminalSnafu)?;
                let Some(event) = terminal.resolve_event(&update.view, event) else {
                    continue;
                };
                match event {
                    UiEvent::Redraw => {}
                    UiEvent::Intent(intent) => {
                        update = app.transition(Input::Intent(intent));
                    }
                }
            }
            ApplicationWake::Adapter(event) => match event {
                Ok(event) => update = app.transition(Input::Adapter(event)),
                Err(error) => {
                    let Some(reason) = connection_failure_reason(&error) else {
                        return Err(error);
                    };
                    disconnected = true;
                    update = app.transition(Input::Adapter(AdapterEvent::ConnectionFailed(reason)));
                }
            },
            ApplicationWake::Backend(completion) => {
                active_effect = None;
                backend = Some(completion.backend);
                match completion.result {
                    Ok(Some(event)) => update = app.transition(Input::Adapter(event)),
                    Ok(None) => {
                        update = Update {
                            view: app.view(),
                            effect: None,
                        };
                    }
                    Err(error) => {
                        let Some(reason) = connection_failure_reason(&error) else {
                            return Err(error);
                        };
                        pending_effects.push_front(completion.effect);
                        disconnected = true;
                        update =
                            app.transition(Input::Adapter(AdapterEvent::ConnectionFailed(reason)));
                    }
                }
            }
        }
    }
}

async fn resume_account(
    credentials: ApplicationCredentials,
    layout: &StoreLayout,
    account: &AccountRecord,
) -> Result<(Backend, BackendEvents, Bootstrap)> {
    let database = AccountDatabase::open(layout, account.id).context(AccountDatabaseSnafu)?;
    let cached = database.cached_account().context(AccountDatabaseSnafu)?;
    let stored =
        database
            .session()
            .context(AccountDatabaseSnafu)?
            .context(MissingSessionSnafu {
                account: account.id,
            })?;
    let session = telegram_session(&stored)?;
    let identity = AuthorizedUser {
        id: account.id.get(),
        display_name: account.display_name.clone(),
        username: None,
    };
    let mut client = Client::connect_existing(credentials, &session, identity)
        .await
        .context(TelegramSnafu)?;
    let mut bootstrap = client.bootstrap(100).await.context(TelegramSnafu)?;
    let cached = cached_bootstrap(account.display_name.clone(), cached);
    bootstrap.drafts = cached.drafts;
    bootstrap.histories = cached.histories;
    let cursor = store_cursor(
        client
            .synchronization_cursor()
            .await
            .context(TelegramSnafu)?,
    );
    database
        .commit_sync(bootstrap_sync_batch(&bootstrap, cursor.clone()))
        .context(AccountDatabaseSnafu)?;
    let store = database.store();
    let live_capacity = NonZeroUsize::new(EFFECT_CAPACITY)
        .expect("the constant MTProto request capacity is positive");
    let (client, updates) = client.into_live(live_capacity);
    Ok((
        Backend {
            client: Box::new(client),
            _database: database,
            store: store.clone(),
            next_local_message_id: 0,
            attachments: AttachmentStore::default(),
        },
        BackendEvents {
            updates,
            store,
            cursor,
            pending: None,
            pending_event: None,
        },
        bootstrap,
    ))
}

async fn authorize_new_account(
    credentials: &ApplicationCredentials,
    config: &Config,
    layout: &StoreLayout,
    global: &GlobalDatabase,
) -> Result<(Backend, BackendEvents, Bootstrap)> {
    let pending = AccountDatabase::begin_login(layout).context(AccountDatabaseSnafu)?;
    let (client, session) = if let Some(stored) = pending.session().context(AccountDatabaseSnafu)? {
        let session = telegram_session(&stored)?;
        match Client::connect_pending(credentials.clone(), &session).await {
            Ok(client) => (client, session),
            Err(error) if error.is_test_data_center() => {
                let connected =
                    Client::connect_new(PRIMARY_DC_ID, PRIMARY_DC_ENDPOINT, credentials.clone())
                        .await
                        .context(TelegramSnafu)?;
                pending
                    .save_session(store_session(&connected.1))
                    .context(AccountDatabaseSnafu)?;
                connected
            }
            Err(source) => return Err(Error::Telegram { source }),
        }
    } else {
        let (client, session) =
            Client::connect_new(PRIMARY_DC_ID, PRIMARY_DC_ENDPOINT, credentials.clone())
                .await
                .context(TelegramSnafu)?;
        pending
            .save_session(store_session(&session))
            .context(AccountDatabaseSnafu)?;
        (client, session)
    };
    let (mut client, session, user) =
        match authorize_with_qr(credentials, &pending, client, session).await? {
            QrAuthorization::Authorized(authorized) => *authorized,
            QrAuthorization::PhoneLogin(login) => {
                let (client, session) = *login;
                let phone_number = match config.telegram.phone_number.as_deref() {
                    Some(number) => number.to_owned(),
                    None => prompt("Phone number", "phone number")?,
                };
                let (mut client, session, code_request) = request_code_with_migration(
                    credentials,
                    &pending,
                    client,
                    session,
                    &phone_number,
                )
                .await?;
                let user = match code_request {
                    CodeRequest::AlreadyAuthorized(user) => user,
                    CodeRequest::Sent(token) => {
                        sign_in_with_delivered_code(&mut client, token).await?
                    }
                };
                (client, session, user)
            }
        };
    let account_id = AccountId::new(user.id).context(InvalidAccountIdSnafu { value: user.id })?;
    pending
        .save_session(store_session(&session))
        .context(AccountDatabaseSnafu)?;
    let database = pending
        .finish_login(layout, account_id)
        .context(AccountDatabaseSnafu)?;
    global
        .register(AccountRecord {
            id: account_id,
            display_name: user.display_name.clone(),
            active: true,
        })
        .context(UpdateAccountRegistrySnafu)?;
    let bootstrap = client.bootstrap(100).await.context(TelegramSnafu)?;
    let cursor = store_cursor(
        client
            .synchronization_cursor()
            .await
            .context(TelegramSnafu)?,
    );
    database
        .commit_sync(bootstrap_sync_batch(&bootstrap, cursor.clone()))
        .context(AccountDatabaseSnafu)?;
    let store = database.store();
    let live_capacity = NonZeroUsize::new(EFFECT_CAPACITY)
        .expect("the constant MTProto request capacity is positive");
    let (client, updates) = client.into_live(live_capacity);
    Ok((
        Backend {
            client: Box::new(client),
            _database: database,
            store: store.clone(),
            next_local_message_id: 0,
            attachments: AttachmentStore::default(),
        },
        BackendEvents {
            updates,
            store,
            cursor,
            pending: None,
            pending_event: None,
        },
        bootstrap,
    ))
}

async fn authorize_with_qr(
    credentials: &ApplicationCredentials,
    pending: &AccountDatabase,
    mut client: Client,
    mut session: Session,
) -> Result<QrAuthorization> {
    let mut terminal = QrLoginUi::enter().context(TerminalSnafu)?;
    let mut state = client.export_qr_login().await.context(TelegramSnafu)?;
    loop {
        match state {
            QrLogin::Pending(token) => loop {
                let expires_in = seconds_until(token.expires_at(), session.time_offset);
                if expires_in == 0 {
                    state = client.export_qr_login().await.context(TelegramSnafu)?;
                    break;
                }
                terminal
                    .draw(token.uri(), expires_in)
                    .context(TerminalSnafu)?;
                match terminal
                    .poll_action(Duration::ZERO)
                    .context(TerminalSnafu)?
                {
                    QrLoginAction::PhoneLogin => {
                        return Ok(QrAuthorization::PhoneLogin(Box::new((client, session))));
                    }
                    QrLoginAction::Cancel => return LoginCancelledSnafu.fail(),
                    QrLoginAction::None | QrLoginAction::Redraw => {}
                }
                if client.poll_qr_login().await.context(TelegramSnafu)? {
                    state = client.export_qr_login().await.context(TelegramSnafu)?;
                    break;
                }
            },
            QrLogin::Migrate(migration) => {
                let dc_id = migration.dc_id();
                let endpoint = client
                    .data_center_endpoint(dc_id)
                    .context(MissingDataCenterSnafu { dc_id })?;
                (client, session) = Client::connect_new(dc_id, endpoint, credentials.clone())
                    .await
                    .context(TelegramSnafu)?;
                pending
                    .save_session(store_session(&session))
                    .context(AccountDatabaseSnafu)?;
                state = client
                    .import_qr_login(migration)
                    .await
                    .context(TelegramSnafu)?;
            }
            QrLogin::PasswordRequired(password) => {
                drop(terminal);
                let user = sign_in_with_password(&mut client, password).await?;
                return Ok(QrAuthorization::Authorized(Box::new((
                    client, session, user,
                ))));
            }
            QrLogin::Authorized(user) => {
                return Ok(QrAuthorization::Authorized(Box::new((
                    client, session, user,
                ))));
            }
        }
    }
}

fn seconds_until(expires_at: i32, server_time_offset: i32) -> u64 {
    let local_now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs());
    let local_now = i64::try_from(local_now).unwrap_or(i64::MAX);
    seconds_until_at(expires_at, local_now, server_time_offset)
}

fn unix_timestamp() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| {
            i64::try_from(duration.as_secs()).unwrap_or(i64::MAX)
        })
}

fn seconds_until_at(expires_at: i32, local_now: i64, server_time_offset: i32) -> u64 {
    let server_now = local_now.saturating_add(i64::from(server_time_offset));
    u64::try_from(i64::from(expires_at).saturating_sub(server_now)).unwrap_or(0)
}

async fn sign_in_with_delivered_code(
    client: &mut Client,
    mut token: LoginCodeToken,
) -> Result<AuthorizedUser> {
    loop {
        print_login_code_delivery(&token);
        let code = prompt("Login code (or 'resend')", "login code")?;
        if code.eq_ignore_ascii_case("resend") {
            match client
                .resend_login_code(&token)
                .await
                .context(TelegramSnafu)?
            {
                CodeRequest::Sent(next_token) => {
                    token = next_token;
                    continue;
                }
                CodeRequest::AlreadyAuthorized(user) => return Ok(user),
            }
        }
        return match client
            .sign_in_with_code(token, code)
            .await
            .context(TelegramSnafu)?
        {
            CodeSignIn::Authorized(user) => Ok(user),
            CodeSignIn::PasswordRequired(password) => sign_in_with_password(client, password).await,
        };
    }
}

async fn sign_in_with_password(
    client: &mut Client,
    prompt: intuigram_telegram::PasswordPrompt,
) -> Result<AuthorizedUser> {
    if let Some(hint) = prompt.hint {
        println!("2FA password hint: {hint}");
    }
    let password = rpassword::prompt_password("2FA password: ").context(PromptSnafu {
        field: "2FA password",
    })?;
    if password.is_empty() {
        return EmptyPromptSnafu {
            field: "2FA password",
        }
        .fail();
    }
    client
        .sign_in_with_password(password.as_bytes())
        .await
        .context(TelegramSnafu)
}

fn print_login_code_delivery(token: &LoginCodeToken) {
    println!("{}", login_code_delivery_message(token.delivery()));
    if let Some(next) = token.next_delivery() {
        let next = login_code_delivery_method_name(next);
        match token.next_delivery_after() {
            Some(seconds) => println!(
                "If it does not arrive, type 'resend' after {seconds} seconds to request {next}."
            ),
            None => println!("If it does not arrive, type 'resend' to request {next}."),
        }
    }
}

fn login_code_delivery_message(delivery: &LoginCodeDelivery) -> String {
    match delivery {
        LoginCodeDelivery::TelegramApp { length } => format!(
            "Telegram sent a {length}-digit code to the Telegram app on another logged-in device."
        ),
        LoginCodeDelivery::Sms { length } => {
            format!("Telegram sent a {length}-digit code by SMS.")
        }
        LoginCodeDelivery::PhoneCall { length } => {
            format!("Telegram will deliver a {length}-digit code by phone call.")
        }
        LoginCodeDelivery::FlashCall { pattern } => format!(
            "Telegram will place a call; use the caller number matching {pattern} as the code."
        ),
        LoginCodeDelivery::MissedCall { prefix, length } => format!(
            "Telegram will place a missed call from a number beginning with {prefix}; use its \
             last {length} digits."
        ),
        LoginCodeDelivery::Email { pattern, length } => {
            format!("Telegram sent a {length}-digit code to {pattern}.")
        }
        LoginCodeDelivery::EmailSetupRequired => {
            "Telegram requires a recovery email to be configured in an official client.".to_owned()
        }
        LoginCodeDelivery::Fragment { length, .. } => {
            format!("Telegram provided a Fragment flow for a {length}-digit code.")
        }
        LoginCodeDelivery::FirebaseSms { length } => {
            format!("Telegram sent a {length}-digit code by Firebase SMS.")
        }
        LoginCodeDelivery::SmsWord { beginning } => match beginning {
            Some(beginning) => {
                format!("Telegram sent an SMS containing a word beginning with {beginning}.")
            }
            None => "Telegram sent an SMS containing a login word.".to_owned(),
        },
        LoginCodeDelivery::SmsPhrase { beginning } => match beginning {
            Some(beginning) => {
                format!("Telegram sent an SMS containing a phrase beginning with {beginning}.")
            }
            None => "Telegram sent an SMS containing a login phrase.".to_owned(),
        },
    }
}

const fn login_code_delivery_method_name(method: LoginCodeDeliveryMethod) -> &'static str {
    match method {
        LoginCodeDeliveryMethod::Sms => "SMS delivery",
        LoginCodeDeliveryMethod::PhoneCall => "a phone call",
        LoginCodeDeliveryMethod::FlashCall => "a caller-number code",
        LoginCodeDeliveryMethod::MissedCall => "a missed-call code",
        LoginCodeDeliveryMethod::Fragment => "Fragment delivery",
    }
}

async fn request_code_with_migration(
    credentials: &ApplicationCredentials,
    pending: &AccountDatabase,
    mut client: Client,
    mut session: Session,
    phone_number: &str,
) -> Result<(Client, Session, CodeRequest)> {
    loop {
        match client.request_login_code(phone_number.to_owned()).await {
            Ok(request) => return Ok((client, session, request)),
            Err(error) => {
                let Some(dc_id) = error.phone_migration_dc() else {
                    return Err(Error::Telegram { source: error });
                };
                let endpoint = client
                    .data_center_endpoint(dc_id)
                    .context(MissingDataCenterSnafu { dc_id })?;
                let connected = Client::connect_new(dc_id, endpoint, credentials.clone())
                    .await
                    .context(TelegramSnafu)?;
                client = connected.0;
                session = connected.1;
                pending
                    .save_session(store_session(&session))
                    .context(AccountDatabaseSnafu)?;
            }
        }
    }
}

fn telegram_credentials(config: &Config) -> Result<ApplicationCredentials> {
    let api_id = config
        .telegram
        .api_id
        .context(MissingTelegramSettingSnafu {
            setting: "telegram.api_id",
        })?;
    let api_hash = config
        .telegram
        .api_hash
        .as_ref()
        .context(MissingTelegramSettingSnafu {
            setting: "telegram.api_hash",
        })?;
    Ok(ApplicationCredentials::new(api_id, api_hash.expose()))
}

fn store_session(session: &Session) -> SessionMaterial {
    SessionMaterial::new(
        session.dc_id,
        session.endpoint.to_string(),
        session.auth_key(),
        session.time_offset,
        session.first_salt,
    )
}

fn telegram_session(session: &SessionMaterial) -> Result<Session> {
    let endpoint = session.endpoint.parse().context(InvalidEndpointSnafu {
        endpoint: session.endpoint.clone(),
    })?;
    Ok(Session::new(
        session.dc_id,
        endpoint,
        session.auth_key(),
        session.time_offset,
        session.first_salt,
    ))
}

fn prompt(label: &str, field: &'static str) -> Result<String> {
    print!("{label}: ");
    io::stdout().flush().context(PromptSnafu { field })?;
    let mut value = String::new();
    io::stdin()
        .read_line(&mut value)
        .context(PromptSnafu { field })?;
    let value = value.trim().to_owned();
    if value.is_empty() {
        return EmptyPromptSnafu { field }.fail();
    }
    Ok(value)
}

fn parse_arguments(arguments: impl IntoIterator<Item = String>) -> Result<Arguments> {
    let mut parsed = Arguments::default();
    let mut arguments = arguments.into_iter();
    while let Some(argument) = arguments.next() {
        let destination = match argument.as_str() {
            "-h" | "--help" => {
                parsed.help = true;
                continue;
            }
            "--config-dir" => &mut parsed.config,
            "--data-dir" => &mut parsed.data,
            "--cache-dir" => &mut parsed.cache,
            "--downloads-dir" => &mut parsed.downloads,
            _ => return UnknownArgumentSnafu { argument }.fail(),
        };
        *destination = Some(
            arguments
                .next()
                .ok_or_else(|| Error::MissingArgumentValue {
                    argument: argument.clone(),
                })?
                .into(),
        );
    }
    Ok(parsed)
}

fn platform_defaults(config_override: Option<PathBuf>) -> Result<PlatformDefaults> {
    let config = match config_override {
        Some(path) => path,
        None => dirs::config_dir()
            .context(MissingPlatformDirectorySnafu {
                kind: "configuration",
            })?
            .join("intuigram"),
    };
    let data = dirs::data_dir()
        .context(MissingPlatformDirectorySnafu { kind: "data" })?
        .join("intuigram");
    let cache = dirs::cache_dir()
        .context(MissingPlatformDirectorySnafu { kind: "cache" })?
        .join("intuigram");
    let downloads =
        dirs::download_dir().context(MissingPlatformDirectorySnafu { kind: "downloads" })?;
    Ok(PlatformDefaults {
        config,
        data,
        cache,
        downloads,
    })
}

fn print_help() {
    println!(
        "Intuigram terminal client\n\n\
         Usage: intuigram [OPTIONS]\n\n\
         Options:\n\
           --config-dir PATH       Override the platform config directory\n\
           --data-dir PATH         Override the platform data directory\n\
           --cache-dir PATH        Override the platform cache directory\n\
           --downloads-dir PATH    Override the platform Downloads directory\n\
           -h, --help              Print this help\n\n\
         Configure telegram.api_id and telegram.api_hash in config.toml, YAML, JSON, or the\n\
         INTUIGRAM_TELEGRAM__API_ID and INTUIGRAM_TELEGRAM__API_HASH environment variables."
    );
}

fn mime_type_for_path(path: &std::path::Path) -> String {
    match path
        .extension()
        .and_then(std::ffi::OsStr::to_str)
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("gif") => "image/gif",
        Some("jpeg" | "jpg") => "image/jpeg",
        Some("png") => "image/png",
        Some("webp") => "image/webp",
        Some("mp4") => "video/mp4",
        Some("mov") => "video/quicktime",
        Some("mp3") => "audio/mpeg",
        Some("ogg") => "audio/ogg",
        Some("pdf") => "application/pdf",
        Some("txt" | "md") => "text/plain",
        _ => "application/octet-stream",
    }
    .to_owned()
}

fn derived_random_id(base: i64, index: usize, domain: u64) -> i64 {
    let index = u64::try_from(index).unwrap_or(u64::MAX);
    let mut value = (base as u64) ^ domain ^ index.wrapping_mul(0x9e37_79b9_7f4a_7c15);
    value ^= value >> 30;
    value = value.wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value ^= value >> 27;
    value = value.wrapping_mul(0x94d0_49bb_1331_11eb);
    (value ^ (value >> 31)) as i64
}

fn store_cursor(cursor: TelegramCursor) -> StoreSyncCursor {
    StoreSyncCursor {
        scope: "account".to_owned(),
        pts: cursor.pts.unwrap_or(0),
        qts: cursor.qts.unwrap_or(0),
        date: cursor.date.unwrap_or(0),
        seq: cursor.seq.unwrap_or(0),
    }
}

fn apply_cursor_delta(cursor: &mut StoreSyncCursor, delta: TelegramCursor) {
    if let Some(pts) = delta.pts {
        cursor.pts = pts;
    }
    if let Some(qts) = delta.qts {
        cursor.qts = qts;
    }
    if let Some(date) = delta.date {
        cursor.date = date;
    }
    if let Some(seq) = delta.seq {
        cursor.seq = seq;
    }
}

fn cached_bootstrap(account_name: String, cached: CachedAccount) -> Bootstrap {
    let chats = cached
        .chats
        .into_iter()
        .map(|chat| ChatView {
            id: ChatId(chat.id),
            title: chat.title,
            preview: chat.preview,
            unread: chat.unread,
            pinned: chat.pinned,
            kind: stored_chat_kind(&chat.kind),
            folders: chat.folders,
        })
        .collect::<Vec<_>>();
    let mut grouped = BTreeMap::<(i64, Option<i64>), Vec<MessageView>>::new();
    for message in cached.messages {
        grouped
            .entry((message.chat_id, message.thread_root))
            .or_default()
            .push(cached_message(message));
    }
    let histories = grouped
        .into_iter()
        .map(|((chat, thread_root), messages)| HistoryView {
            chat: ChatId(chat),
            thread_root: thread_root.map(MessageId),
            messages,
        })
        .collect::<Vec<_>>();
    let messages = chats.first().map_or_else(Vec::new, |active| {
        histories
            .iter()
            .find(|history| history.chat == active.id && history.thread_root.is_none())
            .map_or_else(Vec::new, |history| history.messages.clone())
    });
    Bootstrap {
        connection: intuigram_app::ConnectionState::Connecting,
        account_name,
        folders: cached
            .folders
            .into_iter()
            .map(|folder| FolderView {
                id: folder.id,
                title: folder.title,
                unread: folder.unread,
            })
            .collect(),
        chats,
        messages,
        drafts: cached
            .drafts
            .into_iter()
            .map(|draft| DraftView {
                chat: ChatId(draft.chat_id),
                thread_root: draft.thread_root.map(MessageId),
                text: draft.text,
                reply_to: draft.reply_to.map(MessageId),
            })
            .collect(),
        histories,
    }
}

fn stored_chat_kind(kind: &str) -> ChatKind {
    match kind {
        "saved_messages" => ChatKind::SavedMessages,
        "private" => ChatKind::Private,
        "bot" => ChatKind::Bot,
        "basic_group" => ChatKind::BasicGroup,
        "supergroup" => ChatKind::Supergroup,
        "gigagroup" => ChatKind::Gigagroup,
        "channel" => ChatKind::Channel,
        _ => ChatKind::Inaccessible,
    }
}

fn cached_message(message: StoredMessage) -> MessageView {
    let metadata =
        serde_json::from_str::<StoredMessageMetadata>(&message.metadata).unwrap_or_default();
    let media = stored_media_kind(&message.content_kind).map(|kind| {
        let stored = metadata.media.as_ref();
        MediaCard {
            kind,
            title: stored.map_or_else(|| message.content_kind.clone(), |media| media.title.clone()),
            description: stored.map_or_else(String::new, |media| media.description.clone()),
            remote_id: stored.and_then(|media| media.remote_id.clone()),
        }
    });
    MessageView {
        id: MessageId(message.id),
        sender: message.sender,
        body: message.body,
        timestamp: message.timestamp,
        direction: if message.direction == "outgoing" {
            MessageDirection::Outgoing
        } else {
            MessageDirection::Incoming
        },
        delivery: match message.delivery.as_str() {
            "pending" => DeliveryState::Pending,
            "read" => DeliveryState::Read,
            "failed" => DeliveryState::Failed,
            _ => DeliveryState::Sent,
        },
        reply_to: message.reply_to.map(MessageId),
        details: MessageDetails {
            entities: metadata.entities.into_iter().map(cached_entity).collect(),
            forwarded_from: metadata.forwarded_from,
            reactions: metadata
                .reactions
                .into_iter()
                .map(|reaction| ReactionView {
                    label: reaction.label,
                    count: reaction.count,
                    chosen: reaction.chosen,
                })
                .collect(),
            edited: metadata.edited,
            pinned: metadata.pinned,
            views: metadata.views,
            forwards: metadata.forwards,
            replies: metadata.replies,
            media,
            service: metadata.service,
            thread_root: message.thread_root.map(MessageId),
        },
    }
}

fn stored_media_kind(kind: &str) -> Option<MediaKind> {
    Some(match kind {
        "photo" => MediaKind::Photo,
        "video" => MediaKind::Video,
        "animation" => MediaKind::Animation,
        "sticker" => MediaKind::Sticker,
        "file" => MediaKind::File,
        "audio" => MediaKind::Audio,
        "voice" => MediaKind::Voice,
        "videonote" => MediaKind::VideoNote,
        "linkpreview" => MediaKind::LinkPreview,
        "poll" => MediaKind::Poll,
        "contact" => MediaKind::Contact,
        "location" => MediaKind::Location,
        "venue" => MediaKind::Venue,
        "dice" => MediaKind::Dice,
        "specialized" => MediaKind::Specialized,
        "unsupported" => MediaKind::Unsupported,
        "text" | "service" => return None,
        _ => MediaKind::Unsupported,
    })
}

fn cached_entity(entity: StoredEntity) -> TextEntity {
    TextEntity {
        offset: entity.offset,
        length: entity.length,
        kind: match entity.kind.as_str() {
            "bold" => TextEntityKind::Bold,
            "italic" => TextEntityKind::Italic,
            "underline" => TextEntityKind::Underline,
            "strike" => TextEntityKind::Strike,
            "code" => TextEntityKind::Code,
            "pre" => TextEntityKind::Pre {
                language: entity.value,
            },
            "spoiler" => TextEntityKind::Spoiler,
            "url" => TextEntityKind::Url,
            "text_url" => TextEntityKind::TextUrl {
                url: entity.value.unwrap_or_default(),
            },
            "custom_emoji" => TextEntityKind::CustomEmoji {
                document_id: entity.document_id.unwrap_or_default(),
            },
            _ => TextEntityKind::Semantic,
        },
    }
}

fn bootstrap_sync_batch(bootstrap: &Bootstrap, cursor: StoreSyncCursor) -> SyncBatch {
    let active_chat = bootstrap.chats.first().map(|chat| chat.id);
    SyncBatch {
        cursor,
        folders: bootstrap
            .folders
            .iter()
            .map(|folder| StoredFolder {
                id: folder.id,
                title: folder.title.clone(),
                unread: folder.unread,
            })
            .collect(),
        chats: bootstrap.chats.iter().map(stored_chat).collect(),
        messages: active_chat.map_or_else(Vec::new, |chat| {
            bootstrap
                .messages
                .iter()
                .map(|message| stored_message(chat, message))
                .collect()
        }),
    }
}

fn sync_batch_for_event(cursor: StoreSyncCursor, event: &LiveEvent) -> SyncBatch {
    let messages = match &event.event {
        AdapterEvent::MessageAdded { chat, message } => vec![stored_message(*chat, message)],
        _ => Vec::new(),
    };
    SyncBatch {
        cursor,
        folders: Vec::new(),
        chats: Vec::new(),
        messages,
    }
}

fn stored_chat(chat: &ChatView) -> StoredChat {
    StoredChat {
        id: chat.id.0,
        kind: match chat.kind {
            ChatKind::SavedMessages => "saved_messages",
            ChatKind::Private => "private",
            ChatKind::Bot => "bot",
            ChatKind::BasicGroup => "basic_group",
            ChatKind::Supergroup => "supergroup",
            ChatKind::Gigagroup => "gigagroup",
            ChatKind::Channel => "channel",
            ChatKind::Inaccessible => "inaccessible",
        }
        .to_owned(),
        title: chat.title.clone(),
        preview: chat.preview.clone(),
        unread: chat.unread,
        pinned: chat.pinned,
        folders: chat.folders.clone(),
    }
}

fn stored_message(chat: ChatId, message: &MessageView) -> StoredMessage {
    let content_kind = message.details.media.as_ref().map_or_else(
        || {
            if message.details.service.is_some() {
                "service".to_owned()
            } else {
                "text".to_owned()
            }
        },
        |media| format!("{:?}", media.kind).to_ascii_lowercase(),
    );
    let metadata = serde_json::to_string(&stored_message_metadata(message))
        .expect("fixed Intuigram Message metadata contains only JSON-serializable values");
    StoredMessage {
        chat_id: chat.0,
        id: message.id.0,
        sender: message.sender.clone(),
        body: message.body.clone(),
        timestamp: message.timestamp.clone(),
        direction: match message.direction {
            MessageDirection::Incoming => "incoming",
            MessageDirection::Outgoing => "outgoing",
        }
        .to_owned(),
        delivery: match message.delivery {
            DeliveryState::Pending => "pending",
            DeliveryState::Sent => "sent",
            DeliveryState::Read => "read",
            DeliveryState::Failed => "failed",
        }
        .to_owned(),
        reply_to: message.reply_to.map(|message| message.0),
        thread_root: message.details.thread_root.map(|message| message.0),
        content_kind,
        metadata,
    }
}

fn stored_message_metadata(message: &MessageView) -> StoredMessageMetadata {
    StoredMessageMetadata {
        edited: message.details.edited,
        pinned: message.details.pinned,
        forwarded_from: message.details.forwarded_from.clone(),
        views: message.details.views,
        forwards: message.details.forwards,
        replies: message.details.replies,
        service: message.details.service.clone(),
        media: message
            .details
            .media
            .as_ref()
            .map(|media| StoredMediaMetadata {
                title: media.title.clone(),
                description: media.description.clone(),
                remote_id: media.remote_id.clone(),
            }),
        reactions: message
            .details
            .reactions
            .iter()
            .map(|reaction| StoredReaction {
                label: reaction.label.clone(),
                count: reaction.count,
                chosen: reaction.chosen,
            })
            .collect(),
        entities: message.details.entities.iter().map(stored_entity).collect(),
    }
}

fn stored_entity(entity: &TextEntity) -> StoredEntity {
    let (kind, value, document_id) = match &entity.kind {
        TextEntityKind::Bold => ("bold", None, None),
        TextEntityKind::Italic => ("italic", None, None),
        TextEntityKind::Underline => ("underline", None, None),
        TextEntityKind::Strike => ("strike", None, None),
        TextEntityKind::Code => ("code", None, None),
        TextEntityKind::Pre { language } => ("pre", language.clone(), None),
        TextEntityKind::Spoiler => ("spoiler", None, None),
        TextEntityKind::Url => ("url", None, None),
        TextEntityKind::TextUrl { url } => ("text_url", Some(url.clone()), None),
        TextEntityKind::Semantic => ("semantic", None, None),
        TextEntityKind::CustomEmoji { document_id } => ("custom_emoji", None, Some(*document_id)),
    };
    StoredEntity {
        offset: entity.offset,
        length: entity.length,
        kind: kind.to_owned(),
        value,
        document_id,
    }
}

#[cfg(test)]
fn application_fixture() -> Bootstrap {
    Bootstrap {
        connection: intuigram_app::ConnectionState::Connected,
        account_name: "Intuigram Test".to_owned(),
        folders: vec![
            FolderView {
                id: 0,
                title: "All".to_owned(),
                unread: 5,
            },
            FolderView {
                id: 1,
                title: "Work".to_owned(),
                unread: 2,
            },
            FolderView {
                id: 2,
                title: "Archive".to_owned(),
                unread: 0,
            },
        ],
        chats: vec![
            ChatView {
                id: ChatId(100),
                title: "Saved Messages".to_owned(),
                preview: "Intuigram design notes".to_owned(),
                unread: 0,
                pinned: true,
                kind: ChatKind::SavedMessages,
                folders: vec![0],
            },
            ChatView {
                id: ChatId(101),
                title: "Intuigram Contributors".to_owned(),
                preview: "The dense layout feels right.".to_owned(),
                unread: 3,
                pinned: true,
                kind: ChatKind::Supergroup,
                folders: vec![0, 1],
            },
            ChatView {
                id: ChatId(102),
                title: "Terminal Friends".to_owned(),
                preview: "Ship the runnable slice!".to_owned(),
                unread: 2,
                pinned: false,
                kind: ChatKind::Private,
                folders: vec![0],
            },
        ],
        messages: fixture_messages(),
        drafts: Vec::new(),
        histories: Vec::new(),
    }
}

#[cfg(test)]
fn fixture_messages() -> Vec<MessageView> {
    vec![
        MessageView {
            id: MessageId(1),
            sender: "Intuigram".to_owned(),
            body: "Welcome. This is the live terminal UI, backed by the single-owner app loop."
                .to_owned(),
            timestamp: "09:41".to_owned(),
            direction: MessageDirection::Incoming,
            delivery: DeliveryState::Read,
            reply_to: None,
            details: MessageDetails::default(),
        },
        MessageView {
            id: MessageId(2),
            sender: "You".to_owned(),
            body: "Dense, focus-driven, and no keyboard modes.".to_owned(),
            timestamp: "09:42".to_owned(),
            direction: MessageDirection::Outgoing,
            delivery: DeliveryState::Read,
            reply_to: Some(MessageId(1)),
            details: MessageDetails::default(),
        },
        MessageView {
            id: MessageId(3),
            sender: "Intuigram".to_owned(),
            body: "Press ? for exhaustive context help. Type or paste in any open Chat.".to_owned(),
            timestamp: "09:43".to_owned(),
            direction: MessageDirection::Incoming,
            delivery: DeliveryState::Sent,
            reply_to: None,
            details: MessageDetails::default(),
        },
    ]
}

#[cfg(test)]
mod tests {
    use std::cell::{Cell, RefCell};
    use std::collections::VecDeque;
    use std::io;
    use std::path::PathBuf;
    use std::rc::Rc;
    use std::task::{Context, Poll};

    use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
    use intuigram_app::{
        Action, AdapterEvent, ChatId, ChatKind, DeliveryState, Effect, Intent, MediaCard,
        MediaKind, MessageDetails, MessageDirection, MessageId, MessageView, TextEntity,
        TextEntityKind,
    };
    use intuigram_store::{CachedAccount, StoredChat, StoredDraft, StoredFolder};
    use intuigram_telegram::{LoginCodeDelivery, LoginCodeDeliveryMethod};
    use intuigram_tui::UiEvent;

    use super::{
        AdapterEffect, ApplicationAdapterEvents, ApplicationBackend, ApplicationEvents,
        ApplicationExit, ApplicationUi, AttachmentPayload, AttachmentStore, EFFECT_CAPACITY, Error,
        PRIMARY_DC_ENDPOINT, PendingEffect, Result, application_fixture, cached_bootstrap,
        enqueue_effect, error_lines, login_code_delivery_message, login_code_delivery_method_name,
        parse_arguments, run_application, run_application_state, seconds_until_at, stored_message,
    };

    struct PendingHistoryBackend {
        polls: Rc<Cell<usize>>,
    }

    #[test]
    fn cached_account_restores_rich_thread_history_and_drafts() {
        let message = MessageView {
            id: MessageId(42),
            sender: "Ada".to_owned(),
            body: "cached caption".to_owned(),
            timestamp: "12:00".to_owned(),
            direction: MessageDirection::Incoming,
            delivery: DeliveryState::Read,
            reply_to: Some(MessageId(40)),
            details: MessageDetails {
                entities: vec![TextEntity {
                    offset: 0,
                    length: 6,
                    kind: TextEntityKind::Bold,
                }],
                media: Some(MediaCard {
                    kind: MediaKind::Photo,
                    title: "Photo".to_owned(),
                    description: "image".to_owned(),
                    remote_id: Some("99".to_owned()),
                }),
                thread_root: Some(MessageId(41)),
                ..MessageDetails::default()
            },
        };
        let cached = CachedAccount {
            cursors: Vec::new(),
            folders: vec![StoredFolder {
                id: 0,
                title: "All".to_owned(),
                unread: 1,
            }],
            chats: vec![StoredChat {
                id: 7,
                kind: "private".to_owned(),
                title: "Ada".to_owned(),
                preview: "cached caption".to_owned(),
                unread: 1,
                pinned: false,
                folders: vec![0],
            }],
            messages: vec![stored_message(ChatId(7), &message)],
            drafts: vec![StoredDraft {
                chat_id: 7,
                thread_root: Some(41),
                text: "cached Draft".to_owned(),
                reply_to: Some(42),
                modified_at: 10,
            }],
        };

        let bootstrap = cached_bootstrap("Ada".to_owned(), cached);

        assert_eq!(bootstrap.chats[0].kind, ChatKind::Private);
        assert_eq!(bootstrap.histories[0].thread_root, Some(MessageId(41)));
        assert_eq!(bootstrap.histories[0].messages, vec![message]);
        assert_eq!(bootstrap.drafts[0].text, "cached Draft");
    }

    impl ApplicationBackend for PendingHistoryBackend {
        async fn execute(&mut self, effect: AdapterEffect) -> Result<Option<AdapterEvent>> {
            let Effect::LoadChat { chat } = effect.effect else {
                return Ok(None);
            };
            std::future::poll_fn(|cx| {
                let polls = self.polls.get();
                self.polls.set(polls + 1);
                if polls == 0 {
                    cx.waker().wake_by_ref();
                    Poll::Pending
                } else {
                    Poll::Ready(())
                }
            })
            .await;
            Ok(Some(AdapterEvent::ChatLoaded {
                chat,
                messages: Vec::new(),
            }))
        }
    }

    struct RecordingUi {
        views: Rc<RefCell<Vec<intuigram_app::View>>>,
    }

    impl ApplicationUi for RecordingUi {
        fn draw(&mut self, view: &intuigram_app::View) -> intuigram_tui::Result<()> {
            self.views.borrow_mut().push(view.clone());
            Ok(())
        }

        fn resolve_event(&self, _view: &intuigram_app::View, event: Event) -> Option<UiEvent> {
            let Event::Key(key) = event else {
                return Some(UiEvent::Redraw);
            };
            match key.code {
                KeyCode::Char('o') => Some(UiEvent::Intent(Intent::Action(Action::Open))),
                KeyCode::Char('x') => Some(UiEvent::Intent(Intent::Insert("x".to_owned()))),
                KeyCode::Char('q') => Some(UiEvent::Intent(Intent::Action(Action::Quit))),
                _ => None,
            }
        }
    }

    enum EventStep {
        Ready(Event),
        Pending,
    }

    struct ScriptedEvents {
        steps: VecDeque<EventStep>,
    }

    impl ApplicationEvents for ScriptedEvents {
        fn poll_next_event(&mut self, cx: &mut Context<'_>) -> Poll<intuigram_tui::Result<Event>> {
            match self.steps.pop_front().expect("event script should not end") {
                EventStep::Ready(event) => Poll::Ready(Ok(event)),
                EventStep::Pending => {
                    cx.waker().wake_by_ref();
                    Poll::Pending
                }
            }
        }
    }

    struct NoAdapterEvents;

    impl ApplicationAdapterEvents for NoAdapterEvents {
        fn poll_adapter_event(&mut self, _cx: &mut Context<'_>) -> Poll<Result<AdapterEvent>> {
            Poll::Pending
        }
    }

    struct AlwaysPendingEvents;

    impl ApplicationEvents for AlwaysPendingEvents {
        fn poll_next_event(&mut self, _cx: &mut Context<'_>) -> Poll<intuigram_tui::Result<Event>> {
            Poll::Pending
        }
    }

    struct FailingConnectionBackend {
        observed: Rc<RefCell<Vec<AdapterEffect>>>,
    }

    impl ApplicationBackend for FailingConnectionBackend {
        async fn execute(&mut self, effect: AdapterEffect) -> Result<Option<AdapterEvent>> {
            self.observed.borrow_mut().push(effect);
            Err(Error::TelegramUpdatesClosed)
        }
    }

    fn key(character: char) -> Event {
        Event::Key(KeyEvent::new(KeyCode::Char(character), KeyModifiers::NONE))
    }

    #[test]
    fn chat_history_loading_does_not_block_terminal_input() {
        let views = Rc::new(RefCell::new(Vec::new()));
        let polls = Rc::new(Cell::new(0));
        let mut terminal = RecordingUi {
            views: Rc::clone(&views),
        };
        let mut events = ScriptedEvents {
            steps: [
                EventStep::Ready(key('o')),
                EventStep::Pending,
                EventStep::Ready(key('x')),
                EventStep::Pending,
                EventStep::Ready(key('q')),
            ]
            .into(),
        };
        let backend = PendingHistoryBackend {
            polls: Rc::clone(&polls),
        };
        let mut adapter_events = NoAdapterEvents;
        let runtime = compio::runtime::Runtime::new().expect("test runtime should initialize");

        runtime
            .block_on(run_application(
                &mut terminal,
                &mut events,
                &mut adapter_events,
                backend,
                application_fixture(),
            ))
            .expect("application should stop cleanly");

        assert!(polls.get() >= 2, "history future should make progress");
        assert!(
            views.borrow().iter().any(|view| view.composer.text == "x"),
            "terminal input should update the Draft while history is pending"
        );
    }

    #[test]
    fn a_full_effect_queue_fails_instead_of_blocking_terminal_input() {
        let mut pending = VecDeque::from(vec![
            AdapterEffect {
                effect: Effect::Reconnect,
                random_id: None,
            };
            EFFECT_CAPACITY
        ]);
        let active = None::<PendingEffect<PendingHistoryBackend>>;

        let error = enqueue_effect(&mut pending, &active, Some(Effect::Reconnect))
            .expect_err("a saturated effect queue should be reported");

        assert!(matches!(error, Error::EffectsFull { .. }));
    }

    #[test]
    fn connection_failure_returns_the_same_send_for_retry() {
        let views = Rc::new(RefCell::new(Vec::new()));
        let observed = Rc::new(RefCell::new(Vec::new()));
        let mut terminal = RecordingUi {
            views: Rc::clone(&views),
        };
        let mut events = AlwaysPendingEvents;
        let mut adapter_events = NoAdapterEvents;
        let backend = FailingConnectionBackend {
            observed: Rc::clone(&observed),
        };
        let mut app = intuigram_app::App::new();
        let update = app.transition(intuigram_app::Input::Adapter(AdapterEvent::Bootstrap(
            application_fixture(),
        )));
        let send = AdapterEffect::new(Effect::SendMessage {
            chat: ChatId(10),
            text: "retry once connected".to_owned(),
            reply_to: None,
            thread_root: None,
            attachments: Vec::new(),
            local_id: MessageId(-1),
        })
        .expect("operation id should be generated");
        let expected_random_id = send.random_id;
        let runtime = compio::runtime::Runtime::new().expect("test runtime should initialize");

        let exit = runtime
            .block_on(run_application_state(
                &mut terminal,
                &mut events,
                &mut adapter_events,
                backend,
                app,
                update,
                VecDeque::from([send]),
            ))
            .expect("connection failure should become an application handoff");
        let ApplicationExit::Disconnected(state) = exit else {
            panic!("connection failure should not quit")
        };

        assert_eq!(observed.borrow().len(), 1);
        assert_eq!(state.pending_effects.len(), 1);
        assert_eq!(state.pending_effects[0].random_id, expected_random_id);
        assert_eq!(
            state.app.view().connection,
            intuigram_app::ConnectionState::ReconnectCooldown
        );
        assert!(
            views.borrow().iter().any(|view| {
                view.connection == intuigram_app::ConnectionState::ReconnectCooldown
            }),
            "the TUI should render the disconnect before reconnecting"
        );
    }

    #[test]
    fn reconnect_handoff_preserves_attachment_payloads_and_ids() {
        let mut disconnected = AttachmentStore::default();
        let first = disconnected.register(AttachmentPayload::Image {
            mime_type: "image/png".to_owned(),
            bytes: vec![1, 2, 3],
        });
        let mut connected = AttachmentStore::default();

        connected.merge(disconnected);
        let second = connected.register(AttachmentPayload::Image {
            mime_type: "image/png".to_owned(),
            bytes: vec![4, 5, 6],
        });

        assert!(connected.payloads.contains_key(&first));
        assert!(second.0 > first.0);
    }

    #[test]
    fn bootstrap_uses_the_production_dc_2_endpoint() {
        assert_eq!(PRIMARY_DC_ENDPOINT.to_string(), "149.154.167.41:443");
    }

    #[test]
    fn qr_expiry_uses_the_telegram_server_time_offset() {
        assert_eq!(seconds_until_at(1_030, 1_000, 10), 20);
        assert_eq!(seconds_until_at(1_030, 1_000, 40), 0);
    }

    #[test]
    fn telegram_app_login_codes_name_the_actual_destination() {
        assert_eq!(
            login_code_delivery_message(&LoginCodeDelivery::TelegramApp { length: 5 }),
            "Telegram sent a 5-digit code to the Telegram app on another logged-in device."
        );
    }

    #[test]
    fn login_code_fallback_names_sms_delivery() {
        assert_eq!(
            login_code_delivery_method_name(LoginCodeDeliveryMethod::Sms),
            "SMS delivery"
        );
    }

    #[test]
    fn command_line_paths_are_parsed_and_the_obsolete_demo_flag_is_rejected() {
        let parsed = parse_arguments([
            "--data-dir".to_owned(),
            "/tmp/intuigram-data".to_owned(),
            "--cache-dir".to_owned(),
            "/tmp/intuigram-cache".to_owned(),
        ])
        .expect("valid command line should parse");

        assert_eq!(
            parsed.data.expect("data override should exist"),
            PathBuf::from("/tmp/intuigram-data")
        );
        assert_eq!(
            parsed.cache.expect("cache override should exist"),
            PathBuf::from("/tmp/intuigram-cache")
        );
        assert!(parse_arguments(["--demo".to_owned()]).is_err());
    }

    #[test]
    fn errors_are_rendered_one_line_per_source_layer() {
        let error = Error::Runtime {
            source: io::Error::other("driver setup\nfailed"),
        };

        assert_eq!(
            error_lines(&error),
            [
                "failed to initialize the Compio runtime",
                "driver setup failed"
            ]
        );
    }
}
