//! Telegram API orchestration and Intuigram-owned normalization.

use std::collections::HashMap;
use std::fmt;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicI64, Ordering};
use std::time::Duration;

use base64::Engine as _;
use compio_mtproto::{
    AbridgedConnection, AuthKeyMaterial, EncryptedConnection, InvocationError, generate_auth_key,
};
use grammers_crypto::two_factor_auth::{calculate_2fa, check_p_and_g};
use grammers_tl_types as tl;
use grammers_tl_types::Deserializable as _;
use intuigram_app::{
    Bootstrap, ChatId, ChatView, DeliveryState, FolderView, MessageDirection, MessageId,
    MessageView,
};
use snafu::{OptionExt, ResultExt, Snafu};

static QR_PING_ID: AtomicI64 = AtomicI64::new(1);
const MAX_LOGIN_RESTARTS: usize = 3;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum LoginErrorAction {
    Restart,
    RequestPassword,
    Propagate,
}

/// Telegram application credentials supplied by a technical user.
#[derive(Clone)]
pub struct ApplicationCredentials {
    /// Numeric API identifier from my.telegram.org.
    pub api_id: i32,
    api_hash: String,
}

impl ApplicationCredentials {
    /// Creates application credentials without exposing the API hash through
    /// `Debug`.
    #[must_use]
    pub fn new(api_id: i32, api_hash: impl Into<String>) -> Self {
        Self {
            api_id,
            api_hash: api_hash.into(),
        }
    }
}

/// Continuation token for a delivered Telegram login code.
pub struct LoginCodeToken {
    phone_number: String,
    phone_code_hash: String,
    delivery: LoginCodeDelivery,
    next_delivery: Option<LoginCodeDeliveryMethod>,
    next_delivery_after: Option<i32>,
}

impl LoginCodeToken {
    /// Describes where Telegram sent the current login code.
    #[must_use]
    pub const fn delivery(&self) -> &LoginCodeDelivery {
        &self.delivery
    }

    /// Describes the fallback delivery method Telegram may allow next.
    #[must_use]
    pub const fn next_delivery(&self) -> Option<LoginCodeDeliveryMethod> {
        self.next_delivery
    }

    /// Returns the server-advertised wait before fallback delivery is allowed.
    #[must_use]
    pub const fn next_delivery_after(&self) -> Option<i32> {
        self.next_delivery_after
    }
}

/// Channel and shape of a login code Telegram says it delivered.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LoginCodeDelivery {
    /// A numeric code sent as a Telegram service message to another session.
    TelegramApp { length: i32 },

    /// A numeric code sent by SMS.
    Sms { length: i32 },

    /// A numeric code delivered by a phone call.
    PhoneCall { length: i32 },

    /// A code inferred from the caller number matching this pattern.
    FlashCall { pattern: String },

    /// A code formed from the suffix of a missed-call number.
    MissedCall { prefix: String, length: i32 },

    /// A numeric code sent to the masked email address.
    Email { pattern: String, length: i32 },

    /// Telegram requires an email to be configured before continuing.
    EmailSetupRequired,

    /// A numeric code delivered through the supplied Fragment URL.
    Fragment { url: String, length: i32 },

    /// A numeric code delivered through Firebase SMS.
    FirebaseSms { length: i32 },

    /// A word delivered by SMS, optionally with an expected beginning.
    SmsWord { beginning: Option<String> },

    /// A phrase delivered by SMS, optionally with an expected beginning.
    SmsPhrase { beginning: Option<String> },
}

/// Login-code channel Telegram may offer after the current delivery attempt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LoginCodeDeliveryMethod {
    /// A numeric SMS.
    Sms,

    /// A voice call.
    PhoneCall,

    /// A caller-number pattern.
    FlashCall,

    /// A missed-call number suffix.
    MissedCall,

    /// Delivery through Fragment.
    Fragment,
}

/// Password prompt metadata when Telegram 2FA is enabled.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PasswordPrompt {
    /// Optional user-configured password hint.
    pub hint: Option<String>,
}

/// Intuigram-owned identity returned after authentication.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuthorizedUser {
    /// Stable Telegram user ID.
    pub id: i64,
    /// Best available display name.
    pub display_name: String,
    /// Username without `@`, when configured.
    pub username: Option<String>,
}

/// A Telegram QR-login token suitable for display to the user.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QrLoginToken {
    uri: String,
    expires_at: i32,
}

impl QrLoginToken {
    /// Returns the `tg://login` URI encoded by the QR symbol.
    #[must_use]
    pub fn uri(&self) -> &str {
        &self.uri
    }

    /// Returns the token expiry as a Unix timestamp.
    #[must_use]
    pub const fn expires_at(&self) -> i32 {
        self.expires_at
    }
}

/// Opaque continuation required when Telegram moves QR login to another data
/// center.
pub struct QrLoginMigration {
    dc_id: i32,
    token: Vec<u8>,
}

impl QrLoginMigration {
    /// Returns the target Telegram data-center number.
    #[must_use]
    pub const fn dc_id(&self) -> i32 {
        self.dc_id
    }
}

/// Current state of Telegram QR authentication.
pub enum QrLogin {
    /// Display this token while waiting for another Telegram client to scan it.
    Pending(QrLoginToken),

    /// Continue authorization on another Telegram data center.
    Migrate(QrLoginMigration),

    /// The scanned Account requires its Telegram 2FA password.
    PasswordRequired(PasswordPrompt),

    /// Authentication completed.
    Authorized(AuthorizedUser),
}

/// Authorization and connection state that must survive process restarts.
#[derive(Clone, Eq, PartialEq)]
pub struct Session {
    /// Telegram data-center number.
    pub dc_id: i32,
    /// Direct endpoint selected for the data center.
    pub endpoint: SocketAddr,
    /// Secret authorization key.
    auth_key: [u8; 256],
    /// Difference between local and Telegram server time.
    pub time_offset: i32,
    /// Initial server salt established by the key exchange.
    pub first_salt: i64,
}

