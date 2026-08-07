use std::collections::HashMap;
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
    MediaKind, MessageDetails, MessageDirection, MessageId, MessageView, PollOptionView, PollView,
    ReactionView, TextEntity, TextEntityKind,
};
use snafu::{OptionExt, ResultExt, Snafu};

use crate::thread::read_request;

mod authorization;
mod client_bootstrap;
mod client_connection;
mod client_dialogs;
mod client_download;
mod client_history;
mod client_initialize;
mod client_links;
mod client_phone;
mod client_poll;
mod client_qr;
mod client_send;
mod client_vote;
mod connection;
mod dialog_normalization;
mod entity_conversion;
mod error;
mod live_normalization;
mod login_normalization;
mod media_normalization;
mod message_normalization;
mod message_operations;
mod peer_directory;
mod session_types;

use authorization::{normalize_authorization, password_parameters};
use client_dialogs::DialogBatch;
pub use client_send::{TextSend, UploadSend};
use connection::Connection;
#[cfg(test)]
pub(crate) use connection::flood_wait_delay;
pub use connection::{Client, LiveUpdates};
#[cfg(test)]
pub(crate) use dialog_normalization::contains_login_token_update;
pub use dialog_normalization::normalize_serialized_peer_kind;
use dialog_normalization::take_login_token_update;
pub(crate) use dialog_normalization::{
    chat_traits, cloud_chat_can_pin, dialog_filter_id, dialog_folder_membership,
    normalize_dialog_folders, set_dialog_filter_membership,
};
use entity_conversion::serialize_entities;
use error::*;
pub use error::{Error, Result};
pub(crate) use live_normalization::normalize_live_update;
pub(crate) use login_normalization::{
    direct_data_centers, ensure_production_environment, input_reply_to, login_error_action,
    normalize_code_delivery, normalize_code_delivery_method, qr_login_uri, rpc_migration_dc,
};
pub use media_normalization::normalize_serialized_media;
pub(crate) use media_normalization::service_event_description;
use media_normalization::{
    format_timestamp, media_card_fallback, nonnegative_u32, normalize_forward, normalize_media,
    normalize_reactions, user_display_name,
};
use message_normalization::{
    mark_channel_id, marked_peer_id, message_body, message_chat_id, message_parts,
    normalize_entities, normalize_message, reply_message_id, text_with_entities,
};
pub use peer_directory::{PeerAddress, PeerDirectory};
pub use session_types::{
    CodeRequest, CodeSignIn, LiveEvent, QrLogin, QrLoginMigration, Session, UpdateCursor,
    UpdateScope,
};

static QR_PING_ID: AtomicI64 = AtomicI64::new(1);
const MAX_LOGIN_RESTARTS: usize = 3;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum LoginErrorAction {
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

    /// Telegram media behavior for this payload.
    pub kind: UploadKind,
}

/// Telegram upload presentation selected by the composition adapter.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UploadKind {
    /// Compress and display as a photo.
    Photo,

    /// Display as a streamable video.
    Video,

    /// Preserve as a generic file.
    File,
}

/// Stable Telegram identifiers retained across one upload retry sequence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UploadIds {
    /// Identifier used by Telegram's file-part store.
    pub file: i64,

    /// Idempotency identifier used by `messages.sendMedia`.
    pub message: i64,
}

/// Complete Telegram media payload ready for collision-safe local saving.
pub struct DownloadedMedia {
    /// Suggested filename derived from Telegram metadata.
    pub name: String,

    /// Telegram-provided Internet media type.
    pub mime_type: String,

    /// Complete downloaded bytes.
    pub bytes: Vec<u8>,
}
