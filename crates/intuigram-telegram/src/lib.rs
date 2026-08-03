//! Telegram API orchestration and Intuigram-owned normalization.

use std::collections::{HashMap, VecDeque};
use std::fmt;
use std::future::Future;
use std::net::SocketAddr;
use std::num::NonZeroUsize;
use std::pin::Pin;
use std::sync::atomic::{AtomicI64, Ordering};
use std::task::{Context, Poll};
use std::time::Duration;

use base64::Engine as _;
use compio_mtproto::{
    AbridgedConnection, AuthKeyMaterial, BoxedTransport, ConnectionDriver, EncryptedConnection,
    InvocationError, InvocationHandle, UpdateStream, generate_auth_key,
};
use futures_util::Stream;
use grammers_crypto::two_factor_auth::{calculate_2fa, check_p_and_g};
use grammers_tl_types as tl;
use grammers_tl_types::{Deserializable as _, Identifiable as _};
use intuigram_app::{
    AdapterEvent, Bootstrap, ChatId, ChatKind, ChatView, DeliveryState, FolderView, MediaCard,
    MediaKind, MessageDetails, MessageDirection, MessageId, MessageView, ReactionView, TextEntity,
    TextEntityKind,
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

/// Owned upload candidate supplied by the composition adapter.
pub struct Upload {
    /// Safe display filename.
    pub name: String,

    /// Internet media type.
    pub mime_type: String,

    /// Complete file bytes.
    pub bytes: Vec<u8>,

    /// Use Telegram photo semantics instead of a generic document.
    pub photo: bool,
}

/// Stable Telegram identifiers retained across one upload retry sequence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UploadIds {
    /// Identifier used by Telegram's file-part store.
    pub file: i64,

    /// Idempotency identifier used by `messages.sendMedia`.
    pub message: i64,
}

/// Telegram synchronization position accompanying normalized live events.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct UpdateCursor {
    /// Latest persistent update timestamp when supplied by this envelope.
    pub pts: Option<i32>,

    /// Latest secret update timestamp when supplied by this envelope.
    pub qts: Option<i32>,

    /// Telegram server date when supplied by this envelope.
    pub date: Option<i32>,

    /// Latest global update sequence when supplied by this envelope.
    pub seq: Option<i32>,
}

/// One normalized adapter event with its durable cursor delta.
pub struct LiveEvent {
    /// Intuigram-owned event.
    pub event: AdapterEvent,

    /// Cursor fields advanced by the same Telegram envelope.
    pub cursor: UpdateCursor,
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

    /// A passive Telegram update could not be decoded.
    #[snafu(display("Telegram update payload was invalid"))]
    DecodeUpdate {
        /// Underlying TL decoding failure.
        source: grammers_tl_types::deserialize::Error,
    },

    /// A serialized Telegram cloud peer could not be decoded.
    #[snafu(display("Telegram cloud peer payload was invalid"))]
    DecodePeer {
        /// Underlying TL decoding failure.
        source: grammers_tl_types::deserialize::Error,
    },

