use super::*;

/// One rich text Message submission.
pub struct TextSend {
    /// Destination Chat.
    pub chat: ChatId,

    /// Plain text after composer markup is removed.
    pub text: String,

    /// Telegram rich-text entities using UTF-16 offsets.
    pub entities: Vec<TextEntity>,

    /// Whether Telegram may generate a webpage preview.
    pub link_preview: bool,

    /// Direct reply target.
    pub reply_to: Option<MessageId>,

    /// Active Thread root.
    pub thread_root: Option<MessageId>,

    /// User topic inside an administrator-owned monoforum.
    pub monoforum_peer: Option<ChatId>,

    /// Stable idempotency token for this operation.
    pub random_id: i64,

    /// Server-side delivery time, or immediate delivery when absent.
    pub schedule_date: Option<i32>,
}

/// One uploaded photo, video, or file submission.
pub struct UploadSend {
    /// Destination Chat.
    pub chat: ChatId,

    /// Complete upload payload.
    pub upload: Upload,

    /// Plain caption text.
    pub caption: String,

    /// Caption entities using UTF-16 offsets.
    pub entities: Vec<TextEntity>,

    /// Direct reply target.
    pub reply_to: Option<MessageId>,

    /// Active Thread root.
    pub thread_root: Option<MessageId>,

    /// User topic inside an administrator-owned monoforum.
    pub monoforum_peer: Option<ChatId>,

    /// Stable upload and Message idempotency identifiers.
    pub ids: UploadIds,
}
