use std::env;
use std::io::{self, Write};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::num::NonZeroUsize;
use std::path::PathBuf;
use std::thread::{self, JoinHandle};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use popgram_app::{
    AdapterEvent, App, AppChannels, Bootstrap, ChatId, ChatView, DeliveryState, Effect, FolderView,
    Input, MessageDirection, MessageId, MessageView, bounded_channels,
};
use popgram_config::{Config, ConfigLoader, Overrides, PlatformDefaults};
use popgram_store::{
    AccountDatabase, AccountId, AccountRecord, GlobalDatabase, SessionMaterial, StoreLayout,
};
use popgram_telegram::{
    ApplicationCredentials, AuthorizedUser, Client, CodeRequest, CodeSignIn, LoginCodeDelivery,
    LoginCodeDeliveryMethod, LoginCodeToken, QrLogin, Session,
};
use popgram_tui::{QrLoginAction, QrLoginUi, TerminalUi, UiEvent};
use snafu::{OptionExt, ResultExt, Snafu};

const PRIMARY_DC_ID: i32 = 2;
const PRIMARY_DC_ENDPOINT: SocketAddr =
    SocketAddr::new(IpAddr::V4(Ipv4Addr::new(149, 154, 167, 41)), 443);

#[derive(Debug, Snafu)]
enum Error {
    #[snafu(display("platform did not provide a {kind} directory"))]
    MissingPlatformDirectory { kind: &'static str },

    #[snafu(display("missing value after {argument}"))]
    MissingArgumentValue { argument: String },

    #[snafu(display("unknown argument {argument}"))]
    UnknownArgument { argument: String },

    #[snafu(display("failed to load Popgram configuration"))]
    LoadConfiguration { source: popgram_config::Error },

    #[snafu(display("Telegram setting {setting} is required; configure it or use --demo"))]
    MissingTelegramSetting { setting: &'static str },

    #[snafu(display("failed to open Popgram Account registry"))]
    OpenAccountRegistry { source: popgram_store::GlobalError },

    #[snafu(display("failed to read Popgram Account registry"))]
    ReadAccountRegistry { source: popgram_store::GlobalError },

    #[snafu(display("failed to update Popgram Account registry"))]
    UpdateAccountRegistry { source: popgram_store::GlobalError },

    #[snafu(display("failed to access Popgram Account database"))]
    AccountDatabase { source: popgram_store::Error },

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
    Telegram { source: popgram_telegram::Error },

    #[snafu(display("failed to read {field} from the terminal"))]
    Prompt {
        field: &'static str,
        source: io::Error,
    },

    #[snafu(display("{field} must not be empty"))]
    EmptyPrompt { field: &'static str },

    #[snafu(display("Telegram login was cancelled"))]
    LoginCancelled,

    #[snafu(display("failed to start application state owner"))]
    SpawnStateOwner { source: io::Error },

    #[snafu(display("failed to start terminal UI worker"))]
    SpawnUiWorker { source: io::Error },

    #[snafu(display("backend stopped accepting UI effects"))]
    EffectsClosed,

    #[snafu(display("backend effect queue is full"))]
    EffectsFull,

    #[snafu(display("terminal UI worker panicked"))]
    UiWorkerPanicked,

    #[snafu(display("application state owner stopped accepting input"))]
    InputClosed,

    #[snafu(display("application state owner stopped publishing views"))]
    UpdatesClosed,

    #[snafu(display("terminal UI failed"))]
    Terminal { source: popgram_tui::Error },

    #[snafu(display("application state owner failed"))]
    StateOwner { source: popgram_app::Error },

    #[snafu(display("application state owner panicked"))]
    StateOwnerPanicked,
}

type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Default)]
struct Arguments {
    config: Option<PathBuf>,
    data: Option<PathBuf>,
    cache: Option<PathBuf>,
    downloads: Option<PathBuf>,
    demo: bool,
    help: bool,
}

enum Backend {
    Demo {
        next_message_id: i64,
    },
    Telegram {
        runtime: compio::runtime::Runtime,
        client: Box<Client>,
        _database: AccountDatabase,
        next_local_message_id: i64,
    },
}

enum QrAuthorization {
    Authorized(Box<(Client, Session, AuthorizedUser)>),
    PhoneLogin(Box<(Client, Session)>),
}

impl Backend {
    fn load_chat(&mut self, chat: ChatId) -> Result<Vec<MessageView>> {
        match self {
            Self::Demo { .. } => Ok(demo_messages()),
            Self::Telegram {
                runtime, client, ..
            } => runtime
                .block_on(client.history(chat, 100))
                .context(TelegramSnafu),
        }
    }

