use std::env;
use std::io::{self, Write};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::num::NonZeroUsize;
use std::path::PathBuf;
use std::thread::{self, JoinHandle};

use popgram_app::{
    AdapterEvent, App, AppChannels, Bootstrap, ChatId, ChatView, DeliveryState, Effect, FolderView,
    Input, MessageDirection, MessageId, MessageView, bounded_channels,
};
use popgram_config::{Config, ConfigLoader, Overrides, PlatformDefaults};
use popgram_store::{
    AccountDatabase, AccountId, AccountRecord, GlobalDatabase, SessionMaterial, StoreLayout,
};
use popgram_telegram::{
    ApplicationCredentials, AuthorizedUser, Client, CodeRequest, CodeSignIn, Session,
};
use popgram_tui::{TerminalUi, UiEvent};
use snafu::{OptionExt, ResultExt, Snafu};

const PRIMARY_DC_ID: i32 = 2;
const PRIMARY_DC_ENDPOINT: SocketAddr =
    SocketAddr::new(IpAddr::V4(Ipv4Addr::new(149, 154, 167, 40)), 443);

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

    #[snafu(display("failed to start application state owner"))]
    SpawnStateOwner { source: io::Error },

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

fn main() {
    if let Err(error) = run() {
        eprintln!("popgram: {error}");
        std::process::exit(1);
    }
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
    let mut terminal = TerminalUi::enter().context(TerminalSnafu)?;

    'application: loop {
        terminal.draw(&update.view).context(TerminalSnafu)?;
        if let Some(effect) = update.effect.take() {
            match effect {
                Effect::Quit => break 'application,
                Effect::Reconnect => {}
                Effect::LoadChat { chat } => {
                    let messages = backend.load_chat(chat)?;
                    send_input(
                        &handle.inputs,
                        Input::Adapter(AdapterEvent::ChatLoaded { chat, messages }),
                    )?;
                    update = recv_update(&handle.updates)?;
                    continue;
                }
                Effect::SendMessage {
                    chat,
                    text,
                    reply_to,
                } => {
                    let message = backend.send_message(chat, text, reply_to)?;
                    send_input(
                        &handle.inputs,
                        Input::Adapter(AdapterEvent::MessageAdded(message)),
                    )?;
                    update = recv_update(&handle.updates)?;
                    continue;
                }
            }
        }
        match terminal.read_event(&update.view).context(TerminalSnafu)? {
            UiEvent::Redraw => continue,
            UiEvent::Intent(intent) => send_input(&handle.inputs, Input::Intent(intent))?,
        }
        update = recv_update(&handle.updates)?;
    }

    drop(terminal);
    drop(handle.inputs);
    drop(handle.updates);
    finish_state_owner(state_owner)
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
    let (mut client, session) =
        if let Some(stored) = pending.session().context(AccountDatabaseSnafu)? {
            let session = telegram_session(&stored)?;
            let client = runtime
                .block_on(Client::connect_pending(credentials.clone(), &session))
                .context(TelegramSnafu)?;
            (client, session)
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
    let phone_number = match config.telegram.phone_number.as_deref() {
        Some(number) => number.to_owned(),
        None => prompt("Phone number", "phone number")?,
    };
    let user = match runtime
        .block_on(client.request_login_code(phone_number))
        .context(TelegramSnafu)?
    {
        CodeRequest::AlreadyAuthorized(user) => user,
        CodeRequest::Sent(token) => {
            let code = prompt("Login code", "login code")?;
            match runtime
                .block_on(client.sign_in_with_code(token, code))
                .context(TelegramSnafu)?
            {
                CodeSignIn::Authorized(user) => user,
                CodeSignIn::PasswordRequired(password) => {
                    if let Some(hint) = password.hint {
                        println!("2FA password hint: {hint}");
                    }
                    let password =
                        rpassword::prompt_password("2FA password: ").context(PromptSnafu {
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
                        .context(TelegramSnafu)?
                }
            }
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
    use std::path::PathBuf;

    use super::parse_arguments;

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
}