    /// A serialized Telegram Message media constructor could not be decoded.
    #[snafu(display("Telegram Message media payload was invalid"))]
    DecodeMedia {
        /// Underlying TL decoding failure.
        source: grammers_tl_types::deserialize::Error,
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

    /// A requested Telegram Folder is no longer available.
    #[snafu(display("Telegram Folder {folder_id} is unavailable"))]
    FolderUnavailable {
        /// Missing Telegram Folder identifier.
        folder_id: i32,
    },

    /// Telegram declined a dialog-filter edit without an RPC error.
    #[snafu(display("Telegram declined the Folder membership change"))]
    FolderUpdateRejected,

    /// A Intuigram Message ID could not be represented by Telegram's API.
    #[snafu(display("Message ID {message_id} is outside Telegram's signed 32-bit domain"))]
    InvalidMessageId {
        /// Invalid Intuigram Message ID.
        message_id: i64,
    },

    /// Telegram rejected an uploaded file part without an RPC error.
    #[snafu(display("Telegram rejected upload part {part}"))]
    UploadPartRejected {
        /// Zero-based rejected part.
        part: i32,
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
    /// Reports whether the operation may be retried after reconnecting.
    #[must_use]
    pub const fn is_connection_failure(&self) -> bool {
        matches!(self, Self::Connect { .. })
            || matches!(self, Self::Invoke { source } if source.is_connection_failure())
    }

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

enum Connection {
    Login(Box<EncryptedConnection>),
    Live(InvocationHandle),
}

impl Connection {
    async fn invoke<R>(&mut self, request: &R) -> std::result::Result<R::Return, InvocationError>
    where
        R: tl::RemoteCall + tl::Serializable,
        R::Return: tl::Deserializable,
    {
        match self {
            Self::Login(connection) => connection.invoke(request).await,
            Self::Live(connection) => connection.invoke(request).await,
        }
    }

    fn take_updates(&mut self) -> Vec<Vec<u8>> {
        match self {
            Self::Login(connection) => connection.take_updates(),
            Self::Live(_) => Vec::new(),
        }
    }
}

/// Telegram API client built on Intuigram's Compio `MTProto` sender.
pub struct Client {
    connection: Connection,
    credentials: ApplicationCredentials,
    password: Option<tl::types::account::Password>,
    identity: Option<AuthorizedUser>,
    peers: HashMap<ChatId, tl::enums::InputPeer>,
    names: HashMap<ChatId, String>,
    data_centers: HashMap<i32, SocketAddr>,
}

/// Passive normalized Telegram updates driven by one persistent MTProto
/// connection.
pub struct LiveUpdates {
    driver: Pin<Box<ConnectionDriver>>,
    updates: UpdateStream,
    names: HashMap<ChatId, String>,
    pending: VecDeque<LiveEvent>,
    terminated: bool,
}

impl Stream for LiveUpdates {
    type Item = Result<LiveEvent>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        if let Some(event) = self.pending.pop_front() {
            return Poll::Ready(Some(Ok(event)));
        }
        if self.terminated {
            return Poll::Ready(None);
        }
        match self.driver.as_mut().poll(cx) {
            Poll::Ready(Err(source)) => {
                self.terminated = true;
                return Poll::Ready(Some(Err(Error::Invoke { source })));
            }
            Poll::Ready(Ok(())) => {
                self.terminated = true;
                return Poll::Ready(None);
            }
            Poll::Pending => {}
        }
        loop {
            match Pin::new(&mut self.updates).poll_next(cx) {
                Poll::Ready(Some(bytes)) => match normalize_live_update(&bytes, &mut self.names) {
                    Ok(batch) => {
                        self.pending
                            .extend(batch.events.into_iter().map(|event| LiveEvent {
                                event,
                                cursor: batch.cursor,
                            }));
                        if let Some(event) = self.pending.pop_front() {
                            return Poll::Ready(Some(Ok(event)));
                        }
                    }
                    Err(error) => return Poll::Ready(Some(Err(error))),
                },
                Poll::Ready(None) => {
                    self.terminated = true;
                    return Poll::Ready(None);
                }
                Poll::Pending => return Poll::Pending,
            }
        }
    }
}

impl Client {
    /// Connects to a Telegram data center and generates fresh authorization
    /// material.
    pub async fn connect_new(
        dc_id: i32,
        endpoint: SocketAddr,
        credentials: ApplicationCredentials,
    ) -> Result<(Self, Session)> {
        let transport = AbridgedConnection::connect(endpoint)
            .await
            .context(ConnectSnafu { endpoint })?;
        let mut transport = BoxedTransport::new(transport);
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
            connection: Connection::Login(Box::new(EncryptedConnection::from_boxed(
                transport, &material,
            ))),
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
            connection: Connection::Login(Box::new(EncryptedConnection::new(transport, &material))),
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

    /// Converts an authenticated sequential connection into a cloneable raw
    /// invocation endpoint and continuously driven update stream.
    #[must_use]
    pub fn into_live(self, capacity: NonZeroUsize) -> (Self, LiveUpdates) {
        let Self {
            connection,
            credentials,
            password,
            identity,
            peers,
            names,
            data_centers,
        } = self;
        let Connection::Login(connection) = connection else {
            unreachable!("a Telegram client enters live mode only once after login")
        };
        let (handle, updates, driver) = (*connection).into_driver(capacity);
        (
            Self {
                connection: Connection::Live(handle),
                credentials,
                password,
                identity,
                peers,
                names: names.clone(),
                data_centers,
            },
            LiveUpdates {
                driver: Box::pin(driver),
                updates,
                names,
                pending: VecDeque::new(),
                terminated: false,
            },
        )
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
        let dialog_filters = self
            .connection
            .invoke(&tl::functions::messages::GetDialogFilters {})
            .await
            .context(InvokeSnafu)?;
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
        let traits = chat_traits(
            &chats,
            &users,
            self.identity.as_ref().map(|identity| identity.id),
        );
        let tl::enums::messages::DialogFilters::Filters(dialog_filters) = dialog_filters;
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
                        kind: traits
                            .get(&chat_id)
                            .map_or(ChatKind::Inaccessible, |traits| traits.kind),
                        folders: dialog_folder_membership(
                            dialog,
                            &dialog_filters.filters,
                            traits.get(&chat_id),
                        ),
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
        let folders = normalize_dialog_folders(dialog_filters.filters, &chat_views);
        Ok(Bootstrap {
            connection: intuigram_app::ConnectionState::Connected,
            account_name,
            folders,
            chats: chat_views,
            messages: initial_messages,
            drafts: Vec::new(),
            histories: Vec::new(),
        })
    }

    /// Reads Telegram's complete durable update cursor.
    pub async fn synchronization_cursor(&mut self) -> Result<UpdateCursor> {
        let state = self
            .connection
            .invoke(&tl::functions::updates::GetState {})
            .await
            .context(InvokeSnafu)?;
        let tl::enums::updates::State::State(state) = state;
        Ok(UpdateCursor {
            pts: Some(state.pts),
            qts: Some(state.qts),
            date: Some(state.date),
            seq: Some(state.seq),
        })
    }

    /// Adds or removes a Chat from Archive or a custom Telegram Folder.
    pub async fn set_chat_folder(
        &mut self,
        chat: ChatId,
        folder: i32,
        included: bool,
    ) -> Result<()> {
        let peer = self
            .peers
            .get(&chat)
            .cloned()
            .context(PeerUnavailableSnafu { chat_id: chat.0 })?;
        if folder == -1 {
            self.connection
                .invoke(&tl::functions::folders::EditPeerFolders {
                    folder_peers: vec![
                        tl::types::InputFolderPeer {
                            peer,
                            folder_id: i32::from(included),
                        }
                        .into(),
                    ],
                })
                .await
                .context(InvokeSnafu)?;
            return Ok(());
        }

        let tl::enums::messages::DialogFilters::Filters(mut filters) = self
            .connection
            .invoke(&tl::functions::messages::GetDialogFilters {})
            .await
            .context(InvokeSnafu)?;
        let filter = filters
            .filters
            .iter_mut()
            .find(|candidate| dialog_filter_id(candidate) == Some(folder))
            .context(FolderUnavailableSnafu { folder_id: folder })?;
        set_dialog_filter_membership(filter, peer, included);
        let accepted = self
            .connection
            .invoke(&tl::functions::messages::UpdateDialogFilter {
                id: folder,
                filter: Some(filter.clone()),
            })
            .await
            .context(InvokeSnafu)?;
        if !accepted {
            return FolderUpdateRejectedSnafu.fail();
        }
        Ok(())
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

    /// Loads an ordinary Message Thread or Channel comment history.
    pub async fn thread_history(
        &mut self,
        chat: ChatId,
        root: MessageId,
        limit: i32,
    ) -> Result<Vec<MessageView>> {
        let peer = self
            .peers
            .get(&chat)
            .cloned()
            .context(PeerUnavailableSnafu { chat_id: chat.0 })?;
        let root =
            i32::try_from(root.0).map_err(|_| Error::InvalidMessageId { message_id: root.0 })?;
        let response = self
            .connection
            .invoke(&tl::functions::messages::GetReplies {
                peer,
                msg_id: root,
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
        thread_root: Option<MessageId>,
        random_id: i64,
    ) -> Result<()> {
        let peer = self
            .peers
            .get(&chat)
            .cloned()
            .context(PeerUnavailableSnafu { chat_id: chat.0 })?;
        let reply_to = reply_to
            .or(thread_root)
            .map(|message| {
                let reply_to_msg_id =
                    i32::try_from(message.0).map_err(|_| Error::InvalidMessageId {
                        message_id: message.0,
                    })?;
                Ok(tl::types::InputReplyToMessage {
                    reply_to_msg_id,
                    top_msg_id: thread_root
                        .filter(|root| *root != message)
                        .map(|root| {
                            i32::try_from(root.0)
                                .map_err(|_| Error::InvalidMessageId { message_id: root.0 })
                        })
                        .transpose()?,
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
                random_id,
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

    /// Uploads and sends one photo or generic document.
    pub async fn send_upload(
        &mut self,
        chat: ChatId,
        upload: Upload,
        caption: String,
        reply_to: Option<MessageId>,
        thread_root: Option<MessageId>,
        ids: UploadIds,
    ) -> Result<()> {
        const PART_BYTES: usize = 512 * 1024;
        const BIG_FILE_BYTES: usize = 10 * 1024 * 1024;

        let peer = self
            .peers
            .get(&chat)
            .cloned()
            .context(PeerUnavailableSnafu { chat_id: chat.0 })?;
        let part_count = upload.bytes.len().div_ceil(PART_BYTES);
        let part_count = i32::try_from(part_count).map_err(|_| Error::InvalidMessageId {
            message_id: i64::try_from(upload.bytes.len()).unwrap_or(i64::MAX),
        })?;
        let big = upload.bytes.len() > BIG_FILE_BYTES;
        for (part, bytes) in upload.bytes.chunks(PART_BYTES).enumerate() {
            let part = i32::try_from(part)
                .expect("an in-memory upload cannot exceed Telegram's signed part index");
            let accepted = if big {
                self.connection
                    .invoke(&tl::functions::upload::SaveBigFilePart {
                        file_id: ids.file,
                        file_part: part,
                        file_total_parts: part_count,
                        bytes: bytes.to_vec(),
                    })
                    .await
                    .context(InvokeSnafu)?
            } else {
                self.connection
                    .invoke(&tl::functions::upload::SaveFilePart {
                        file_id: ids.file,
                        file_part: part,
                        bytes: bytes.to_vec(),
                    })
                    .await
                    .context(InvokeSnafu)?
            };
            if !accepted {
                return UploadPartRejectedSnafu { part }.fail();
            }
        }
        let input_file = if big {
            tl::types::InputFileBig {
                id: ids.file,
                parts: part_count,
                name: upload.name.clone(),
            }
            .into()
        } else {
            tl::types::InputFile {
                id: ids.file,
                parts: part_count,
                name: upload.name.clone(),
                md5_checksum: format!("{:x}", md5::compute(&upload.bytes)),
            }
            .into()
        };
        let media = if upload.photo {
            tl::types::InputMediaUploadedPhoto {
                spoiler: false,
                live_photo: false,
                file: input_file,
                stickers: None,
                ttl_seconds: None,
                video: None,
            }
            .into()
        } else {
            tl::types::InputMediaUploadedDocument {
                nosound_video: false,
                force_file: false,
                spoiler: false,
                file: input_file,
                thumb: None,
                mime_type: upload.mime_type,
                attributes: vec![
                    tl::types::DocumentAttributeFilename {
                        file_name: upload.name,
                    }
                    .into(),
                ],
                stickers: None,
                video_cover: None,
                video_timestamp: None,
                ttl_seconds: None,
            }
            .into()
        };
        self.connection
            .invoke(&tl::functions::messages::SendMedia {
                silent: false,
                background: false,
                clear_draft: true,
                noforwards: false,
                update_stickersets_order: false,
                invert_media: false,
                allow_paid_floodskip: false,
                peer,
                reply_to: input_reply_to(reply_to, thread_root)?,
                media,
                message: caption,
                random_id: ids.message,
                reply_markup: None,
                entities: None,
                schedule_date: None,
                schedule_repeat_period: None,
                send_as: None,
                quick_reply_shortcut: None,
                effect: None,
                allow_paid_stars: None,
                suggested_post: None,
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

fn input_reply_to(
    reply_to: Option<MessageId>,
    thread_root: Option<MessageId>,
) -> Result<Option<tl::enums::InputReplyTo>> {
    reply_to
        .or(thread_root)
        .map(|message| {
            let reply_to_msg_id =
                i32::try_from(message.0).map_err(|_| Error::InvalidMessageId {
                    message_id: message.0,
                })?;
            Ok(tl::types::InputReplyToMessage {
                reply_to_msg_id,
                top_msg_id: thread_root
                    .filter(|root| *root != message)
                    .map(|root| {
                        i32::try_from(root.0)
                            .map_err(|_| Error::InvalidMessageId { message_id: root.0 })
                    })
                    .transpose()?,
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
        .transpose()
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

struct NormalizedLive {
    events: Vec<AdapterEvent>,
    cursor: UpdateCursor,
}

fn normalize_live_update(
    bytes: &[u8],
    names: &mut HashMap<ChatId, String>,
) -> Result<NormalizedLive> {
    if let Ok(updates) = tl::enums::Updates::from_bytes(bytes) {
        let cursor = updates_cursor(&updates);
        let events = match updates {
            tl::enums::Updates::TooLong | tl::enums::Updates::UpdateShortSentMessage(_) => {
                Vec::new()
            }
            tl::enums::Updates::UpdateShortMessage(message) => {
                vec![AdapterEvent::MessageAdded {
                    chat: ChatId(message.user_id),
                    message: Box::new(MessageView {
                        id: MessageId(i64::from(message.id)),
                        sender: if message.out {
                            "You".to_owned()
                        } else {
                            names
                                .get(&ChatId(message.user_id))
                                .cloned()
                                .unwrap_or_else(|| "Unknown user".to_owned())
                        },
                        body: message.message,
                        timestamp: format_timestamp(message.date),
                        direction: if message.out {
                            MessageDirection::Outgoing
                        } else {
                            MessageDirection::Incoming
                        },
                        delivery: DeliveryState::Sent,
                        reply_to: message.reply_to.as_ref().and_then(reply_message_id),
                        details: MessageDetails {
                            entities: normalize_entities(message.entities.as_deref()),
                            ..MessageDetails::default()
                        },
                    }),
                }]
            }
            tl::enums::Updates::UpdateShortChatMessage(message) => {
                let chat = ChatId(-message.chat_id);
                vec![AdapterEvent::MessageAdded {
                    chat,
                    message: Box::new(MessageView {
                        id: MessageId(i64::from(message.id)),
                        sender: if message.out {
                            "You".to_owned()
                        } else {
                            names
                                .get(&ChatId(message.from_id))
                                .cloned()
                                .unwrap_or_else(|| "Unknown user".to_owned())
                        },
                        body: message.message,
                        timestamp: format_timestamp(message.date),
                        direction: if message.out {
                            MessageDirection::Outgoing
                        } else {
                            MessageDirection::Incoming
                        },
                        delivery: DeliveryState::Sent,
                        reply_to: message.reply_to.as_ref().and_then(reply_message_id),
                        details: MessageDetails {
                            entities: normalize_entities(message.entities.as_deref()),
                            ..MessageDetails::default()
                        },
                    }),
                }]
            }
            tl::enums::Updates::UpdateShort(update) => {
                normalize_update(update.update, names).into_iter().collect()
            }
            tl::enums::Updates::Combined(updates) => {
                update_live_names(names, &updates.chats, &updates.users);
                updates
                    .updates
                    .into_iter()
                    .filter_map(|update| normalize_update(update, names))
                    .collect()
            }
            tl::enums::Updates::Updates(updates) => {
                update_live_names(names, &updates.chats, &updates.users);
                updates
                    .updates
                    .into_iter()
                    .filter_map(|update| normalize_update(update, names))
                    .collect()
            }
        };
        return Ok(NormalizedLive { events, cursor });
    }
    let update = tl::enums::Update::from_bytes(bytes).context(DecodeUpdateSnafu)?;
    let cursor = update_cursor(&update);
    Ok(NormalizedLive {
        events: normalize_update(update, names).into_iter().collect(),
        cursor,
    })
}

fn updates_cursor(updates: &tl::enums::Updates) -> UpdateCursor {
    match updates {
        tl::enums::Updates::TooLong => UpdateCursor::default(),
        tl::enums::Updates::UpdateShortMessage(update) => UpdateCursor {
            pts: Some(update.pts),
            date: Some(update.date),
            ..UpdateCursor::default()
        },
        tl::enums::Updates::UpdateShortChatMessage(update) => UpdateCursor {
            pts: Some(update.pts),
            date: Some(update.date),
            ..UpdateCursor::default()
        },
        tl::enums::Updates::UpdateShortSentMessage(update) => UpdateCursor {
            pts: Some(update.pts),
            date: Some(update.date),
            ..UpdateCursor::default()
        },
        tl::enums::Updates::UpdateShort(update) => {
            let mut cursor = update_cursor(&update.update);
            cursor.date = Some(update.date);
            cursor
        }
        tl::enums::Updates::Combined(updates) => {
            merge_update_cursors(&updates.updates, Some(updates.date), Some(updates.seq))
        }
        tl::enums::Updates::Updates(updates) => {
            merge_update_cursors(&updates.updates, Some(updates.date), Some(updates.seq))
        }
    }
}

fn merge_update_cursors(
    updates: &[tl::enums::Update],
    date: Option<i32>,
    seq: Option<i32>,
) -> UpdateCursor {
    updates.iter().fold(
        UpdateCursor {
            date,
            seq,
            ..UpdateCursor::default()
        },
        |mut cursor, update| {
            let next = update_cursor(update);
            cursor.pts = next.pts.or(cursor.pts);
            cursor.qts = next.qts.or(cursor.qts);
            cursor
        },
    )
}

fn update_cursor(update: &tl::enums::Update) -> UpdateCursor {
    let pts = match update {
        tl::enums::Update::NewMessage(update) => Some(update.pts),
        tl::enums::Update::NewChannelMessage(update) => Some(update.pts),
        tl::enums::Update::EditMessage(update) => Some(update.pts),
        tl::enums::Update::EditChannelMessage(update) => Some(update.pts),
        tl::enums::Update::DeleteMessages(update) => Some(update.pts),
        tl::enums::Update::DeleteChannelMessages(update) => Some(update.pts),
        tl::enums::Update::ReadHistoryInbox(update) => Some(update.pts),
        tl::enums::Update::ReadHistoryOutbox(update) => Some(update.pts),
        tl::enums::Update::ReadMessagesContents(update) => Some(update.pts),
        tl::enums::Update::FolderPeers(update) => Some(update.pts),
        _ => None,
    };
    UpdateCursor {
        pts,
        ..UpdateCursor::default()
    }
}

fn normalize_update(
    update: tl::enums::Update,
    names: &HashMap<ChatId, String>,
) -> Option<AdapterEvent> {
    let message = match update {
        tl::enums::Update::NewMessage(update) => update.message,
        tl::enums::Update::NewChannelMessage(update) => update.message,
        _ => return None,
    };
    let chat = message_chat_id(&message);
    normalize_message(&message, names).map(|message| AdapterEvent::MessageAdded {
        chat,
        message: Box::new(message),
    })
}

fn update_live_names(
    names: &mut HashMap<ChatId, String>,
    chats: &[tl::enums::Chat],
    users: &[tl::enums::User],
) {
    for user in users {
        match user {
            tl::enums::User::User(user) => {
                names.insert(ChatId(user.id), user_display_name(user));
            }
            tl::enums::User::Empty(user) => {
                names.insert(ChatId(user.id), "Inaccessible user".to_owned());
            }
        }
    }
    for chat in chats {
        match chat {
            tl::enums::Chat::Chat(chat) => {
                names.insert(ChatId(-chat.id), chat.title.clone());
            }
            tl::enums::Chat::Channel(channel) => {
                names.insert(ChatId(mark_channel_id(channel.id)), channel.title.clone());
            }
            tl::enums::Chat::Forbidden(chat) => {
                names.insert(ChatId(-chat.id), chat.title.clone());
            }
            tl::enums::Chat::ChannelForbidden(channel) => {
                names.insert(ChatId(mark_channel_id(channel.id)), channel.title.clone());
            }
            tl::enums::Chat::Empty(chat) => {
                names.insert(ChatId(-chat.id), "Inaccessible group".to_owned());
            }
        }
    }
}

/// Normalizes one serialized current-layer Telegram cloud peer into an
/// Intuigram-owned root Chat category.
pub fn normalize_serialized_peer_kind(bytes: &[u8], account_id: Option<i64>) -> Result<ChatKind> {
    let constructor = bytes
        .get(..4)
        .and_then(|bytes| <[u8; 4]>::try_from(bytes).ok())
        .map(u32::from_le_bytes);
    if matches!(
        constructor,
        Some(tl::types::User::CONSTRUCTOR_ID) | Some(tl::types::UserEmpty::CONSTRUCTOR_ID)
    ) {
        let user = tl::enums::User::from_bytes(bytes).context(DecodePeerSnafu)?;
        return Ok(user_chat_kind(&user, account_id));
    }
    let chat = tl::enums::Chat::from_bytes(bytes).context(DecodePeerSnafu)?;
    Ok(cloud_chat_kind(&chat))
}

fn take_login_token_update(connection: &mut Connection) -> bool {
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

fn normalize_dialog_folders(
    filters: Vec<tl::enums::DialogFilter>,
    chats: &[ChatView],
) -> Vec<FolderView> {
    let mut folders = filters
        .into_iter()
        .map(|filter| match filter {
            tl::enums::DialogFilter::Default => FolderView {
                id: 0,
                title: "All".to_owned(),
                unread: folder_unread(chats, 0),
            },
            tl::enums::DialogFilter::Filter(filter) => FolderView {
                id: filter.id,
                title: text_with_entities(filter.title),
                unread: folder_unread(chats, filter.id),
            },
            tl::enums::DialogFilter::Chatlist(filter) => FolderView {
                id: filter.id,
                title: text_with_entities(filter.title),
                unread: folder_unread(chats, filter.id),
            },
        })
        .collect::<Vec<_>>();
    if !folders.iter().any(|folder| folder.id == 0) {
        folders.insert(
            0,
            FolderView {
                id: 0,
                title: "All".to_owned(),
                unread: folder_unread(chats, 0),
            },
        );
    }
    folders.push(FolderView {
        id: -1,
        title: "Archive".to_owned(),
        unread: folder_unread(chats, -1),
    });
    folders
}

fn folder_unread(chats: &[ChatView], folder: i32) -> u32 {
    chats
        .iter()
        .filter(|chat| chat.folders.contains(&folder))
        .fold(0_u32, |total, chat| total.saturating_add(chat.unread))
}

fn dialog_filter_id(filter: &tl::enums::DialogFilter) -> Option<i32> {
    match filter {
        tl::enums::DialogFilter::Default => None,
        tl::enums::DialogFilter::Filter(filter) => Some(filter.id),
        tl::enums::DialogFilter::Chatlist(filter) => Some(filter.id),
    }
}

fn set_dialog_filter_membership(
    filter: &mut tl::enums::DialogFilter,
    peer: tl::enums::InputPeer,
    included: bool,
) {
    match filter {
        tl::enums::DialogFilter::Default => {}
        tl::enums::DialogFilter::Filter(filter) => {
            filter.pinned_peers.retain(|candidate| candidate != &peer);
            filter.include_peers.retain(|candidate| candidate != &peer);
            filter.exclude_peers.retain(|candidate| candidate != &peer);
            if included {
                filter.include_peers.push(peer);
            } else {
                filter.exclude_peers.push(peer);
            }
        }
        tl::enums::DialogFilter::Chatlist(filter) => {
            filter.pinned_peers.retain(|candidate| candidate != &peer);
            filter.include_peers.retain(|candidate| candidate != &peer);
            if included {
                filter.include_peers.push(peer);
            }
        }
    }
}

#[derive(Clone, Copy)]
struct ChatTraits {
    kind: ChatKind,
    contact: bool,
}

fn chat_traits(
    chats: &[tl::enums::Chat],
    users: &[tl::enums::User],
    account_id: Option<i64>,
) -> HashMap<ChatId, ChatTraits> {
    let mut result = HashMap::new();
    for user in users {
        let contact = matches!(user, tl::enums::User::User(user) if user.contact);
        result.insert(
            ChatId(user.id()),
            ChatTraits {
                kind: user_chat_kind(user, account_id),
                contact,
            },
        );
    }
    for chat in chats {
        let id = match chat {
            tl::enums::Chat::Chat(chat) => ChatId(-chat.id),
            tl::enums::Chat::Forbidden(chat) => ChatId(-chat.id),
            tl::enums::Chat::Empty(chat) => ChatId(-chat.id),
            tl::enums::Chat::Channel(channel) => ChatId(mark_channel_id(channel.id)),
            tl::enums::Chat::ChannelForbidden(channel) => ChatId(mark_channel_id(channel.id)),
        };
        result.insert(
            id,
            ChatTraits {
                kind: cloud_chat_kind(chat),
                contact: false,
            },
        );
    }
    result
}

fn user_chat_kind(user: &tl::enums::User, account_id: Option<i64>) -> ChatKind {
    match user {
        tl::enums::User::User(user) if user.is_self || account_id == Some(user.id) => {
            ChatKind::SavedMessages
        }
        tl::enums::User::User(user) if user.bot => ChatKind::Bot,
        tl::enums::User::User(_) => ChatKind::Private,
        tl::enums::User::Empty(_) => ChatKind::Inaccessible,
    }
}

fn cloud_chat_kind(chat: &tl::enums::Chat) -> ChatKind {
    match chat {
        tl::enums::Chat::Chat(_) => ChatKind::BasicGroup,
        tl::enums::Chat::Channel(channel) if channel.gigagroup => ChatKind::Gigagroup,
        tl::enums::Chat::Channel(channel) if channel.broadcast => ChatKind::Channel,
        tl::enums::Chat::Channel(_) => ChatKind::Supergroup,
        tl::enums::Chat::Forbidden(_)
        | tl::enums::Chat::ChannelForbidden(_)
        | tl::enums::Chat::Empty(_) => ChatKind::Inaccessible,
    }
}

fn dialog_folder_membership(
    dialog: &tl::types::Dialog,
    filters: &[tl::enums::DialogFilter],
    traits: Option<&ChatTraits>,
) -> Vec<i32> {
    let chat = marked_peer_id(&dialog.peer);
    let archived = dialog.folder_id == Some(1);
    let mut memberships = vec![if archived { -1 } else { 0 }];
    for filter in filters {
        let id = match filter {
            tl::enums::DialogFilter::Default => continue,
            tl::enums::DialogFilter::Filter(filter) => {
                let explicitly_excluded = filter_contains_peer(&filter.exclude_peers, chat, traits);
                let explicitly_included = filter_contains_peer(&filter.pinned_peers, chat, traits)
                    || filter_contains_peer(&filter.include_peers, chat, traits);
                let included_by_kind = traits.is_some_and(|traits| match traits.kind {
                    ChatKind::SavedMessages | ChatKind::Private => {
                        (traits.contact && filter.contacts)
                            || (!traits.contact && filter.non_contacts)
                    }
                    ChatKind::Bot => filter.bots,
                    ChatKind::BasicGroup | ChatKind::Supergroup | ChatKind::Gigagroup => {
                        filter.groups
                    }
                    ChatKind::Channel => filter.broadcasts,
                    ChatKind::Inaccessible => false,
                });
                let excluded_by_state = (filter.exclude_archived && archived)
                    || (filter.exclude_read && dialog.unread_count == 0);
                if explicitly_excluded
                    || excluded_by_state
                    || (!explicitly_included && !included_by_kind)
                {
                    continue;
                }
                filter.id
            }
            tl::enums::DialogFilter::Chatlist(filter) => {
                if !filter_contains_peer(&filter.pinned_peers, chat, traits)
                    && !filter_contains_peer(&filter.include_peers, chat, traits)
                {
                    continue;
                }
                filter.id
            }
        };
        memberships.push(id);
    }
    memberships
}

fn filter_contains_peer(
    peers: &[tl::enums::InputPeer],
    chat: ChatId,
    traits: Option<&ChatTraits>,
) -> bool {
    peers.iter().any(|peer| match peer {
        tl::enums::InputPeer::PeerSelf => {
            traits.is_some_and(|traits| traits.kind == ChatKind::SavedMessages)
        }
        tl::enums::InputPeer::User(peer) => ChatId(peer.user_id) == chat,
        tl::enums::InputPeer::Chat(peer) => ChatId(-peer.chat_id) == chat,
        tl::enums::InputPeer::Channel(peer) => ChatId(mark_channel_id(peer.channel_id)) == chat,
        tl::enums::InputPeer::Empty
        | tl::enums::InputPeer::UserFromMessage(_)
        | tl::enums::InputPeer::ChannelFromMessage(_) => false,
    })
}

fn text_with_entities(text: tl::enums::TextWithEntities) -> String {
    let tl::enums::TextWithEntities::Entities(text) = text;
    text.text
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
            let media = message.media.as_ref().map(normalize_media);
            let body = if message.message.is_empty() {
                media
                    .as_ref()
                    .map_or_else(|| "[Unsupported content]".to_owned(), media_card_fallback)
            } else {
                message.message.clone()
            };
            Some(MessageView {
                id: MessageId(i64::from(message.id)),
                sender,
                body,
                timestamp: format_timestamp(message.date),
                direction: if message.out {
                    MessageDirection::Outgoing
                } else {
                    MessageDirection::Incoming
                },
                delivery: DeliveryState::Sent,
                reply_to,
                details: MessageDetails {
                    entities: normalize_entities(message.entities.as_deref()),
                    forwarded_from: normalize_forward(message.fwd_from.as_ref(), names),
                    reactions: normalize_reactions(message.reactions.as_ref()),
                    edited: message.edit_date.is_some(),
                    pinned: message.pinned,
                    views: nonnegative_u32(message.views),
                    forwards: nonnegative_u32(message.forwards),
                    replies: message.replies.as_ref().and_then(|replies| match replies {
                        tl::enums::MessageReplies::Replies(replies) => {
                            u32::try_from(replies.replies).ok()
                        }
                    }),
                    media,
                    service: None,
                    thread_root: message.reply_to.as_ref().and_then(thread_root_message_id),
                },
            })
        }
        tl::enums::Message::Service(message) => {
            let description = service_event_description(&message.action);
            Some(MessageView {
                id: MessageId(i64::from(message.id)),
                sender: message
                    .from_id
                    .as_ref()
                    .map(marked_peer_id)
                    .and_then(|id| names.get(&id).cloned())
                    .unwrap_or_else(|| "Telegram".to_owned()),
                body: description.clone(),
                timestamp: format_timestamp(message.date),
                direction: if message.out {
                    MessageDirection::Outgoing
                } else {
                    MessageDirection::Incoming
                },
                delivery: DeliveryState::Sent,
                reply_to: message.reply_to.as_ref().and_then(reply_message_id),
                details: MessageDetails {
                    service: Some(description),
                    ..MessageDetails::default()
                },
            })
        }
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

fn thread_root_message_id(header: &tl::enums::MessageReplyHeader) -> Option<MessageId> {
    match header {
        tl::enums::MessageReplyHeader::Header(header) => header
            .reply_to_top_id
            .or(header.reply_to_msg_id)
            .map(|id| MessageId(i64::from(id))),
        tl::enums::MessageReplyHeader::MessageReplyStoryHeader(_) => None,
    }
}

fn message_body(message: &tl::enums::Message) -> String {
    match message {
        tl::enums::Message::Message(message) if !message.message.is_empty() => {
            message.message.clone()
        }
        tl::enums::Message::Message(message) if message.media.is_some() => message
            .media
            .as_ref()
            .map(normalize_media)
            .as_ref()
            .map_or_else(|| "[Unsupported content]".to_owned(), media_card_fallback),
        tl::enums::Message::Empty(_) | tl::enums::Message::Message(_) => {
            "[Unsupported content]".to_owned()
        }
        tl::enums::Message::Service(_) => "[Service event]".to_owned(),
    }
}

fn normalize_entities(entities: Option<&[tl::enums::MessageEntity]>) -> Vec<TextEntity> {
    entities
        .unwrap_or_default()
        .iter()
        .map(|entity| TextEntity {
            offset: usize::try_from(entity.offset()).unwrap_or(0),
            length: usize::try_from(entity.length()).unwrap_or(0),
            kind: match entity {
                tl::enums::MessageEntity::Bold(_) => TextEntityKind::Bold,
                tl::enums::MessageEntity::Italic(_) => TextEntityKind::Italic,
                tl::enums::MessageEntity::Underline(_) => TextEntityKind::Underline,
                tl::enums::MessageEntity::Strike(_) => TextEntityKind::Strike,
                tl::enums::MessageEntity::Code(_) => TextEntityKind::Code,
                tl::enums::MessageEntity::Pre(entity) => TextEntityKind::Pre {
                    language: (!entity.language.is_empty()).then(|| entity.language.clone()),
                },
                tl::enums::MessageEntity::Spoiler(_) => TextEntityKind::Spoiler,
                tl::enums::MessageEntity::Url(_) => TextEntityKind::Url,
                tl::enums::MessageEntity::TextUrl(entity) => TextEntityKind::TextUrl {
                    url: entity.url.clone(),
                },
                tl::enums::MessageEntity::CustomEmoji(entity) => TextEntityKind::CustomEmoji {
                    document_id: entity.document_id,
                },
                _ => TextEntityKind::Semantic,
            },
        })
        .collect()
}

fn normalize_forward(
    forward: Option<&tl::enums::MessageFwdHeader>,
    names: &HashMap<ChatId, String>,
) -> Option<String> {
    let tl::enums::MessageFwdHeader::Header(forward) = forward?;
    forward
        .from_name
        .clone()
        .or_else(|| {
            forward
                .from_id
                .as_ref()
                .map(marked_peer_id)
                .and_then(|id| names.get(&id).cloned())
        })
        .or_else(|| forward.post_author.clone())
        .or_else(|| Some("Unknown source".to_owned()))
}

fn normalize_reactions(reactions: Option<&tl::enums::MessageReactions>) -> Vec<ReactionView> {
    let Some(tl::enums::MessageReactions::Reactions(reactions)) = reactions else {
        return Vec::new();
    };
    reactions
        .results
        .iter()
        .map(|result| {
            let tl::enums::ReactionCount::Count(result) = result;
            ReactionView {
                label: match &result.reaction {
                    tl::enums::Reaction::Empty => "reaction".to_owned(),
                    tl::enums::Reaction::Emoji(reaction) => reaction.emoticon.clone(),
                    tl::enums::Reaction::CustomEmoji(reaction) => {
                        format!("custom:{}", reaction.document_id)
                    }
                    tl::enums::Reaction::Paid => "⭐".to_owned(),
                },
                count: u32::try_from(result.count).unwrap_or(0),
                chosen: result.chosen_order.is_some(),
            }
        })
        .collect()
}

fn normalize_media(media: &tl::enums::MessageMedia) -> MediaCard {
    let (kind, title, description, remote_id) = match media {
        tl::enums::MessageMedia::Photo(media) => (
            MediaKind::Photo,
            "Photo".to_owned(),
            if media.spoiler { "spoiler" } else { "image" }.to_owned(),
            media.photo.as_ref().and_then(photo_remote_id),
        ),
        tl::enums::MessageMedia::Document(media) => normalize_document_media(media),
        tl::enums::MessageMedia::WebPage(_) => (
            MediaKind::LinkPreview,
            "Link preview".to_owned(),
            "web page".to_owned(),
            None,
        ),
        tl::enums::MessageMedia::Poll(media) => (
            MediaKind::Poll,
            "Poll".to_owned(),
            poll_question(&media.poll),
            None,
        ),
        tl::enums::MessageMedia::Contact(media) => (
            MediaKind::Contact,
            "Contact".to_owned(),
            [media.first_name.as_str(), media.last_name.as_str()]
                .into_iter()
                .filter(|part| !part.is_empty())
                .collect::<Vec<_>>()
                .join(" "),
            None,
        ),
        tl::enums::MessageMedia::Geo(_) => (
            MediaKind::Location,
            "Location".to_owned(),
            "map coordinates".to_owned(),
            None,
        ),
        tl::enums::MessageMedia::Venue(media) => (
            MediaKind::Venue,
            media.title.clone(),
            media.address.clone(),
            None,
        ),
        tl::enums::MessageMedia::Dice(media) => (
            MediaKind::Dice,
            media.emoticon.clone(),
            format!("result {}", media.value),
            None,
        ),
        tl::enums::MessageMedia::Empty | tl::enums::MessageMedia::Unsupported => (
            MediaKind::Unsupported,
            "Unsupported Content".to_owned(),
            "Telegram media constructor is not available in this client".to_owned(),
            None,
        ),
        tl::enums::MessageMedia::GeoLive(_)
        | tl::enums::MessageMedia::Game(_)
        | tl::enums::MessageMedia::Invoice(_)
        | tl::enums::MessageMedia::Story(_)
        | tl::enums::MessageMedia::Giveaway(_)
        | tl::enums::MessageMedia::GiveawayResults(_)
        | tl::enums::MessageMedia::PaidMedia(_)
        | tl::enums::MessageMedia::ToDo(_)
        | tl::enums::MessageMedia::VideoStream(_) => (
            MediaKind::Specialized,
            "Specialized Telegram content".to_owned(),
            "open Details for available metadata".to_owned(),
            None,
        ),
    };
    MediaCard {
        kind,
        title,
        description,
        remote_id,
    }
}

/// Normalizes one serialized current-layer Telegram media constructor into an
/// informative Intuigram-owned card, including unsupported constructors.
pub fn normalize_serialized_media(bytes: &[u8]) -> Result<MediaCard> {
    let media = tl::enums::MessageMedia::from_bytes(bytes).context(DecodeMediaSnafu)?;
    Ok(normalize_media(&media))
}

fn normalize_document_media(
    media: &tl::types::MessageMediaDocument,
) -> (MediaKind, String, String, Option<String>) {
    let Some(tl::enums::Document::Document(document)) = media.document.as_ref() else {
        return (
            MediaKind::Unsupported,
            "Unavailable file".to_owned(),
            "Telegram did not include document metadata".to_owned(),
            None,
        );
    };
    let filename = document
        .attributes
        .iter()
        .find_map(|attribute| match attribute {
            tl::enums::DocumentAttribute::Filename(attribute) => Some(attribute.file_name.clone()),
            _ => None,
        });
    let kind = if media.round {
        MediaKind::VideoNote
    } else if media.voice {
        MediaKind::Voice
    } else if document.attributes.iter().any(|attribute| {
        matches!(
            attribute,
            tl::enums::DocumentAttribute::Sticker(_) | tl::enums::DocumentAttribute::CustomEmoji(_)
        )
    }) {
        MediaKind::Sticker
    } else if document
        .attributes
        .iter()
        .any(|attribute| matches!(attribute, tl::enums::DocumentAttribute::Animated))
    {
        MediaKind::Animation
    } else if media.video
        || document
            .attributes
            .iter()
            .any(|attribute| matches!(attribute, tl::enums::DocumentAttribute::Video(_)))
    {
        MediaKind::Video
    } else if document
        .attributes
        .iter()
        .any(|attribute| matches!(attribute, tl::enums::DocumentAttribute::Audio(_)))
    {
        MediaKind::Audio
    } else {
        MediaKind::File
    };
    let title = filename.unwrap_or_else(|| format!("{:?}", kind));
    (
        kind,
        title,
        format!("{} · {} bytes", document.mime_type, document.size),
        Some(document.id.to_string()),
    )
}

fn photo_remote_id(photo: &tl::enums::Photo) -> Option<String> {
    match photo {
        tl::enums::Photo::Photo(photo) => Some(photo.id.to_string()),
        tl::enums::Photo::Empty(_) => None,
    }
}

fn poll_question(poll: &tl::enums::Poll) -> String {
    let tl::enums::Poll::Poll(poll) = poll;
    text_with_entities(poll.question.clone())
}

fn media_card_fallback(card: &MediaCard) -> String {
    if card.description.is_empty() {
        format!("[{}]", card.title)
    } else {
        format!("[{}] {}", card.title, card.description)
    }
}

fn nonnegative_u32(value: Option<i32>) -> Option<u32> {
    value.and_then(|value| u32::try_from(value).ok())
}

fn service_event_description(action: &tl::enums::MessageAction) -> String {
    match action {
        tl::enums::MessageAction::ChatCreate(action) => {
            format!("Created group “{}”", action.title)
        }
        tl::enums::MessageAction::ChatEditTitle(action) => {
            format!("Changed the Chat title to “{}”", action.title)
        }
        tl::enums::MessageAction::ChatEditPhoto(_) => "Changed the Chat photo".to_owned(),
        tl::enums::MessageAction::ChatDeletePhoto => "Removed the Chat photo".to_owned(),
        tl::enums::MessageAction::ChatAddUser(action) => {
            format!("Added {} member(s)", action.users.len())
        }
        tl::enums::MessageAction::ChatDeleteUser(_) => "Removed a member".to_owned(),
        tl::enums::MessageAction::ChatJoinedByLink(_) => "Joined through an invite link".to_owned(),
        tl::enums::MessageAction::ChannelCreate(action) => {
            format!("Created Channel “{}”", action.title)
        }
        tl::enums::MessageAction::PinMessage => "Pinned a Message".to_owned(),
        tl::enums::MessageAction::HistoryClear => "Cleared Chat history".to_owned(),
        tl::enums::MessageAction::PhoneCall(_) => "Telegram call".to_owned(),
        tl::enums::MessageAction::ScreenshotTaken => "Took a screenshot".to_owned(),
        tl::enums::MessageAction::CustomAction(action) => action.message.clone(),
        tl::enums::MessageAction::ContactSignUp => "Joined Telegram".to_owned(),
        tl::enums::MessageAction::TopicCreate(action) => {
            format!("Created Topic “{}”", action.title)
        }
        tl::enums::MessageAction::TopicEdit(_) => "Changed a Topic".to_owned(),
        _ => "Telegram service event".to_owned(),
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
    use intuigram_app::{
        AdapterEvent, ChatId, ChatKind, ChatView, MediaKind, MessageDirection, MessageId,
    };

    use super::{
        Error, LoginCodeDelivery, LoginCodeDeliveryMethod, LoginErrorAction,
        contains_login_token_update, direct_data_centers, ensure_production_environment,
        login_error_action, normalize_code_delivery, normalize_code_delivery_method,
        normalize_dialog_folders, normalize_live_update, normalize_serialized_media,
        normalize_serialized_peer_kind, qr_login_uri, rpc_migration_dc,
        set_dialog_filter_membership,
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
    fn passive_short_message_is_normalized_at_the_serialized_tl_boundary() {
        let update = tl::enums::Updates::UpdateShortMessage(tl::types::UpdateShortMessage {
            out: false,
            mentioned: false,
            media_unread: false,
            silent: false,
            id: 42,
            user_id: 7,
            message: "hello".to_owned(),
            pts: 9,
            pts_count: 1,
            date: 1_700_000_000,
            fwd_from: None,
            via_bot_id: None,
            reply_to: None,
            entities: None,
            ttl_period: None,
        });
        let mut names = [(ChatId(7), "Ada".to_owned())].into_iter().collect();

        let batch = normalize_live_update(&update.to_bytes(), &mut names)
            .expect("serialized short update should normalize");

        assert_eq!(batch.cursor.pts, Some(9));
        assert_eq!(batch.cursor.date, Some(1_700_000_000));
        assert_eq!(batch.events.len(), 1);
        let AdapterEvent::MessageAdded { chat, message } = &batch.events[0] else {
            panic!("short message should produce a message event")
        };
        assert_eq!(*chat, ChatId(7));
        assert_eq!(message.id, MessageId(42));
        assert_eq!(message.sender, "Ada");
        assert_eq!(message.body, "hello");
        assert_eq!(message.direction, MessageDirection::Incoming);
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

    #[test]
    fn dialog_filters_include_custom_and_shared_folders_in_server_order() {
        let title = |text: &str| {
            tl::types::TextWithEntities {
                text: text.to_owned(),
                entities: Vec::new(),
            }
            .into()
        };
        let filters = vec![
            tl::enums::DialogFilter::Default,
            tl::types::DialogFilter {
                contacts: false,
                non_contacts: false,
                groups: false,
                broadcasts: false,
                bots: false,
                exclude_muted: false,
                exclude_read: false,
                exclude_archived: false,
                title_noanimate: false,
                id: 2,
                title: title("Work"),
                emoticon: None,
                color: None,
                pinned_peers: Vec::new(),
                include_peers: Vec::new(),
                exclude_peers: Vec::new(),
            }
            .into(),
            tl::types::DialogFilterChatlist {
                has_my_invites: false,
                title_noanimate: false,
                id: 3,
                title: title("Shared"),
                emoticon: None,
                color: None,
                pinned_peers: Vec::new(),
                include_peers: Vec::new(),
            }
            .into(),
        ];

        let chats = vec![
            ChatView {
                id: ChatId(10),
                title: "Ada".to_owned(),
                preview: String::new(),
                unread: 5,
                pinned: false,
                kind: ChatKind::Private,
                folders: vec![0, 2],
            },
            ChatView {
                id: ChatId(20),
                title: "Archived".to_owned(),
                preview: String::new(),
                unread: 2,
                pinned: false,
                kind: ChatKind::Supergroup,
                folders: vec![-1, 3],
            },
        ];
        let folders = normalize_dialog_folders(filters, &chats);

        assert_eq!(
            folders
                .iter()
                .map(|folder| (folder.id, folder.title.as_str(), folder.unread))
                .collect::<Vec<_>>(),
            vec![
                (0, "All", 5),
                (2, "Work", 5),
                (3, "Shared", 2),
                (-1, "Archive", 2),
            ]
        );
    }

    #[test]
    fn folder_membership_edit_overrides_rule_based_inclusion_explicitly() {
        let peer: tl::enums::InputPeer = tl::types::InputPeerUser {
            user_id: 7,
            access_hash: 9,
        }
        .into();
        let mut filter: tl::enums::DialogFilter = tl::types::DialogFilter {
            contacts: true,
            non_contacts: false,
            groups: false,
            broadcasts: false,
            bots: false,
            exclude_muted: false,
            exclude_read: false,
            exclude_archived: false,
            title_noanimate: false,
            id: 2,
            title: tl::types::TextWithEntities {
                text: "Work".to_owned(),
                entities: Vec::new(),
            }
            .into(),
            emoticon: None,
            color: None,
            pinned_peers: vec![peer.clone()],
            include_peers: Vec::new(),
            exclude_peers: Vec::new(),
        }
        .into();

        set_dialog_filter_membership(&mut filter, peer.clone(), false);
        {
            let tl::enums::DialogFilter::Filter(contents) = &filter else {
                panic!("ordinary filter fixture should remain ordinary")
            };
            assert!(!contents.pinned_peers.contains(&peer));
            assert!(!contents.include_peers.contains(&peer));
            assert_eq!(contents.exclude_peers, vec![peer.clone()]);
        }

        set_dialog_filter_membership(&mut filter, peer.clone(), true);
        let tl::enums::DialogFilter::Filter(contents) = &filter else {
            panic!("ordinary filter fixture should remain ordinary")
        };
        assert_eq!(contents.include_peers, vec![peer.clone()]);
        assert!(!contents.exclude_peers.contains(&peer));
    }

    #[test]
    fn serialized_cloud_peers_cover_every_root_chat_kind() {
        let cases = [
            (
                tl::enums::User::User(user(1, true, false)).to_bytes(),
                Some(1),
                ChatKind::SavedMessages,
            ),
            (
                tl::enums::User::User(user(2, false, false)).to_bytes(),
                Some(1),
                ChatKind::Private,
            ),
            (
                tl::enums::User::User(user(3, false, true)).to_bytes(),
                Some(1),
                ChatKind::Bot,
            ),
            (
                tl::enums::User::Empty(tl::types::UserEmpty { id: 4 }).to_bytes(),
                Some(1),
                ChatKind::Inaccessible,
            ),
            (
                tl::enums::Chat::Chat(basic_group()).to_bytes(),
                Some(1),
                ChatKind::BasicGroup,
            ),
            (
                tl::enums::Chat::Channel(channel(false, false)).to_bytes(),
                Some(1),
                ChatKind::Supergroup,
            ),
            (
                tl::enums::Chat::Channel(channel(false, true)).to_bytes(),
                Some(1),
                ChatKind::Gigagroup,
            ),
            (
                tl::enums::Chat::Channel(channel(true, false)).to_bytes(),
                Some(1),
                ChatKind::Channel,
            ),
            (
                tl::enums::Chat::Forbidden(tl::types::ChatForbidden {
                    id: 9,
                    title: "Unavailable".to_owned(),
                })
                .to_bytes(),
                Some(1),
                ChatKind::Inaccessible,
            ),
        ];

        for (bytes, account_id, expected) in cases {
            assert_eq!(
                normalize_serialized_peer_kind(&bytes, account_id)
                    .expect("current TL peer fixture should normalize"),
                expected
            );
        }
    }

    #[test]
    fn unsupported_and_specialized_media_keep_informative_cards() {
        let unsupported =
            normalize_serialized_media(&tl::enums::MessageMedia::Unsupported.to_bytes())
                .expect("unsupported constructor should remain representable");
        assert_eq!(unsupported.kind, MediaKind::Unsupported);
        assert_eq!(unsupported.title, "Unsupported Content");
        assert!(!unsupported.description.is_empty());

        let live_location = tl::enums::MessageMedia::GeoLive(tl::types::MessageMediaGeoLive {
            geo: tl::enums::GeoPoint::Point(tl::types::GeoPoint {
                long: 139.6917,
                lat: 35.6895,
                access_hash: 1,
                accuracy_radius: Some(10),
            }),
            heading: None,
            period: 900,
            proximity_notification_radius: None,
        });
        let specialized = normalize_serialized_media(&live_location.to_bytes())
            .expect("specialized constructor should remain representable");
        assert_eq!(specialized.kind, MediaKind::Specialized);
        assert!(!specialized.title.is_empty());
        assert!(!specialized.description.is_empty());
    }

    fn user(id: i64, is_self: bool, bot: bool) -> tl::types::User {
        tl::types::User {
            is_self,
            contact: false,
            mutual_contact: false,
            deleted: false,
            bot,
            bot_chat_history: false,
            bot_nochats: false,
            verified: false,
            restricted: false,
            min: false,
            bot_inline_geo: false,
            support: false,
            scam: false,
            apply_min_photo: false,
            fake: false,
            bot_attach_menu: false,
            premium: false,
            attach_menu_enabled: false,
            bot_can_edit: false,
            close_friend: false,
            stories_hidden: false,
            stories_unavailable: false,
            contact_require_premium: false,
            bot_business: false,
            bot_has_main_app: false,
            bot_forum_view: false,
            bot_forum_can_manage_topics: false,
            bot_can_manage_bots: false,
            bot_guestchat: false,
            bot_guard: false,
            id,
            access_hash: None,
            first_name: Some("Peer".to_owned()),
            last_name: None,
            username: None,
            phone: None,
            photo: None,
            status: None,
            bot_info_version: bot.then_some(1),
            restriction_reason: None,
            bot_inline_placeholder: None,
            lang_code: None,
            emoji_status: None,
            usernames: None,
            stories_max_id: None,
            color: None,
            profile_color: None,
            bot_active_users: None,
            bot_verification_icon: None,
            send_paid_messages_stars: None,
        }
    }

    fn basic_group() -> tl::types::Chat {
        tl::types::Chat {
            creator: false,
            left: false,
            deactivated: false,
            call_active: false,
            call_not_empty: false,
            noforwards: false,
            id: 5,
            title: "Basic group".to_owned(),
            photo: tl::enums::ChatPhoto::Empty,
            participants_count: 2,
            date: 0,
            version: 1,
            migrated_to: None,
            admin_rights: None,
            default_banned_rights: None,
        }
    }

    fn channel(broadcast: bool, gigagroup: bool) -> tl::types::Channel {
        tl::types::Channel {
            creator: false,
            left: false,
            broadcast,
            verified: false,
            megagroup: !broadcast,
            restricted: false,
            signatures: false,
            min: false,
            scam: false,
            has_link: false,
            has_geo: false,
            slowmode_enabled: false,
            call_active: false,
            call_not_empty: false,
            fake: false,
            gigagroup,
            noforwards: false,
            join_to_send: false,
            join_request: false,
            forum: false,
            stories_hidden: false,
            stories_hidden_min: false,
            stories_unavailable: false,
            signature_profiles: false,
            autotranslation: false,
            broadcast_messages_allowed: false,
            monoforum: false,
            forum_tabs: false,
            id: 6,
            access_hash: Some(7),
            title: "Channel".to_owned(),
            username: None,
            photo: tl::enums::ChatPhoto::Empty,
            date: 0,
            restriction_reason: None,
            admin_rights: None,
            banned_rights: None,
            default_banned_rights: None,
            participants_count: None,
            usernames: None,
            stories_max_id: None,
            color: None,
            profile_color: None,
            emoji_status: None,
            level: None,
            subscription_until_date: None,
            bot_verification_icon: None,
            send_paid_messages_stars: None,
            linked_monoforum_id: None,
        }
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
