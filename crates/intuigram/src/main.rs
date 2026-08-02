use std::collections::VecDeque;
use std::env;
use std::future::{Future, poll_fn};
use std::io::{self, Write};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::PathBuf;
use std::pin::Pin;
use std::task::Poll;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use intuigram_app::{
    AdapterEvent, App, Bootstrap, ChatId, ChatView, DeliveryState, Effect, FolderView, Input,
    MessageDirection, MessageId, MessageView, Update,
};
use intuigram_config::{Config, ConfigLoader, Overrides, PlatformDefaults};
use intuigram_store::{
    AccountDatabase, AccountId, AccountRecord, GlobalDatabase, SessionMaterial, StoreLayout,
};
use intuigram_telegram::{
    ApplicationCredentials, AuthorizedUser, Client, CodeRequest, CodeSignIn, LoginCodeDelivery,
    LoginCodeDeliveryMethod, LoginCodeToken, QrLogin, Session,
};
use intuigram_tui::{QrLoginAction, QrLoginUi, TerminalEvents, TerminalUi, UiEvent};
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

    #[snafu(display("Telegram setting {setting} is required; configure it or use --demo"))]
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
    demo: bool,
    help: bool,
}

enum Backend {
    Demo {
        next_message_id: i64,
    },
    Telegram {
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
    async fn load_chat(&mut self, chat: ChatId) -> Result<Vec<MessageView>> {
        match self {
            Self::Demo { .. } => Ok(demo_messages()),
            Self::Telegram { client, .. } => client.history(chat, 100).await.context(TelegramSnafu),
        }
    }

    async fn send_message(
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
                client,
                next_local_message_id,
                ..
            } => {
                client
                    .send_text(chat, text.clone(), reply_to)
                    .await
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

    async fn execute(&mut self, effect: Effect) -> Result<Option<AdapterEvent>> {
        match effect {
            Effect::Quit | Effect::Reconnect => Ok(None),
            Effect::LoadChat { chat } => {
                let messages = self.load_chat(chat).await?;
                Ok(Some(AdapterEvent::ChatLoaded { chat, messages }))
            }
            Effect::SendMessage {
                chat,
                text,
                reply_to,
            } => {
                let message = self.send_message(chat, text, reply_to).await?;
                Ok(Some(AdapterEvent::MessageAdded { chat, message }))
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
    let (backend, bootstrap) = if arguments.demo {
        (
            Backend::Demo {
                next_message_id: 10_000,
            },
            demo_data(),
        )
    } else {
        initialize_telegram(&config, &layout, &global).await?
    };

    let mut terminal = TerminalUi::enter().context(TerminalSnafu)?;
    let mut events = TerminalEvents::new().context(TerminalSnafu)?;
    run_application(&mut terminal, &mut events, backend, bootstrap).await
}

trait ApplicationBackend: Sized + 'static {
    async fn execute(&mut self, effect: Effect) -> Result<Option<AdapterEvent>>;
}

impl ApplicationBackend for Backend {
    async fn execute(&mut self, effect: Effect) -> Result<Option<AdapterEvent>> {
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

struct BackendCompletion<B> {
    backend: B,
    result: Result<Option<AdapterEvent>>,
}

type PendingEffect<B> = Pin<Box<dyn Future<Output = BackendCompletion<B>>>>;

enum ApplicationWake<B> {
    Terminal(intuigram_tui::Result<crossterm::event::Event>),
    Backend(BackendCompletion<B>),
}

fn start_effect<B: ApplicationBackend>(mut backend: B, effect: Effect) -> PendingEffect<B> {
    Box::pin(async move {
        let result = backend.execute(effect).await;
        BackendCompletion { backend, result }
    })
}

fn enqueue_effect<B>(
    pending: &mut VecDeque<Effect>,
    active: &Option<PendingEffect<B>>,
    effect: Option<Effect>,
) -> Result<bool> {
    let Some(effect) = effect else {
        return Ok(false);
    };
    if effect == Effect::Quit {
        return Ok(true);
    }
    if pending.len() + usize::from(active.is_some()) >= EFFECT_CAPACITY {
        return EffectsFullSnafu {
            capacity: EFFECT_CAPACITY,
        }
        .fail();
    }
    pending.push_back(effect);
    Ok(false)
}

async fn run_application<U, E, B>(
    terminal: &mut U,
    events: &mut E,
    backend: B,
    bootstrap: Bootstrap,
) -> Result<()>
where
    U: ApplicationUi,
    E: ApplicationEvents,
    B: ApplicationBackend,
{
    let mut app = App::new();
    let mut update = app.transition(Input::Adapter(AdapterEvent::Bootstrap(bootstrap)));
    let mut backend = Some(backend);
    let mut active_effect: Option<PendingEffect<B>> = None;
    let mut pending_effects = VecDeque::with_capacity(EFFECT_CAPACITY);

    loop {
        terminal.draw(&update.view).context(TerminalSnafu)?;
        if enqueue_effect(&mut pending_effects, &active_effect, update.effect.take())? {
            return Ok(());
        }

        if active_effect.is_none()
            && let Some(effect) = pending_effects.pop_front()
        {
            let available = backend
                .take()
                .expect("backend is available whenever no effect owns it");
            active_effect = Some(start_effect(available, effect));
        }

        let wake = poll_fn(|cx| {
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
            ApplicationWake::Backend(completion) => {
                active_effect = None;
                backend = Some(completion.backend);
                if let Some(event) = completion.result? {
                    update = app.transition(Input::Adapter(event));
                } else {
                    update = Update {
                        view: app.view(),
                        effect: None,
                    };
                }
            }
        }
    }
}

async fn initialize_telegram(
    config: &Config,
    layout: &StoreLayout,
    global: &GlobalDatabase,
) -> Result<(Backend, Bootstrap)> {
    let credentials = telegram_credentials(config)?;
    let accounts = global.accounts().context(ReadAccountRegistrySnafu)?;
    if let Some(account) = accounts.iter().find(|account| account.active) {
        return resume_account(credentials, layout, account).await;
    }
    authorize_new_account(&credentials, config, layout, global).await
}

async fn resume_account(
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
    let mut client = Client::connect_existing(credentials, &session, identity)
        .await
        .context(TelegramSnafu)?;
    let bootstrap = client.bootstrap(100).await.context(TelegramSnafu)?;
    Ok((
        Backend::Telegram {
            client: Box::new(client),
            _database: database,
            next_local_message_id: 0,
        },
        bootstrap,
    ))
}

async fn authorize_new_account(
    credentials: &ApplicationCredentials,
    config: &Config,
    layout: &StoreLayout,
    global: &GlobalDatabase,
) -> Result<(Backend, Bootstrap)> {
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
    Ok((
        Backend::Telegram {
            client: Box::new(client),
            _database: database,
            next_local_message_id: 0,
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
           --demo                  Run without Telegram credentials or network access\n\
           --config-dir PATH       Override the platform config directory\n\
           --data-dir PATH         Override the platform data directory\n\
           --cache-dir PATH        Override the platform cache directory\n\
           --downloads-dir PATH    Override the platform Downloads directory\n\
           -h, --help              Print this help\n\n\
         Configure telegram.api_id and telegram.api_hash in config.toml, YAML, JSON, or the\n\
         INTUIGRAM_TELEGRAM__API_ID and INTUIGRAM_TELEGRAM__API_HASH environment variables."
    );
}

fn demo_data() -> Bootstrap {
    Bootstrap {
        account_name: "Intuigram Demo".to_owned(),
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
            },
            ChatView {
                id: ChatId(101),
                title: "Intuigram Contributors".to_owned(),
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
            sender: "Intuigram".to_owned(),
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
            sender: "Intuigram".to_owned(),
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
    use std::cell::{Cell, RefCell};
    use std::collections::VecDeque;
    use std::io;
    use std::path::PathBuf;
    use std::rc::Rc;
    use std::task::{Context, Poll};

    use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
    use intuigram_app::{Action, AdapterEvent, Effect, Intent};
    use intuigram_telegram::{LoginCodeDelivery, LoginCodeDeliveryMethod};
    use intuigram_tui::UiEvent;

    use super::{
        ApplicationBackend, ApplicationEvents, ApplicationUi, EFFECT_CAPACITY, Error,
        PRIMARY_DC_ENDPOINT, PendingEffect, Result, demo_data, enqueue_effect, error_lines,
        login_code_delivery_message, login_code_delivery_method_name, parse_arguments,
        run_application, seconds_until_at,
    };

    struct PendingHistoryBackend {
        polls: Rc<Cell<usize>>,
    }

    impl ApplicationBackend for PendingHistoryBackend {
        async fn execute(&mut self, effect: Effect) -> Result<Option<AdapterEvent>> {
            let Effect::LoadChat { chat } = effect else {
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
        let runtime = compio::runtime::Runtime::new().expect("test runtime should initialize");

        runtime
            .block_on(run_application(
                &mut terminal,
                &mut events,
                backend,
                demo_data(),
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
        let mut pending = VecDeque::from(vec![Effect::Reconnect; EFFECT_CAPACITY]);
        let active = None::<PendingEffect<PendingHistoryBackend>>;

        let error = enqueue_effect(&mut pending, &active, Some(Effect::Reconnect))
            .expect_err("a saturated effect queue should be reported");

        assert!(matches!(error, Error::EffectsFull { .. }));
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
            "/tmp/intuigram-data".to_owned(),
            "--cache-dir".to_owned(),
            "/tmp/intuigram-cache".to_owned(),
        ])
        .expect("valid command line should parse");

        assert!(parsed.demo);
        assert_eq!(
            parsed.data.expect("data override should exist"),
            PathBuf::from("/tmp/intuigram-data")
        );
        assert_eq!(
            parsed.cache.expect("cache override should exist"),
            PathBuf::from("/tmp/intuigram-cache")
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
