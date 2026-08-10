use super::*;

pub(super) fn paid_media_details(media: &PaidMediaView) -> Vec<String> {
    media
        .items
        .iter()
        .enumerate()
        .map(|(index, item)| match item {
            PaidMediaItemView::Preview {
                width,
                height,
                duration_seconds,
            } => {
                let dimensions = width
                    .zip(*height)
                    .map(|(width, height)| format!(" · {width}×{height}"))
                    .unwrap_or_default();
                let duration = duration_seconds
                    .map(|seconds| format!(" · {seconds}s"))
                    .unwrap_or_default();
                format!("{}. preview{dimensions}{duration}", index + 1)
            }
            PaidMediaItemView::Available {
                kind,
                title,
                remote_id,
            } => {
                let identity = remote_id
                    .as_ref()
                    .map(|id| format!(" · id {id}"))
                    .unwrap_or_default();
                format!("{}. {kind:?} · {title}{identity}", index + 1)
            }
        })
        .collect()
}

pub(super) fn giveaway_description(giveaway: &GiveawayView) -> String {
    if giveaway.state == GiveawayStateView::Results {
        return format!(
            "{} winners · {} unclaimed",
            giveaway.winners_count.unwrap_or(0),
            giveaway.unclaimed_count.unwrap_or(0),
        );
    }
    let prize = giveaway
        .stars
        .map(|stars| format!("{stars} Stars"))
        .or_else(|| {
            giveaway
                .premium_months
                .map(|months| format!("{months} months Premium"))
        })
        .or_else(|| giveaway.prize_description.clone())
        .unwrap_or_else(|| "prize details unavailable".to_owned());
    format!("{} winners · {prize}", giveaway.quantity)
}

pub(super) fn giveaway_details(giveaway: &GiveawayView) -> Vec<String> {
    let deadline = match giveaway.state {
        GiveawayStateView::Active => format!("ends {}", giveaway.until_date),
        GiveawayStateView::Results => format!("ended {}", giveaway.until_date),
    };
    let mut details = vec![deadline];
    let mut eligibility = Vec::new();
    if giveaway.only_new_subscribers {
        eligibility.push("new subscribers");
    }
    if giveaway.winners_visible {
        eligibility.push("winners visible");
    }
    if giveaway.refunded {
        eligibility.push("refunded");
    }
    if !eligibility.is_empty() {
        details.push(eligibility.join(" · "));
    }
    if !giveaway.country_codes.is_empty() || giveaway.channel_count > 0 {
        let countries = giveaway.country_codes.join(", ");
        let channels = format!("{} channels", giveaway.channel_count);
        details.push(
            [(!countries.is_empty()).then_some(countries), Some(channels)]
                .into_iter()
                .flatten()
                .collect::<Vec<_>>()
                .join(" · "),
        );
    }
    if let Some(info) = &giveaway.info {
        match info {
            GiveawayInfoView::Active {
                participating,
                preparing_results,
                start_date,
                eligibility_issue,
            } => {
                details.push(format!(
                    "{} · started {start_date}",
                    if *participating {
                        "participating"
                    } else {
                        "not participating"
                    }
                ));
                if *preparing_results {
                    details.push("preparing results".to_owned());
                }
                if let Some(issue) = eligibility_issue {
                    details.push(format!("ineligible · {issue}"));
                }
            }
            GiveawayInfoView::Results {
                winner,
                start_date,
                finish_date,
                activated_count,
                gift_code_slug,
            } => {
                details.push(format!(
                    "{} · {start_date} to {finish_date}",
                    if *winner { "winner" } else { "not a winner" }
                ));
                if let Some(count) = activated_count {
                    details.push(format!("{count} prizes activated"));
                }
                if gift_code_slug.is_some() {
                    details.push("prize code available".to_owned());
                }
            }
        }
    }
    details
}

pub(super) fn gift_description(gift: &GiftView) -> String {
    match gift.kind {
        GiftKindView::Premium => format!("{} days Premium", gift.days.unwrap_or(0)),
        GiftKindView::Stars => format!("{} Stars", gift.stars.unwrap_or(0)),
        GiftKindView::Ton => gift
            .crypto_currency
            .as_ref()
            .zip(gift.crypto_amount_minor_units)
            .map_or_else(
                || gift.title.clone(),
                |(currency, amount)| format!("{amount} {currency}"),
            ),
        GiftKindView::Code => format!("{}-day Premium code", gift.days.unwrap_or(0)),
        GiftKindView::StarGift | GiftKindView::UniqueStarGift => gift.title.clone(),
    }
}

