use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(in crate::sync) enum StoredSpecializedMedia {
    LiveLocation {
        latitude_microdegrees: i32,
        longitude_microdegrees: i32,
        heading_degrees: Option<u16>,
        period_seconds: u32,
        proximity_radius_metres: Option<u32>,
        accuracy_radius_metres: Option<u32>,
    },
    Game {
        id: i64,
        short_name: String,
        title: String,
        description: String,
    },
    Invoice {
        title: String,
        description: String,
        currency: String,
        total_minor_units: i64,
        receipt_message: Option<i64>,
        shipping_address_requested: bool,
        test: bool,
        extended_media: bool,
    },
    PaidMedia {
        stars_amount: u64,
        items: Vec<StoredPaidMediaItem>,
    },
    Giveaway {
        results: bool,
        quantity: u32,
        premium_months: Option<u32>,
        stars: Option<u64>,
        prize_description: Option<String>,
        until_date: String,
        only_new_subscribers: bool,
        winners_visible: bool,
        country_codes: Vec<String>,
        channel_count: u32,
        winners_count: Option<u32>,
        unclaimed_count: Option<u32>,
        refunded: bool,
        info: Option<StoredGiveawayInfo>,
    },
    Gift {
        gift_kind: StoredGiftKind,
        title: String,
        stars: Option<u64>,
        days: Option<u32>,
        currency: Option<String>,
        amount_minor_units: Option<i64>,
        crypto_currency: Option<String>,
        crypto_amount_minor_units: Option<i64>,
        identifier: Option<String>,
        saved: bool,
        converted: bool,
        upgraded: bool,
        refunded: bool,
        anonymous: bool,
    },
    Story {
        peer: i64,
        id: i32,
        state: StoredStoryState,
        caption: Option<String>,
        date: String,
        expires: String,
        via_mention: bool,
        close_friends: bool,
        live: bool,
    },
    TodoList {
        title: String,
        items: Vec<StoredTodoItem>,
        others_can_append: bool,
        others_can_complete: bool,
    },
}

#[derive(Deserialize, Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub(in crate::sync) enum StoredGiveawayInfo {
    Active {
        participating: bool,
        preparing_results: bool,
        start_date: String,
        eligibility_issue: Option<String>,
    },
    Results {
        winner: bool,
        start_date: String,
        finish_date: String,
        activated_count: Option<u32>,
        gift_code_slug: Option<String>,
    },
}

#[derive(Deserialize, Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub(in crate::sync) enum StoredPaidMediaItem {
    Preview {
        width: Option<u32>,
        height: Option<u32>,
        duration_seconds: Option<u32>,
    },
    Available {
        media_kind: StoredMediaKind,
        title: String,
        remote_id: Option<String>,
    },
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub(in crate::sync) enum StoredMediaKind {
    Photo,
    Video,
    Animation,
    Sticker,
    File,
    Audio,
    Voice,
    VideoNote,
    LinkPreview,
    Poll,
    Contact,
    Location,
    Venue,
    Dice,
    LiveLocation,
    Game,
    Invoice,
    PaidMedia,
    Giveaway,
    Gift,
    Story,
    TodoList,
    Unsupported,
}

#[derive(Deserialize, Serialize)]
pub(in crate::sync) struct StoredTodoItem {
    pub(super) id: i32,
    pub(super) title: String,
    pub(super) completed: bool,
    pub(super) completed_by: Option<i64>,
    pub(super) completed_date: Option<String>,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub(in crate::sync) enum StoredStoryState {
    Available,
    Skipped,
    Deleted,
    Reference,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub(in crate::sync) enum StoredGiftKind {
    Premium,
    Stars,
    Ton,
    Code,
    StarGift,
    UniqueStarGift,
}
