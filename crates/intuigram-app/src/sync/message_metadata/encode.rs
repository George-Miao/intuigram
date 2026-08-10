use intuigram_lib::{
    GiftKindView, GiveawayInfoView, GiveawayStateView, MediaKind, PaidMediaItemView,
    SpecializedMediaView,
};

use super::types::{
    StoredGiftKind, StoredGiveawayInfo, StoredMediaKind, StoredPaidMediaItem,
    StoredSpecializedMedia, StoredStoryState, StoredTodoItem,
};

pub(in crate::sync) fn stored_specialized_media(
    media: &SpecializedMediaView,
) -> StoredSpecializedMedia {
    match media {
        SpecializedMediaView::LiveLocation(location) => StoredSpecializedMedia::LiveLocation {
            latitude_microdegrees: location.latitude_microdegrees,
            longitude_microdegrees: location.longitude_microdegrees,
            heading_degrees: location.heading_degrees,
            period_seconds: location.period_seconds,
            proximity_radius_metres: location.proximity_radius_metres,
            accuracy_radius_metres: location.accuracy_radius_metres,
        },
        SpecializedMediaView::Game(game) => StoredSpecializedMedia::Game {
            id: game.id,
            short_name: game.short_name.clone(),
            title: game.title.clone(),
            description: game.description.clone(),
        },
        SpecializedMediaView::Invoice(invoice) => StoredSpecializedMedia::Invoice {
            title: invoice.title.clone(),
            description: invoice.description.clone(),
            currency: invoice.currency.clone(),
            total_minor_units: invoice.total_minor_units,
            receipt_message: invoice.receipt_message.map(|message| message.0),
            shipping_address_requested: invoice.shipping_address_requested,
            test: invoice.test,
            extended_media: invoice.extended_media,
        },
        SpecializedMediaView::PaidMedia(media) => StoredSpecializedMedia::PaidMedia {
            stars_amount: media.stars_amount,
            items: media
                .items
                .iter()
                .map(|item| match item {
                    PaidMediaItemView::Preview {
                        width,
                        height,
                        duration_seconds,
                    } => StoredPaidMediaItem::Preview {
                        width: *width,
                        height: *height,
                        duration_seconds: *duration_seconds,
                    },
                    PaidMediaItemView::Available {
                        kind,
                        title,
                        remote_id,
                    } => StoredPaidMediaItem::Available {
                        media_kind: stored_media_kind(*kind),
                        title: title.clone(),
                        remote_id: remote_id.clone(),
                    },
                })
                .collect(),
        },
        SpecializedMediaView::Giveaway(giveaway) => StoredSpecializedMedia::Giveaway {
            results: giveaway.state == GiveawayStateView::Results,
            quantity: giveaway.quantity,
            premium_months: giveaway.premium_months,
            stars: giveaway.stars,
            prize_description: giveaway.prize_description.clone(),
            until_date: giveaway.until_date.clone(),
            only_new_subscribers: giveaway.only_new_subscribers,
            winners_visible: giveaway.winners_visible,
            country_codes: giveaway.country_codes.clone(),
            channel_count: giveaway.channel_count,
            winners_count: giveaway.winners_count,
            unclaimed_count: giveaway.unclaimed_count,
            refunded: giveaway.refunded,
            info: giveaway.info.as_ref().map(|info| match info {
                GiveawayInfoView::Active {
                    participating,
                    preparing_results,
                    start_date,
                    eligibility_issue,
                } => StoredGiveawayInfo::Active {
                    participating: *participating,
                    preparing_results: *preparing_results,
                    start_date: start_date.clone(),
                    eligibility_issue: eligibility_issue.clone(),
                },
                GiveawayInfoView::Results {
                    winner,
                    start_date,
                    finish_date,
                    activated_count,
                    gift_code_slug,
                } => StoredGiveawayInfo::Results {
                    winner: *winner,
                    start_date: start_date.clone(),
                    finish_date: finish_date.clone(),
                    activated_count: *activated_count,
                    gift_code_slug: gift_code_slug.clone(),
                },
            }),
        },
        SpecializedMediaView::Gift(gift) => StoredSpecializedMedia::Gift {
            gift_kind: match gift.kind {
                GiftKindView::Premium => StoredGiftKind::Premium,
                GiftKindView::Stars => StoredGiftKind::Stars,
                GiftKindView::Ton => StoredGiftKind::Ton,
                GiftKindView::Code => StoredGiftKind::Code,
                GiftKindView::StarGift => StoredGiftKind::StarGift,
                GiftKindView::UniqueStarGift => StoredGiftKind::UniqueStarGift,
            },
            title: gift.title.clone(),
            stars: gift.stars,
            days: gift.days,
            currency: gift.currency.clone(),
            amount_minor_units: gift.amount_minor_units,
            crypto_currency: gift.crypto_currency.clone(),
            crypto_amount_minor_units: gift.crypto_amount_minor_units,
            identifier: gift.identifier.clone(),
            saved: gift.saved,
            converted: gift.converted,
            upgraded: gift.upgraded,
            refunded: gift.refunded,
            anonymous: gift.anonymous,
        },
        SpecializedMediaView::Story(story) => StoredSpecializedMedia::Story {
            peer: story.peer.0,
            id: story.id,
            state: match story.state {
                intuigram_lib::StoryStateView::Available => StoredStoryState::Available,
                intuigram_lib::StoryStateView::Skipped => StoredStoryState::Skipped,
                intuigram_lib::StoryStateView::Deleted => StoredStoryState::Deleted,
                intuigram_lib::StoryStateView::Reference => StoredStoryState::Reference,
            },
            caption: story.caption.clone(),
            date: story.date.clone(),
            expires: story.expires.clone(),
            via_mention: story.via_mention,
            close_friends: story.close_friends,
            live: story.live,
        },
        SpecializedMediaView::TodoList(todo) => StoredSpecializedMedia::TodoList {
            title: todo.title.clone(),
            items: todo
                .items
                .iter()
                .map(|item| StoredTodoItem {
                    id: item.id,
                    title: item.title.clone(),
                    completed: item.completed,
                    completed_by: item.completed_by.map(|peer| peer.0),
                    completed_date: item.completed_date.clone(),
                })
                .collect(),
            others_can_append: todo.others_can_append,
            others_can_complete: todo.others_can_complete,
        },
    }
}

