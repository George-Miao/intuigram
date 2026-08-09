use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

/// Telegram server identifier for one Scheduled Message.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct ScheduledMessageId(pub i32);

/// Server-owned delivery trigger normalized for application use.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScheduledDeliveryView {
    /// Deliver at this UTC Unix timestamp.
    At(i32),
    /// Deliver next time the recipient is online.
    WhenOnline,
}

impl ScheduledDeliveryView {
    /// Parses `online` or an RFC 3339 time carrying an explicit UTC offset.
    pub fn parse(value: &str) -> Option<Self> {
        if value.trim() == "online" {
            return Some(Self::WhenOnline);
        }
        OffsetDateTime::parse(value.trim(), &Rfc3339)
            .ok()
            .and_then(|date| i32::try_from(date.unix_timestamp()).ok())
            .map(Self::At)
    }

    /// Returns a stable, editable representation with explicit UTC offset.
    #[must_use]
    pub fn editable(self) -> String {
        match self {
            Self::WhenOnline => "online".to_owned(),
            Self::At(timestamp) => OffsetDateTime::from_unix_timestamp(i64::from(timestamp))
                .ok()
                .and_then(|date| date.format(&Rfc3339).ok())
                .unwrap_or_else(|| timestamp.to_string()),
        }
    }
}

/// One Scheduled Message kept outside ordinary Message History.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScheduledMessageView {
    /// Server-owned identity.
    pub id: ScheduledMessageId,

    /// Planned delivery trigger.
    pub delivery: ScheduledDeliveryView,

    /// Text or stable media fallback.
    pub summary: String,
}

/// Active Scheduled Message management surface.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScheduledManagerView {
    /// Chat whose server-owned history is displayed.
    pub chat: super::ChatId,

    /// Original peer for a Channel direct-message dialog.
    pub saved_peer: Option<super::ChatId>,

    /// Scheduled history, separate from the Transcript.
    pub messages: Vec<ScheduledMessageView>,

    /// Active Scheduled Message row.
    pub selected: usize,

    /// Nested editor, when creating or changing a message.
    pub editor: Option<ScheduledEditorView>,

    /// Destructive or immediate-send confirmation.
    pub confirmation: Option<ScheduledConfirmationView>,

    /// Whether Telegram work is pending.
    pub pending: bool,
}

/// One create, edit, or reschedule form.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScheduledEditorView {
    /// Operation represented by the form.
    pub operation: ScheduledEditorOperation,

    /// Scheduled text for create/edit operations.
    pub text: String,

    /// `online` or RFC 3339 timestamp with an explicit offset.
    pub delivery: String,

    /// Active editable row.
    pub selected: usize,
}

/// Operation owning a Scheduled Message editor.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScheduledEditorOperation {
    /// Create a new Scheduled Message.
    Create,
    /// Replace the text of an existing Scheduled Message.
    Edit(ScheduledMessageId),
    /// Change only its delivery trigger.
    Reschedule(ScheduledMessageId),
}

/// Explicit confirmation before delete or immediate delivery.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ScheduledConfirmationView {
    /// Target Scheduled Message.
    pub message: ScheduledMessageId,

    /// Whether this confirms immediate delivery rather than deletion.
    pub send_now: bool,
}

/// Typed mutation requested from the Scheduled Message adapter.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ScheduledRequest {
    /// Create a new Scheduled Message.
    Create {
        /// Delivery trigger.
        delivery: ScheduledDeliveryView,

        /// Plain Message text.
        text: String,
    },
    /// Replace Message text.
    Edit {
        /// Server-owned identity.
        message: ScheduledMessageId,

        /// Replacement text.
        text: String,
    },
    /// Change the delivery trigger.
    Reschedule {
        /// Server-owned identity.
        message: ScheduledMessageId,

        /// Replacement trigger.
        delivery: ScheduledDeliveryView,
    },
    /// Delete without sending.
    Delete {
        /// Server-owned identity.
        message: ScheduledMessageId,
    },
    /// Request immediate delivery.
    SendNow {
        /// Server-owned identity.
        message: ScheduledMessageId,
    },
}