impl Session {
    /// Reconstructs a session loaded from protected Account storage.
    #[must_use]
    pub const fn new(
        dc_id: i32,
        endpoint: SocketAddr,
        auth_key: [u8; 256],
        time_offset: i32,
        first_salt: i64,
    ) -> Self {
        Self {
            dc_id,
            endpoint,
            auth_key,
            time_offset,
            first_salt,
        }
    }

    /// Copies the key into durable Account storage.
    #[must_use]
    pub const fn auth_key(&self) -> [u8; 256] {
        self.auth_key
    }
}

impl fmt::Debug for Session {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Session")
            .field("dc_id", &self.dc_id)
            .field("endpoint", &self.endpoint)
            .field("auth_key", &"[REDACTED]")
            .field("time_offset", &self.time_offset)
            .field("first_salt", &self.first_salt)
            .finish()
    }
}

/// Result of sending a login code.
pub enum CodeRequest {
    /// Telegram delivered a code and expects it to be submitted.
    Sent(LoginCodeToken),
    /// This authorization key was already signed in.
    AlreadyAuthorized(AuthorizedUser),
}

/// Result of submitting a login code.
pub enum CodeSignIn {
    /// Authentication completed.
    Authorized(AuthorizedUser),
    /// Telegram requires the Account's 2FA password.
    PasswordRequired(PasswordPrompt),
}

/// Failure while authenticating or invoking Telegram.
#[derive(Debug, Snafu)]
#[snafu(visibility(pub))]
pub enum Error {
    /// The direct Telegram connection failed.
    #[snafu(display("failed to connect to Telegram at {endpoint}"))]
    Connect {
        /// Telegram data-center endpoint.
        endpoint: SocketAddr,
        /// Underlying transport failure.
        source: compio_mtproto::TransportError,
    },

    /// A fresh `MTProto` authorization key could not be generated.
    #[snafu(display("failed to generate Telegram authorization key"))]
    GenerateKey {
        /// Underlying key-exchange failure.
        source: compio_mtproto::KeyExchangeError,
    },

    /// Telegram rejected an API invocation.
    #[snafu(display("Telegram API invocation failed"))]
    Invoke {
        /// Underlying encrypted invocation failure.
        source: InvocationError,
    },

    /// Telegram returned a login-code result requiring a paid official flow.
    #[snafu(display("Telegram requires a paid or official-client login-code flow"))]
    LoginPaymentRequired,

    /// Telegram requires Account creation in an official client.
    #[snafu(display("Telegram Account sign-up must be completed in an official client"))]
    SignUpRequired,

    /// Telegram returned an empty user where an authorized identity was
    /// required.
    #[snafu(display("Telegram returned an empty authorized user"))]
    EmptyAuthorizedUser,

    /// Telegram requested 2FA but did not supply complete SRP parameters.
    #[snafu(display("Telegram returned incomplete 2FA password parameters"))]
    IncompletePasswordParameters,

    /// Telegram supplied unsupported or unsafe SRP parameters.
    #[snafu(display("Telegram returned unsupported or unsafe 2FA parameters"))]
    UnsupportedPasswordAlgorithm,

    /// No 2FA challenge is pending.
    #[snafu(display("no Telegram 2FA challenge is pending"))]
    MissingPasswordChallenge,

    /// Telegram did not return complete dialog data for a zero-hash request.
    #[snafu(display("Telegram returned dialogs without dialog contents"))]
    DialogsNotModified,

    /// The requested Chat is not present in the current Telegram peer cache.
    #[snafu(display("Telegram peer for Chat {chat_id} is unavailable"))]
    PeerUnavailable {
        /// Intuigram Chat identifier.
        chat_id: i64,
    },

    /// A secure random message identifier could not be generated.
    #[snafu(display("failed to generate Telegram message random ID"))]
    RandomId {
        /// Operating-system random source failure.
        source: getrandom::Error,
    },

    /// A Intuigram Message ID could not be represented by Telegram's API.
    #[snafu(display("Message ID {message_id} is outside Telegram's signed 32-bit domain"))]
    InvalidMessageId {
        /// Invalid Intuigram Message ID.
        message_id: i64,
    },

    /// Phone authorization must continue on another Telegram data center.
    #[snafu(display("Telegram phone authorization must migrate to data center {dc_id}"))]
    PhoneMigration {
        /// Target Telegram data-center number.
        dc_id: i32,
    },

    /// Intuigram connected to Telegram's isolated test environment.
    #[snafu(display("connected to a Telegram test data center instead of production"))]
    TestDataCenter,
}

/// Result returned by Telegram operations.
pub type Result<T, E = Error> = std::result::Result<T, E>;

impl Error {
    /// Returns the target data center for a phone-login migration.
    #[must_use]
    pub const fn phone_migration_dc(&self) -> Option<i32> {
        match self {
            Self::PhoneMigration { dc_id } => Some(*dc_id),
            _ => None,
        }
    }

    /// Reports whether this connection reached Telegram's isolated test
    /// environment.
    #[must_use]
    pub const fn is_test_data_center(&self) -> bool {
        matches!(self, Self::TestDataCenter)
    }
}

/// Sequential Telegram API client built on Intuigram's Compio `MTProto` sender.
pub struct Client {
    connection: EncryptedConnection,
    credentials: ApplicationCredentials,
    password: Option<tl::types::account::Password>,
    identity: Option<AuthorizedUser>,
    peers: HashMap<ChatId, tl::enums::InputPeer>,
    names: HashMap<ChatId, String>,
    data_centers: HashMap<i32, SocketAddr>,
}