pub(super) fn gift_details(gift: &GiftView) -> Vec<String> {
    let mut details = Vec::new();
    if let Some((currency, amount)) = gift.currency.as_ref().zip(gift.amount_minor_units) {
        details.push(format!("{currency} {amount} minor units"));
    }
    if let Some(identifier) = &gift.identifier {
        details.push(format!("reference · {identifier}"));
    }
    let mut state = Vec::new();
    for (set, label) in [
        (gift.saved, "saved"),
        (gift.converted, "converted"),
        (gift.upgraded, "upgraded"),
        (gift.refunded, "refunded"),
        (gift.anonymous, "anonymous sender"),
    ] {
        if set {
            state.push(label);
        }
    }
    if !state.is_empty() {
        details.push(state.join(" · "));
    }
    details
}

pub(super) fn invoice_details(invoice: &InvoiceView) -> Vec<String> {
    let mut details = vec![format!(
        "{} {} minor units",
        invoice.currency, invoice.total_minor_units
    )];
    if let Some(receipt) = invoice.receipt_message {
        details.push(format!("receipt · message {}", receipt.0));
    }
    if invoice.shipping_address_requested {
        details.push("shipping address requested".to_owned());
    }
    if invoice.test {
        details.push("test invoice".to_owned());
    }
    if invoice.extended_media {
        details.push("extended media preview".to_owned());
    }
    details
}

pub(super) fn live_location_details(location: &LiveLocationView) -> Vec<String> {
    let minutes = location.period_seconds.div_ceil(60);
    let mut details = vec![format!("sharing for {minutes} min")];
    let heading = location
        .heading_degrees
        .map(|heading| format!("heading {heading}°"));
    let proximity = location
        .proximity_radius_metres
        .map(|radius| format!("within {radius} m"));
    let accuracy = location
        .accuracy_radius_metres
        .map(|radius| format!("accuracy ±{radius} m"));
    if heading.is_some() || proximity.is_some() || accuracy.is_some() {
        details.push(
            [heading, proximity, accuracy]
                .into_iter()
                .flatten()
                .collect::<Vec<_>>()
                .join(" · "),
        );
    }
    details
}

pub(super) fn story_description(story: &SharedStoryView) -> String {
    match story.state {
        StoryStateView::Available => story
            .caption
            .clone()
            .filter(|caption| !caption.is_empty())
            .unwrap_or_else(|| format!("Story {} from peer {}", story.id, story.peer.0)),
        StoryStateView::Skipped => format!("Story {} preview unavailable", story.id),
        StoryStateView::Deleted => format!("Story {} deleted or expired", story.id),
        StoryStateView::Reference => format!("Story {} not loaded", story.id),
    }
}

pub(super) fn story_details(story: &SharedStoryView) -> Vec<String> {
    let mut details = vec![format!("peer {} · story {}", story.peer.0, story.id)];
    if !story.date.is_empty() || !story.expires.is_empty() {
        details.push(match (story.date.is_empty(), story.expires.is_empty()) {
            (false, false) => format!("{} · expires {}", story.date, story.expires),
            (false, true) => story.date.clone(),
            (true, false) => format!("expires {}", story.expires),
            (true, true) => String::new(),
        });
    }
    let mut audience = Vec::new();
    if story.via_mention {
        audience.push("mention");
    }
    if story.close_friends {
        audience.push("close friends");
    }
    if story.live {
        audience.push("live");
    }
    if !audience.is_empty() {
        details.push(audience.join(" · "));
    }
    details
}

pub(super) fn todo_details(todo: &TodoListView) -> Vec<String> {
    let mut details = todo
        .items
        .iter()
        .map(|item| {
            let marker = if item.completed { "☒" } else { "☐" };
            let mut line = format!("{marker} {}", item.title);
            if let Some(peer) = item.completed_by {
                line.push_str(&format!(" · peer {}", peer.0));
            }
            if let Some(date) = &item.completed_date {
                line.push_str(&format!(" · {date}"));
            }
            line
        })
        .collect::<Vec<_>>();
    let mut permissions = Vec::new();
    if todo.others_can_append {
        permissions.push("members may add");
    }
    if todo.others_can_complete {
        permissions.push("members may complete");
    }
    if !permissions.is_empty() {
        details.push(permissions.join(" · "));
    }
    details
}
