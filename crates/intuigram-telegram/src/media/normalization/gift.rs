use super::*;

pub(super) fn normalize_gift_action(action: &tl::enums::MessageAction) -> Option<MediaCard> {
    let gift = match action {
        tl::enums::MessageAction::GiftPremium(gift) => GiftView {
            kind: GiftKindView::Premium,
            title: "Premium gift".to_owned(),
            stars: None,
            days: nonnegative_u32(Some(gift.days)),
            currency: Some(gift.currency.clone()),
            amount_minor_units: Some(gift.amount),
            crypto_currency: gift.crypto_currency.clone(),
            crypto_amount_minor_units: gift.crypto_amount,
            identifier: None,
            saved: false,
            converted: false,
            upgraded: false,
            refunded: false,
            anonymous: false,
        },
        tl::enums::MessageAction::GiftStars(gift) => GiftView {
            kind: GiftKindView::Stars,
            title: "Stars gift".to_owned(),
            stars: u64::try_from(gift.stars).ok(),
            days: None,
            currency: Some(gift.currency.clone()),
            amount_minor_units: Some(gift.amount),
            crypto_currency: gift.crypto_currency.clone(),
            crypto_amount_minor_units: gift.crypto_amount,
            identifier: gift.transaction_id.clone(),
            saved: false,
            converted: false,
            upgraded: false,
            refunded: false,
            anonymous: false,
        },
        tl::enums::MessageAction::GiftTon(gift) => GiftView {
            kind: GiftKindView::Ton,
            title: "TON gift".to_owned(),
            stars: None,
            days: None,
            currency: Some(gift.currency.clone()),
            amount_minor_units: Some(gift.amount),
            crypto_currency: Some(gift.crypto_currency.clone()),
            crypto_amount_minor_units: Some(gift.crypto_amount),
            identifier: gift.transaction_id.clone(),
            saved: false,
            converted: false,
            upgraded: false,
            refunded: false,
            anonymous: false,
        },
        tl::enums::MessageAction::GiftCode(gift) => GiftView {
            kind: GiftKindView::Code,
            title: "Premium gift code".to_owned(),
            stars: None,
            days: nonnegative_u32(Some(gift.days)),
            currency: gift.currency.clone(),
            amount_minor_units: gift.amount,
            crypto_currency: gift.crypto_currency.clone(),
            crypto_amount_minor_units: gift.crypto_amount,
            identifier: Some(gift.slug.clone()),
            saved: false,
            converted: false,
            upgraded: false,
            refunded: gift.unclaimed,
            anonymous: false,
        },
        tl::enums::MessageAction::StarGift(action) => star_gift(action),
        tl::enums::MessageAction::StarGiftUnique(action) => unique_star_gift(action),
        _ => return None,
    };
    Some(gift_card(gift))
}

fn star_gift(action: &tl::types::MessageActionStarGift) -> GiftView {
    let (kind, title, stars, identifier) = gift_identity(&action.gift);
    GiftView {
        kind,
        title,
        stars,
        days: None,
        currency: None,
        amount_minor_units: None,
        crypto_currency: None,
        crypto_amount_minor_units: None,
        identifier,
        saved: action.saved,
        converted: action.converted,
        upgraded: action.upgraded,
        refunded: action.refunded,
        anonymous: action.name_hidden,
    }
}

fn unique_star_gift(action: &tl::types::MessageActionStarGiftUnique) -> GiftView {
    let (kind, title, stars, identifier) = gift_identity(&action.gift);
    GiftView {
        kind,
        title,
        stars,
        days: None,
        currency: None,
        amount_minor_units: None,
        crypto_currency: None,
        crypto_amount_minor_units: None,
        identifier,
        saved: action.saved,
        converted: false,
        upgraded: action.upgrade,
        refunded: action.refunded,
        anonymous: false,
    }
}

fn gift_identity(
    gift: &tl::enums::StarGift,
) -> (GiftKindView, String, Option<u64>, Option<String>) {
    match gift {
        tl::enums::StarGift::Gift(gift) => (
            GiftKindView::StarGift,
            gift.title
                .clone()
                .unwrap_or_else(|| format!("Star Gift #{}", gift.id)),
            u64::try_from(gift.stars).ok(),
            Some(gift.id.to_string()),
        ),
        tl::enums::StarGift::Unique(gift) => (
            GiftKindView::UniqueStarGift,
            format!("{} #{}", gift.title, gift.num),
            None,
            Some(gift.slug.clone()),
        ),
    }
}

fn gift_card(gift: GiftView) -> MediaCard {
    MediaCard {
        kind: MediaKind::Gift,
        title: gift.title.clone(),
        description: String::new(),
        details: Vec::new(),
        poll: None,
        specialized: Some(SpecializedMediaView::Gift(gift)),
        remote_id: None,
    }
}