impl Client {
    /// Connects to a Telegram data center and generates fresh authorization
    /// material.
    pub async fn connect_new(
        dc_id: i32,
        endpoint: SocketAddr,
        credentials: ApplicationCredentials,
    ) -> Result<(Self, Session)> {
        let mut transport = AbridgedConnection::connect(endpoint)
            .await
            .context(ConnectSnafu { endpoint })?;
        let material = generate_auth_key(&mut transport)
            .await
            .context(GenerateKeySnafu)?;
        let session = Session {
            dc_id,
            endpoint,
            auth_key: material.auth_key,
            time_offset: material.time_offset,
            first_salt: material.first_salt,
        };
        let mut client = Self {
            connection: EncryptedConnection::new(transport, &material),
            credentials,
            password: None,
            identity: None,
            peers: HashMap::new(),
            names: HashMap::new(),
            data_centers: HashMap::new(),
        };
        client.initialize().await?;
        Ok((client, session))
    }

    /// Reconnects with authorization material loaded from Account storage.
    pub async fn connect_existing(
        credentials: ApplicationCredentials,
        session: &Session,
        identity: AuthorizedUser,
    ) -> Result<Self> {
        Self::connect_with_session(credentials, session, Some(identity)).await
    }

    /// Reconnects an incomplete login using authorization material saved in
    /// `.pending.db`.
    pub async fn connect_pending(
        credentials: ApplicationCredentials,
        session: &Session,
    ) -> Result<Self> {
        Self::connect_with_session(credentials, session, None).await
    }

    async fn connect_with_session(
        credentials: ApplicationCredentials,
        session: &Session,
        identity: Option<AuthorizedUser>,
    ) -> Result<Self> {
        let endpoint = session.endpoint;
        let transport = AbridgedConnection::connect(endpoint)
            .await
            .context(ConnectSnafu { endpoint })?;
        let material = AuthKeyMaterial {
            auth_key: session.auth_key,
            time_offset: session.time_offset,
            first_salt: session.first_salt,
        };
        let mut client = Self {
            connection: EncryptedConnection::new(transport, &material),
            credentials,
            password: None,
            identity,
            peers: HashMap::new(),
            names: HashMap::new(),
            data_centers: HashMap::new(),
        };
        client.initialize().await?;
        Ok(client)
    }

    /// Exports a fresh QR-login token for this authorization key.
    pub async fn export_qr_login(&mut self) -> Result<QrLogin> {
        let mut restarts = 0;
        loop {
            let response = self
                .connection
                .invoke(&tl::functions::auth::ExportLoginToken {
                    api_id: self.credentials.api_id,
                    api_hash: self.credentials.api_hash.clone(),
                    except_ids: self.identity.iter().map(|identity| identity.id).collect(),
                })
                .await;
            match response {
                Ok(response) => return self.normalize_qr_login(response),
                Err(source) => match login_error_action(&source) {
                    LoginErrorAction::Restart if restarts < MAX_LOGIN_RESTARTS => {
                        restarts += 1;
                        compio::time::sleep(Duration::from_millis(250)).await;
                    }
                    LoginErrorAction::RequestPassword => {
                        return self
                            .begin_password_challenge()
                            .await
                            .map(QrLogin::PasswordRequired);
                    }
                    LoginErrorAction::Restart | LoginErrorAction::Propagate => {
                        return Err(Error::Invoke { source });
                    }
                },
            }
        }
    }

    /// Imports a QR-login token after Telegram requests data-center migration.
    pub async fn import_qr_login(&mut self, migration: QrLoginMigration) -> Result<QrLogin> {
        let response = self
            .connection
            .invoke(&tl::functions::auth::ImportLoginToken {
                token: migration.token,
            })
            .await;
        match response {
            Ok(response) => self.normalize_qr_login(response),
            Err(source) if login_error_action(&source) == LoginErrorAction::RequestPassword => self
                .begin_password_challenge()
                .await
                .map(QrLogin::PasswordRequired),
            Err(source) => Err(Error::Invoke { source }),
        }
    }

    /// Polls once for Telegram's `updateLoginToken` notification.
    ///
    /// The short delay keeps the server from being flooded while the poll RPC
    /// also drives the underlying `MTProto` receive loop.
    pub async fn poll_qr_login(&mut self) -> Result<bool> {
        if take_login_token_update(&mut self.connection) {
            return Ok(true);
        }
        compio::time::sleep(Duration::from_millis(500)).await;
        self.connection
            .invoke(&tl::functions::PingDelayDisconnect {
                ping_id: QR_PING_ID.fetch_add(1, Ordering::Relaxed),
                disconnect_delay: 30,
            })
            .await
            .context(InvokeSnafu)?;
        Ok(take_login_token_update(&mut self.connection))
    }

    fn normalize_qr_login(&mut self, response: tl::enums::auth::LoginToken) -> Result<QrLogin> {
        match response {
            tl::enums::auth::LoginToken::Token(token) => Ok(QrLogin::Pending(QrLoginToken {
                uri: qr_login_uri(&token.token),
                expires_at: token.expires,
            })),
            tl::enums::auth::LoginToken::MigrateTo(migration) => {
                Ok(QrLogin::Migrate(QrLoginMigration {
                    dc_id: migration.dc_id,
                    token: migration.token,
                }))
            }
            tl::enums::auth::LoginToken::Success(success) => {
                normalize_authorization(success.authorization).map(|identity| {
                    self.identity = Some(identity.clone());
                    QrLogin::Authorized(identity)
                })
            }
        }
    }

