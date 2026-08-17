use super::*;

pub(super) fn normalize_game(media: &tl::types::MessageMediaGame) -> MediaCard {
    let tl::enums::Game::Game(game) = &media.game;
    MediaCard {
        kind: MediaKind::Game,
        title: game.title.clone(),
        description: String::new(),
        details: Vec::new(),
        poll: None,
        specialized: Some(SpecializedMediaView::Game(GameView {
            id: game.id,
            short_name: game.short_name.clone(),
            title: game.title.clone(),
            description: game.description.clone(),
        })),
        remote_id: None,
    }
}

pub(super) fn normalize_giveaway(media: &tl::types::MessageMediaGiveaway) -> MediaCard {
    giveaway_card(GiveawayView {
        state: GiveawayStateView::Active,
        quantity: nonnegative_u32(Some(media.quantity)).unwrap_or(0),
        premium_months: nonnegative_u32(media.months),
        stars: media.stars.and_then(|stars| u64::try_from(stars).ok()),
        prize_description: media.prize_description.clone(),
        until_date: format_date(media.until_date),
        only_new_subscribers: media.only_new_subscribers,
        winners_visible: media.winners_are_visible,
        country_codes: media.countries_iso2.clone().unwrap_or_default(),
        channel_count: u32::try_from(media.channels.len()).unwrap_or(u32::MAX),
        winners_count: None,
        unclaimed_count: None,
        refunded: false,
        info: None,
    })
}

pub(super) fn normalize_giveaway_results(
    media: &tl::types::MessageMediaGiveawayResults,
) -> MediaCard {
    let winners_count = nonnegative_u32(Some(media.winners_count)).unwrap_or(0);
    let unclaimed_count = nonnegative_u32(Some(media.unclaimed_count)).unwrap_or(0);
    giveaway_card(GiveawayView {
        state: GiveawayStateView::Results,
        quantity: winners_count.saturating_add(unclaimed_count),
        premium_months: nonnegative_u32(media.months),
        stars: media.stars.and_then(|stars| u64::try_from(stars).ok()),
        prize_description: media.prize_description.clone(),
        until_date: format_date(media.until_date),
        only_new_subscribers: media.only_new_subscribers,
        winners_visible: !media.winners.is_empty(),
        country_codes: Vec::new(),
        channel_count: media
            .additional_peers_count
            .and_then(|count| u32::try_from(count).ok())
            .unwrap_or(0)
            .saturating_add(1),
        winners_count: Some(winners_count),
        unclaimed_count: Some(unclaimed_count),
        refunded: media.refunded,
        info: None,
    })
}

fn giveaway_card(giveaway: GiveawayView) -> MediaCard {
    let title = if giveaway.state == GiveawayStateView::Active {
        "Giveaway"
    } else {
        "Giveaway results"
    };
    MediaCard {
        kind: MediaKind::Giveaway,
        title: title.to_owned(),
        description: String::new(),
        details: Vec::new(),
        poll: None,
        specialized: Some(SpecializedMediaView::Giveaway(giveaway)),
        remote_id: None,
    }
}

pub(super) fn normalize_invoice(media: &tl::types::MessageMediaInvoice) -> MediaCard {
    MediaCard {
        kind: MediaKind::Invoice,
        title: media.title.clone(),
        description: String::new(),
        details: Vec::new(),
        poll: None,
        specialized: Some(SpecializedMediaView::Invoice(InvoiceView {
            title: media.title.clone(),
            description: media.description.clone(),
            currency: media.currency.clone(),
            total_minor_units: media.total_amount,
            receipt_message: media
                .receipt_msg_id
                .map(|message| MessageId(i64::from(message))),
            shipping_address_requested: media.shipping_address_requested,
            test: media.test,
            extended_media: media.extended_media.is_some(),
        })),
        remote_id: None,
    }
}

pub(super) fn normalize_paid_media(media: &tl::types::MessageMediaPaidMedia) -> MediaCard {
    let items = normalize_paid_media_items(&media.extended_media);
    MediaCard {
        kind: MediaKind::PaidMedia,
        title: "Paid media".to_owned(),
        description: String::new(),
        details: Vec::new(),
        poll: None,
        specialized: Some(SpecializedMediaView::PaidMedia(PaidMediaView {
            stars_amount: u64::try_from(media.stars_amount).unwrap_or(0),
            items,
        })),
        remote_id: None,
    }
}

pub(crate) fn normalize_paid_media_items(
    extended_media: &[tl::enums::MessageExtendedMedia],
) -> Vec<PaidMediaItemView> {
    extended_media
        .iter()
        .map(|media| match media {
            tl::enums::MessageExtendedMedia::Preview(preview) => PaidMediaItemView::Preview {
                width: nonnegative_u32(preview.w),
                height: nonnegative_u32(preview.h),
                duration_seconds: nonnegative_u32(preview.video_duration),
            },
            tl::enums::MessageExtendedMedia::Media(media) => {
                let card = normalize_media(&media.media);
                PaidMediaItemView::Available {
                    kind: card.kind,
                    title: card.title,
                    remote_id: card.remote_id,
                }
            }
        })
        .collect()
}

pub(super) fn normalize_live_location(media: &tl::types::MessageMediaGeoLive) -> MediaCard {
    let Some((latitude_microdegrees, longitude_microdegrees, accuracy_radius_metres)) =
        geo_microdegrees(&media.geo)
    else {
        return card(
            MediaKind::LiveLocation,
            "Live location",
            "coordinates unavailable",
            vec![format!(
                "sharing for {} min",
                nonnegative_u32(Some(media.period))
                    .unwrap_or(0)
                    .div_ceil(60)
            )],
            None,
        );
    };
    MediaCard {
        kind: MediaKind::LiveLocation,
        title: "Live location".to_owned(),
        description: String::new(),
        details: Vec::new(),
        poll: None,
        specialized: Some(SpecializedMediaView::LiveLocation(LiveLocationView {
            latitude_microdegrees,
            longitude_microdegrees,
            heading_degrees: media
                .heading
                .and_then(|heading| u16::try_from(heading).ok()),
            period_seconds: nonnegative_u32(Some(media.period)).unwrap_or(0),
            proximity_radius_metres: nonnegative_u32(media.proximity_notification_radius),
            accuracy_radius_metres,
        })),
        remote_id: None,
    }
}

fn geo_microdegrees(geo: &tl::enums::GeoPoint) -> Option<(i32, i32, Option<u32>)> {
    match geo {
        tl::enums::GeoPoint::Point(point) => Some((
            (point.lat * 1_000_000.0).round() as i32,
            (point.long * 1_000_000.0).round() as i32,
            nonnegative_u32(point.accuracy_radius),
        )),
        tl::enums::GeoPoint::Empty => None,
    }
}