    fn send_message(
        &mut self,
        chat: ChatId,
        text: String,
        reply_to: Option<MessageId>,
    ) -> Result<MessageView> {
        let message_id = match self {
            Self::Demo { next_message_id } => {
                *next_message_id += 1;
                *next_message_id
            }
            Self::Telegram {
                runtime,
                client,
                next_local_message_id,
                ..
            } => {
                runtime
                    .block_on(client.send_text(chat, text.clone(), reply_to))
                    .context(TelegramSnafu)?;
                *next_local_message_id -= 1;
                *next_local_message_id
            }
        };
        Ok(MessageView {
            id: MessageId(message_id),
            sender: "You".to_owned(),
            body: text,
            timestamp: "now".to_owned(),
            direction: MessageDirection::Outgoing,
            delivery: DeliveryState::Sent,
            reply_to,
        })
    }
}

trait ApplicationBackend {
    fn load_chat(&mut self, chat: ChatId) -> Result<Vec<MessageView>>;

    fn send_message(
        &mut self,
        chat: ChatId,
        text: String,
        reply_to: Option<MessageId>,
    ) -> Result<MessageView>;
}

impl ApplicationBackend for Backend {
    fn load_chat(&mut self, chat: ChatId) -> Result<Vec<MessageView>> {
        Self::load_chat(self, chat)
    }

    fn send_message(
        &mut self,
        chat: ChatId,
        text: String,
        reply_to: Option<MessageId>,
    ) -> Result<MessageView> {
        Self::send_message(self, chat, text, reply_to)
    }
}

trait ApplicationUi {
    fn draw(&mut self, view: &popgram_app::View) -> popgram_tui::Result<()>;

    async fn next_event(&mut self, view: &popgram_app::View) -> popgram_tui::Result<UiEvent>;
}

impl ApplicationUi for TerminalUi {
    fn draw(&mut self, view: &popgram_app::View) -> popgram_tui::Result<()> {
        Self::draw(self, view)
    }