    /// Requests delivery of a Telegram login code.
    pub async fn request_login_code(&mut self, phone_number: String) -> Result<CodeRequest> {
        let mut restarts = 0;
        loop {
            let response = self
                .connection
                .invoke(&tl::functions::auth::SendCode {
                    phone_number: phone_number.clone(),
                    api_id: self.credentials.api_id,
                    api_hash: self.credentials.api_hash.clone(),
                    settings: tl::types::CodeSettings {
                        allow_flashcall: false,
                        current_number: false,
                        allow_app_hash: false,
                        allow_missed_call: false,
                        allow_firebase: false,
                        unknown_number: false,
                        logout_tokens: None,
                        token: None,
                        app_sandbox: None,
                    }
                    .into(),
                })
                .await;
            match response {
                Ok(response) => return self.normalize_code_request(phone_number, response),
                Err(source) => {
                    if let Some(dc_id) = rpc_migration_dc(&source, "PHONE_MIGRATE_") {
                        return PhoneMigrationSnafu { dc_id }.fail();
                    }
                    if login_error_action(&source) == LoginErrorAction::Restart
                        && restarts < MAX_LOGIN_RESTARTS
                    {
                        restarts += 1;
                        compio::time::sleep(Duration::from_millis(250)).await;
                        continue;
                    }
                    return Err(Error::Invoke { source });
                }
            }
        }
    }

    /// Requests Telegram's next available delivery method for a login code.
    pub async fn resend_login_code(&mut self, token: &LoginCodeToken) -> Result<CodeRequest> {
        let response = self
            .connection
            .invoke(&tl::functions::auth::ResendCode {
                phone_number: token.phone_number.clone(),
                phone_code_hash: token.phone_code_hash.clone(),
                reason: None,
            })
            .await
            .context(InvokeSnafu)?;
        self.normalize_code_request(token.phone_number.clone(), response)
    }

    /// Submits the delivered login code.
    pub async fn sign_in_with_code(
        &mut self,
        token: LoginCodeToken,
        code: String,
    ) -> Result<CodeSignIn> {
        let response = self
            .connection
            .invoke(&tl::functions::auth::SignIn {
                phone_number: token.phone_number,
                phone_code_hash: token.phone_code_hash,
                phone_code: Some(code),
                email_verification: None,
            })
            .await;
        match response {
            Ok(authorization) => normalize_authorization(authorization).map(|identity| {
                self.identity = Some(identity.clone());
                CodeSignIn::Authorized(identity)
            }),
            Err(error) if error.is_rpc("SESSION_PASSWORD_NEEDED") => self
                .begin_password_challenge()
                .await
                .map(CodeSignIn::PasswordRequired),
            Err(source) => Err(Error::Invoke { source }),
        }
    }

    async fn begin_password_challenge(&mut self) -> Result<PasswordPrompt> {
        let password: tl::types::account::Password = self
            .connection
            .invoke(&tl::functions::account::GetPassword {})
            .await
            .context(InvokeSnafu)?
            .into();
        let prompt = PasswordPrompt {
            hint: password.hint.clone(),
        };
        self.password = Some(password);
        Ok(prompt)
    }