pub(in crate::sync) fn stored_paid_media_items_json(items: &[PaidMediaItemView]) -> String {
    serde_json::to_string(
        &items
            .iter()
            .map(|item| match item {
                PaidMediaItemView::Preview {
                    width,
                    height,
                    duration_seconds,
                } => StoredPaidMediaItem::Preview {
                    width: *width,
                    height: *height,
                    duration_seconds: *duration_seconds,
                },
                PaidMediaItemView::Available {
                    kind,
                    title,
                    remote_id,
                } => StoredPaidMediaItem::Available {
                    media_kind: stored_media_kind(*kind),
                    title: title.clone(),
                    remote_id: remote_id.clone(),
                },
            })
            .collect::<Vec<_>>(),
    )
    .expect("fixed paid-media child metadata is always JSON-serializable")
}

fn stored_media_kind(kind: MediaKind) -> StoredMediaKind {
    match kind {
        MediaKind::Photo => StoredMediaKind::Photo,
        MediaKind::Video => StoredMediaKind::Video,
        MediaKind::Animation => StoredMediaKind::Animation,
        MediaKind::Sticker => StoredMediaKind::Sticker,
        MediaKind::File => StoredMediaKind::File,
        MediaKind::Audio => StoredMediaKind::Audio,
        MediaKind::Voice => StoredMediaKind::Voice,
        MediaKind::VideoNote => StoredMediaKind::VideoNote,
        MediaKind::LinkPreview => StoredMediaKind::LinkPreview,
        MediaKind::Poll => StoredMediaKind::Poll,
        MediaKind::Contact => StoredMediaKind::Contact,
        MediaKind::Location => StoredMediaKind::Location,
        MediaKind::Venue => StoredMediaKind::Venue,
        MediaKind::Dice => StoredMediaKind::Dice,
        MediaKind::LiveLocation => StoredMediaKind::LiveLocation,
        MediaKind::Game => StoredMediaKind::Game,
        MediaKind::Invoice => StoredMediaKind::Invoice,
        MediaKind::PaidMedia => StoredMediaKind::PaidMedia,
        MediaKind::Giveaway => StoredMediaKind::Giveaway,
        MediaKind::Gift => StoredMediaKind::Gift,
        MediaKind::Story => StoredMediaKind::Story,
        MediaKind::TodoList => StoredMediaKind::TodoList,
        MediaKind::Unsupported => StoredMediaKind::Unsupported,
    }
}