    async fn next_event(&mut self, view: &popgram_app::View) -> popgram_tui::Result<UiEvent> {
        Self::next_event(self, view).await
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
            eprintln!("popgram: {line}");
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
    let (mut backend, bootstrap) = if arguments.demo {
        (
            Backend::Demo {
                next_message_id: 10_000,
            },
            demo_data(),
        )
    } else {
        initialize_telegram(&config, &layout, &global)?
    };

    let capacity = NonZeroUsize::new(64).expect("constant channel capacity is positive");
    let (handle, channels) = bounded_channels(capacity);
    let state_owner = spawn_state_owner(channels)?;
    send_input(
        &handle.inputs,
        Input::Adapter(AdapterEvent::Bootstrap(bootstrap)),
    )?;

    let mut update = recv_update(&handle.updates)?;
    update = recv_update(&handle.updates).unwrap_or(update);
    let adapter_inputs = handle.inputs.clone();
    let (effects_tx, effects_rx) = async_channel::bounded(capacity.get());
    let (shutdown_tx, shutdown_rx) = async_channel::bounded(1);
    let ui_worker = spawn_ui_worker(
        handle.inputs,
        handle.updates,
        effects_tx,
        shutdown_rx,
        update,
    )?;

    let backend_result = run_backend_loop(&mut backend, &adapter_inputs, &effects_rx);
    let _ = shutdown_tx.send_blocking(());
    drop(adapter_inputs);
    drop(effects_rx);
    let ui_result = finish_ui_worker(ui_worker);
    let state_owner_result = finish_state_owner(state_owner);
    backend_result?;
    ui_result?;
    state_owner_result
}

fn spawn_ui_worker(
    inputs: async_channel::Sender<Input>,
    updates: async_channel::Receiver<popgram_app::Update>,
    effects: async_channel::Sender<Effect>,
    shutdown: async_channel::Receiver<()>,
    update: popgram_app::Update,
) -> Result<JoinHandle<Result<()>>> {
    thread::Builder::new()
        .name("popgram-tui".to_owned())
        .spawn(move || {
            let runtime = compio::runtime::Runtime::new().context(RuntimeSnafu)?;
            let mut terminal = TerminalUi::enter().context(TerminalSnafu)?;
            runtime.block_on(run_ui_loop(
                &mut terminal,
                &inputs,
                &updates,
                &effects,
                &shutdown,
                update,
            ))
        })
        .context(SpawnUiWorkerSnafu)
}

fn finish_ui_worker(worker: JoinHandle<Result<()>>) -> Result<()> {
    worker.join().map_err(|_| Error::UiWorkerPanicked)?
}

async fn run_ui_loop(
    terminal: &mut impl ApplicationUi,
    inputs: &async_channel::Sender<Input>,
    updates: &async_channel::Receiver<popgram_app::Update>,
    effects: &async_channel::Sender<Effect>,
    shutdown: &async_channel::Receiver<()>,
    mut update: popgram_app::Update,
) -> Result<()> {
    loop {
        terminal.draw(&update.view).context(TerminalSnafu)?;
        if let Some(effect) = update.effect.take() {
            let quitting = effect == Effect::Quit;
            match effects.try_send(effect) {
                Ok(()) => {}
                Err(async_channel::TrySendError::Closed(_)) => {
                    return EffectsClosedSnafu.fail();
                }
                Err(async_channel::TrySendError::Full(_)) => return EffectsFullSnafu.fail(),
            }
            if quitting {
                return Ok(());
            }
        }

        match next_ui_wake(terminal, &update.view, updates, shutdown).await {
            UiWake::Shutdown => return Ok(()),
            UiWake::Update(next) => {
                update = (*next).map_err(|_| Error::UpdatesClosed)?;
            }
            UiWake::Event(event) => match event.context(TerminalSnafu)? {
                UiEvent::Redraw => continue,
                UiEvent::Intent(intent) => {
                    send_input(inputs, Input::Intent(intent))?;
                    update = match next_update_or_shutdown(updates, shutdown).await {
                        UpdateWake::Shutdown => return Ok(()),
                        UpdateWake::Update(next) => (*next).map_err(|_| Error::UpdatesClosed)?,
                    };
                }
            },
        }
    }
}

enum UiWake {
    Event(popgram_tui::Result<UiEvent>),
    Update(Box<std::result::Result<popgram_app::Update, async_channel::RecvError>>),
    Shutdown,
}

enum UpdateWake {
    Update(Box<std::result::Result<popgram_app::Update, async_channel::RecvError>>),
    Shutdown,
}

async fn next_ui_wake(
    terminal: &mut impl ApplicationUi,
    view: &popgram_app::View,
    updates: &async_channel::Receiver<popgram_app::Update>,
    shutdown: &async_channel::Receiver<()>,
) -> UiWake {
    futures_lite::future::race(
        async { UiWake::Event(terminal.next_event(view).await) },
        async {
            match next_update_or_shutdown(updates, shutdown).await {
                UpdateWake::Update(update) => UiWake::Update(update),
                UpdateWake::Shutdown => UiWake::Shutdown,
            }
        },
    )
    .await
}

async fn next_update_or_shutdown(
    updates: &async_channel::Receiver<popgram_app::Update>,
    shutdown: &async_channel::Receiver<()>,
) -> UpdateWake {
    futures_lite::future::race(
        async { UpdateWake::Update(Box::new(updates.recv().await)) },
        async {
            let _ = shutdown.recv().await;
            UpdateWake::Shutdown
        },
    )
    .await
}

fn run_backend_loop(
    backend: &mut impl ApplicationBackend,
    inputs: &async_channel::Sender<Input>,
    effects: &async_channel::Receiver<Effect>,
) -> Result<()> {
    let mut pending_effect = None;
    loop {
        let effect = match pending_effect.take() {
            Some(effect) => effect,
            None => match effects.recv_blocking() {
                Ok(effect) => effect,
                Err(_) => break,
            },
        };
        match effect {
            Effect::Quit => break,
            Effect::Reconnect => {}
            Effect::LoadChat { chat } => {
                let messages = backend.load_chat(chat)?;
                if stop_after_adapter_work(effects, &mut pending_effect) {
                    break;
                }
                send_input(
                    inputs,
                    Input::Adapter(AdapterEvent::ChatLoaded { chat, messages }),
                )?;
            }
            Effect::SendMessage {
                chat,
                text,
                reply_to,
            } => {
                let message = backend.send_message(chat, text, reply_to)?;
                if stop_after_adapter_work(effects, &mut pending_effect) {
                    break;
                }
                send_input(inputs, Input::Adapter(AdapterEvent::MessageAdded(message)))?;
            }
        }
    }
    Ok(())
}

fn stop_after_adapter_work(
    effects: &async_channel::Receiver<Effect>,
    pending_effect: &mut Option<Effect>,
) -> bool {
    match effects.try_recv() {
        Ok(Effect::Quit) | Err(async_channel::TryRecvError::Closed) => true,
        Ok(effect) => {
            *pending_effect = Some(effect);
            false
        }
        Err(async_channel::TryRecvError::Empty) => false,
    }
}

fn initialize_telegram(
    config: &Config,
    layout: &StoreLayout,
    global: &GlobalDatabase,
) -> Result<(Backend, Bootstrap)> {
    let credentials = telegram_credentials(config)?;
    let runtime = compio::runtime::Runtime::new().context(RuntimeSnafu)?;
    let accounts = global.accounts().context(ReadAccountRegistrySnafu)?;
    if let Some(account) = accounts.iter().find(|account| account.active) {
        return resume_account(runtime, credentials, layout, account);
    }
    authorize_new_account(runtime, &credentials, config, layout, global)
}

fn resume_account(
    runtime: compio::runtime::Runtime,
    credentials: ApplicationCredentials,
    layout: &StoreLayout,
    account: &AccountRecord,
) -> Result<(Backend, Bootstrap)> {
    let database = AccountDatabase::open(layout, account.id).context(AccountDatabaseSnafu)?;
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
    let mut client = runtime
        .block_on(Client::connect_existing(credentials, &session, identity))
        .context(TelegramSnafu)?;
    let bootstrap = runtime
        .block_on(client.bootstrap(100))
        .context(TelegramSnafu)?;
    Ok((
        Backend::Telegram {
            runtime,
            client: Box::new(client),
            _database: database,
            next_local_message_id: 0,
        },
        bootstrap,
    ))
}

fn authorize_new_account(
    runtime: compio::runtime::Runtime,
    credentials: &ApplicationCredentials,
    config: &Config,
    layout: &StoreLayout,
    global: &GlobalDatabase,
) -> Result<(Backend, Bootstrap)> {
    let pending = AccountDatabase::begin_login(layout).context(AccountDatabaseSnafu)?;
    let (client, session) = if let Some(stored) = pending.session().context(AccountDatabaseSnafu)? {
        let session = telegram_session(&stored)?;
        match runtime.block_on(Client::connect_pending(credentials.clone(), &session)) {
            Ok(client) => (client, session),
            Err(error) if error.is_test_data_center() => {
                let connected = runtime
                    .block_on(Client::connect_new(
                        PRIMARY_DC_ID,
                        PRIMARY_DC_ENDPOINT,
                        credentials.clone(),
                    ))
                    .context(TelegramSnafu)?;
                pending
                    .save_session(store_session(&connected.1))
                    .context(AccountDatabaseSnafu)?;
                connected
            }
            Err(source) => return Err(Error::Telegram { source }),
        }
    } else {
        let (client, session) = runtime
            .block_on(Client::connect_new(
                PRIMARY_DC_ID,
                PRIMARY_DC_ENDPOINT,
                credentials.clone(),
            ))
            .context(TelegramSnafu)?;
        pending
            .save_session(store_session(&session))
            .context(AccountDatabaseSnafu)?;
        (client, session)
    };
    let (mut client, session, user) =
        match authorize_with_qr(&runtime, credentials, &pending, client, session)? {
            QrAuthorization::Authorized(authorized) => *authorized,
            QrAuthorization::PhoneLogin(login) => {
                let (client, session) = *login;
                let phone_number = match config.telegram.phone_number.as_deref() {
                    Some(number) => number.to_owned(),
                    None => prompt("Phone number", "phone number")?,
                };
                let (mut client, session, code_request) = request_code_with_migration(
                    &runtime,
                    credentials,
                    &pending,
                    client,
                    session,
                    &phone_number,
                )?;
                let user = match code_request {
                    CodeRequest::AlreadyAuthorized(user) => user,
                    CodeRequest::Sent(token) => {
                        sign_in_with_delivered_code(&runtime, &mut client, token)?
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
    let bootstrap = runtime
        .block_on(client.bootstrap(100))
        .context(TelegramSnafu)?;
    Ok((
        Backend::Telegram {
            runtime,
            client: Box::new(client),
            _database: database,
            next_local_message_id: 0,
        },
        bootstrap,
    ))
}

fn authorize_with_qr(
    runtime: &compio::runtime::Runtime,
    credentials: &ApplicationCredentials,
    pending: &AccountDatabase,
    mut client: Client,
    mut session: Session,
) -> Result<QrAuthorization> {
    let mut terminal = QrLoginUi::enter().context(TerminalSnafu)?;
    let mut state = runtime
        .block_on(client.export_qr_login())
        .context(TelegramSnafu)?;
    loop {
        match state {
            QrLogin::Pending(token) => loop {
                let expires_in = seconds_until(token.expires_at(), session.time_offset);
                if expires_in == 0 {
                    state = runtime
                        .block_on(client.export_qr_login())
                        .context(TelegramSnafu)?;
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
                if runtime
                    .block_on(client.poll_qr_login())
                    .context(TelegramSnafu)?
                {
                    state = runtime
                        .block_on(client.export_qr_login())
                        .context(TelegramSnafu)?;
                    break;
                }
            },
            QrLogin::Migrate(migration) => {
                let dc_id = migration.dc_id();
                let endpoint = client
                    .data_center_endpoint(dc_id)
                    .context(MissingDataCenterSnafu { dc_id })?;
                (client, session) = runtime
                    .block_on(Client::connect_new(dc_id, endpoint, credentials.clone()))
                    .context(TelegramSnafu)?;
                pending
                    .save_session(store_session(&session))
                    .context(AccountDatabaseSnafu)?;
                state = runtime
                    .block_on(client.import_qr_login(migration))
                    .context(TelegramSnafu)?;
            }
            QrLogin::PasswordRequired(password) => {
                drop(terminal);
                let user = sign_in_with_password(runtime, &mut client, password)?;
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

fn seconds_until_at(expires_at: i32, local_now: i64, server_time_offset: i32) -> u64 {
    let server_now = local_now.saturating_add(i64::from(server_time_offset));
    u64::try_from(i64::from(expires_at).saturating_sub(server_now)).unwrap_or(0)
}

fn sign_in_with_delivered_code(
    runtime: &compio::runtime::Runtime,
    client: &mut Client,
    mut token: LoginCodeToken,
) -> Result<AuthorizedUser> {
    loop {
        print_login_code_delivery(&token);
        let code = prompt("Login code (or 'resend')", "login code")?;
        if code.eq_ignore_ascii_case("resend") {
            match runtime
                .block_on(client.resend_login_code(&token))
                .context(TelegramSnafu)?
            {
                CodeRequest::Sent(next_token) => {
                    token = next_token;
                    continue;
                }
                CodeRequest::AlreadyAuthorized(user) => return Ok(user),
            }
        }
        return match runtime
            .block_on(client.sign_in_with_code(token, code))
            .context(TelegramSnafu)?
        {
            CodeSignIn::Authorized(user) => Ok(user),
            CodeSignIn::PasswordRequired(password) => {
                sign_in_with_password(runtime, client, password)
            }
        };
    }
}

fn sign_in_with_password(
    runtime: &compio::runtime::Runtime,
    client: &mut Client,
    prompt: popgram_telegram::PasswordPrompt,
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
    runtime
        .block_on(client.sign_in_with_password(password.as_bytes()))
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

fn request_code_with_migration(
    runtime: &compio::runtime::Runtime,
    credentials: &ApplicationCredentials,
    pending: &AccountDatabase,
    mut client: Client,
    mut session: Session,
    phone_number: &str,
) -> Result<(Client, Session, CodeRequest)> {
    loop {
        match runtime.block_on(client.request_login_code(phone_number.to_owned())) {
            Ok(request) => return Ok((client, session, request)),
            Err(error) => {
                let Some(dc_id) = error.phone_migration_dc() else {
                    return Err(Error::Telegram { source: error });
                };
                let endpoint = client
                    .data_center_endpoint(dc_id)
                    .context(MissingDataCenterSnafu { dc_id })?;
                let connected = runtime
                    .block_on(Client::connect_new(dc_id, endpoint, credentials.clone()))
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
            "--demo" => {
                parsed.demo = true;
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
            .join("popgram"),
    };
    let data = dirs::data_dir()
        .context(MissingPlatformDirectorySnafu { kind: "data" })?
        .join("popgram");
    let cache = dirs::cache_dir()
        .context(MissingPlatformDirectorySnafu { kind: "cache" })?
        .join("popgram");
    let downloads =
        dirs::download_dir().context(MissingPlatformDirectorySnafu { kind: "downloads" })?;
    Ok(PlatformDefaults {
        config,
        data,
        cache,
        downloads,
    })
}

fn spawn_state_owner(channels: AppChannels) -> Result<JoinHandle<popgram_app::Result<()>>> {
    thread::Builder::new()
        .name("popgram-app".to_owned())
        .spawn(move || futures_lite::future::block_on(App::new().run(channels)))
        .context(SpawnStateOwnerSnafu)
}

fn finish_state_owner(worker: JoinHandle<popgram_app::Result<()>>) -> Result<()> {
    worker
        .join()
        .map_err(|_| Error::StateOwnerPanicked)?
        .context(StateOwnerSnafu)
}

fn send_input(sender: &async_channel::Sender<Input>, input: Input) -> Result<()> {
    sender.send_blocking(input).map_err(|_| Error::InputClosed)
}

fn recv_update(
    receiver: &async_channel::Receiver<popgram_app::Update>,
) -> Result<popgram_app::Update> {
    receiver.recv_blocking().map_err(|_| Error::UpdatesClosed)
}

fn print_help() {
    println!(
        "Popgram terminal client\n\n\
         Usage: popgram [OPTIONS]\n\n\
         Options:\n\
           --demo                  Run without Telegram credentials or network access\n\
           --config-dir PATH       Override the platform config directory\n\
           --data-dir PATH         Override the platform data directory\n\
           --cache-dir PATH        Override the platform cache directory\n\
           --downloads-dir PATH    Override the platform Downloads directory\n\
           -h, --help              Print this help\n\n\
         Configure telegram.api_id and telegram.api_hash in config.toml, YAML, JSON, or the\n\
         POPGRAM_TELEGRAM__API_ID and POPGRAM_TELEGRAM__API_HASH environment variables."
    );
}

fn demo_data() -> Bootstrap {
    Bootstrap {
        account_name: "Popgram Demo".to_owned(),
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
                preview: "Popgram design notes".to_owned(),
                unread: 0,
                pinned: true,
            },
            ChatView {
                id: ChatId(101),
                title: "Popgram Contributors".to_owned(),
                preview: "The dense layout feels right.".to_owned(),
                unread: 3,
                pinned: true,
            },
            ChatView {
                id: ChatId(102),
                title: "Terminal Friends".to_owned(),
                preview: "Ship the runnable slice!".to_owned(),
                unread: 2,
                pinned: false,
            },
        ],
        messages: demo_messages(),
    }
}

fn demo_messages() -> Vec<MessageView> {
    vec![
        MessageView {
            id: MessageId(1),
            sender: "Popgram".to_owned(),
            body: "Welcome. This is the live terminal UI, backed by the single-owner app loop."
                .to_owned(),
            timestamp: "09:41".to_owned(),
            direction: MessageDirection::Incoming,
            delivery: DeliveryState::Read,
            reply_to: None,
        },
        MessageView {
            id: MessageId(2),
            sender: "You".to_owned(),
            body: "Dense, focus-driven, and no keyboard modes.".to_owned(),
            timestamp: "09:42".to_owned(),
            direction: MessageDirection::Outgoing,
            delivery: DeliveryState::Read,
            reply_to: Some(MessageId(1)),
        },
        MessageView {
            id: MessageId(3),
            sender: "Popgram".to_owned(),
            body: "Press ? for exhaustive context help. Type or paste in any open Chat.".to_owned(),
            timestamp: "09:43".to_owned(),
            direction: MessageDirection::Incoming,
            delivery: DeliveryState::Sent,
            reply_to: None,
        },
    ]
}

#[cfg(test)]
mod tests {
    use std::io;
    use std::num::NonZeroUsize;
    use std::path::PathBuf;
    use std::sync::mpsc;
    use std::time::Duration;

    use popgram_app::{Action, ChatId, Effect, Intent, MessageId, MessageView, bounded_channels};
    use popgram_telegram::{LoginCodeDelivery, LoginCodeDeliveryMethod};
    use popgram_tui::UiEvent;

    use super::{
        ApplicationBackend, ApplicationUi, Error, PRIMARY_DC_ENDPOINT, Result, error_lines,
        finish_state_owner, login_code_delivery_message, login_code_delivery_method_name,
        parse_arguments, recv_update, run_backend_loop, run_ui_loop, seconds_until_at,
        spawn_state_owner,
    };

    struct BlockingHistoryBackend {
        started: mpsc::Sender<()>,
        release: mpsc::Receiver<()>,
    }

    impl ApplicationBackend for BlockingHistoryBackend {
        fn load_chat(&mut self, _chat: ChatId) -> Result<Vec<MessageView>> {
            self.started
                .send(())
                .expect("test should observe history loading");
            self.release
                .recv()
                .expect("test should release history loading");
            Ok(Vec::new())
        }

        fn send_message(
            &mut self,
            _chat: ChatId,
            _text: String,
            _reply_to: Option<MessageId>,
        ) -> Result<MessageView> {
            unreachable!("the responsiveness fixture never sends a Message")
        }
    }

    struct QuitUi {
        read: mpsc::Sender<()>,
    }

    impl ApplicationUi for QuitUi {
        fn draw(&mut self, _view: &popgram_app::View) -> popgram_tui::Result<()> {
            Ok(())
        }

        async fn next_event(&mut self, _view: &popgram_app::View) -> popgram_tui::Result<UiEvent> {
            self.read
                .send(())
                .expect("test should observe terminal input polling");
            Ok(UiEvent::Intent(Intent::Action(Action::Quit)))
        }
    }

    #[test]
    fn chat_history_loading_does_not_block_terminal_input() {
        let capacity = NonZeroUsize::new(8).expect("fixture capacity should be positive");
        let (handle, channels) = bounded_channels(capacity);
        let state_owner = spawn_state_owner(channels).expect("state owner should start");
        let mut update = recv_update(&handle.updates).expect("initial view should arrive");
        update.effect = Some(Effect::LoadChat { chat: ChatId(1) });
        let adapter_inputs = handle.inputs.clone();
        let (effects_tx, effects_rx) = async_channel::bounded(capacity.get());
        let (shutdown_tx, shutdown_rx) = async_channel::bounded(1);

        let (load_started_tx, load_started_rx) = mpsc::channel();
        let (release_load_tx, release_load_rx) = mpsc::channel();
        let (read_tx, read_rx) = mpsc::channel();
        let (ui_finished_tx, ui_finished_rx) = mpsc::channel();
        let ui_worker = std::thread::spawn(move || {
            let mut terminal = QuitUi { read: read_tx };
            let result = futures_lite::future::block_on(run_ui_loop(
                &mut terminal,
                &handle.inputs,
                &handle.updates,
                &effects_tx,
                &shutdown_rx,
                update,
            ));
            ui_finished_tx
                .send(())
                .expect("test should observe the UI worker finishing");
            result
        });
        let backend_worker = std::thread::spawn(move || {
            let mut backend = BlockingHistoryBackend {
                started: load_started_tx,
                release: release_load_rx,
            };
            run_backend_loop(&mut backend, &adapter_inputs, &effects_rx)
        });

        load_started_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("history loading should start");
        let input_was_polled = read_rx.recv_timeout(Duration::from_millis(50)).is_ok();
        let quit_completed = ui_finished_rx
            .recv_timeout(Duration::from_millis(50))
            .is_ok();
        release_load_tx
            .send(())
            .expect("history loading should be released");
        backend_worker
            .join()
            .expect("backend loop should not panic")
            .expect("backend loop should stop cleanly");
        let _ = shutdown_tx.send_blocking(());
        ui_worker
            .join()
            .expect("UI loop should not panic")
            .expect("UI loop should stop cleanly");
        finish_state_owner(state_owner).expect("state owner should stop cleanly");

        assert!(input_was_polled, "history loading blocked terminal input");
        assert!(quit_completed, "history loading blocked terminal shutdown");
    }

    #[test]
    fn a_full_effect_queue_fails_instead_of_blocking_terminal_input() {
        let capacity = NonZeroUsize::new(1).expect("fixture capacity should be positive");
        let (handle, channels) = bounded_channels(capacity);
        let state_owner = spawn_state_owner(channels).expect("state owner should start");
        let mut update = recv_update(&handle.updates).expect("initial view should arrive");
        update.effect = Some(Effect::LoadChat { chat: ChatId(1) });
        let (effects_tx, _effects_rx) = async_channel::bounded(capacity.get());
        effects_tx
            .send_blocking(Effect::Reconnect)
            .expect("fixture should fill the effect queue");
        let (_shutdown_tx, shutdown_rx) = async_channel::bounded(1);
        let (read_tx, _read_rx) = mpsc::channel();
        let mut terminal = QuitUi { read: read_tx };

        let error = futures_lite::future::block_on(run_ui_loop(
            &mut terminal,
            &handle.inputs,
            &handle.updates,
            &effects_tx,
            &shutdown_rx,
            update,
        ))
        .expect_err("a saturated backend queue should be reported");

        drop(handle.inputs);
        finish_state_owner(state_owner).expect("state owner should stop cleanly");
        assert!(matches!(error, Error::EffectsFull));
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
    fn command_line_paths_and_demo_are_parsed_as_overrides() {
        let parsed = parse_arguments([
            "--demo".to_owned(),
            "--data-dir".to_owned(),
            "/tmp/popgram-data".to_owned(),
            "--cache-dir".to_owned(),
            "/tmp/popgram-cache".to_owned(),
        ])
        .expect("valid command line should parse");

        assert!(parsed.demo);
        assert_eq!(
            parsed.data.expect("data override should exist"),
            PathBuf::from("/tmp/popgram-data")
        );
        assert_eq!(
            parsed.cache.expect("cache override should exist"),
            PathBuf::from("/tmp/popgram-cache")
        );
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