    fn normalize_code_request(
        &mut self,
        phone_number: String,
        response: tl::enums::auth::SentCode,
    ) -> Result<CodeRequest> {
        match response {
            tl::enums::auth::SentCode::Code(code) => Ok(CodeRequest::Sent(LoginCodeToken {
                phone_number,
                phone_code_hash: code.phone_code_hash,
                delivery: normalize_code_delivery(code.r#type),
                next_delivery: code.next_type.as_ref().map(normalize_code_delivery_method),
                next_delivery_after: code.timeout.filter(|timeout| *timeout >= 0),
            })),
            tl::enums::auth::SentCode::Success(success) => {
                normalize_authorization(success.authorization).map(|identity| {
                    self.identity = Some(identity.clone());
                    CodeRequest::AlreadyAuthorized(identity)
                })
            }
            tl::enums::auth::SentCode::PaymentRequired(_) => LoginPaymentRequiredSnafu.fail(),
        }
    }

    /// Completes Telegram SRP two-factor authentication.
    pub async fn sign_in_with_password(&mut self, password: &[u8]) -> Result<AuthorizedUser> {
        let info = self
            .password
            .take()
            .context(MissingPasswordChallengeSnafu)?;
        let algorithm = info
            .current_algo
            .as_ref()
            .context(IncompletePasswordParametersSnafu)?;
        let (salt1, salt2, prime, generator) = password_parameters(algorithm)?;
        if !check_p_and_g(prime, generator) {
            return UnsupportedPasswordAlgorithmSnafu.fail();
        }
        let server_b = info
            .srp_b
            .as_ref()
            .context(IncompletePasswordParametersSnafu)?;
        let srp_id = info.srp_id.context(IncompletePasswordParametersSnafu)?;
        let (proof, client_a) = calculate_2fa(
            salt1,
            salt2,
            prime,
            generator,
            server_b.clone(),
            info.secure_random,
            password,
        );
        let authorization = self
            .connection
            .invoke(&tl::functions::auth::CheckPassword {
                password: tl::types::InputCheckPasswordSrp {
                    srp_id,
                    a: client_a.to_vec(),
                    m1: proof.to_vec(),
                }
                .into(),
            })
            .await
            .context(InvokeSnafu)?;
        normalize_authorization(authorization).inspect(|identity| {
            self.identity = Some(identity.clone());
        })
    }

    /// Loads the first dialog page and normalizes it into application-owned
    /// data without leaking Telegram TL values.
    pub async fn bootstrap(&mut self, limit: i32) -> Result<Bootstrap> {
        let response = self
            .connection
            .invoke(&tl::functions::messages::GetDialogs {
                exclude_pinned: false,
                folder_id: None,
                offset_date: 0,
                offset_id: 0,
                offset_peer: tl::enums::InputPeer::Empty,
                limit,
                hash: 0,
            })
            .await
            .context(InvokeSnafu)?;
        let (dialogs, messages, chats, users) = dialog_parts(response)?;
        self.update_peer_cache(&chats, &users);
        let top_messages: HashMap<(ChatId, i32), &tl::enums::Message> = messages
            .iter()
            .map(|message| ((message_chat_id(message), message.id()), message))
            .collect();
        let chat_views = dialogs
            .iter()
            .filter_map(|dialog| match dialog {
                tl::enums::Dialog::Dialog(dialog) => {
                    let chat_id = marked_peer_id(&dialog.peer);
                    let title = self
                        .names
                        .get(&chat_id)
                        .cloned()
                        .unwrap_or_else(|| "Inaccessible peer".to_owned());
                    let preview = top_messages
                        .get(&(chat_id, dialog.top_message))
                        .map_or_else(String::new, |message| message_body(message));
                    Some(ChatView {
                        id: chat_id,
                        title,
                        preview,
                        unread: u32::try_from(dialog.unread_count.max(0)).unwrap_or(0),
                        pinned: dialog.pinned,
                    })
                }
                tl::enums::Dialog::Folder(_) => None,
            })
            .collect::<Vec<_>>();
        let initial_messages = match chat_views.first() {
            Some(chat) => self.history(chat.id, 60).await?,
            None => Vec::new(),
        };
        let account_name = self
            .identity
            .as_ref()
            .map_or_else(|| "Telegram".to_owned(), |user| user.display_name.clone());
        Ok(Bootstrap {
            account_name,
            folders: vec![
                FolderView {
                    id: 0,
                    title: "All".to_owned(),
                    unread: chat_views.iter().map(|chat| chat.unread).sum(),
                },
                FolderView {
                    id: 1,
                    title: "Archive".to_owned(),
                    unread: 0,
                },
            ],
            chats: chat_views,
            messages: initial_messages,
        })
    }

    /// Loads and normalizes recent history for one cached Chat.
    pub async fn history(&mut self, chat: ChatId, limit: i32) -> Result<Vec<MessageView>> {
        let peer = self
            .peers
            .get(&chat)
            .cloned()
            .context(PeerUnavailableSnafu { chat_id: chat.0 })?;
        let response = self
            .connection
            .invoke(&tl::functions::messages::GetHistory {
                peer,
                offset_id: 0,
                offset_date: 0,
                add_offset: 0,
                limit,
                max_id: 0,
                min_id: 0,
                hash: 0,
            })
            .await
            .context(InvokeSnafu)?;
        let (mut messages, chats, users) = message_parts(response);
        self.update_peer_cache(&chats, &users);
        messages.reverse();
        Ok(messages
            .iter()
            .filter_map(|message| normalize_message(message, &self.names))
            .collect())
    }

    /// Sends a plain-text Message, optionally as a reply.
    pub async fn send_text(
        &mut self,
        chat: ChatId,
        text: String,
        reply_to: Option<MessageId>,
    ) -> Result<()> {
        let peer = self
            .peers
            .get(&chat)
            .cloned()
            .context(PeerUnavailableSnafu { chat_id: chat.0 })?;
        let mut random_bytes = [0_u8; 8];
        getrandom::fill(&mut random_bytes).context(RandomIdSnafu)?;
        let reply_to = reply_to
            .map(|message| {
                let reply_to_msg_id =
                    i32::try_from(message.0).map_err(|_| Error::InvalidMessageId {
                        message_id: message.0,
                    })?;
                Ok(tl::types::InputReplyToMessage {
                    reply_to_msg_id,
                    top_msg_id: None,
                    reply_to_peer_id: None,
                    quote_text: None,
                    quote_entities: None,
                    quote_offset: None,
                    monoforum_peer_id: None,
                    todo_item_id: None,
                    poll_option: None,
                }
                .into())
            })
            .transpose()?;
        self.connection
            .invoke(&tl::functions::messages::SendMessage {
                no_webpage: false,
                silent: false,
                background: false,
                clear_draft: true,
                noforwards: false,
                update_stickersets_order: false,
                invert_media: false,
                allow_paid_floodskip: false,
                peer,
                reply_to,
                message: text,
                random_id: i64::from_le_bytes(random_bytes),
                reply_markup: None,
                entities: None,
                schedule_date: None,
                schedule_repeat_period: None,
                send_as: None,
                quick_reply_shortcut: None,
                effect: None,
                allow_paid_stars: None,
                suggested_post: None,
                rich_message: None,
            })
            .await
            .context(InvokeSnafu)?;
        Ok(())
    }

    /// Returns a direct IPv4 endpoint advertised by Telegram for a data center.
    #[must_use]
    pub fn data_center_endpoint(&self, dc_id: i32) -> Option<SocketAddr> {
        self.data_centers.get(&dc_id).copied()
    }

    async fn initialize(&mut self) -> Result<()> {
        let config = self
            .connection
            .invoke(&tl::functions::InvokeWithLayer {
                layer: tl::LAYER,
                query: tl::functions::InitConnection {
                    api_id: self.credentials.api_id,
                    device_model: "Terminal".to_owned(),
                    system_version: std::env::consts::OS.to_owned(),
                    app_version: env!("CARGO_PKG_VERSION").to_owned(),
                    system_lang_code: "en".to_owned(),
                    lang_pack: String::new(),
                    lang_code: "en".to_owned(),
                    proxy: None,
                    params: None,
                    query: tl::functions::help::GetConfig {},
                },
            })
            .await
            .context(InvokeSnafu)?;
        let tl::enums::Config::Config(config) = config;
        ensure_production_environment(config.test_mode)?;
        self.data_centers = direct_data_centers(config.dc_options);
        Ok(())
    }

    fn update_peer_cache(&mut self, chats: &[tl::enums::Chat], users: &[tl::enums::User]) {
        for user in users {
            match user {
                tl::enums::User::User(user) => {
                    let id = ChatId(user.id);
                    self.names.insert(id, user_display_name(user));
                    let peer = if user.is_self {
                        Some(tl::enums::InputPeer::PeerSelf)
                    } else {
                        user.access_hash.map(|access_hash| {
                            tl::types::InputPeerUser {
                                user_id: user.id,
                                access_hash,
                            }
                            .into()
                        })
                    };
                    if let Some(peer) = peer {
                        self.peers.insert(id, peer);
                    }
                }
                tl::enums::User::Empty(user) => {
                    self.names
                        .insert(ChatId(user.id), "Inaccessible user".to_owned());
                }
            }
        }
        for chat in chats {
            match chat {
                tl::enums::Chat::Chat(chat) => {
                    let id = ChatId(-chat.id);
                    self.names.insert(id, chat.title.clone());
                    self.peers
                        .insert(id, tl::types::InputPeerChat { chat_id: chat.id }.into());
                }
                tl::enums::Chat::Channel(channel) => {
                    let id = ChatId(mark_channel_id(channel.id));
                    self.names.insert(id, channel.title.clone());
                    if let Some(access_hash) = channel.access_hash {
                        self.peers.insert(
                            id,
                            tl::types::InputPeerChannel {
                                channel_id: channel.id,
                                access_hash,
                            }
                            .into(),
                        );
                    }
                }
                tl::enums::Chat::Forbidden(chat) => {
                    self.names.insert(ChatId(-chat.id), chat.title.clone());
                }
                tl::enums::Chat::ChannelForbidden(channel) => {
                    self.names
                        .insert(ChatId(mark_channel_id(channel.id)), channel.title.clone());
                }
                tl::enums::Chat::Empty(chat) => {
                    self.names
                        .insert(ChatId(-chat.id), "Inaccessible group".to_owned());
                }
            }
        }
    }
}

fn normalize_code_delivery(delivery: tl::enums::auth::SentCodeType) -> LoginCodeDelivery {
    match delivery {
        tl::enums::auth::SentCodeType::App(delivery) => LoginCodeDelivery::TelegramApp {
            length: delivery.length,
        },
        tl::enums::auth::SentCodeType::Sms(delivery) => LoginCodeDelivery::Sms {
            length: delivery.length,
        },
        tl::enums::auth::SentCodeType::Call(delivery) => LoginCodeDelivery::PhoneCall {
            length: delivery.length,
        },
        tl::enums::auth::SentCodeType::FlashCall(delivery) => LoginCodeDelivery::FlashCall {
            pattern: delivery.pattern,
        },
        tl::enums::auth::SentCodeType::MissedCall(delivery) => LoginCodeDelivery::MissedCall {
            prefix: delivery.prefix,
            length: delivery.length,
        },
        tl::enums::auth::SentCodeType::EmailCode(delivery) => LoginCodeDelivery::Email {
            pattern: delivery.email_pattern,
            length: delivery.length,
        },
        tl::enums::auth::SentCodeType::SetUpEmailRequired(_) => {
            LoginCodeDelivery::EmailSetupRequired
        }
        tl::enums::auth::SentCodeType::FragmentSms(delivery) => LoginCodeDelivery::Fragment {
            url: delivery.url,
            length: delivery.length,
        },
        tl::enums::auth::SentCodeType::FirebaseSms(delivery) => LoginCodeDelivery::FirebaseSms {
            length: delivery.length,
        },
        tl::enums::auth::SentCodeType::SmsWord(delivery) => LoginCodeDelivery::SmsWord {
            beginning: delivery.beginning,
        },
        tl::enums::auth::SentCodeType::SmsPhrase(delivery) => LoginCodeDelivery::SmsPhrase {
            beginning: delivery.beginning,
        },
    }
}

const fn normalize_code_delivery_method(
    delivery: &tl::enums::auth::CodeType,
) -> LoginCodeDeliveryMethod {
    match delivery {
        tl::enums::auth::CodeType::Sms => LoginCodeDeliveryMethod::Sms,
        tl::enums::auth::CodeType::Call => LoginCodeDeliveryMethod::PhoneCall,
        tl::enums::auth::CodeType::FlashCall => LoginCodeDeliveryMethod::FlashCall,
        tl::enums::auth::CodeType::MissedCall => LoginCodeDeliveryMethod::MissedCall,
        tl::enums::auth::CodeType::FragmentSms => LoginCodeDeliveryMethod::Fragment,
    }
}

fn direct_data_centers(options: Vec<tl::enums::DcOption>) -> HashMap<i32, SocketAddr> {
    options
        .into_iter()
        .filter_map(|option| {
            let tl::enums::DcOption::Option(option) = option;
            if option.ipv6 || option.media_only || option.cdn || option.tcpo_only {
                return None;
            }
            let ip = option.ip_address.parse().ok()?;
            let port = u16::try_from(option.port).ok()?;
            Some((option.id, SocketAddr::new(ip, port)))
        })
        .collect()
}

fn ensure_production_environment(test_mode: bool) -> Result<()> {
    if test_mode {
        TestDataCenterSnafu.fail()
    } else {
        Ok(())
    }
}

fn rpc_migration_dc(error: &InvocationError, prefix: &str) -> Option<i32> {
    match error {
        InvocationError::Rpc { message, .. } => message.strip_prefix(prefix)?.parse().ok(),
        _ => None,
    }
}

fn login_error_action(error: &InvocationError) -> LoginErrorAction {
    match error {
        InvocationError::Rpc { message, .. }
            if message == "AUTH_RESTART" || message.starts_with("AUTH_RESTART_") =>
        {
            LoginErrorAction::Restart
        }
        InvocationError::Rpc { message, .. } if message == "SESSION_PASSWORD_NEEDED" => {
            LoginErrorAction::RequestPassword
        }
        _ => LoginErrorAction::Propagate,
    }
}

fn qr_login_uri(token: &[u8]) -> String {
    let encoded = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(token);
    format!("tg://login?token={encoded}")
}

fn take_login_token_update(connection: &mut EncryptedConnection) -> bool {
    connection
        .take_updates()
        .iter()
        .any(|update| contains_login_token_update(update))
}

fn contains_login_token_update(bytes: &[u8]) -> bool {
    if let Ok(update) = tl::enums::Update::from_bytes(bytes) {
        return matches!(update, tl::enums::Update::LoginToken);
    }
    tl::enums::Updates::from_bytes(bytes).is_ok_and(|updates| match updates {
        tl::enums::Updates::UpdateShort(update) => {
            matches!(update.update, tl::enums::Update::LoginToken)
        }
        tl::enums::Updates::Combined(updates) => updates
            .updates
            .iter()
            .any(|update| matches!(update, tl::enums::Update::LoginToken)),
        tl::enums::Updates::Updates(updates) => updates
            .updates
            .iter()
            .any(|update| matches!(update, tl::enums::Update::LoginToken)),
        tl::enums::Updates::TooLong
        | tl::enums::Updates::UpdateShortMessage(_)
        | tl::enums::Updates::UpdateShortChatMessage(_)
        | tl::enums::Updates::UpdateShortSentMessage(_) => false,
    })
}

type DialogParts = (
    Vec<tl::enums::Dialog>,
    Vec<tl::enums::Message>,
    Vec<tl::enums::Chat>,
    Vec<tl::enums::User>,
);

fn dialog_parts(dialogs: tl::enums::messages::Dialogs) -> Result<DialogParts> {
    match dialogs {
        tl::enums::messages::Dialogs::Dialogs(dialogs) => Ok((
            dialogs.dialogs,
            dialogs.messages,
            dialogs.chats,
            dialogs.users,
        )),
        tl::enums::messages::Dialogs::Slice(dialogs) => Ok((
            dialogs.dialogs,
            dialogs.messages,
            dialogs.chats,
            dialogs.users,
        )),
        tl::enums::messages::Dialogs::NotModified(_) => DialogsNotModifiedSnafu.fail(),
    }
}

fn message_parts(
    messages: tl::enums::messages::Messages,
) -> (
    Vec<tl::enums::Message>,
    Vec<tl::enums::Chat>,
    Vec<tl::enums::User>,
) {
    match messages {
        tl::enums::messages::Messages::Messages(messages) => {
            (messages.messages, messages.chats, messages.users)
        }
        tl::enums::messages::Messages::Slice(messages) => {
            (messages.messages, messages.chats, messages.users)
        }
        tl::enums::messages::Messages::ChannelMessages(messages) => {
            (messages.messages, messages.chats, messages.users)
        }
        tl::enums::messages::Messages::NotModified(_) => (Vec::new(), Vec::new(), Vec::new()),
    }
}

const fn mark_channel_id(id: i64) -> i64 {
    -1_000_000_000_000 - id
}

const fn marked_peer_id(peer: &tl::enums::Peer) -> ChatId {
    match peer {
        tl::enums::Peer::User(peer) => ChatId(peer.user_id),
        tl::enums::Peer::Chat(peer) => ChatId(-peer.chat_id),
        tl::enums::Peer::Channel(peer) => ChatId(mark_channel_id(peer.channel_id)),
    }
}

fn message_chat_id(message: &tl::enums::Message) -> ChatId {
    match message {
        tl::enums::Message::Empty(_) => ChatId(0),
        tl::enums::Message::Message(message) => marked_peer_id(&message.peer_id),
        tl::enums::Message::Service(message) => marked_peer_id(&message.peer_id),
    }
}

fn normalize_message(
    message: &tl::enums::Message,
    names: &HashMap<ChatId, String>,
) -> Option<MessageView> {
    match message {
        tl::enums::Message::Empty(_) => None,
        tl::enums::Message::Message(message) => {
            let sender_id = message.from_id.as_ref().map(marked_peer_id);
            let sender = if message.out {
                "You".to_owned()
            } else {
                sender_id
                    .and_then(|id| names.get(&id).cloned())
                    .unwrap_or_else(|| "Unknown sender".to_owned())
            };
            let reply_to = message.reply_to.as_ref().and_then(reply_message_id);
            Some(MessageView {
                id: MessageId(i64::from(message.id)),
                sender,
                body: message_body(&tl::enums::Message::Message(message.clone())),
                timestamp: format_timestamp(message.date),
                direction: if message.out {
                    MessageDirection::Outgoing
                } else {
                    MessageDirection::Incoming
                },
                delivery: DeliveryState::Sent,
                reply_to,
            })
        }
        tl::enums::Message::Service(message) => Some(MessageView {
            id: MessageId(i64::from(message.id)),
            sender: message
                .from_id
                .as_ref()
                .map(marked_peer_id)
                .and_then(|id| names.get(&id).cloned())
                .unwrap_or_else(|| "Telegram".to_owned()),
            body: "[Service event]".to_owned(),
            timestamp: format_timestamp(message.date),
            direction: if message.out {
                MessageDirection::Outgoing
            } else {
                MessageDirection::Incoming
            },
            delivery: DeliveryState::Sent,
            reply_to: message.reply_to.as_ref().and_then(reply_message_id),
        }),
    }
}

fn reply_message_id(header: &tl::enums::MessageReplyHeader) -> Option<MessageId> {
    match header {
        tl::enums::MessageReplyHeader::Header(header) => {
            header.reply_to_msg_id.map(|id| MessageId(i64::from(id)))
        }
        tl::enums::MessageReplyHeader::MessageReplyStoryHeader(_) => None,
    }
}

fn message_body(message: &tl::enums::Message) -> String {
    match message {
        tl::enums::Message::Message(message) if !message.message.is_empty() => {
            message.message.clone()
        }
        tl::enums::Message::Message(message) if message.media.is_some() => {
            "[Media — specialized rendering pending]".to_owned()
        }
        tl::enums::Message::Empty(_) | tl::enums::Message::Message(_) => {
            "[Unsupported content]".to_owned()
        }
        tl::enums::Message::Service(_) => "[Service event]".to_owned(),
    }
}

fn format_timestamp(timestamp: i32) -> String {
    let Ok(utc) = time::OffsetDateTime::from_unix_timestamp(i64::from(timestamp)) else {
        return "--:--".to_owned();
    };
    let offset = time::UtcOffset::local_offset_at(utc).unwrap_or(time::UtcOffset::UTC);
    let local = utc.to_offset(offset);
    format!("{:02}:{:02}", local.hour(), local.minute())
}

fn user_display_name(user: &tl::types::User) -> String {
    let display_name = [user.first_name.as_deref(), user.last_name.as_deref()]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join(" ");
    if display_name.is_empty() {
        user.username.clone().unwrap_or_else(|| user.id.to_string())
    } else {
        display_name
    }
}

fn normalize_authorization(
    authorization: tl::enums::auth::Authorization,
) -> Result<AuthorizedUser> {
    let authorization = match authorization {
        tl::enums::auth::Authorization::Authorization(authorization) => authorization,
        tl::enums::auth::Authorization::SignUpRequired(_) => return SignUpRequiredSnafu.fail(),
    };
    match authorization.user {
        tl::enums::User::User(user) => Ok(AuthorizedUser {
            id: user.id,
            display_name: user_display_name(&user),
            username: user.username,
        }),
        tl::enums::User::Empty(_) => EmptyAuthorizedUserSnafu.fail(),
    }
}

type PasswordParameters<'a> = (&'a Vec<u8>, &'a Vec<u8>, &'a Vec<u8>, &'a i32);

fn password_parameters(algorithm: &tl::enums::PasswordKdfAlgo) -> Result<PasswordParameters<'_>> {
    match algorithm {
        tl::enums::PasswordKdfAlgo::Sha256Sha256Pbkdf2Hmacsha512iter100000Sha256ModPow(
            algorithm,
        ) => Ok((
            &algorithm.salt1,
            &algorithm.salt2,
            &algorithm.p,
            &algorithm.g,
        )),
        tl::enums::PasswordKdfAlgo::Unknown => UnsupportedPasswordAlgorithmSnafu.fail(),
    }
}

