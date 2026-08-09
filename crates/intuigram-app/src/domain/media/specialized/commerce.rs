/// Telegram invoice metadata retained without authorizing a purchase.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InvoiceView {
    /// Merchant-provided title.
    pub title: String,

    /// Merchant-provided description.
    pub description: String,

    /// ISO 4217 currency code supplied by Telegram.
    pub currency: String,

    /// Amount in the currency's smallest units.
    pub total_minor_units: i64,

    /// Receipt Message when Telegram reports that payment already completed.
    pub receipt_message: Option<super::super::super::MessageId>,

    /// Whether checkout would request a shipping address.
    pub shipping_address_requested: bool,

    /// Whether Telegram marks the invoice as a test transaction.
    pub test: bool,

    /// Whether the invoice includes an extended media preview.
    pub extended_media: bool,
}

/// Paid-media price and disclosure state without a purchase capability.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PaidMediaView {
    /// Telegram Stars requested to unlock the media.
    pub stars_amount: u64,

    /// Media entries in Telegram-defined order.
    pub items: Vec<PaidMediaItemView>,
}

/// One paid-media entry before or after Telegram reveals its full constructor.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PaidMediaItemView {
    /// Locked entry represented only by disclosure-safe preview metadata.
    Preview {
        /// Preview width, when Telegram provides it.
        width: Option<u32>,

        /// Preview height, when Telegram provides it.
        height: Option<u32>,

        /// Video duration, when this preview represents a video.
        duration_seconds: Option<u32>,
    },

    /// Full media constructor made available by Telegram.
    Available {
        /// Normalized media family.
        kind: super::super::MediaKind,

        /// Informative title or filename.
        title: String,

        /// Stable Telegram media identity when available.
        remote_id: Option<String>,
    },
}
