use intuigram_app::{
    ChatId, GameView, GiftKindView, GiftView, GiveawayInfoView, GiveawayStateView, GiveawayView,
    InvoiceView, LiveLocationView, MediaKind, MessageId, PaidMediaItemView, PaidMediaView,
    SharedStoryView, SpecializedMediaView, StoryStateView, TodoItemView, TodoListView,
};

use super::types::{
    StoredGiftKind, StoredGiveawayInfo, StoredMediaKind, StoredPaidMediaItem,
    StoredSpecializedMedia, StoredStoryState,
};

pub(in crate::sync) fn cached_specialized_media(
    media: &StoredSpecializedMedia,
) -> SpecializedMediaView {
    match media {
        StoredSpecializedMedia::LiveLocation {
            latitude_microdegrees,
            longitude_microdegrees,
            heading_degrees,
            period_seconds,
            proximity_radius_metres,
            accuracy_radius_metres,
        } => SpecializedMediaView::LiveLocation(LiveLocationView {
            latitude_microdegrees: *latitude_microdegrees,
            longitude_microdegrees: *longitude_microdegrees,
            heading_degrees: *heading_degrees,
            period_seconds: *period_seconds,
            proximity_radius_metres: *proximity_radius_metres,
            accuracy_radius_metres: *accuracy_radius_metres,
        }),
        StoredSpecializedMedia::Game {
            id,
            short_name,
            title,
            description,
        } => SpecializedMediaView::Game(GameView {
            id: *id,
            short_name: short_name.clone(),
            title: title.clone(),
            description: description.clone(),
        }),
        StoredSpecializedMedia::Invoice {
            title,
            description,
            currency,
            total_minor_units,
            receipt_message,
            shipping_address_requested,
            test,
            extended_media,
        } => SpecializedMediaView::Invoice(InvoiceView {
            title: title.clone(),
            description: description.clone(),
            currency: currency.clone(),
            total_minor_units: *total_minor_units,
            receipt_message: receipt_message.map(MessageId),
            shipping_address_requested: *shipping_address_requested,
            test: *test,
            extended_media: *extended_media,
        }),
        StoredSpecializedMedia::PaidMedia {
            stars_amount,
            items,
        } => SpecializedMediaView::PaidMedia(PaidMediaView {
            stars_amount: *stars_amount,
            items: items
                .iter()
                .map(|item| match item {
                    StoredPaidMediaItem::Preview {
                        width,
                        height,
                        duration_seconds,
                    } => PaidMediaItemView::Preview {
                        width: *width,
                        height: *height,
                        duration_seconds: *duration_seconds,
                    },
                    StoredPaidMediaItem::Available {
                        media_kind,
                        title,
                        remote_id,
                    } => PaidMediaItemView::Available {
                        kind: cached_media_kind(media_kind),
                        title: title.clone(),
                        remote_id: remote_id.clone(),
                    },
                })
                .collect(),
        }),
        StoredSpecializedMedia::Giveaway {
            results,
            quantity,
            premium_months,
            stars,
            prize_description,
            until_date,
            only_new_subscribers,
            winners_visible,
            country_codes,
            channel_count,
            winners_count,
            unclaimed_count,
            refunded,
            info,
        } => SpecializedMediaView::Giveaway(GiveawayView {
            state: if *results {
                GiveawayStateView::Results
            } else {
                GiveawayStateView::Active
            },
            quantity: *quantity,
            premium_months: *premium_months,
            stars: *stars,
            prize_description: prize_description.clone(),
            until_date: until_date.clone(),
            only_new_subscribers: *only_new_subscribers,
            winners_visible: *winners_visible,
            country_codes: country_codes.clone(),
            channel_count: *channel_count,
            winners_count: *winners_count,
            unclaimed_count: *unclaimed_count,
            refunded: *refunded,
            info: info.as_ref().map(|info| match info {
                StoredGiveawayInfo::Active {
                    participating,
                    preparing_results,
                    start_date,
                    eligibility_issue,
                } => GiveawayInfoView::Active {
                    participating: *participating,
                    preparing_results: *preparing_results,
                    start_date: start_date.clone(),
                    eligibility_issue: eligibility_issue.clone(),
                },
                StoredGiveawayInfo::Results {
                    winner,
                    start_date,
                    finish_date,
                    activated_count,
                    gift_code_slug,
                } => GiveawayInfoView::Results {
                    winner: *winner,
                    start_date: start_date.clone(),
                    finish_date: finish_date.clone(),
                    activated_count: *activated_count,
                    gift_code_slug: gift_code_slug.clone(),
                },
            }),
        }),
        StoredSpecializedMedia::Gift {
            gift_kind,
            title,
            stars,
            days,
            currency,
            amount_minor_units,
            crypto_currency,
            crypto_amount_minor_units,
            identifier,
            saved,
            converted,
            upgraded,
            refunded,
            anonymous,
        } => SpecializedMediaView::Gift(GiftView {
            kind: match gift_kind {
                StoredGiftKind::Premium => GiftKindView::Premium,
                StoredGiftKind::Stars => GiftKindView::Stars,
                StoredGiftKind::Ton => GiftKindView::Ton,
                StoredGiftKind::Code => GiftKindView::Code,
                StoredGiftKind::StarGift => GiftKindView::StarGift,
                StoredGiftKind::UniqueStarGift => GiftKindView::UniqueStarGift,
            },
            title: title.clone(),
            stars: *stars,
            days: *days,
            currency: currency.clone(),
            amount_minor_units: *amount_minor_units,
            crypto_currency: crypto_currency.clone(),
            crypto_amount_minor_units: *crypto_amount_minor_units,
            identifier: identifier.clone(),
            saved: *saved,
            converted: *converted,
            upgraded: *upgraded,
            refunded: *refunded,
            anonymous: *anonymous,
        }),
        StoredSpecializedMedia::Story {
            peer,
            id,
            state,
            caption,
            date,
            expires,
            via_mention,
            close_friends,
            live,
        } => SpecializedMediaView::Story(SharedStoryView {
            peer: ChatId(*peer),
            id: *id,
            state: match state {
                StoredStoryState::Available => StoryStateView::Available,
                StoredStoryState::Skipped => StoryStateView::Skipped,
                StoredStoryState::Deleted => StoryStateView::Deleted,
                StoredStoryState::Reference => StoryStateView::Reference,
            },
            caption: caption.clone(),
            date: date.clone(),
            expires: expires.clone(),
            via_mention: *via_mention,
            close_friends: *close_friends,
            live: *live,
        }),
        StoredSpecializedMedia::TodoList {
            title,
            items,
            others_can_append,
            others_can_complete,
        } => SpecializedMediaView::TodoList(TodoListView {
            title: title.clone(),
            items: items
                .iter()
                .map(|item| TodoItemView {
                    id: item.id,
                    title: item.title.clone(),
                    completed: item.completed,
                    completed_by: item.completed_by.map(ChatId),
                    completed_date: item.completed_date.clone(),
                })
                .collect(),
            others_can_append: *others_can_append,
            others_can_complete: *others_can_complete,
        }),
    }
}