#[cfg(test)]
mod tests {
    use std::net::{IpAddr, Ipv4Addr, SocketAddr};

    use compio_mtproto::InvocationError;
    use grammers_tl_types::{self as tl, Serializable as _};

    use super::{
        Error, LoginCodeDelivery, LoginCodeDeliveryMethod, LoginErrorAction,
        contains_login_token_update, direct_data_centers, ensure_production_environment,
        login_error_action, normalize_code_delivery, normalize_code_delivery_method, qr_login_uri,
        rpc_migration_dc,
    };

    #[test]
    fn qr_login_routes_session_password_needed_to_2fa() {
        let error = InvocationError::Rpc {
            code: 401,
            message: "SESSION_PASSWORD_NEEDED".to_owned(),
        };

        assert_eq!(
            login_error_action(&error),
            LoginErrorAction::RequestPassword
        );
    }

    #[test]
    fn phone_login_retries_plain_and_diagnostic_auth_restarts() {
        for message in ["AUTH_RESTART", "AUTH_RESTART_7"] {
            let error = InvocationError::Rpc {
                code: 500,
                message: message.to_owned(),
            };

            assert_eq!(login_error_action(&error), LoginErrorAction::Restart);
        }
    }

    #[test]
    fn test_data_center_configuration_is_rejected() {
        assert!(matches!(
            ensure_production_environment(true),
            Err(Error::TestDataCenter)
        ));
        assert!(ensure_production_environment(false).is_ok());
    }

    #[test]
    fn qr_login_uri_uses_unpadded_url_safe_base64() {
        assert_eq!(qr_login_uri(&[0xfb, 0xff]), "tg://login?token=-_8");
    }

    #[test]
    fn login_token_update_is_detected_inside_update_short() {
        let update = tl::enums::Updates::UpdateShort(tl::types::UpdateShort {
            update: tl::enums::Update::LoginToken,
            date: 1_700_000_000,
        });

        assert!(contains_login_token_update(&update.to_bytes()));
    }

    #[test]
    fn unrelated_update_is_not_treated_as_a_login_scan() {
        assert!(!contains_login_token_update(
            &tl::enums::Updates::TooLong.to_bytes()
        ));
    }

    #[test]
    fn login_code_delivery_preserves_the_telegram_app_destination() {
        let delivery =
            normalize_code_delivery(tl::types::auth::SentCodeTypeApp { length: 5 }.into());

        assert_eq!(delivery, LoginCodeDelivery::TelegramApp { length: 5 });
    }

    #[test]
    fn login_code_fallback_preserves_sms_delivery() {
        assert_eq!(
            normalize_code_delivery_method(&tl::enums::auth::CodeType::Sms),
            LoginCodeDeliveryMethod::Sms
        );
    }

    #[test]
    fn phone_migration_rpc_error_exposes_its_target_data_center() {
        let error = InvocationError::Rpc {
            code: 303,
            message: "PHONE_MIGRATE_1".to_owned(),
        };

        assert_eq!(rpc_migration_dc(&error, "PHONE_MIGRATE_"), Some(1));
        assert_eq!(rpc_migration_dc(&error, "NETWORK_MIGRATE_"), None);
    }

    #[test]
    fn direct_data_center_selection_ignores_incompatible_endpoints() {
        let direct = dc_option(1, "149.154.175.53", 443, false, false);
        let ipv6 = dc_option(1, "2001:db8::1", 443, true, false);
        let media = dc_option(2, "149.154.167.151", 443, false, true);

        let selected = direct_data_centers(vec![direct, ipv6, media]);

        assert_eq!(
            selected.get(&1),
            Some(&SocketAddr::new(
                IpAddr::V4(Ipv4Addr::new(149, 154, 175, 53)),
                443
            ))
        );
        assert!(!selected.contains_key(&2));
    }

    fn dc_option(
        id: i32,
        ip_address: &str,
        port: i32,
        ipv6: bool,
        media_only: bool,
    ) -> tl::enums::DcOption {
        tl::types::DcOption {
            ipv6,
            media_only,
            tcpo_only: false,
            cdn: false,
            r#static: false,
            this_port_only: false,
            id,
            ip_address: ip_address.to_owned(),
            port,
            secret: None,
        }
        .into()
    }
}