fn cached_media_kind(kind: &StoredMediaKind) -> MediaKind {
    match kind {
        StoredMediaKind::Photo => MediaKind::Photo,
        StoredMediaKind::Video => MediaKind::Video,
        StoredMediaKind::Animation => MediaKind::Animation,
        StoredMediaKind::Sticker => MediaKind::Sticker,
        StoredMediaKind::File => MediaKind::File,
        StoredMediaKind::Audio => MediaKind::Audio,
        StoredMediaKind::Voice => MediaKind::Voice,
        StoredMediaKind::VideoNote => MediaKind::VideoNote,
        StoredMediaKind::LinkPreview => MediaKind::LinkPreview,
        StoredMediaKind::Poll => MediaKind::Poll,
        StoredMediaKind::Contact => MediaKind::Contact,
        StoredMediaKind::Location => MediaKind::Location,
        StoredMediaKind::Venue => MediaKind::Venue,
        StoredMediaKind::Dice => MediaKind::Dice,
        StoredMediaKind::LiveLocation => MediaKind::LiveLocation,
        StoredMediaKind::Game => MediaKind::Game,
        StoredMediaKind::Invoice => MediaKind::Invoice,
        StoredMediaKind::PaidMedia => MediaKind::PaidMedia,
        StoredMediaKind::Giveaway => MediaKind::Giveaway,
        StoredMediaKind::Gift => MediaKind::Gift,
        StoredMediaKind::Story => MediaKind::Story,
        StoredMediaKind::TodoList => MediaKind::TodoList,
        StoredMediaKind::Unsupported => MediaKind::Unsupported,
    }
}
